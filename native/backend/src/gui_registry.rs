//! Bounded frontend registry discovery. Cached allocation/RTTI candidates
//! are never published as state; gui_observer requalifies all selected widgets.
use crate::{
    gui_observer::{Sample, snapshot},
    live::Memory,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
type Result<T> = std::result::Result<T, String>;
fn number(bytes: &[u8]) -> Result<u64> {
    Ok(u64::from_le_bytes(
        bytes.try_into().map_err(|_| "Partial GUI pointer")?,
    ))
}
fn pointer(v: &Value) -> Result<u64> {
    v.as_u64().ok_or("Invalid GUI pointer".into())
}
struct Candidates<'a, M> {
    memory: &'a mut M,
    base: u64,
    profile: &'a Value,
    pages: HashMap<u64, Vec<u8>>,
    failed: HashSet<u64>,
    types: HashMap<u64, Option<(String, u32)>>,
    used: usize,
}
impl<M: Memory> Candidates<'_, M> {
    fn read(&mut self, a: u64, n: usize) -> Result<Vec<u8>> {
        self.used = self
            .used
            .checked_add(n)
            .ok_or("GUI discovery exceeds read limit")?;
        if self.used > 64 * 1024 * 1024
            || !(0x10000..0x7fff_ffff_ffff).contains(&a)
            || n > 65536
            || a.checked_add(n as u64).is_none_or(|e| e > 0x7fff_ffff_ffff)
        {
            return Err("GUI discovery exceeds read limit".into());
        }
        let bytes = self.memory.discovery_read(a, n)?;
        if bytes.len() != n {
            return Err("Partial GUI discovery read".into());
        }
        Ok(bytes)
    }
    fn u(&mut self, a: u64) -> Result<u64> {
        // Large registries contain scattered widgets. A 4 KiB fill per candidate
        // exhausts the cold-discovery budget despite valid pointers. Small
        // aligned blocks retain caching without sampling unrelated page data.
        const BLOCK: u64 = 128;
        let page = a & !(BLOCK - 1);
        if a.checked_add(8).is_none_or(|end| end > page + BLOCK) {
            return number(&self.read(a, 8)?);
        }
        if self.failed.contains(&page) {
            return Err("Pagina candidato GUI non leggibile".into());
        }
        if !self.pages.contains_key(&page) {
            match self.read(page, BLOCK as usize) {
                Ok(bytes) => {
                    self.pages.insert(page, bytes);
                }
                Err(e) => {
                    self.failed.insert(page);
                    return Err(e);
                }
            }
        }
        let off = (a - page) as usize;
        number(&self.pages[&page][off..off + 8])
    }
    fn kind(&mut self, vt: u64) -> Option<(String, u32)> {
        if let Some(size) = self.profile["image_size"].as_u64()
            && !(self.base..self.base.saturating_add(size)).contains(&vt)
        {
            return None;
        }
        if let Some(kind) = self.types.get(&vt) {
            return kind.clone();
        }
        let result = Sample::new(self.memory, self.base, self.profile)
            .rtti(vt)
            .ok();
        if self.types.len() < 16384 {
            self.types.insert(vt, result.clone());
        }
        result
    }
    fn string(&mut self, a: u64) -> Result<String> {
        Sample::new(self.memory, self.base, self.profile).string(a)
    }
    fn array(&mut self, a: u64, max: i32) -> Result<Vec<u64>> {
        Sample::new(self.memory, self.base, self.profile).array(a, max)
    }
}
#[derive(Clone, Copy, PartialEq)]
struct Header {
    data: u64,
    capacity: i32,
    count: i32,
}

#[cfg(test)]
mod discovery_budget_tests {
    use super::*;
    struct Scattered;
    impl Memory for Scattered {
        fn read(&mut self, _address: u64, size: usize) -> Result<Vec<u8>> {
            Ok(vec![0; size])
        }
        fn class_name(&mut self, _rva: u64) -> Result<String> {
            Err("not a type fixture".into())
        }
    }
    #[test]
    fn scattered_candidates_do_not_consume_whole_pages() {
        let mut memory = Scattered;
        let profile = json!({});
        let mut candidates = Candidates {
            memory: &mut memory,
            base: 0x140000000,
            profile: &profile,
            pages: HashMap::new(),
            failed: HashSet::new(),
            types: HashMap::new(),
            used: 0,
        };
        for i in 0..18000 {
            assert_eq!(candidates.u(0x10000 + i * 4096).unwrap(), 0)
        }
        assert!(candidates.used < 3 * 1024 * 1024);
    }
}
fn headers(memory: &mut impl Memory, root: u64, profile: &Value) -> Result<Vec<Header>> {
    let offsets = profile["gui_layout"]["registry"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_else(|| vec![0x108, 0x120]);
    if offsets.len() != 2 {
        return Err("Invalid GUI registry profile".into());
    }
    offsets
        .iter()
        .map(|off| {
            let b = memory.discovery_read(root + off, 16)?;
            if b.len() != 16 {
                return Err("Partial GUI registry header".into());
            }
            let row = Header {
                data: number(&b[..8])?,
                capacity: i32::from_le_bytes(b[8..12].try_into().unwrap()),
                count: i32::from_le_bytes(b[12..].try_into().unwrap()),
            };
            if row.count < 0 || row.count > row.capacity || row.capacity > 500000 {
                return Err("Registro GUI fuori intervallo".into());
            }
            Ok(row)
        })
        .collect()
}
fn allocation(h: &[Header]) -> Vec<(u64, i32)> {
    h.iter().map(|h| (h.data, h.capacity)).collect()
}
// Widget pointers remain candidates only: snapshot rechecks type, owner, ID,
// visibility and every witness. A panel mid-transition invalidates the sample,
// not the registry discovery. Structural errors still invalidate both.
fn transient_sample(error: &str) -> bool {
    matches!(error, "Stato cambiato durante la lettura"
        | "Cartella ricerca visibile mancante o ambigua"
        | "Viste sovrapposte: geometria non esclusiva")
}
fn witnessed_snapshot(memory: &mut impl Memory, args: &Value, bindings: &Value) -> Result<Value> {
    let observed = std::time::Instant::now();
    let mut result = snapshot(memory, args, bindings)?;
    result["sample_age_ms"] = json!(observed.elapsed().as_secs_f64()*1000.);
    Ok(result)
}
#[derive(Default)]
pub(crate) struct Registry {
    root: u64,
    headers: Vec<Header>,
    bindings: Option<Value>,
    seen: HashSet<u64>,
    missing: VecDeque<String>,
    unsettled_folder: Option<std::time::Instant>,
    pub(crate) diagnostics: Value,
}
impl Registry {
    fn discover(
        &mut self,
        memory: &mut impl Memory,
        base: u64,
        profile: &Value,
        root: u64,
    ) -> Result<()> {
        let h = headers(memory, root, profile)?;
        if self.bindings.is_some()
            && self.root == root
            && allocation(&self.headers) == allocation(&h)
        {
            return Ok(());
        }
        let mut s = Candidates {
            memory,
            base,
            profile,
            pages: HashMap::new(),
            failed: HashSet::new(),
            types: HashMap::new(),
            used: 0,
        };
        let mut owners: BTreeMap<u64, String> = BTreeMap::new();
        let mut seen = HashSet::new();
        let mut candidates = vec![];
        let allowed = [
            "CNationalFocusView",
            "CCountryTechTreeView",
            "CEventWindow",
            "CNationalFocusTreeItem",
            "CTechnologyTreeTechItem",
            "CTopBar",
            "CCountryProductionLineView",
            "CProductionLineMilitaryItem",
            "CProductionEquipmentWindow",
        ];
        for (index, h) in h.iter().enumerate() {
            let extent = if index == 0 { h.count } else { h.capacity } as usize;
            for offset in (0..extent).step_by(8192) {
                let bytes = s.read(h.data + offset as u64 * 8, (extent - offset).min(8192) * 8)?;
                for raw in bytes.as_chunks::<8>().0 {
                    let gui = number(raw)?;
                    if gui == 0 || !seen.insert(gui) {
                        continue;
                    }
                    let result = (|| -> Result<()> {
                        if s.u(gui)? == 0 {
                            return Ok(());
                        }
                        candidates.push(gui);
                        let observer = s.u(gui + 80)?;
                        if !(0x10000..0x7fff_ffff_ffff).contains(&observer) {
                            return Ok(());
                        }
                        let vt = s.u(observer)?;
                        let Some((name, off)) = s.kind(vt) else {
                            return Ok(());
                        };
                        if let Some(short) =
                            name.strip_prefix(".?AV").and_then(|n| n.strip_suffix("@@"))
                            && allowed.contains(&short)
                        {
                            owners.insert(
                                observer
                                    .checked_sub(off as u64)
                                    .ok_or("Invalid GUI observer")?,
                                short.into(),
                            );
                        }
                        Ok(())
                    })();
                    let _ = result;
                }
            }
        }
        let mut bindings = json!({"views":{},"focus":{},"research":{},"events":[],"folders":{},"tabs_container":null,"production_owners":{},"production_access":[],"production_add":[],"production_pickers":[]});
        // The research panel is witnessed through CCountryTechTreeView below.
        // CCountryTechnologyView is an unrelated screen: requiring it made
        // both supported trees unavailable when that screen was absent.
        for name in ["CNationalFocusView"] {
            let views: Vec<_> = owners
                .iter()
                .filter(|(_, kind)| *kind == name)
                .map(|(a, _)| *a)
                .collect();
            if views.len() != 1 {
                return Err(format!("Vista GUI {name} mancante o ambigua ({} candidati)", views.len()));
            }
            bindings["views"][name] = json!(views[0]);
        }
        let view_container = profile["gui_layout"]["view_container"]
            .as_u64()
            .unwrap_or(0x578);
        let children = profile["gui_layout"]["container_children"]
            .as_u64()
            .unwrap_or(0x10F0);
        let mut trees = vec![];
        for (&a, name) in &owners {
            if name != "CCountryTechTreeView" {
                continue;
            }
            let result = (|| -> Result<()> {
                let c = s.u(a + view_container)?;
                let def = s.u(c + 0x90)?;
                if s.string(def + 0x28)? == "countrytechtreeview" {
                    trees.push(a);
                }
                Ok(())
            })();
            let _ = result;
        }
        if trees.len() != 1 {
            return Err("Albero ricerca mancante o ambiguo".into());
        }
        bindings["views"]["CCountryTechTreeView"] = json!(trees[0]);
        let tc = s.u(trees[0] + view_container)?;
        for gui in s.array(tc + children, 2048)? {
            let result = (|| -> Result<()> {
                let vt = s.u(gui)?;
                if s.kind(vt) != Some((".?AVCContainerWindow@@".into(), 48)) {
                    return Ok(());
                }
                let def = s.u(gui + 0x60)?;
                let name = s.string(def + 0x28)?;
                if name == "folder_tabs" {
                    bindings["tabs_container"] = json!(gui - 48);
                }
                if name.ends_with("_folder")
                    || ["mtgnavalfolder", "mtgnavalsupportfolder"].contains(&name.as_str())
                {
                    bindings["folders"][name] = json!(gui - 48);
                }
                Ok(())
            })();
            let _ = result;
        }
        if bindings["folders"].as_object().unwrap().is_empty() {
            return Err("Cartelle albero ricerca non individuate".into());
        }
        for (&a, name) in &owners {
            if name == "CEventWindow" {
                bindings["events"].as_array_mut().unwrap().push(json!(a));
            }
            if [
                "CTopBar",
                "CCountryProductionLineView",
                "CProductionLineMilitaryItem",
                "CProductionEquipmentWindow",
            ]
            .contains(&name.as_str())
            {
                bindings["production_owners"][a.to_string()] = json!(name);
            }
            let result = (|| -> Result<()> {
                let (panel, key) = if name == "CNationalFocusTreeItem" {
                    let def = s.u(a + 0x20)?;
                    ("focus", s.string(def + 0x18)?)
                } else if name == "CTechnologyTreeTechItem" {
                    let model = s.u(a + 0xC8)?;
                    let def = s.u(model + 0x160)?;
                    ("research", s.string(def + 0x10)?)
                } else {
                    return Ok(());
                };
                let entries = bindings[panel]
                    .as_object_mut()
                    .unwrap()
                    .entry(key)
                    .or_insert_with(|| json!([]));
                entries.as_array_mut().unwrap().push(json!(a));
                Ok(())
            })();
            let _ = result;
        }
        for gui in candidates.iter().copied() {
            let result = (|| -> Result<()> {
                let vt = s.u(gui)?;
                let Some((kind, off)) = s.kind(vt) else {
                    return Ok(());
                };
                if kind == ".?AVCContainerWindow@@" && (off == 0 || off == 48) {
                    let c = gui - off as u64;
                    let observer = s.u(c + 0x80)?;
                    let ovt = s.u(observer)?;
                    if let Some((name, displacement)) = s.kind(ovt) {
                        let owner = observer
                            .checked_sub(displacement as u64)
                            .ok_or("Invalid GUI observer")?;
                        if name == ".?AVCProductionEquipmentWindow@@"
                            && owners
                                .get(&owner)
                                .is_some_and(|n| n == "CProductionEquipmentWindow")
                        {
                            let def = s.u(c + 0x90)?;
                            let name = s.string(def + 0x28)?;
                            if [
                                "production_equipment_window_military",
                                "production_equipment_window",
                            ]
                            .contains(&name.as_str())
                            {
                                bindings["production_pickers"].as_array_mut().unwrap().push(json!({"container":c,"owner":owner,"observer_offset":displacement,"name":name}));
                            }
                        }
                    }
                }
                if kind != ".?AVCButtonStandard@@" || off != 0 {
                    return Ok(());
                }
                let observer = s.u(gui + 0x50)?;
                let ovt = s.u(observer)?;
                let Some((kind, off)) = s.kind(ovt) else {
                    return Ok(());
                };
                let owner = observer
                    .checked_sub(off as u64)
                    .ok_or("Invalid GUI observer")?;
                let def = s.u(gui + 0x60)?;
                let name = s.string(def + 0x28)?;
                if kind == ".?AVCTopBar@@"
                    && owners.get(&owner).is_some_and(|n| n == "CTopBar")
                    && name == "production_button"
                {
                    bindings["production_access"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!(gui));
                }
                if kind == ".?AVCCountryProductionLineView@@"
                    && owners
                        .get(&owner)
                        .is_some_and(|n| n == "CCountryProductionLineView")
                    && [
                        "infantry_button",
                        "tank_button",
                        "aircraft_button",
                        "naval_button",
                    ]
                    .contains(&name.as_str())
                {
                    bindings["production_add"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"address":gui,"owner":owner,"name":name}));
                }
                Ok(())
            })();
            let _ = result;
        }
        bindings["root"] = json!(root);
        let mut sample = Sample::new(s.memory, base, profile);
        bindings["action_roots"] = crate::gui_actions::action_roots(&mut sample, root, &seen);
        self.diagnostics = json!({"blocks":s.pages.len(),"block_bytes":128,"pages":s.pages.len(),"failed_pages":s.failed.len(),"types":s.types.len(),"cache_bytes":s.pages.values().map(Vec::len).sum::<usize>(),"registry_objects":seen.len(),"non_null_widgets":candidates.len()});
        self.root = root;
        self.headers = h;
        self.bindings = Some(bindings);
        self.seen = seen;
        Ok(())
    }
    pub(crate) fn observe(&mut self, memory: &mut impl Memory, args: &Value) -> Result<Value> {
        let started = std::time::Instant::now();
        for attempt in 0..2 {
            match self.inner(memory, args) {
                Ok(mut v) => {
                    self.unsettled_folder = None;
                    v["diagnostics"]["snapshot_ms"] =
                        json!(started.elapsed().as_secs_f64() * 1000.);
                    v["diagnostics"]["retry_count"] = json!(attempt);
                    v["diagnostics"]["discovery"] = self.diagnostics.clone();
                    return Ok(v);
                }
                Err(e) if transient_sample(&e) && attempt == 0 => continue,
                Err(e) => {
                    self.reject_sample(&e);
                    return Err(e);
                }
            }
        }
        Err("Campione incoerente: riprovare".into())
    }
    fn reject_sample(&mut self, error: &str) {
        self.reject_sample_at(error, std::time::Instant::now());
    }
    fn reject_sample_at(&mut self, error: &str, now:std::time::Instant) {
        let persistent_folder = if error == "Cartella ricerca visibile mancante o ambigua" {
            let since = self.unsettled_folder.get_or_insert(now);
            now.duration_since(*since) >= std::time::Duration::from_millis(750)
        } else { false };
        // A genuinely rebuilt folder must eventually be discovered. Ordinary
        // overlap/animation witnesses never justify a registry-wide rescan.
        if !transient_sample(error) || persistent_folder {
            self.bindings = None;
            self.unsettled_folder = None;
        }
    }
    fn refresh_extensions(
        &mut self,
        memory: &mut impl Memory,
        base: u64,
        profile: &Value,
        root: u64,
        hints: &Value,
        events_only: bool,
    ) -> Result<()> {
        let old = self.headers.clone();
        let current = headers(memory, root, profile)?;
        let mut candidates = Candidates {
            memory,
            base,
            profile,
            pages: HashMap::new(),
            failed: HashSet::new(),
            types: HashMap::new(),
            used: 0,
        };
        let mut events: HashSet<u64> =
            crate::hud_geometry::rows(&self.bindings.as_ref().unwrap()["events"])
                .iter()
                .filter_map(Value::as_u64)
                .collect();
        let mut added = HashSet::new();
        for (index, h) in current.iter().enumerate() {
            if old.get(index) == Some(h) {
                continue;
            }
            let previous = old.get(index);
            let start = if index == 0
                && previous.is_some_and(|p| {
                    p.data == h.data && p.capacity == h.capacity && h.count >= p.count
                }) {
                previous.unwrap().count as usize
            } else {
                0
            };
            let extent = if index == 0 { h.count } else { h.capacity } as usize;
            for off in (start..extent).step_by(8192) {
                let bytes =
                    candidates.read(h.data + off as u64 * 8, (extent - off).min(8192) * 8)?;
                for raw in bytes.as_chunks::<8>().0 {
                    let gui = number(raw)?;
                    if gui == 0 || !self.seen.insert(gui) {
                        continue;
                    }
                    if self.seen.len() > 1_000_000 {
                        return Err("Inventario candidati GUI eventi oltre limite".into());
                    }
                    added.insert(gui);
                    let result = (|| -> Result<()> {
                        let observer = candidates.u(gui + 80)?;
                        let vt = candidates.u(observer)?;
                        if let Some((name, off)) = candidates.kind(vt)
                            && name == ".?AVCEventWindow@@"
                        {
                            events.insert(
                                observer
                                    .checked_sub(off as u64)
                                    .ok_or("Invalid event observer")?,
                            );
                        }
                        Ok(())
                    })();
                    let _ = result;
                }
            }
        }
        if crate::hud_geometry::rows(hints).len() > 256 {
            return Err("Troppi candidati finestra evento".into());
        }
        for hint in crate::hud_geometry::rows(hints) {
            let value = if hint.is_object() {
                &hint["instance_id"]
            } else {
                hint
            };
            let text = value
                .as_str()
                .ok_or("Candidato finestra evento non valido")?;
            if text.is_empty() || text.len() > 16 || !text.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("Candidato finestra evento non valido".into());
            }
            let a = u64::from_str_radix(text, 16)
                .map_err(|_| "Candidato finestra evento non valido")?;
            if !(0x10000..0x7fff_ffff_ffff).contains(&a) {
                return Err("Candidato finestra evento fuori intervallo".into());
            }
            events.insert(a);
        }
        if events.len() > 256 {
            return Err("Troppe finestre eventi candidate".into());
        }
        let mut events: Vec<_> = events.into_iter().collect();
        events.sort_unstable();
        let bindings = self.bindings.as_mut().unwrap();
        bindings["events"] = json!(events);
        if !added.is_empty() && !events_only {
            let roots = crate::gui_actions::action_roots(
                &mut Sample::new(candidates.memory, base, profile),
                root,
                &added,
            );
            bindings["action_roots"]
                .as_object_mut()
                .ok_or("Invalid action roots cache")?
                .extend(roots.as_object().unwrap().clone());
        }
        self.headers = current;
        Ok(())
    }
    fn inner(&mut self, memory: &mut impl Memory, args: &Value) -> Result<Value> {
        let profile = &args["profile"];
        let base = pointer(&args["base"])?;
        let root_rva = profile["gui_layout"]["root_rva"]
            .as_u64()
            .unwrap_or(0x34531E0);
        let root = {
            let mut s = Sample::new(memory, base, profile);
            s.ptr(base + root_rva, "CGameGui")?
        };
        self.discover(memory, base, profile, root)?;
        if !profile["gui_layout"].is_object() {
            self.refresh_extensions(memory, base, profile, root, &args["event_instances"], false)?;
        } else {
            // Event popups open after discovery: find new CEventWindow objects
            // by type so a visible popup is known (its answer geometry stays
            // unqualified on this layout). A failed scan changes nothing else.
            let _ = self.refresh_extensions(memory, base, profile, root, &json!([]), true);
        }
        let mut result = witnessed_snapshot(memory, args, self.bindings.as_ref().unwrap())?;
        for (panel, field) in [("focus", "focus_ids"), ("research", "technology_ids")] {
            if result["panels"][panel]["visible"] != true {
                continue;
            }
            let targets = crate::hud_geometry::rows(&args[field]);
            if targets
                .iter()
                .filter_map(Value::as_str)
                .any(|id| self.bindings.as_ref().unwrap()[panel].get(id).is_none())
            {
                let key = json!([
                    root,
                    result["country"],
                    result["scope"],
                    panel,
                    result["panels"][panel]["folder"],
                    targets
                ])
                .to_string();
                if !self.missing.contains(&key) {
                    self.missing.push_back(key);
                    if self.missing.len() > 128 {
                        self.missing.pop_front();
                    }
                    self.bindings = None;
                    self.discover(memory, base, profile, root)?;
                    result = witnessed_snapshot(memory, args, self.bindings.as_ref().unwrap())?;
                }
            }
        }
        if result["production_ui"]["visible"] == true {
            let key = json!([
                "production",
                root,
                result["country"],
                result["production_ui"]["discovery_context"]
            ])
            .to_string();
            if !self.missing.contains(&key) {
                self.missing.push_back(key);
                if self.missing.len() > 128 {
                    self.missing.pop_front();
                }
                self.bindings = None;
                self.discover(memory, base, profile, root)?;
                result = witnessed_snapshot(memory, args, self.bindings.as_ref().unwrap())?;
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;
    #[test]
    fn changing_panels_discard_samples_without_restarting_registry_discovery() {
        let bindings=json!({"root":123,"views":{"focus":456,"research":789}});
        let mut registry=Registry {bindings:Some(bindings.clone()),..Default::default()};
        for error in ["Stato cambiato durante la lettura","Cartella ricerca visibile mancante o ambigua","Viste sovrapposte: geometria non esclusiva"] {
            registry.reject_sample(error);
            assert_eq!(registry.bindings.as_ref(),Some(&bindings));
        }
        for error in ["Contesto GUI cambiato durante la ricerca","ID ricerca GUI cambiato","Vista focus di un’altra nazione","Tipo GUI inatteso: CCountryTechTreeView"] {
            registry.bindings=Some(bindings.clone());
            registry.reject_sample(error);
            assert!(registry.bindings.is_none(),"{error}");
        }
    }
    #[test]
    fn persistently_recreated_folders_can_rebuild_candidates_without_accepting_a_sample() {
        let now=std::time::Instant::now();
        let mut registry=Registry {bindings:Some(json!({"root":123})),..Default::default()};
        let error="Cartella ricerca visibile mancante o ambigua";
        registry.reject_sample_at(error,now);
        registry.reject_sample_at(error,now+std::time::Duration::from_millis(700));
        assert!(registry.bindings.is_some());
        registry.reject_sample_at(error,now+std::time::Duration::from_millis(750));
        assert!(registry.bindings.is_none());
    }
}

/// Read-only probe of the event popup layout on the running game (run by hand
/// with the popup open): `HOI4_EXE=<hoi4.exe> HOI4_PROBE_OUT=<file.json>
/// cargo test --release --lib event_window_probe -- --ignored`. It only reads
/// process memory, like the HUD, and writes the typed fields it finds.
#[cfg(test)]
mod event_window_probe {
    use super::*;
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
            crate::gui_actions::events(&mut s, root, &registry.bindings.as_ref().unwrap()["events"], &json!(["bftb_turkey.4"]), resolution)
        };
        let report = json!({"root":format!("{root:x}"),"refresh_error":refreshed,"native_events":match native {Ok(v)=>v,Err(e)=>json!({"error":e})},"event_windows":windows});
        std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
}
