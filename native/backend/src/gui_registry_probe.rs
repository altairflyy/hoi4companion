//! Event popup probe for the live GUI registry. It needs the backend process
//! readers (`live_process`, `game_process`), so it lives here, not in `live`.
/// Read-only probe of the event popup layout on the running game (run by hand
/// with the popup open): `HOI4_EXE=<hoi4.exe> HOI4_PROBE_OUT=<file.json>
/// cargo test --release --lib event_window_probe -- --ignored`. It only reads
/// process memory, like the HUD, and writes the typed fields it finds.
mod event_window_probe {
    use crate::gui_registry::*;
    use crate::gui_observer::Sample;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    type M<'a> = crate::live_process::Memory<'a>;
    fn valid(p: u64) -> bool {
        (0x10000..0x7fff_ffff_ffff).contains(&p) && p % 8 == 0
    }
    /// Fields of `a` (first `len` bytes): pointers to typed objects, embedded
    /// vtables, short strings and arrays of typed objects.
    fn fields(memory: &mut M, base: u64, profile: &Value, a: u64, len: u64, arrays: bool) -> Vec<Value> {
        let mut out = vec![];
        for off in (0..len).step_by(8) {
            let mut s = Sample::new(memory, base, profile);
            let Ok(v) = s.u(a + off) else { continue };
            if valid(v) {
                if let Ok((name, _)) = s.rtti(v) {
                    out.push(json!({"off":off,"embedded":name}));
                    continue;
                }
                if let Ok(vt) = s.u(v)
                    && let Ok((name, _)) = s.rtti(vt)
                {
                    out.push(json!({"off":off,"ptr":format!("{v:x}"),"type":name}));
                    continue;
                }
            }
            if let Ok(text) = s.string(a + off)
                && text.len() >= 3
                && text.len() <= 80
                && text.chars().all(|c| c.is_ascii_graphic() || c == ' ')
            {
                out.push(json!({"off":off,"string":text}));
                continue;
            }
            if arrays && let Ok(items) = s.array(a + off, 64) && !items.is_empty() {
                let mut kinds = vec![];
                for item in items.iter().take(4) {
                    let kind = s.u(*item).ok().and_then(|vt| s.rtti(vt).ok()).map(|(n, _)| n).unwrap_or_default();
                    kinds.push(json!(kind));
                }
                if kinds.iter().any(|k| k != "") {
                    out.push(json!({"off":off,"array":items.len(),"items":kinds,"first":format!("{:x}",items[0])}));
                }
            }
        }
        out
    }
    #[test]
    #[ignore]
    fn event_window_probe() {
        let exe = std::env::var("HOI4_EXE").expect("HOI4_EXE");
        let path = std::env::var("HOI4_PROBE_OUT").expect("HOI4_PROBE_OUT");
        let manifest = format!("{:x}", Sha256::digest(include_bytes!("../../../src/hoi4companion/live-profiles-v1.json")));
        let opened = std::sync::Arc::new(crate::profile::Profile::open(std::path::Path::new(&exe), &manifest).unwrap());
        let game = crate::game_process::Game::open(opened).unwrap();
        let mut profile = crate::live_process::profile_value(&game.profile.metadata()["selected"]);
        for key in ["image_base", "image_size", "sha256"] {
            profile[key] = game.profile.metadata()[key].clone();
        }
        let mut memory = M::new(&game, 512 * 1024 * 1024).unwrap();
        let base = game.base;
        let root = {
            let mut s = Sample::new(&mut memory, base, &profile);
            s.ptr(base + profile["gui_layout"]["root_rva"].as_u64().unwrap_or(0x34531E0), "CGameGui").unwrap()
        };
        let mut registry = Registry::default();
        registry.discover(&mut memory, base, &profile, root).unwrap();
        let refreshed = registry.refresh_extensions(&mut memory, base, &profile, root, &json!([]), true).err();
        let events: Vec<u64> = crate::hud_geometry::rows(&registry.bindings.as_ref().unwrap()["events"]).iter().filter_map(Value::as_u64).collect();
        let mut windows = vec![];
        for e in events.iter().take(8) {
            let top = fields(&mut memory, base, &profile, *e, 0x2000, false);
            let mut children = vec![];
            for f in &top {
                let kind = f["type"].as_str().unwrap_or("");
                if ["CEvent", "CStandardGridBox", "CContainerWindow", "CGridBox"].iter().any(|k| kind.contains(k)) {
                    let p = u64::from_str_radix(f["ptr"].as_str().unwrap(), 16).unwrap();
                    let len = if kind.contains("CEvent") { 0x600 } else { 0x1800 };
                    children.push(json!({"off":f["off"],"type":kind,"fields":fields(&mut memory, base, &profile, p, len, true)}));
                }
            }
            // Answer rows: grid rows -> item container -> drag buttons.
            let mut buttons = vec![];
            for f in top.iter().filter(|f| f["type"] == ".?AVCStandardGridBox@@") {
                let grid = u64::from_str_radix(f["ptr"].as_str().unwrap(), 16).unwrap();
                let mut s = Sample::new(&mut memory, base, &profile);
                let rows = s.array(grid + 0x1A0, 64).unwrap_or_default();
                for row in rows {
                    let mut s = Sample::new(&mut memory, base, &profile);
                    let Ok(item) = s.u(row + 0x10) else { continue };
                    let drags = s.array(item + profile["gui_layout"]["container_drags"].as_u64().unwrap_or(0x1258), 16).unwrap_or_default();
                    for button in drags {
                        let mut s = Sample::new(&mut memory, base, &profile);
                        let option = s.u(button + 8).unwrap_or(0);
                        let option_id = s.string(option + 0x178).ok();
                        let option_index = s.i(option + 0x1F0).ok();
                        let outer = s.u(item + profile["gui_layout"]["container_outer"].as_u64().unwrap_or(0x17F0)).unwrap_or(0);
                        let anchor = s.u(button + 280).unwrap_or(1);
                        buttons.push(json!({"row":format!("{row:x}"),"item":format!("{item:x}"),"button":format!("{button:x}"),
                            "option_id":option_id,"option_index":option_index,"anchor_is_outer":anchor == outer,
                            "fields":fields(&mut memory, base, &profile, button, 0x300, false)}));
                    }
                }
            }
            // Popup container size and its position object's resolution.
            let mut s = Sample::new(&mut memory, base, &profile);
            let c = s.u(e + 0x50).unwrap_or(0);
            let clip = profile["gui_layout"]["container_clip"].as_u64().unwrap_or(0x1794);
            let size = [s.i(c + clip + 0x18).ok(), s.i(c + clip + 0x1C).ok()];
            let position = s.u(c + profile["gui_layout"]["container_outer"].as_u64().unwrap_or(0x17F0)).unwrap_or(0);
            let resolution = [s.i(position + 0x130).ok(), s.i(position + 0x134).ok()];
            windows.push(json!({"address":format!("{e:x}"),"popup_size":size,"position_resolution":resolution,"fields":top,"children":children,"buttons":buttons}));
        }
        // The production reader on the same windows (profile from the file).
        let native = {
            let mut s = Sample::new(&mut memory, base, &profile);
            let resolution = [2560, 1440];
            hoi4companion_live::gui_actions::events(&mut s, root, &registry.bindings.as_ref().unwrap()["events"], &json!(["bftb_turkey.4"]), resolution)
        };
        let report = json!({"root":format!("{root:x}"),"refresh_error":refreshed,"native_events":match native {Ok(v)=>v,Err(e)=>json!({"error":e})},"event_windows":windows});
        std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
}
/// Read-only timing of the event window discovery scan of the `events`
/// section (HUD plan part 2 step 4b): registry size, distinct parents and the
/// cost of each read pass, on the running game. Run by hand:
/// `HOI4_EXE=<hoi4.exe> node scripts/dev-native.cjs test backend --lib event_discovery_profile -- --ignored --nocapture`.
mod event_discovery_profile {
    use crate::live::Memory as _;
    use sha2::{Digest, Sha256};
    use std::collections::{HashMap, HashSet};
    use std::time::Instant;
    type M<'a> = crate::live_process::Memory<'a>;
    fn u(m: &mut M, a: u64) -> Option<u64> {
        m.discovery_read(a, 8).ok().map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    }
    #[test]
    #[ignore]
    fn event_discovery_profile() {
        let exe = std::env::var("HOI4_EXE").expect("HOI4_EXE");
        let manifest = format!("{:x}", Sha256::digest(include_bytes!("../../../src/hoi4companion/live-profiles-v1.json")));
        let opened = std::sync::Arc::new(crate::profile::Profile::open(std::path::Path::new(&exe), &manifest).unwrap());
        let game = crate::game_process::Game::open(opened).unwrap();
        let profile = crate::live_process::profile_value(&game.profile.metadata()["selected"]);
        let base = game.base;
        for round in 0..3 {
            let mut memory = M::new(&game, 512 * 1024 * 1024).unwrap();
            let root = u(&mut memory, base + profile["gui_layout"]["root_rva"].as_u64().unwrap_or(0x34531E0)).unwrap();
            let registry: Vec<u64> = profile["gui_layout"]["registry"].as_array()
                .map(|v| v.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or(vec![264, 288]);
            let started = Instant::now();
            let mut guis = vec![];
            let mut seen = HashSet::new();
            let mut extents = vec![];
            for (offset, dynamic) in [(registry[0], false), (registry[1], true)] {
                let raw = memory.discovery_read(root + offset, 16).unwrap();
                let data = u64::from_le_bytes(raw[..8].try_into().unwrap());
                let capacity = i32::from_le_bytes(raw[8..12].try_into().unwrap());
                let count = i32::from_le_bytes(raw[12..].try_into().unwrap());
                let extent = if dynamic { capacity } else { count } as usize;
                extents.push((capacity, count));
                for start in (0..extent).step_by(8192) {
                    let bytes = memory.discovery_read(data + start as u64 * 8, (extent - start).min(8192) * 8).unwrap();
                    for row in bytes.as_chunks::<8>().0 {
                        let gui = u64::from_le_bytes(*row);
                        if gui != 0 && seen.insert(gui) { guis.push(gui); }
                    }
                }
            }
            let arrays = started.elapsed();
            let started = Instant::now();
            let parents: Vec<Option<u64>> = guis.iter().map(|g| u(&mut memory, g + 80)).collect();
            let parent_pass = started.elapsed();
            let distinct: HashSet<u64> = parents.iter().flatten().copied().filter(|p| *p != 0).collect();
            let started = Instant::now();
            let mut vts = HashMap::new();
            for p in &distinct { if let Some(vt) = u(&mut memory, *p) { vts.insert(*p, vt); } }
            let vt_pass = started.elapsed();
            let distinct_vts: HashSet<u64> = vts.values().copied().collect();
            let nonzero = parents.iter().flatten().filter(|p| **p != 0).count();
            println!("round {round}: registries {extents:?}, guis {}, nonzero parents {nonzero}, distinct parents {}, distinct vts {}",
                guis.len(), distinct.len(), distinct_vts.len());
            println!("  arrays {:.1} ms, gui+80 pass {:.1} ms, parent vt pass (distinct) {:.1} ms", arrays.as_secs_f64() * 1e3,
                parent_pass.as_secs_f64() * 1e3, vt_pass.as_secs_f64() * 1e3);
            // Page-batched gui+80 pass: one read per distinct page or block.
            for block in [4096u64, 16384, 65536] {
                let mut blocks: Vec<u64> = guis.iter().map(|g| (g + 80) & !(block - 1)).collect();
                blocks.sort_unstable();
                blocks.dedup();
                let started = Instant::now();
                let mut failed = 0;
                let mut bytes = 0usize;
                for b in &blocks {
                    match memory.discovery_read(*b, block as usize) { Ok(v) => bytes += v.len(), Err(_) => failed += 1 }
                }
                println!("  block {block}: {} blocks, {failed} failed, {:.1} MB, {:.1} ms", blocks.len(), bytes as f64 / 1048576.,
                    started.elapsed().as_secs_f64() * 1e3);
            }
        }
    }
}
