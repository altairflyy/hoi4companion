//! Technology definitions; GUI layout is a separate source projection.
use crate::{
    focus_text,
    script_document::{self, blocks, entries, first, scalar},
    source_eligibility,
};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeSet, HashSet},
    path::Path,
};
fn values(node: &Value) -> Vec<&str> {
    entries(node).filter_map(|(_, v)| v.as_str()).collect()
}
fn normalize(node: &Value) -> Value {
    json!({"entries":entries(node).map(|(key,value)|{
    let upper=key.to_uppercase();let key=if ["AND","OR","NOT","NAND","NOR"].contains(&upper.as_str()){upper.as_str()}else{key};
    json!([key,if value.is_object(){normalize(value)}else{value.clone()}])
}).collect::<Vec<_>>()})
}
fn check(node: Option<&Value>, country: &str, dlcs: &Value, scripts: &Value) -> Option<bool> {
    if !dlcs.is_object() {
        return None;
    }
    source_eligibility::condition(
        &normalize(node.unwrap_or(&Value::Null)),
        country,
        dlcs,
        scripts,
    )
}
fn number(raw: &str, variables: &Map<String, Value>) -> Value {
    let mut raw = raw;
    let mut seen = HashSet::new();
    while raw.starts_with('@') && seen.insert(raw) {
        raw = variables.get(raw).and_then(Value::as_str).unwrap_or("");
    }
    json!(source_eligibility::number(raw))
}
fn paths<'a>(args: &'a Value, key: &str) -> Result<impl Iterator<Item = &'a Value>, String> {
    Ok(args[key]
        .as_array()
        .ok_or_else(|| format!("Missing {key}"))?
        .iter())
}
pub fn load(args: &Value) -> Result<Value, String> {
    let labels = &args["labels"];
    let country = args["country"].as_str().unwrap_or("AUS");
    let dlcs = &args["dlcs"];
    let scripts = &args["scripts"];
    let (mut folders, mut technologies, mut metadata_folders, mut metadata_technologies) =
        (Map::new(), Map::new(), Map::new(), Map::new());
    let (mut warnings, mut excluded, mut excluded_technologies, mut folder_conditions) =
        (vec![], vec![], BTreeSet::new(), Map::new());
    let metadata = args["definitions"] == true;
    for path in paths(args, "tag_files")? {
        let path = Path::new(path.as_str().ok_or("Invalid technology folder path")?);
        let root = match script_document::read(path) {
            Ok(root) => root,
            Err(error) => {
                warnings.push(format!(
                    "Categorie ricerca non leggibili ({}): {error}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
                continue;
            }
        };
        for group in blocks(&root, "technology_folders") {
            for (key, node) in entries(group) {
                if !node.is_object() || scalar(node, "doctrine") == "yes" {
                    continue;
                }
                if metadata {
                    metadata_folders.insert(key.into(), node.clone());
                }
                let available = if blocks(node, "available").next().is_some() {
                    check(first(node, "available"), country, dlcs, scripts)
                } else {
                    Some(true)
                };
                folder_conditions.insert(key.into(), json!(available));
                if available != Some(true) {
                    excluded.push(json!({"id":key,"reason":if available==Some(false){"Condizioni DLC non corrispondenti"}else{"Disponibilità non verificata"}}));
                    continue;
                }
                folders.insert(key.into(),json!({"id":key,"name":focus_text::localise(labels[key].as_str().unwrap_or(key),labels),"ledger":scalar(node,"ledger"),"technologies":[]}));
            }
        }
    }
    for path in paths(args, "files")? {
        let name = path.as_str().ok_or("Invalid technology path")?;
        let path = Path::new(name);
        let root = match script_document::read(path) {
            Ok(root) => root,
            Err(error) => {
                warnings.push(format!(
                    "Tecnologie non leggibili ({}): {error}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
                continue;
            }
        };
        for group in blocks(&root, "technologies") {
            let variables: Map<_, _> = entries(&root)
                .chain(entries(group))
                .filter(|(k, v)| k.starts_with('@') && v.is_string())
                .map(|(k, v)| (k.into(), v.clone()))
                .collect();
            for (key, node) in entries(group) {
                if !node.is_object() || key.starts_with('@') {
                    continue;
                }
                if metadata {
                    metadata_technologies.insert(key.into(), node.clone());
                }
                let fields = &node["projection"];
                let field = |key: &str| {
                    if fields["kind"] == "technology" {
                        fields[key].as_str().unwrap_or("")
                    } else {
                        scalar(node, key)
                    }
                };
                let branch = if blocks(node, "allow_branch").next().is_some() {
                    check(first(node, "allow_branch"), country, dlcs, scripts)
                } else {
                    Some(true)
                };
                if branch != Some(true) {
                    excluded_technologies.insert(key.to_owned());
                    continue;
                }
                excluded_technologies.remove(key);
                let declared: Vec<_> = blocks(node, "folder")
                    .map(|f| scalar(f, "name"))
                    .filter(|s| !s.is_empty())
                    .collect();
                let availability: Vec<_> = declared
                    .iter()
                    .map(|id| folder_conditions.get(*id).and_then(Value::as_bool))
                    .collect();
                let eligible = if availability.contains(&Some(true)) {
                    Some(true)
                } else if !availability.is_empty() && availability.iter().all(|a| *a == Some(false))
                {
                    Some(false)
                } else {
                    None
                };
                let memberships:Vec<_>=blocks(node,"folder").filter(|n|folders.contains_key(scalar(n,"name"))).map(|n|{
                    let position=first(n,"position").unwrap_or(&Value::Null);
                    json!({"id":scalar(n,"name"),"x":number(scalar(position,"x"),&variables),"y":number(scalar(position,"y"),&variables)})
                }).collect();
                let paths:Vec<_>=blocks(node,"path").filter(|n|!scalar(n,"leads_to_tech").is_empty()).map(|n|json!({"target":scalar(n,"leads_to_tech"),"coefficient":number(scalar(n,"research_cost_coeff"),&variables)})).collect();
                let mut unlocks = vec![];
                for (field, title) in [
                    ("enable_equipments", "Equipaggiamento"),
                    ("enable_equipment_modules", "Moduli"),
                    ("enable_subunits", "Unità"),
                ] {
                    for block in blocks(node, field) {
                        for item in values(block) {
                            unlocks.push(json!({"kind":title,"id":item,"name":focus_text::localise(labels[item].as_str().unwrap_or(item),labels)}));
                        }
                    }
                }
                let requirements: Vec<_> = blocks(node, "dependencies")
                    .flat_map(|n| {
                        entries(n)
                            .filter(|(k, v)| !k.is_empty() && v.is_string())
                            .map(|(k, v)| json!({"id":k,"value":v}))
                    })
                    .collect();
                let equipment = unlocks
                    .iter()
                    .find(|u| u["kind"] == "Equipaggiamento")
                    .and_then(|u| u["id"].as_str())
                    .unwrap_or("");
                let keys = [
                    format!("{country}_{key}"),
                    format!("{country}_{equipment}"),
                    key.into(),
                    equipment.into(),
                ];
                let name_key = keys
                    .iter()
                    .find(|k| labels.get(k.as_str()).is_some())
                    .map(String::as_str)
                    .unwrap_or(key);
                let description = keys
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(name_key))
                    .map(|k| format!("{k}_desc"))
                    .find(|k| labels.get(k.as_str()).is_some())
                    .unwrap_or_default();
                let exclusions: Vec<_> = entries(node)
                    .filter(|(k, _)| k.to_lowercase() == "xor")
                    .flat_map(|(_, n)| values(n))
                    .collect();
                let sub: Vec<_> = blocks(node, "sub_technologies").flat_map(values).collect();
                technologies.insert(key.into(),json!({"id":key,"name":focus_text::localise(labels[name_key].as_str().unwrap_or(key),labels),"description":focus_text::localise(labels[description].as_str().unwrap_or(""),labels),"source":crate::installation::display(path),"research_cost":number(field("research_cost"),&variables),"start_year":number(field("start_year"),&variables),"folders":memberships,"paths":paths,"dependencies":requirements,"declared_folders":declared,"catalog_eligible":eligible,"exclusions":exclusions,"sub_technologies":sub,"special_project":field("is_special_project_tech")=="yes","has_conditions":blocks(node,"allow").next().is_some(),"unlocks":unlocks}));
            }
        }
    }
    for tech in technologies.values() {
        for folder in tech["folders"].as_array().into_iter().flatten() {
            folders[folder["id"].as_str().unwrap()]["technologies"]
                .as_array_mut()
                .unwrap()
                .push(tech["id"].clone());
        }
    }
    folders.retain(|_, v| v["technologies"].as_array().is_some_and(|a| !a.is_empty()));
    let mut result = json!({"catalog":{"folders":folders,"technologies":technologies,"warnings":warnings,"excluded_folders":excluded,"excluded_technologies":excluded_technologies},"definitions":{"folders":metadata_folders,"technologies":metadata_technologies}});
    if args["gui_files"]
        .as_array()
        .is_some_and(|files| !files.is_empty())
    {
        result["catalog"] =
            crate::technology_layout::apply(result["catalog"].take(), &args["gui_files"], labels)?;
    }
    Ok(result)
}
