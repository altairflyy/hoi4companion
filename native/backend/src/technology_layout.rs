//! Source GUI geometry, independent of screen capture, browser and memory I/O.
use crate::{
    focus_text,
    script_document::{self, blocks, entries, first, scalar},
    source_eligibility::number,
};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
};
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
fn numeric(node: &Value, key: &str) -> Option<f64> {
    number(scalar(node, key))
}
fn metric(raw: &str, reference: f64) -> Option<f64> {
    let value = number(raw.trim_end_matches('%'))?;
    let value = if raw.ends_with('%') {
        value * reference / 100.
    } else {
        value
    };
    value.is_finite().then_some(value)
}
fn zero(value: Option<f64>) -> f64 {
    value.unwrap_or(0.)
}
fn nonzero(value: f64, default: f64) -> f64 {
    if value == 0. { default } else { value }
}
struct Item<'a> {
    kind: String,
    node: &'a Value,
    x: f64,
    y: f64,
}
fn walk<'a>(node: &'a Value, x: f64, y: f64, width: f64, height: f64, out: &mut Vec<Item<'a>>) {
    for (kind, child) in entries(node) {
        if !child.is_object() {
            continue;
        }
        let position = first(child, "position").unwrap_or(&Value::Null);
        let mut px = metric(scalar(position, "x"), width);
        let py = metric(scalar(position, "y"), height);
        if scalar(child, "name") == "armour_subtitle_heavy" && scalar(position, "x") == "275k" {
            px = Some(275.);
        }
        if !scalar(position, "x").is_empty() && px.is_none()
            || !scalar(position, "y").is_empty() && py.is_none()
        {
            continue;
        }
        let (mut cx, mut cy) = (x + zero(px), y + zero(py));
        let size = first(child, "size").unwrap_or(&Value::Null);
        let minimum = first(size, "min").unwrap_or(&Value::Null);
        let cw = nonzero(
            zero(metric(scalar(size, "width"), width))
                .max(zero(metric(scalar(minimum, "width"), width))),
            width,
        );
        let ch = nonzero(
            zero(metric(scalar(size, "height"), height))
                .max(zero(metric(scalar(minimum, "height"), height))),
            height,
        );
        let orientation = first(child, "Orientation")
            .and_then(Value::as_str)
            .unwrap_or(scalar(child, "orientation"))
            .to_uppercase();
        if ["CENTER_UP", "CENTER_DOWN", "CENTER"].contains(&orientation.as_str()) {
            cx += width / 2.;
        }
        if ["CENTER_UP", "CENTER_DOWN", "CENTER"]
            .contains(&scalar(child, "origo").to_uppercase().as_str())
        {
            cx -= cw / 2.;
        }
        if ["UPPER_RIGHT", "LOWER_RIGHT"].contains(&orientation.as_str()) {
            cx += width;
        }
        if ["LOWER_LEFT", "LOWER_RIGHT"].contains(&orientation.as_str()) {
            cy += height;
        }
        let kind = kind.to_lowercase();
        out.push(Item {
            kind: kind.clone(),
            node: child,
            x: cx,
            y: cy,
        });
        if ["containerwindowtype", "guitypes"].contains(&kind.as_str()) {
            walk(child, cx, cy, cw, ch, out);
        }
    }
}
fn defaults(node: &Value) -> Vec<Item<'_>> {
    let mut items = vec![];
    walk(node, 0., 0., 2522., 1276., &mut items);
    items
}
fn metadata(technologies: &Value, warnings: &mut Vec<Value>) -> Map<String, Value> {
    let mut metadata = Map::new();
    let mut sources = HashSet::new();
    for tech in technologies
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
    {
        let Some(path) = tech["source"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        if !sources.insert(path) {
            continue;
        }
        let root = match script_document::read(Path::new(path)) {
            Ok(root) => root,
            Err(error) => {
                warnings.push(json!(format!(
                    "Metadati GUI ricerca non leggibili: {error}"
                )));
                continue;
            }
        };
        for block in blocks(&root, "technologies") {
            for (key, node) in entries(block) {
                if !node.is_object() {
                    continue;
                }
                let mut paths = vec![];
                let mut ignored = vec![];
                for p in blocks(node, "path") {
                    if scalar(p, "ignore_for_layout") == "yes" {
                        ignored.push(scalar(p, "leads_to_tech"));
                    } else {
                        paths.push(scalar(p, "leads_to_tech"));
                    }
                }
                let equipment = (blocks(node, "enable_equipments").next().is_some()
                    || scalar(node, "show_equipment_icon") == "yes")
                    && scalar(node, "force_use_small_tech_layout") != "yes";
                metadata.insert(key.into(),json!({"paths":paths,"ignored":ignored,"equipment":equipment,"gated":blocks(node,"allow_branch").next().is_some()}));
            }
        }
    }
    metadata
}
fn successors(key: &str, metadata: &Map<String, Value>, technologies: &Value) -> Vec<String> {
    if let Some(paths) = metadata.get(key).and_then(|m| m["paths"].as_array()) {
        paths.iter().map(|v| text(v).into()).collect()
    } else {
        technologies[key]["paths"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| text(&p["target"]).into())
            .collect()
    }
}
pub fn apply(mut catalog: Value, files: &Value, labels: &Value) -> Result<Value, String> {
    let mut warnings = catalog["warnings"]
        .as_array()
        .cloned()
        .ok_or("Missing technology warnings")?;
    let metadata = metadata(&catalog["technologies"], &mut warnings);
    for (key, tech) in catalog["technologies"]
        .as_object_mut()
        .ok_or("Missing technologies")?
    {
        tech["layout_ignored_paths"] = metadata
            .get(key)
            .map(|m| m["ignored"].clone())
            .unwrap_or(json!([]));
    }
    let (mut definitions, mut tabs, mut templates) = (Map::new(), Map::new(), Map::new());
    for path in files.as_array().ok_or("Missing GUI files")? {
        let path = Path::new(path.as_str().ok_or("Invalid GUI path")?);
        let root = match script_document::read(path) {
            Ok(root) => root,
            Err(error) => {
                warnings.push(json!(format!(
                    "Disposizione ricerca non leggibile ({}): {error}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )));
                continue;
            }
        };
        for item in defaults(&root) {
            if item.kind != "containerwindowtype" {
                continue;
            }
            let window = item.node;
            let name = scalar(window, "name");
            if name.starts_with("techtree_") && name.ends_with("_item") {
                let position = first(window, "position").unwrap_or(&Value::Null);
                let size = first(window, "size").unwrap_or(&Value::Null);
                templates.insert(name.into(),json!({"offset_x":zero(numeric(position,"x")),"offset_y":zero(numeric(position,"y")),"declared_width":numeric(size,"width"),"declared_height":numeric(size,"height"),"has_name":entries(window).any(|(_,v)|v.is_object()&&scalar(v,"name")=="Name")}));
            }
            if name == "folder_tabs" {
                for child in defaults(window) {
                    if child.kind == "buttontype"
                        && let Some(folder) = scalar(child.node, "name").strip_suffix("_tab")
                    {
                        tabs.insert(folder.into(),json!({"order":child.x,"sprite_name":scalar(child.node,"quadTextureSprite"),"source":crate::installation::display(path)}));
                    }
                }
            } else if catalog["folders"].get(name).is_some() {
                let (mut grids, mut captions) = (vec![], vec![]);
                let margin = first(window, "margin").unwrap_or(&Value::Null);
                let width = 2560. - zero(numeric(margin, "left")) - zero(numeric(margin, "right"));
                let height =
                    1360. - 47. - zero(numeric(margin, "top")) - zero(numeric(margin, "bottom"));
                let mut children = vec![];
                walk(window, 0., 0., width, height, &mut children);
                for child in children {
                    let node = child.node;
                    let child_name = scalar(node, "name");
                    if child.kind == "gridboxtype" && child_name.ends_with("_tree") {
                        let slot = first(node, "slotsize").unwrap_or(&Value::Null);
                        let sw = zero(numeric(slot, "width"));
                        let sh = zero(numeric(slot, "height"));
                        let direction = scalar(node, "format").to_uppercase();
                        if sw > 0.
                            && sh > 0.
                            && ["LEFT", "UP", "UPPER_LEFT"].contains(&direction.as_str())
                        {
                            let size = first(node, "size").unwrap_or(&Value::Null);
                            grids.push(json!({"id":child_name,"root":child_name.strip_suffix("_tree").unwrap(),"x":child.x,"y":child.y,"slot_width":sw,"slot_height":sh,"direction":direction,"width":zero(metric(scalar(size,"width"),width)),"height":zero(metric(scalar(size,"height"),height))}));
                        }
                    } else if child.kind == "instanttextboxtype"
                        && (child_name.contains("subtitle") || child_name.contains("_year"))
                    {
                        let text = scalar(node, "text");
                        if !text.is_empty() {
                            captions.push(json!({"text":focus_text::localise(labels[text].as_str().unwrap_or(text),labels),"x":child.x,"y":child.y,"width":nonzero(zero(numeric(node,"maxWidth")),200.),"height":nonzero(zero(numeric(node,"maxHeight")),32.),"align":first(node,"format").and_then(Value::as_str).unwrap_or("left"),"font":scalar(node,"font"),"kind":if child_name.contains("_year"){"year"}else{"title"}}));
                        }
                    }
                }
                definitions.insert(name.into(),json!({"source":crate::installation::display(path),"grids":grids,"captions":captions,"width":width,"height":height}));
            }
        }
    }
    let folder_ids: Vec<_> = catalog["folders"]
        .as_object()
        .ok_or("Missing folders")?
        .keys()
        .cloned()
        .collect();
    for folder_id in folder_ids {
        catalog["folders"][&folder_id]["tab"] = tabs
            .get(&folder_id)
            .cloned()
            .unwrap_or(json!({"order":10000,"sprite_name":""}));
        catalog["folders"][&folder_id]["sections"] = json!([]);
        let Some(definition) = definitions.get(&folder_id) else {
            continue;
        };
        let grids = definition["grids"].as_array().unwrap();
        let ids: BTreeSet<_> = catalog["folders"][&folder_id]["technologies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| text(v).to_owned())
            .collect();
        let roots: HashSet<_> = grids
            .iter()
            .map(|g| text(&g["root"]))
            .filter(|root| ids.contains(*root))
            .collect();
        let (mut assignments, mut hidden) =
            (HashMap::<String, Vec<&Value>>::new(), BTreeMap::new());
        for grid in grids {
            let root = text(&grid["root"]);
            if ids.contains(root) || !metadata.contains_key(root) {
                continue;
            }
            let (mut pending, mut visited) = (vec![root.to_owned()], HashSet::new());
            while let Some(key) = pending.pop() {
                if !visited.insert(key.clone()) {
                    continue;
                }
                if ids.contains(&key) {
                    hidden.insert(key.clone(),format!("Radice {root} esclusa dalle condizioni DLC; il discendente rimane nei dati locali."));
                }
                if let Some(m) = metadata.get(&key) {
                    pending.extend(
                        m["paths"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|v| text(v).to_owned()),
                    );
                }
            }
        }
        for grid in grids {
            let root = text(&grid["root"]);
            if !roots.contains(root) {
                continue;
            }
            let (mut pending, mut visited) = (vec![root.to_owned()], HashSet::new());
            while let Some(key) = pending.pop() {
                if !visited.insert(key.clone())
                    || !ids.contains(&key)
                    || roots.contains(key.as_str()) && key != root
                {
                    continue;
                }
                assignments.entry(key.clone()).or_default().push(grid);
                pending.extend(successors(&key, &metadata, &catalog["technologies"]));
            }
        }
        let mut mapped = 0;
        let mut counts = HashMap::new();
        for key in &ids {
            let Some(tech) = catalog["technologies"].get_mut(key) else {
                return Err("Technology membership without definition".into());
            };
            if let Some(reason) = hidden.get(key) {
                tech["layout_hidden_reason"] = json!(reason);
            }
            let membership = tech["folders"]
                .as_array_mut()
                .ok_or("Invalid memberships")?
                .iter_mut()
                .find(|m| m["id"] == folder_id)
                .ok_or("Missing folder membership")?;
            if let Some(reason) = hidden.get(key) {
                membership["layout_hidden_reason"] = json!(reason);
            }
            let Some(assigned) = assignments.get(key).filter(|v| v.len() == 1) else {
                continue;
            };
            let (Some(x), Some(y)) = (membership["x"].as_f64(), membership["y"].as_f64()) else {
                continue;
            };
            let grid = assigned[0];
            let direction = text(&grid["direction"]);
            let (sx, sy) = if direction == "LEFT" { (y, x) } else { (x, y) };
            let sw = grid["slot_width"].as_f64().unwrap();
            let sh = grid["slot_height"].as_f64().unwrap();
            let (mut ax, mut ay) = (
                grid["x"].as_f64().unwrap() + sx * sw,
                grid["y"].as_f64().unwrap() + sy * sh,
            );
            if direction == "UP" {
                let width = grid["width"].as_f64().unwrap();
                if width != 0. {
                    ax += width / 2. - sw / 2.;
                }
            } else if direction == "LEFT" {
                ay += grid["height"].as_f64().unwrap() / 2. - sh / 2.;
            }
            let equipment = metadata.get(key).is_some_and(|m| m["equipment"] == true);
            let mut template_name = format!(
                "techtree_{folder_id}{}_item",
                if equipment { "" } else { "_small" }
            );
            if !templates.contains_key(&template_name) {
                template_name = format!("techtree_{folder_id}_item");
            }
            let default = json!({"offset_x":0,"offset_y":0,"has_name":false});
            let template = templates.get(&template_name).unwrap_or(&default);
            let named = equipment && template["has_name"] == true;
            let (width, height) = if named {
                if ["naval_folder", "mtgnavalfolder", "mtgnavalsupportfolder"]
                    .contains(&folder_id.as_str())
                {
                    (332., 69.)
                } else {
                    (183., 84.)
                }
            } else {
                (72., 72.)
            };
            let left = ax + template["offset_x"].as_f64().unwrap();
            let top = ay + template["offset_y"].as_f64().unwrap();
            membership["layout"] = json!({"x":left,"y":top,"width":width,"height":height,"center_x":left+width/2.,"center_y":top+height/2.,"anchor_x":ax,"anchor_y":ay,"template":template_name,"template_declared_width":template["declared_width"],"template_declared_height":template["declared_height"],"variant":if named{"equipment"}else{"small"},"direction":direction,"tree_id":grid["id"],"section_id":grid["root"],"basis":"gui_alignment_item_template_and_path"});
            *counts.entry(text(&grid["root"])).or_insert(0) += 1;
            mapped += 1;
        }
        let sections:Vec<_>=grids.iter().filter_map(|grid|{let root=text(&grid["root"]);Some(json!({"id":root,"name":catalog["technologies"][root]["name"],"count":counts.get(root)?,"direction":grid["direction"]}))}).collect();
        catalog["folders"][&folder_id]["sections"] = json!(sections);
        catalog["folders"][&folder_id]["layout"] = json!({"basis":"gui_alignment_and_item_templates","source":definition["source"],"mapped":mapped,"total":ids.len(),"captions":definition["captions"],"width":definition["width"],"height":definition["height"],"reference_viewport_width":2560,"hidden":hidden.iter().map(|(key,reason)|json!({"id":key,"reason":reason})).collect::<Vec<_>>(),"limitations":"Ricostruzione delle griglie GUI su viewport di riferimento 2560 px; controllata sui landmark degli screenshot forniti."});
    }
    catalog["warnings"] = json!(warnings);
    Ok(catalog)
}
