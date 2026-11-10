//! Ordered focus catalog, shared branch closure and relative geometry.
use crate::{
    focus_source_rules, focus_text,
    script_document::{self, blocks, entries, first, scalar},
};
use serde_json::{Map, Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn strings(node: &Value, key: &str) -> Vec<Value> {
    script_document::all(node, key)
        .filter(|v| v.is_string())
        .cloned()
        .collect()
}
fn unique(values: Vec<Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|v| seen.insert(v.to_string()))
        .collect()
}
fn resets(node: &Value) -> bool {
    entries(node).any(|(key, value)| {
        [
            "load_focus_tree",
            "uncomplete_national_focus",
            "uncomplete_national_focuses",
        ]
        .contains(&key)
            || value.is_object() && resets(value)
    })
}
fn number(value: &str, default: f64) -> Value {
    json!(
        value
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .unwrap_or(default)
    )
}
pub(crate) fn build(node: &Value, labels: &Value, source: &str, joint: bool) -> Option<Value> {
    let fields = &node["projection"];
    let typed = fields["kind"] == "focus";
    let field = |key: &str| {
        if typed {
            text(&fields[key])
        } else {
            scalar(node, key)
        }
    };
    let id = field("id");
    if id.is_empty() {
        return None;
    }
    let prerequisites = if typed {
        fields["prerequisites"].clone()
    } else {
        json!(
            blocks(node, "prerequisite")
                .map(|n| strings(n, "focus"))
                .filter(|g| !g.is_empty())
                .collect::<Vec<_>>()
        )
    };
    let mutex = if typed {
        fields["mutually_exclusive"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        blocks(node, "mutually_exclusive")
            .flat_map(|n| strings(n, "focus"))
            .collect()
    };
    let filters = first(node, "search_filters")
        .map(|n| strings(n, ""))
        .unwrap_or_default();
    let variants = blocks(node, "icon")
        .flat_map(|n| {
            entries(n)
                .filter(|(k, _)| k.starts_with("GFX_"))
                .map(|(k, _)| json!(k))
        })
        .collect();
    let key = if scalar(node, "text").is_empty() {
        id
    } else {
        scalar(node, "text")
    };
    Some(
        json!({"id":id,"name":focus_text::localise(labels[key].as_str().unwrap_or(key),labels),"description":focus_text::localise(labels[format!("{key}_desc")].as_str().unwrap_or(""),labels),"icon":field("icon"),"x":number(field("x"),0.),"y":number(field("y"),0.),"relative_to":field("relative_position_id"),"cost":number(field("cost"),10.),"prerequisites":prerequisites,"mutually_exclusive":unique(mutex),"has_conditions":(["available","allow_branch","visible"].iter().any(|key|blocks(node,key).next().is_some())),"has_bypass":blocks(node,"bypass").next().is_some(),"filters":filters,"source":source,"position_warning":false,"icon_variants":unique(variants),"tooltip_sections":focus_text::tooltips(node,labels),"localization_key":key,"bypass_possible":blocks(node,"bypass").any(|n|entries(n).next().is_some()),"sequence_effects_safe":!blocks(node,"completion_reward").any(resets),"external_completion_possible":joint,"repeatable":scalar(node,"repeatable")=="yes","criteria":focus_source_rules::rules(node)}),
    )
}
fn position(tree: &mut Value) {
    fn resolve(
        id: &str,
        nodes: &mut Value,
        coords: &mut HashMap<String, (f64, f64)>,
        visiting: &mut HashSet<String>,
    ) -> (f64, f64) {
        if let Some(value) = coords.get(id) {
            return *value;
        }
        let focus = &nodes[id];
        let x = focus["x"].as_f64().unwrap_or(0.);
        let y = focus["y"].as_f64().unwrap_or(0.);
        let relative = text(&focus["relative_to"]).to_owned();
        if visiting.contains(id) || visiting.len() > 1024 {
            nodes[id]["position_warning"] = json!(true);
            return (x, y);
        }
        visiting.insert(id.into());
        let result = if !relative.is_empty() && nodes.get(&relative).is_some() {
            let (px, py) = resolve(&relative, nodes, coords, visiting);
            (x + px, y + py)
        } else {
            if !relative.is_empty() {
                nodes[id]["position_warning"] = json!(true);
            }
            (x, y)
        };
        visiting.remove(id);
        coords.insert(id.into(), result);
        result
    }
    let nodes = &mut tree["focuses"];
    let ids: Vec<_> = nodes
        .as_object()
        .into_iter()
        .flat_map(|m| m.keys())
        .cloned()
        .collect();
    let mut coords = HashMap::new();
    for id in ids {
        resolve(&id, nodes, &mut coords, &mut HashSet::new());
    }
    for (id, (x, y)) in coords {
        nodes[&id]["x"] = json!(x);
        nodes[&id]["y"] = json!(y);
    }
}
pub fn load(files: &Value, labels: &Value, continuous: bool) -> Result<Value, String> {
    let mut trees = Map::new();
    let mut shared = Map::new();
    let mut shared_branches = Map::new();
    let mut warnings = vec![];
    let mut palettes = Map::new();
    for file in files.as_array().ok_or("Missing focus files")? {
        let path = Path::new(file.as_str().ok_or("Invalid focus path")?);
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let root = match script_document::read(path) {
            Ok(root) => root,
            Err(error) => {
                warnings.push(if continuous {
                    format!("Continuous focus palette {name}: {error}")
                } else {
                    format!("File ignorato: {name} ({error})")
                });
                continue;
            }
        };
        if continuous {
            for palette in blocks(&root, "continuous_focus_palette") {
                for node in blocks(palette, "focus") {
                    if let Some(focus) = build(node, labels, file.as_str().unwrap(), false) {
                        palettes.insert(text(&focus["id"]).into(), focus);
                    }
                }
            }
            continue;
        }
        for (key, value) in entries(&root) {
            if ["shared_focus", "joint_focus"].contains(&key) && value.is_object() {
                if let Some(mut focus) = build(value, labels, &name, key == "joint_focus") {
                    focus["shared_definition"] = json!(true);
                    let id = text(&focus["id"]).to_owned();
                    shared.insert(id.clone(), focus);
                    if blocks(value, "allow_branch").next().is_some() {
                        shared_branches.insert(id, first(value, "allow_branch").unwrap().clone());
                    } else {
                        shared_branches.remove(&id);
                    }
                }
            } else if key == "focus_tree" && value.is_object() {
                let id = scalar(value, "id");
                if id.is_empty() {
                    continue;
                }
                let mut tree = json!({"id":id,"source":name,"focuses":{},"shared":[],"country_selector":first(value,"country"),"default":scalar(value,"default")=="yes","eligible_countries":[],"branch_conditions":{}});
                for (k, v) in entries(value) {
                    if k == "focus" && v.is_object() {
                        if let Some(focus) = build(v, labels, &name, false) {
                            let id = text(&focus["id"]).to_owned();
                            tree["focuses"][&id] = focus;
                            if blocks(v, "allow_branch").next().is_some() {
                                tree["branch_conditions"][&id] =
                                    first(v, "allow_branch").unwrap().clone();
                            }
                        }
                    } else if ["shared_focus", "joint_focus"].contains(&k) && v.is_string() {
                        tree["shared"].as_array_mut().unwrap().push(v.clone());
                    }
                }
                trees.insert(id.into(), tree);
            }
        }
    }
    if continuous {
        return Ok(json!({"focuses":palettes,"warnings":warnings}));
    }
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for (id, focus) in &shared {
        let parents: HashSet<_> = focus["prerequisites"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|g| g.as_array().into_iter().flatten())
            .map(text)
            .collect();
        for parent in parents {
            children.entry(parent.into()).or_default().push(id.clone());
        }
    }
    for tree in trees.values_mut() {
        let mut branch: Vec<_> = tree["shared"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| text(v).to_owned())
            .collect();
        let mut expanded = HashSet::new();
        while let Some(id) = branch.pop() {
            if !expanded.insert(id.clone()) {
                continue;
            }
            if let Some(children) = children.get(&id) {
                branch.extend(children.iter().cloned());
            }
        }
        let mut pending: Vec<_> = expanded.into_iter().collect();
        pending.sort_by(|a, b| b.cmp(a));
        while let Some(id) = pending.pop() {
            if tree["focuses"].get(&id).is_some() {
                continue;
            }
            let Some(focus) = shared.get(&id) else {
                warnings.push(format!(
                    "Focus condiviso non trovato: {id} ({})",
                    text(&tree["id"])
                ));
                continue;
            };
            tree["focuses"][&id] = focus.clone();
            if let Some(branch) = shared_branches.get(&id) {
                tree["branch_conditions"][&id] = branch.clone();
            }
            for parent in focus["prerequisites"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|g| g.as_array().into_iter().flatten())
                .map(text)
            {
                if shared.contains_key(parent) && tree["focuses"].get(parent).is_none() {
                    pending.push(parent.into());
                }
            }
        }
        position(tree);
    }
    Ok(json!({"trees":trees,"warnings":warnings}))
}

/// Effective, tree-scoped shared definitions for the native owner check. Local
/// focuses and shared definitions not actually expanded into a tree are absent.
pub(crate) fn shared_members(trees: &Value) -> Value {
    Value::Object(trees.as_object().into_iter().flatten().map(|(tree, value)| {
        let members = value["focuses"].as_object().into_iter().flatten()
            .filter(|(_, focus)| focus["shared_definition"] == true)
            .map(|(id, _)| (id.clone(), json!(true))).collect();
        (tree.clone(), Value::Object(members))
    }).collect())
}

#[cfg(test)]
mod membership_tests {
    use super::*;
    #[test]
    fn only_expanded_shared_definitions_are_admitted_per_tree() {
        let trees=json!({"one":{"focuses":{"local":{},"shared":{"shared_definition":true}}},"two":{"focuses":{"local":{}}}});
        let members=shared_members(&trees);
        assert_eq!(members,json!({"one":{"shared":true},"two":{}}));
        assert_eq!(shared_members(&Value::Null),json!({}));
    }
}
