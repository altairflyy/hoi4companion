//! DLC preflight is distinct from unresolved identities and dynamic eligibility.
use crate::{
    entity_catalog,
    import_catalog::Index,
    script_document::{blocks, entries, first, scalar},
    scripted_names, source_eligibility,
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn and(v: &[Option<bool>]) -> Option<bool> {
    if v.contains(&Some(false)) {
        Some(false)
    } else if v.contains(&None) {
        None
    } else {
        Some(true)
    }
}
fn or(v: &[Option<bool>]) -> Option<bool> {
    if v.contains(&Some(true)) {
        Some(true)
    } else if v.contains(&None) {
        None
    } else {
        Some(false)
    }
}
fn logic(node: &Value, country: &str) -> Value {
    json!({"entries":entries(node).map(|(key,value)|{let upper=key.to_uppercase();let key=if ["AND","OR","NOT","NAND","NOR"].contains(&upper.as_str()){upper.as_str()}else if country.is_empty()&&["tag","original_tag"].contains(&key){"unobserved_country"}else{key};json!([key,if value.get("entries").is_some(){logic(value,country)}else{value.clone()}])}).collect::<Vec<_>>()})
}
fn tags(node: &Value, scripts: &Value, seen: &mut HashSet<String>, result: &mut BTreeSet<String>) {
    for (key, value) in entries(node) {
        if ["tag", "original_tag"].contains(&key) && crate::recipient::tag(s(value)) {
            result.insert(s(value).into());
        } else if value.get("entries").is_some() {
            tags(value, scripts, seen, result);
        } else if let Some(script) = scripts.get(key)
            && seen.len() < 30
            && seen.insert(key.into())
        {
            tags(script, scripts, seen, result);
            seen.remove(key);
        }
    }
}
fn dlc_names(
    node: &Value,
    scripts: &Value,
    seen: &mut HashSet<String>,
    names: &mut BTreeSet<String>,
) {
    for (key, value) in entries(node) {
        if key == "has_dlc" && value.is_string() {
            names.insert(s(value).into());
        } else if value.get("entries").is_some() {
            dlc_names(value, scripts, seen, names);
        } else if let Some(script) = scripts.get(key)
            && seen.len() < 30
            && seen.insert(key.into())
        {
            dlc_names(script, scripts, seen, names);
            seen.remove(key);
        }
    }
}
fn inventory(base: &Value, changes: &[(String, bool)]) -> Value {
    let mut result = base.clone();
    let mut items = arr(&base["items"]).to_vec();
    for (name, enabled) in changes {
        let norm = hoi4companion_catalog_core::dlc_name(name);
        items.retain(|item| hoi4companion_catalog_core::dlc_name(s(&item["name"])) != norm);
        items.push(json!({"name":name,"status":if *enabled{"enabled"}else{"disabled"}}));
    }
    result["items"] = json!(items);
    result
}
pub fn index_data(source: &Value, metadata: &Value) -> Result<Value, String> {
    let mut raw = Map::new();
    for (key, node) in source["technology_definitions"]["technologies"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let unlocks = blocks(node, "enable_equipments")
            .flat_map(entries)
            .filter_map(|(_, v)| {
                v.as_str()
                    .map(|id| json!({"kind":"Equipaggiamento","id":id}))
            })
            .collect::<Vec<_>>();
        raw.insert(
            key.clone(),
            json!({"name":source["labels"].get(key).unwrap_or(&json!(key)),"unlocks":unlocks}),
        );
    }
    let mut rows = entity_catalog::entities(
        &json!({"trees":source["all_trees"],"technologies":raw,"labels":source["labels"],"reference":source["reference_labels"],"continuous":source["continuous_focuses"]}),
    )?;
    let expander =
        scripted_names::Expander::new(&source["scripted_names"], &source["reference_labels"]);
    for row in rows.as_array_mut().unwrap() {
        if row["kind"] == "focus" && row["continuous"] != true && arr(&row["countries"]).is_empty()
        {
            let selector = &source["all_trees"][s(&row["scope"])]["country_selector"];
            if selector.get("entries").is_some() {
                let mut values = BTreeSet::new();
                tags(
                    selector,
                    &source["scripts"],
                    &mut HashSet::new(),
                    &mut values,
                );
                row["countries"] = json!(values);
            }
        }
        let countries = arr(&row["countries"]).iter().map(s).collect::<Vec<_>>();
        let aliases = expander.expand(s(&row["name"]), &countries);
        let mut values = arr(&row["aliases"]).to_vec();
        for alias in aliases {
            if !values.iter().any(|v| v == &alias) {
                values.push(json!(alias));
            }
        }
        row["aliases"] = json!(values);
    }
    Ok(
        json!({"entries":rows,"country_names":metadata.get("country_names").unwrap_or(&json!({})),"adjectives":metadata.get("adjectives").unwrap_or(&json!([]))}),
    )
}
pub struct Check {
    source: Arc<Value>,
    index: Index,
    cache: Map<String, Value>,
}
impl Check {
    pub fn new(source: Value, metadata: &Value) -> Result<Self, String> {
        Self::shared(Arc::new(source), metadata)
    }
    pub fn shared(source: Arc<Value>, metadata: &Value) -> Result<Self, String> {
        let data = index_data(&source, metadata)?;
        Ok(Self {
            source,
            index: Index::new(&data)?,
            cache: Map::new(),
        })
    }
    fn focus(&self, entry: &Value, country: &str, inventory: &Value) -> Option<bool> {
        if entry["continuous"] == true {
            return Some(true);
        }
        let tree = self.source["all_trees"].get(s(&entry["scope"]))?;
        let selector = &tree["country_selector"];
        let mut countries = BTreeSet::new();
        if !country.is_empty() {
            countries.insert(country.into());
        } else if selector.get("entries").is_some() {
            tags(
                selector,
                &self.source["scripts"],
                &mut HashSet::new(),
                &mut countries,
            );
        }
        if countries.is_empty() {
            countries.insert(String::new());
        }
        let scores = if selector.get("entries").is_some() {
            countries
                .iter()
                .map(|tag| {
                    source_eligibility::score(
                        &logic(selector, tag),
                        tag,
                        inventory,
                        &self.source["scripts"],
                    )
                    .map(|v| v > 0.)
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let root = if scores.is_empty() { None } else { or(&scores) };
        fn visit(
            key: &str,
            tree: &Value,
            country: &str,
            inventory: &Value,
            scripts: &Value,
            seen: &mut HashSet<String>,
            memo: &mut HashMap<String, Option<bool>>,
        ) -> Option<bool> {
            if let Some(answer) = memo.get(key) {
                return *answer;
            }
            let focus = tree["focuses"].get(key)?;
            if seen.len() > 512 || !seen.insert(key.into()) {
                return None;
            }
            let node = &tree["branch_conditions"][key];
            let own = if node.get("entries").is_some() {
                source_eligibility::condition(&logic(node, country), country, inventory, scripts)
            } else {
                Some(true)
            };
            let mut values = vec![own];
            for group in arr(&focus["prerequisites"]) {
                values.push(or(&arr(group)
                    .iter()
                    .map(|p| visit(s(p), tree, country, inventory, scripts, seen, memo))
                    .collect::<Vec<_>>()));
            }
            let answer = and(&values);
            seen.remove(key);
            memo.insert(key.into(), answer);
            answer
        }
        and(&[
            root,
            visit(
                s(&entry["id"]),
                tree,
                country,
                inventory,
                &self.source["scripts"],
                &mut HashSet::new(),
                &mut HashMap::new(),
            ),
        ])
    }
    fn tech(&self, entry: &Value, country: &str, inventory: &Value) -> Option<bool> {
        let defs = &self.source["technology_definitions"];
        let node = defs["technologies"].get(s(&entry["id"]))?;
        let own = first(node, "allow_branch")
            .filter(|n| n.get("entries").is_some())
            .map(|n| {
                source_eligibility::condition(
                    &logic(n, country),
                    country,
                    inventory,
                    &self.source["scripts"],
                )
            })
            .unwrap_or(Some(true));
        let folders = blocks(node, "folder")
            .map(|folder| {
                let item = defs["folders"].get(scalar(folder, "name"))?;
                first(item, "available")
                    .filter(|n| n.get("entries").is_some())
                    .map(|n| {
                        source_eligibility::condition(
                            &logic(n, country),
                            country,
                            inventory,
                            &self.source["scripts"],
                        )
                    })
                    .unwrap_or(Some(true))
            })
            .collect::<Vec<_>>();
        and(&[
            own,
            if folders.is_empty() {
                Some(true)
            } else {
                or(&folders)
            },
        ])
    }
    fn names(&self, entry: &Value) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        let scripts = &self.source["scripts"];
        if entry["kind"] == "technology" {
            let defs = &self.source["technology_definitions"];
            if let Some(node) = defs["technologies"].get(s(&entry["id"])) {
                if let Some(branch) = first(node, "allow_branch") {
                    dlc_names(branch, scripts, &mut HashSet::new(), &mut names);
                }
                for folder in blocks(node, "folder") {
                    if let Some(item) = defs["folders"].get(scalar(folder, "name"))
                        && let Some(rule) = first(item, "available")
                    {
                        dlc_names(rule, scripts, &mut HashSet::new(), &mut names);
                    }
                }
            }
            return names;
        }
        let Some(tree) = self.source["all_trees"].get(s(&entry["scope"])) else {
            return names;
        };
        dlc_names(
            &tree["country_selector"],
            scripts,
            &mut HashSet::new(),
            &mut names,
        );
        let mut pending = vec![s(&entry["id"]).to_owned()];
        let mut seen = HashSet::new();
        while let Some(key) = pending.pop() {
            if !seen.insert(key.clone()) {
                continue;
            }
            dlc_names(
                &tree["branch_conditions"][&key],
                scripts,
                &mut HashSet::new(),
                &mut names,
            );
            if let Some(focus) = tree["focuses"].get(&key) {
                pending.extend(
                    arr(&focus["prerequisites"])
                        .iter()
                        .flat_map(arr)
                        .map(|p| s(p).to_owned()),
                );
            }
        }
        names
    }
    fn evaluate(
        &self,
        matches: &[Value],
        country: &str,
        changes: &[(String, bool)],
    ) -> Option<bool> {
        let inventory = inventory(&self.source["dlcs"], changes);
        or(&matches
            .iter()
            .map(|entry| {
                if entry["kind"] == "focus" {
                    self.focus(entry, country, &inventory)
                } else {
                    self.tech(entry, country, &inventory)
                }
            })
            .collect::<Vec<_>>())
    }
    pub fn assess(&mut self, guide: &Value) -> Result<Value, String> {
        let refs = guide["tracks"]
            .as_object()
            .into_iter()
            .flat_map(|m| m.values())
            .flat_map(arr)
            .filter(|r| ["select_focus", "research"].contains(&s(&r["action"]["kind"])))
            .map(|r| (r["key"].clone(), r["action"]["entity"].clone()))
            .collect::<Vec<_>>();
        let scope = &guide["applies_to"];
        let cache_key = json!([guide["title"], scope, refs]).to_string();
        if let Some(result) = self.cache.get(&cache_key) {
            return Ok(result.clone());
        }
        let country = scope["country"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                self.index
                    .query(json!({"op":"country_hint","query":guide["title"],"guide":guide}))
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
            })
            .unwrap_or_default();
        let mut result = json!({"status":"no_known_missing_dlc","blocked":false,"requirements":[],"basis":"declared_identities_and_local_dlc_conditions","unknown":refs.is_empty()});
        for (key, reference) in &refs {
            let kind = s(&reference["kind"]);
            let ref_scope = reference
                .get("scope")
                .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                .unwrap_or(if kind == "focus" {
                    &scope["tree_id"]
                } else {
                    &Value::Null
                });
            let matches = self.index.matches(
                s(&reference["id"]),
                &[kind],
                &json!(country),
                ref_scope,
                false,
            )?;
            if matches.is_empty() {
                result["unknown"] = json!(true);
                continue;
            }
            let current = self.evaluate(&matches, &country, &[]);
            if current != Some(false) {
                if current.is_none() {
                    result["unknown"] = json!(true);
                }
                continue;
            }
            let names = matches
                .iter()
                .flat_map(|e| self.names(e))
                .filter(|name| {
                    source_eligibility::dlc_enabled(&self.source["dlcs"], name).is_some()
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if !(1..=7).contains(&names.len()) {
                result["unknown"] = json!(true);
                continue;
            }
            let mut alternatives = vec![];
            for size in 1..=names.len() {
                let mut groups = vec![];
                fn choose(
                    n: usize,
                    size: usize,
                    start: usize,
                    group: &mut Vec<usize>,
                    out: &mut Vec<Vec<usize>>,
                ) {
                    if group.len() == size {
                        out.push(group.clone());
                        return;
                    }
                    for i in start..n {
                        group.push(i);
                        choose(n, size, i + 1, group, out);
                        group.pop();
                    }
                }
                choose(names.len(), size, 0, &mut vec![], &mut groups);
                for group in groups {
                    let changes = group
                        .iter()
                        .map(|i| {
                            (
                                names[*i].clone(),
                                !source_eligibility::dlc_enabled(&self.source["dlcs"], &names[*i])
                                    .unwrap(),
                            )
                        })
                        .collect::<Vec<_>>();
                    if self.evaluate(&matches, &country, &changes) != Some(false) {
                        alternatives.push(json!({"enable":changes.iter().filter(|(_,v)|*v).map(|(n,_)|n).collect::<Vec<_>>(),"disable":changes.iter().filter(|(_,v)|!*v).map(|(n,_)|n).collect::<Vec<_>>()}));
                    }
                }
                if !alternatives.is_empty() {
                    break;
                }
            }
            if !alternatives.is_empty() {
                result["status"] = json!("dlc_incompatible");
                result["blocked"] = json!(true);
                result["requirements"] = json!(alternatives);
                result["witness"] = json!({"key":key,"query":reference["id"],"candidate_ids":matches.iter().map(|r|r["id"].clone()).collect::<Vec<_>>()});
                break;
            }
            result["unknown"] = json!(true);
        }
        if self.cache.len() >= 256
            && let Some(key) = self.cache.keys().next().cloned()
        {
            self.cache.shift_remove(&key);
        }
        self.cache.insert(cache_key, result.clone());
        Ok(result)
    }
}
pub fn assess(args: &Value) -> Result<Value, String> {
    Check::new(args["sources"].clone(), args)?.assess(&args["guide"])
}
pub fn execute(op: &str, args: &Value) -> Result<Value, String> {
    use std::sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    };
    static CHECKS: OnceLock<Mutex<HashMap<u64, Arc<Mutex<Check>>>>> = OnceLock::new();
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let registry = CHECKS.get_or_init(Default::default);
    match op {
        "compatibility_open" => {
            let check = Check::new(args["sources"].clone(), args)?;
            let mut checks = registry
                .lock()
                .map_err(|_| "Compatibility registry unavailable")?;
            if checks.len() >= 32 {
                return Err("Compatibility handle limit exceeded".into());
            }
            let handle = NEXT.fetch_add(1, Ordering::Relaxed);
            checks.insert(handle, Arc::new(Mutex::new(check)));
            Ok(json!({"handle":handle}))
        }
        "compatibility_release" => Ok(json!(
            registry
                .lock()
                .map_err(|_| "Compatibility registry unavailable")?
                .remove(
                    &args["handle"]
                        .as_u64()
                        .ok_or("Missing compatibility handle")?
                )
                .is_some()
        )),
        "compatibility_assess" => {
            let check = registry
                .lock()
                .map_err(|_| "Compatibility registry unavailable")?
                .get(
                    &args["handle"]
                        .as_u64()
                        .ok_or("Missing compatibility handle")?,
                )
                .cloned()
                .ok_or("Unknown compatibility handle")?;
            check
                .lock()
                .map_err(|_| "Compatibility engine unavailable")?
                .assess(&args["guide"])
        }
        _ => Err("Unknown compatibility operation".into()),
    }
}
impl crate::catalog_generation::Catalog {
    pub fn guide_compatibility(&mut self, guide: &Value) -> Result<Value, String> {
        if self.sources["demo"] == true || self.sources["dlcs"].is_null() {
            return Ok(
                json!({"blocked":false,"status":"unknown","requirements":[],"unknown":true}),
            );
        }
        if self.compatibility.is_none() {
            let metadata = self.identity_data()?;
            self.compatibility = Some(Check::shared(self.sources.clone(), &metadata)?);
        }
        self.compatibility.as_mut().unwrap().assess(guide)
    }
    pub fn word_inventory(&mut self, path: &std::path::Path) -> Result<Value, String> {
        let mut result = crate::word_sources::documents(path)?;
        result["compatibility"] = json!({});
        if self.sources["demo"] == true
            || self.sources["dlcs"].is_null()
            || result["single_document"] == true
        {
            return Ok(result);
        }
        let sizes = crate::word_sources::document_sizes(path)?;
        let documents = arr(&result["documents"])
            .iter()
            .map(|v| s(v).to_owned())
            .collect::<Vec<_>>();
        let country_docs = documents
            .iter()
            .filter(|p| {
                let p = format!("/{p}").to_lowercase();
                p.contains("/country guides/") && !p.contains("/tools/")
            })
            .collect::<Vec<_>>();
        let selected = if country_docs.is_empty() {
            documents.iter().collect::<Vec<_>>()
        } else {
            country_docs
        };
        let mut budget = 64_000_000i64;
        for (count, entry) in selected.into_iter().enumerate() {
            budget = budget.saturating_sub(sizes.get(entry).copied().unwrap_or(0) as i64);
            let unknown =
                json!({"blocked":false,"status":"unknown","requirements":[],"unknown":true});
            let answer = if budget < 0 || count >= 128 {
                unknown
            } else {
                crate::word_sources::prepare_with_vocabulary(
                    path,
                    Some(entry),
                    None,
                    &self.vocabulary,
                )
                .and_then(|packet| crate::authoring::compile(&packet["document"]))
                .and_then(|packet| self.guide_compatibility(&packet["guide"]))
                .unwrap_or(unknown)
            };
            result["compatibility"][entry] = answer;
        }
        Ok(result)
    }
}
