//! Browser/HUD data projections over one immutable installation generation.
use crate::{catalog_generation::Catalog, snapshot};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeSet, HashSet},
    path::Path,
};
type Result<T> = std::result::Result<T, String>;
impl Catalog {
    pub fn overview(&self) -> Value {
        let data = &self.sources;
        let mut dlcs = data["dlcs"].clone();
        if dlcs.is_object() {
            let enabled = dlcs["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| ["enabled", "integrated"].contains(&d["status"].as_str().unwrap_or("")))
                .count();
            dlcs.as_object_mut().unwrap().remove("warnings");
            dlcs["enabled_count"] = json!(enabled);
            dlcs["basis"] =
                json!("File installati e disattivazioni del launcher; non legge la partita.");
        }
        json!({"application":"hoi4companion","settings":data["settings"],"demo":data["demo"],"parser":{"engine":"jomini","engine_version":"0.37.2","transport":"in_process_rust"},"warnings":data["warnings"].as_array().into_iter().flatten().take(70).collect::<Vec<_>>(),"dlcs":dlcs,"mods":data["mod_inventory"],"excluded_trees":data["excluded_trees"],"tree_filter":if data["demo"]==true{"demo"}else{"initial_dlc_compatibility"},"trees":data["trees"].as_object().into_iter().flatten().map(|(key,tree)|json!({"id":key,"count":tree["focuses"].as_object().map_or(0,Map::len),"source":tree["source"],"countries":tree["eligible_countries"]})).collect::<Vec<_>>()})
    }
    pub fn tree(&mut self, id: Option<&str>, include_snapshot: bool) -> Result<Value> {
        let data = &self.sources;
        let id = id.unwrap_or_else(|| data["settings"]["tree_id"].as_str().unwrap_or(""));
        let Some(tree) = data["trees"].get(id) else {
            return Ok(json!({"error":"Albero non trovato"}));
        };
        let (mut snap, mut error) = (Value::Null, String::new());
        let path = data["settings"]["snapshot_path"].as_str().unwrap_or("");
        if include_snapshot && !path.is_empty() {
            match snapshot::read(
                Path::new(path),
                data["settings"]["country"].as_str().unwrap_or("AUS"),
            ) {
                Ok(value) => snap = value,
                Err(e) => error = e,
            }
        }
        let mut statuses = Map::new();
        let mut texts = vec![];
        let mut focuses = vec![];
        let completed: HashSet<_> = snap["completed"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        for focus in tree["focuses"]
            .as_object()
            .into_iter()
            .flat_map(|m| m.values())
        {
            let mut focus = focus.clone();
            focus["estimated_days"] =
                json!((focus["cost"].as_f64().unwrap_or(10.) * 7.).round_ties_even());
            if !snap.is_null() {
                let id = focus["id"].as_str().unwrap_or("");
                let status =
                    if completed.contains(id) {
                        "completed"
                    } else if snap["current"] == id {
                        "active"
                    } else if focus["mutually_exclusive"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|p| p.as_str().is_some_and(|id| completed.contains(id)))
                    {
                        "excluded"
                    } else if !focus["prerequisites"].as_array().into_iter().flatten().all(
                        |group| {
                            group
                                .as_array()
                                .into_iter()
                                .flatten()
                                .any(|p| p.as_str().is_some_and(|id| completed.contains(id)))
                        },
                    ) {
                        "locked"
                    } else if focus["has_conditions"] == true {
                        "verify"
                    } else {
                        "ready"
                    };
                statuses.insert(id.into(), json!(status));
            }
            if let Some(library) = &mut self.assets {
                focus["sprite"] =
                    library.resolve(focus["icon"].as_str().unwrap_or(""), 1)?["sprite"].clone();
                let mut variants = vec![];
                for name in focus["icon_variants"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    let value = library.resolve(name, 1)?["sprite"].clone();
                    if !value.is_null() {
                        variants.push(value);
                    }
                }
                focus["variant_sprites"] = json!(variants);
                for key in ["name", "description"] {
                    texts.push(focus[key].as_str().unwrap_or("").to_owned());
                }
                texts.extend(
                    focus["tooltip_sections"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|s| s["text"].as_str())
                        .map(str::to_owned),
                );
            }
            focuses.push(focus);
        }
        let mut result = json!({"id":id,"source":tree["source"],"count":focuses.len(),"focuses":focuses,"visuals":data["focus_visuals"],"status":statuses,"snapshot":snap,"snapshot_error":error,"demo":data["demo"],"has_snapshot":!snap.is_null()});
        if let Some(library) = &mut self.assets {
            result["inline_sprites"] = library.inline(&texts)?;
            result["theme"] = library.theme()?;
            result["asset_coverage"] = json!({"resolved":focuses.iter().filter(|f|!f["sprite"].is_null()).count(),"conditional":focuses.iter().filter(|f|f["icon_variants"].as_array().is_some_and(|a|!a.is_empty())).count(),"total":focuses.len()});
        }
        Ok(result)
    }
    pub fn technology_catalog(&mut self) -> Result<Value> {
        let mut folders = vec![];
        for folder in self.sources["technologies"]["folders"]
            .as_object()
            .into_iter()
            .flat_map(|m| m.values())
        {
            let mut item = folder.clone();
            item["count"] = json!(folder["technologies"].as_array().map_or(0, Vec::len));
            item.as_object_mut().unwrap().remove("technologies");
            if folder["tab"].is_object() {
                let name = folder["tab"]["sprite_name"].as_str().unwrap_or("");
                for (key, frame) in [("sprite", 1), ("inactive_sprite", 2)] {
                    item["tab"][key] = match &mut self.assets {
                        Some(a) => a.resolve(name, frame)?["sprite"].clone(),
                        None => Value::Null,
                    };
                }
            }
            folders.push(item);
        }
        Ok(
            json!({"folders":folders,"excluded_folders":self.sources["technologies"]["excluded_folders"],"country":self.sources["settings"]["country"].as_str().unwrap_or("AUS").to_uppercase(),"warnings":self.sources["technologies"]["warnings"],"demo":self.sources["demo"]}),
        )
    }
    pub fn technology_tree(&mut self, id: &str) -> Result<Value> {
        let data = &self.sources;
        let Some(folder) = data["technologies"]["folders"].get(id) else {
            return Ok(json!({"error":"Categoria di ricerca non trovata"}));
        };
        let catalog = &data["technologies"]["technologies"];
        let ids: HashSet<_> = folder["technologies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let mut related: BTreeSet<_> = ids.iter().copied().collect();
        let mut pending: Vec<_> = ids.iter().copied().collect();
        while let Some(key) = pending.pop() {
            for child in catalog[key]["sub_technologies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if catalog.get(child).is_some() && related.insert(child) {
                    pending.push(child);
                }
            }
        }
        let country = data["settings"]["country"]
            .as_str()
            .unwrap_or("AUS")
            .to_uppercase();
        let mut result = vec![];
        let mut references = Map::new();
        let mut texts = vec![];
        for key in &related {
            let mut tech = catalog[*key].clone();
            tech["in_folder"] = json!(ids.contains(key));
            tech["sprite"] = Value::Null;
            let membership = tech["folders"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|f| f["id"] == id)
                .cloned()
                .unwrap_or(Value::Null);
            let template = membership["layout"]["template"].as_str().unwrap_or("");
            tech["visual"] = data["technology_visuals"]["templates"][template].clone();
            if let Some(library) = &mut self.assets {
                let compact = membership["layout"]["variant"] == "small";
                let names = if compact {
                    vec![
                        format!("GFX_{country}_{key}"),
                        format!("GFX_{key}"),
                        format!("GFX_{country}_{key}_medium"),
                        format!("GFX_{key}_medium"),
                    ]
                } else {
                    vec![
                        format!("GFX_{country}_{key}_medium"),
                        format!("GFX_{key}_medium"),
                        format!("GFX_{country}_{key}"),
                        format!("GFX_{key}"),
                    ]
                };
                for name in names {
                    let value = library.resolve(&name, 1)?["sprite"].clone();
                    if !value.is_null() {
                        tech["sprite"] = value;
                        break;
                    }
                }
            }
            let refs: Vec<_> = tech["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| &p["target"])
                .chain(
                    tech["dependencies"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|d| &d["id"]),
                )
                .chain(tech["exclusions"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .collect();
            for key in refs {
                if !related.contains(key)
                    && let Some(target) = catalog.get(key)
                {
                    let destination =
                        target["folders"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .find_map(|f| {
                                let id = f["id"].as_str()?;
                                data["technologies"]["folders"].get(id).map(|_| id)
                            });
                    references.insert(
                        key.into(),
                        json!({"name":target["name"],"folder":destination}),
                    );
                }
            }
            texts.extend([
                tech["name"].as_str().unwrap_or("").to_owned(),
                tech["description"].as_str().unwrap_or("").to_owned(),
            ]);
            result.push(tech);
        }
        let inline = if let Some(a) = &mut self.assets {
            a.inline(&texts)?
        } else {
            json!({})
        };
        Ok(
            json!({"id":folder["id"],"name":folder["name"],"technologies":result,"references":references,"layout":folder["layout"],"sections":folder.get("sections").cloned().unwrap_or(json!([])),"visuals":data["technology_visuals"]["folders"].get(id).cloned().unwrap_or(json!({})),"country":country,"inline_sprites":inline,"asset_coverage":{"resolved":result.iter().filter(|t|t["in_folder"]==true&&!t["sprite"].is_null()).count(),"total":ids.len()}}),
        )
    }
}
