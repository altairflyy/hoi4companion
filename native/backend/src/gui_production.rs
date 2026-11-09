//! Same profile-pinned production rows, add buttons and equipment pickers as
//! the compatibility reader. Reuses the qualified country design/name decoder.
use crate::{
    gui_observer::Sample,
    hud_geometry::{intersect, rows},
    live::Memory,
};
use serde_json::{Value, json};
use std::collections::HashSet;
type Result<T> = std::result::Result<T, String>;
const BUILD: &str = "cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f";
fn pointer(v: &Value) -> Result<u64> {
    v.as_u64().ok_or("Invalid production GUI pointer".into())
}
fn button(
    s: &mut Sample<impl Memory>,
    root: u64,
    address: u64,
    owner: u64,
    kind: &str,
    name: &str,
    clip: [f64; 4],
) -> Result<Value> {
    s.typed(address, "CButtonStandard")?;
    let observer = s.u(address + 0x50)?;
    let vt = s.u(observer)?;
    let (typ, offset) = s.rtti(vt)?;
    if typ != format!(".?AV{kind}@@") || observer.checked_sub(offset as u64) != Some(owner) {
        return Err("Foreign production button observer".into());
    }
    if s.u(address + 0x68)? != root {
        return Err("Foreign production GUI root".into());
    }
    let def = s.ptr(address + 0x60, "CButtonType")?;
    if s.string(def + 0x28)? != name {
        return Err("Production button identity changed".into());
    }
    let parent = s.ptr(
        address + s.offset("button_anchor", 0x1B0),
        "C2dPositionObject",
    )?;
    let matrix = s.matrix(parent)?;
    let sprite = s.u(address + s.offset("button_sprite", 0xF0))?;
    let mut bounds = s.bounds(sprite, parent, matrix, clip)?;
    bounds["visible"] = json!(bounds["visible"] == true && s.shown(address + 0x75)?);
    bounds["id"] = json!(name);
    Ok(bounds)
}
fn child(s: &mut Sample<impl Memory>, container: u64, name: &str) -> Result<u64> {
    let mut candidates = vec![];
    for entry in s.array(container + s.offset("container_children", 0x10F0), 2048)? {
        let vt = s.u(entry)?;
        if s.rtti(vt)? != (".?AVCContainerWindow@@".into(), 48) {
            continue;
        }
        let c = entry.checked_sub(48).ok_or("Invalid child container")?;
        let def = s.ptr(c + 0x90, "CContainerWindowType")?;
        if s.string(def + 0x28)? == name {
            candidates.push(c);
        }
    }
    if candidates.len() != 1 {
        return Err("Production viewport missing or ambiguous".into());
    }
    Ok(candidates[0])
}
fn variant(s: &mut Sample<impl Memory>, variant: u64) -> Result<(String, String)> {
    let eq = s.ptr(variant + 0x3F0, "CEquipmentType")?;
    let id = s.string(eq + 0x18)?;
    let base = s.base;
    Ok((id, crate::country::native_name(s, base, variant + 0x28)?))
}
fn controls(
    s: &mut Sample<impl Memory>,
    root: u64,
    bindings: &Value,
    resolution: [i32; 2],
    view: u64,
) -> Result<Value> {
    let screen = [0., 0., resolution[0] as f64, resolution[1] as f64];
    let mut add = vec![];
    let mut context = vec![];
    let mut windows = vec![];
    let mut pickers = vec![];
    let mut entries = vec![];
    for record in rows(&bindings["production_add"]) {
        if record["owner"].as_u64() != Some(view) {
            continue;
        }
        add.push(button(
            s,
            root,
            pointer(&record["address"])?,
            view,
            "CCountryProductionLineView",
            record["name"]
                .as_str()
                .ok_or("Invalid production button name")?,
            screen,
        )?);
    }
    for offset in [0x2730, 0x2738] {
        let address = s.u(view + offset)?;
        if address == 0 {
            continue;
        }
        s.typed(address, "CProductionEquipmentWindow")?;
        if s.u(address + 0x10F8)? != view {
            return Err("Foreign production equipment picker".into());
        }
        let mut members = s.array(address + 0x10C0, 4096)?;
        members.extend(s.array(address + 0x10D8, 4096)?);
        if members.iter().copied().collect::<HashSet<_>>().len() != members.len() {
            return Err("Duplicate production picker entries".into());
        }
        context.push(json!([address, members]));
        windows.push((address, members));
    }
    for record in rows(&bindings["production_pickers"]) {
        let owner = pointer(&record["owner"])?;
        let candidates: Vec<_> = windows.iter().filter(|(a, _)| *a == owner).collect();
        if candidates.len() != 1 {
            continue;
        }
        let c = pointer(&record["container"])?;
        let observer = owner + pointer(&record["observer_offset"])?;
        if !s.container(c, root, observer)? {
            continue;
        }
        let def = s.ptr(c + 0x90, "CContainerWindowType")?;
        if s.string(def + 0x28)? != record["name"].as_str().unwrap_or("") {
            return Err("Production picker identity changed".into());
        }
        let viewport = child(s, c, "equipments")?;
        if !s.container(viewport, root, observer)? {
            continue;
        }
        let clip = s.clip(viewport)?;
        pickers.push(json!({"visible":true,"name":record["name"]}));
        for &item in &candidates[0].1 {
            s.typed(item, "CProductionInfoEquipmentItem")?;
            if s.u(item + 0x11D8)? != owner {
                return Err("Foreign production picker entry".into());
            }
            let var = s.ptr(item + 0x1158, "CEquipmentVariant")?;
            s.ptr(var + 0x3F0, "CEquipmentType")?;
            let row = s.u(item + 0x10)?;
            if !s.container(row, root, item + 24)? {
                continue;
            }
            let mut buttons = vec![];
            for a in s.array(row + s.offset("container_buttons", 0x1120), 256)? {
                let vt = s.u(a)?;
                if s.rtti(vt)? != (".?AVCButtonStandard@@".into(), 0) {
                    continue;
                }
                let def = s.ptr(a + 0x60, "CButtonType")?;
                if s.string(def + 0x28)? == "equipment_button" {
                    buttons.push(a);
                }
            }
            if buttons.len() != 1 {
                continue;
            }
            let mut bounds = button(
                s,
                root,
                buttons[0],
                item,
                "CProductionInfoEquipmentItem",
                "equipment_button",
                clip,
            )?;
            let (equipment, name) = variant(s, var)?;
            let mut attributes = s.profile.clone();
            attributes["variant"] = json!(var);
            attributes["executable_bytes"] =
                json!(!s.memory.fixture_transport() || s.profile["executable_bytes"] == true);
            let request = crate::country::Request {
                schema_version: 1,
                base: s.base,
                world: 0,
                country: 0,
                tag: 0,
                section: "design".into(),
                qualify: false,
                attributes,
            };
            let design = crate::country::collect(s, request)?["value"].clone();
            bounds["equipment"] = json!(equipment);
            bounds["variant_name"] = json!(name);
            bounds["design"] = design;
            entries.push(bounds);
        }
    }
    Ok(
        json!({"add":add,"picker_open":!pickers.is_empty(),"picker_count":pickers.len(),"equipment":entries,"context":context}),
    )
}
pub(crate) fn read(
    s: &mut Sample<impl Memory>,
    root: u64,
    bindings: &Value,
    resolution: [i32; 2],
    country: u64,
) -> Result<Value> {
    if s.profile["sha256"] != BUILD {
        return Ok(json!({"status":"unavailable","reason":"Production GUI profile unavailable"}));
    }
    let owners = &bindings["production_owners"];
    let views: Vec<_> = owners
        .as_object()
        .ok_or("Missing production owners")?
        .iter()
        .filter(|(_, kind)| *kind == "CCountryProductionLineView")
        .map(|(a, _)| a.parse::<u64>())
        .collect();
    if views.len() != 1 {
        return Ok(json!({"status":"unavailable","reason":"Production view missing or ambiguous"}));
    }
    let view = s.typed(
        views[0].clone().map_err(|_| "Invalid production view")?,
        "CCountryProductionLineView",
    )?;
    let c = s.u(view + s.offset("view_container", 0x578))?;
    let visible = s.container(c, root, view + 40)?;
    let def = s.ptr(c + 0x90, "CContainerWindowType")?;
    if s.string(def + 0x28)? != "countryproductionlineview" {
        return Err("Production view identity changed".into());
    }
    let controls = controls(s, root, bindings, resolution, view)?;
    let mut access = vec![];
    let mut lines = vec![];
    let mut pool = vec![];
    let screen = [0., 0., resolution[0] as f64, resolution[1] as f64];
    for address in rows(&bindings["production_access"]) {
        let a = pointer(address)?;
        s.typed(a, "CButtonStandard")?;
        let observer = s.u(a + 0x50)?;
        let vt = s.u(observer)?;
        let (name, offset) = s.rtti(vt)?;
        let owner = observer
            .checked_sub(offset as u64)
            .ok_or("Invalid production access owner")?;
        if name != ".?AVCTopBar@@" || owners[owner.to_string()] != "CTopBar" {
            return Err("Foreign production access control".into());
        }
        let mut b = button(s, root, a, owner, "CTopBar", "production_button", screen)?;
        b.as_object_mut().unwrap().remove("id");
        access.push(b);
    }
    if visible {
        let mut viewport = c;
        for name in ["military_tab", "production_lines"] {
            viewport = child(s, viewport, name)?;
            if !s.container(viewport, root, view + 40)? {
                return Err("Production viewport hidden".into());
            }
        }
        let viewport_clip = s.clip(viewport)?;
        let status = s.ptr(country + 0xF68, "CProductionStatus")?;
        let off = s.profile["production_owner_offset"]
            .as_u64()
            .unwrap_or(0x290);
        if s.u(status + off)? != country {
            return Err("Foreign production owner".into());
        }
        pool = s.array(status + 0x58, 4096)?;
        for (address, kind) in owners.as_object().unwrap() {
            if kind != "CProductionLineMilitaryItem" {
                continue;
            }
            let item = address
                .parse::<u64>()
                .map_err(|_| "Invalid production row")?;
            s.typed(item, "CProductionLineMilitaryItem")?;
            let line = s.u(item + 0x3878)?;
            let Some(index) = pool.iter().position(|p| *p == line) else {
                continue;
            };
            if s.u(item + 0xB90)? != view {
                return Err("Foreign production view".into());
            }
            s.typed(line, "CMilitaryProductionLine")?;
            if s.u(line + 0x20)? != status {
                return Err("Foreign production line".into());
            }
            let var = s.ptr(line + 0x88, "CEquipmentVariant")?;
            s.ptr(var + 0x3F0, "CEquipmentType")?;
            let row = s.u(item + 0x10)?;
            if !s.container(row, root, item + 0xB48)? {
                continue;
            }
            let pos = s.ptr(
                row + s.offset("container_outer", 0x15F0),
                "C2dPositionObject",
            )?;
            let matrix = s.matrix(pos)?;
            let width = s.i(row + 0x17AC)?;
            let height = s.i(row + 0x17B0)?;
            if !(8..=resolution[0]).contains(&width) || !(8..=resolution[1]).contains(&height) {
                return Err("Production row size unqualified".into());
            }
            if matrix.iter().any(|v| !v.is_finite())
                || !(0.01..=10.).contains(&matrix[0])
                || (matrix[0] - matrix[5]).abs() > 0.0001
                || [1, 2, 3, 4, 6, 7, 8, 9, 11]
                    .iter()
                    .any(|i| matrix[*i].abs() > 0.0001)
            {
                return Err("Production row transform unqualified".into());
            }
            let rect = [
                matrix[12],
                -matrix[13],
                width as f64 * matrix[0],
                height as f64 * matrix[5],
            ];
            let clip = intersect(
                intersect(
                    intersect(intersect(rect, s.clip(c)?), viewport_clip),
                    s.clip(row)?,
                ),
                screen,
            );
            let (equipment, name) = variant(s, var)?;
            lines.push(json!({"line_id":(index+1).to_string(),"equipment":equipment,"variant_name":name,"rect":rect,"clip":clip,"visible":clip[2]>0. && clip[3]>0.}));
        }
    }
    Ok(
        json!({"status":"observed","visible":visible,"access":access,"lines":lines,"add":controls["add"],"picker_open":controls["picker_open"],"picker_count":controls["picker_count"],"equipment":controls["equipment"],"discovery_context":[pool,controls["context"]]}),
    )
}
