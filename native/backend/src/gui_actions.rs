//! Event answers and office/project controls on the already qualified GUI tree.
//! Unknown payloads remain anonymous, never guessed from their captions.
use crate::{
    gui_observer::Sample,
    hud_geometry::{anchored, intersect, rows},
    live::Memory,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
type Result<T> = std::result::Result<T, String>;
pub(crate) fn events(
    s: &mut Sample<impl Memory>,
    root: u64,
    owners: &Value,
    requested: &Value,
    resolution: [i32; 2],
) -> Result<Value> {
    let screen = [0., 0., resolution[0] as f64, resolution[1] as f64];
    let mut windows = vec![];
    let mut nodes = vec![];
    let mut ids = vec![];
    for address in rows(owners) {
        let owner = s.typed(
            address.as_u64().ok_or("Invalid event candidate")?,
            "CEventWindow",
        )?;
        let c = s.u(owner + 0x50)?;
        let shown = s.container(c, root, owner + 56)?;
        let event = s.u(owner + 0x68)?;
        if !shown {
            continue;
        }
        if event == 0 {
            return Err("Evento visibile senza identità nativa".into());
        }
        s.typed(event, "CEvent")?;
        let id = s.string(event + 0x20)?;
        let position = s.ptr(c + 0x15F0, "C2dPositionObject")?;
        if [s.i(position + 0x130)?, s.i(position + 0x134)?] != resolution {
            return Err("Risoluzione evento differente dal contesto GUI".into());
        }
        let matrix = s.matrix(position)?;
        let size = [s.i(c + 0x15AC)? as f64, s.i(c + 0x15B0)? as f64];
        let mut popup = anchored(matrix, [0., 0.], size, 1., screen)?;
        let pc: [f64; 4] = std::array::from_fn(|i| popup["clip"][i].as_f64().unwrap());
        let clip = intersect(pc, s.clip(c)?);
        let instance = format!("{owner:x}");
        popup["event_id"] = json!(id);
        popup["instance_id"] = json!(instance);
        popup["clip"] = json!(clip);
        popup["visible"] = json!(true);
        popup.as_object_mut().unwrap().remove("scale");
        windows.push(popup);
        ids.push(id.clone());
        if !rows(requested).iter().any(|v| v == &id) {
            continue;
        }
        let definitions = s.array(event + 0x368, 256)?;
        let grid = s.ptr(owner + 0x1280, "CStandardGridBox")?;
        if s.u(grid + 0x68)? != root || s.u(grid + 0x50)? != owner + 56 {
            return Err("Griglia scelte di un altro evento".into());
        }
        let mut indices = HashSet::new();
        let mut options = HashSet::new();
        for row in s.array(grid + 0x1A0, 256)? {
            s.typed(row, "CStandardGridBoxItem")?;
            let item = s.ptr(row + 0x10, "CContainerWindow")?;
            let shown = s.container(item, root, owner + 56)?;
            let parent = s.ptr(item + 0x15F0, "C2dPositionObject")?;
            let matrix = s.matrix(parent)?;
            let buttons = s.array(item + 0x10D8, 64)?;
            if buttons.is_empty() {
                return Err("Scelta evento priva di pulsante nativo".into());
            }
            let mut row_options = HashSet::new();
            let mut row_nodes = vec![];
            for button in buttons {
                s.typed(button, "CButtonDrag")?;
                if s.u(button + 0x68)? != root || s.u(button + 0x50)? != owner + 56 {
                    return Err("Pulsante scelta di un altro evento".into());
                }
                let option = s.ptr(button + 8, "CEventOption")?;
                if !definitions.contains(&option) {
                    return Err("Scelta estranea all’evento visualizzato".into());
                }
                row_options.insert(option);
                if s.u(button + 0x110)? != parent {
                    return Err("Ancoraggio pulsante evento non qualificato".into());
                }
                let sprite = s.u(button + 0x118)?;
                let mut bounds = s.bounds(sprite, parent, matrix, clip)?;
                bounds["visible"] =
                    json!(bounds["visible"] == true && shown && s.shown(button + 0x75)?);
                row_nodes.push(bounds);
            }
            if row_options.len() != 1 {
                return Err("Riga evento associata a opzioni diverse".into());
            }
            let option = *row_options.iter().next().unwrap();
            let index = s.i(option + 0x1F0)?;
            let identifier = s.string(option + 0x178)?;
            if !(0..=255).contains(&index)
                || !indices.insert(index)
                || !options.insert(identifier.clone())
            {
                return Err("Indice o identità scelta evento duplicati".into());
            }
            if row_nodes.len() != 1 {
                return Err("Geometria scelta evento ambigua".into());
            }
            let mut node = row_nodes.remove(0);
            node["panel"] = json!("event");
            node["identity"] = json!(format!("{id}:{identifier}"));
            node["event_id"] = json!(id);
            node["option_id"] = json!(identifier);
            node["option_index"] = json!(index);
            node["instance_id"] = json!(instance);
            nodes.push(node);
        }
    }
    ids.sort();
    ids.dedup();
    Ok(
        json!({"status":if windows.is_empty(){"unknown"}else{"observed"},"event_windows":windows,"nodes":nodes,"visible_ids":ids}),
    )
}
fn buttons(root: &str) -> &[&str] {
    match root {
        "topbar" => &["player_flag", "technology_button"],
        "countrytechnologyview" => &["facilities_tab"],
        "countrypoliticsview" => &[
            "add_idea_button",
            "open_roster_button",
            "idea_entry",
            "button",
        ],
        "political_ideas_window" => &["idea_entry_bg", "button"],
        "facilities_view" => &["open_facility_button", "facility_button"],
        "program_window" => &["start_research_button", "project_button", "select_project"],
        _ => &[],
    }
}
pub(crate) const ROOTS: [&str; 6] = [
    "topbar",
    "countrypoliticsview",
    "political_ideas_window",
    "countrytechnologyview",
    "facilities_view",
    "program_window",
];
fn payload(s: &mut Sample<impl Memory>, button: u64, country: u64) -> Result<Value> {
    let p = s.u(button + 8)?;
    let vt = s.u(p)?;
    let (name, off) = s.rtti(vt)?;
    if off != 0 {
        return Ok(json!({}));
    }
    match name.as_str() {
        ".?AVCIdeaGroupType@@" => {
            s.typed(p, "CIdeaGroupType")?;
            Ok(json!({"category_id":s.string(p+0x38)?}))
        }
        ".?AVCIdea@@" => {
            s.typed(p, "CIdea")?;
            let group = s.ptr(p + 0xAA0, "CIdeaGroupType")?;
            let flag = s.read(group + 0x58, 1)?[0];
            if flag > 1 {
                return Ok(json!({}));
            }
            Ok(
                json!({"entity_kind":if flag==1{"law"}else{"idea"},"entity_id":s.string(p+0x18)?,"category_id":s.string(group+0x38)?}),
            )
        }
        ".?AVCAdvisor@@" => {
            s.typed(p, "CAdvisor")?;
            let person = s.ptr(p + 0x10, "CCharacter")?;
            if s.i(person + 0x68)? != s.i(country + 8)? {
                return Ok(json!({}));
            }
            Ok(
                json!({"entity_kind":"advisor","entity_id":s.string(p+0x30)?,"character_id":s.string(person+0x48)?}),
            )
        }
        _ => Ok(json!({})),
    }
}
struct Walk {
    root: u64,
    country: u64,
    controls: Vec<Value>,
    seen: HashSet<u64>,
}
impl Walk {
    fn visit(
        &mut self,
        s: &mut Sample<impl Memory>,
        c: u64,
        name: &str,
        observer: u64,
        clip: [f64; 4],
        depth: usize,
    ) -> Result<()> {
        if depth > 12 || !self.seen.insert(c) || self.seen.len() > 2048 {
            return Err("Gerarchia GUI azioni oltre limite".into());
        }
        if !s.container(c, self.root, observer)? {
            return Ok(());
        }
        let clip = intersect(clip, s.clip(c)?);
        let mut anchor = None;
        for button in s.array(c + 0x1120, 256)? {
            let vt = s.u(button)?;
            if s.rtti(vt)? != (".?AVCButtonStandard@@".into(), 0) {
                continue;
            }
            s.typed(button, "CButtonStandard")?;
            if self.controls.len() >= 4096 {
                return Err("Troppi controlli GUI azioni".into());
            }
            if s.u(button + 0x68)? != self.root || s.u(button + 0x50)? != observer {
                return Err("Pulsante azione di un altro proprietario".into());
            }
            let def = s.ptr(button + 0x60, "CButtonType")?;
            let control_name = s.string(def + 0x28)?;
            if !buttons(name).contains(&control_name.as_str()) {
                continue;
            }
            let (parent, matrix) = if let Some(pair) = anchor {
                pair
            } else {
                let p = s.ptr(c + 0x15F8, "C2dPositionObject")?;
                let pair = (p, s.matrix(p)?);
                anchor = Some(pair);
                pair
            };
            if s.u(button + 0x1B0)? != parent {
                return Err("Ancoraggio azione incoerente".into());
            }
            let sprite = s.u(button + 0xF0)?;
            let mut bounds = s.bounds(sprite, parent, matrix, clip)?;
            bounds["root"] = json!(name);
            bounds["name"] = json!(control_name);
            bounds["visible"] = json!(bounds["visible"] == true && s.shown(button + 0x75)?);
            if let Ok(identity) = payload(s, button, self.country) {
                bounds
                    .as_object_mut()
                    .unwrap()
                    .extend(identity.as_object().unwrap().clone());
            }
            self.controls.push(bounds);
        }
        for pointer in s.array(c + 0x10F0, 2048)? {
            let vt = s.u(pointer)?;
            if s.rtti(vt)? != (".?AVCContainerWindow@@".into(), 48) {
                continue;
            }
            if name == "topbar" {
                let def = s.ptr(pointer - 48 + 0x90, "CContainerWindowType")?;
                if ![
                    "country_menu_window",
                    "after_faction_button",
                    "after_intel_agency_button",
                ]
                .contains(&s.string(def + 0x28)?.as_str())
                {
                    continue;
                }
            }
            self.visit(s, pointer - 48, name, observer, clip, depth + 1)?;
        }
        Ok(())
    }
}
pub(crate) fn controls(
    s: &mut Sample<impl Memory>,
    root: u64,
    roots: &Value,
    resolution: [i32; 2],
    country: u64,
) -> Value {
    let result = (|| -> Result<Value> {
        let mut panels = json!({});
        let mut controls = vec![];
        for (address, name) in roots.as_object().ok_or("Missing action GUI roots")? {
            let c = address
                .parse::<u64>()
                .map_err(|_| "Invalid action GUI root")?;
            let name = name.as_str().ok_or("Invalid action root name")?;
            s.typed(c, "CContainerWindow")?;
            let def = s.ptr(c + 0x90, "CContainerWindowType")?;
            if s.string(def + 0x28)? != name {
                return Err("Contesto GUI azione cambiato".into());
            }
            let observer = s.u(c + 0x80)?;
            let shown = s.container(c, root, observer)?;
            if panels.get(name).is_some() {
                return Err("Viste azione duplicate".into());
            }
            panels[name] = json!(shown);
            if shown {
                let mut walk = Walk {
                    root,
                    country,
                    controls: vec![],
                    seen: HashSet::new(),
                };
                walk.visit(
                    s,
                    c,
                    name,
                    observer,
                    [0., 0., resolution[0] as f64, resolution[1] as f64],
                    0,
                )?;
                controls.extend(walk.controls);
            }
        }
        let politics = ["topbar", "countrypoliticsview", "political_ideas_window"]
            .iter()
            .all(|k| panels.get(k).is_some());
        let projects = [
            "topbar",
            "countrytechnologyview",
            "facilities_view",
            "program_window",
        ]
        .iter()
        .all(|k| panels.get(k).is_some());
        Ok(
            json!({"status":if politics||projects{"observed"}else{"partial"},"panels":panels,"controls":controls,"coverage":{"politics":politics,"projects":projects},"reason":"Controlli nativi qualificati; modelli o specializzazioni non riconosciuti restano sconosciuti."}),
        )
    })();
    result.unwrap_or_else(|e| json!({"status":"unavailable","panels":{},"controls":[],"reason":e}))
}
pub(crate) fn action_roots(s: &mut Sample<impl Memory>, root: u64, seen: &HashSet<u64>) -> Value {
    let mut roots = BTreeMap::new();
    let mut candidates: Vec<_> = seen.iter().copied().collect();
    candidates.sort_unstable();
    for candidate in candidates {
        let result = (|| -> Result<()> {
            s.typed(candidate, "CContainerWindow")?;
            let def = s.ptr(candidate + 0x90, "CContainerWindowType")?;
            let name = s.string(def + 0x28)?;
            if ROOTS.contains(&name.as_str()) && s.u(candidate + 0x98)? == root {
                roots.insert(candidate.to_string(), name);
            }
            Ok(())
        })();
        let _ = result;
    }
    json!(roots)
}
