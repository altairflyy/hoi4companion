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
        let page = a & !4095;
        if a.checked_add(8).is_none_or(|end| end > page + 4096) {
            return number(&self.read(a, 8)?);
        }
        if self.failed.contains(&page) {
            return Err("Pagina candidato GUI non leggibile".into());
        }
        if !self.pages.contains_key(&page) {
            match self.read(page, 4096) {
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
#[derive(Default)]
pub(crate) struct Registry {
    root: u64,
    headers: Vec<Header>,
    bindings: Option<Value>,
    seen: HashSet<u64>,
    missing: VecDeque<String>,
    pub(crate) diagnostics: Value,
}
impl Registry {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
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
            "CCountryTechnologyView",
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
        for name in ["CNationalFocusView", "CCountryTechnologyView"] {
            let views: Vec<_> = owners
                .iter()
                .filter(|(_, kind)| *kind == name)
                .map(|(a, _)| *a)
                .collect();
            if views.len() != 1 {
                return Err("Viste GUI mancanti o ambigue".into());
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
        self.diagnostics = json!({"pages":s.pages.len(),"failed_pages":s.failed.len(),"types":s.types.len(),"cache_bytes":s.pages.len()*4096,"registry_objects":seen.len(),"non_null_widgets":candidates.len()});
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
                    v["diagnostics"]["snapshot_ms"] =
                        json!(started.elapsed().as_secs_f64() * 1000.);
                    v["diagnostics"]["retry_count"] = json!(attempt);
                    v["diagnostics"]["discovery"] = self.diagnostics.clone();
                    return Ok(v);
                }
                Err(e) if e == "Stato cambiato durante la lettura" && attempt == 0 => continue,
                Err(e) => {
                    self.bindings = None;
                    return Err(e);
                }
            }
        }
        Err("Campione incoerente: riprovare".into())
    }
    fn refresh_extensions(
        &mut self,
        memory: &mut impl Memory,
        base: u64,
        profile: &Value,
        root: u64,
        hints: &Value,
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
        if !added.is_empty() {
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
            self.refresh_extensions(memory, base, profile, root, &args["event_instances"])?;
        }
        let mut result = snapshot(memory, args, self.bindings.as_ref().unwrap())?;
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
                    result = snapshot(memory, args, self.bindings.as_ref().unwrap())?;
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
                result = snapshot(memory, args, self.bindings.as_ref().unwrap())?;
            }
        }
        Ok(result)
    }
}
