//! Static guide presentation enrichment. Acquisition and authored intent remain
//! separate; a catalog match never proves availability or live completion.
use crate::{focus_text, guide_schema};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::OnceLock,
};
type Result<T> = std::result::Result<T, String>;
fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.),
    }
}
fn quote(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => {
                out.push(*b as char)
            }
            b' ' => out.push('+'),
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
struct Glossary {
    spelling: String,
    rows: Vec<Value>,
}
fn glossary_items(vocabulary: &Value) -> Vec<Glossary> {
    let mut values: Map<String, Value> = Map::new();
    for (domain, codes) in vocabulary["abbreviations"]
        .as_object()
        .into_iter()
        .flatten()
    {
        for (code, meaning) in codes.as_object().into_iter().flatten() {
            if code.chars().count() < 2 {
                continue;
            }
            for spelling in BTreeSet::from([code.clone(), code.to_uppercase()]) {
                values
                    .entry(spelling)
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"code":code,"meaning":meaning,"domain":domain}));
            }
        }
    }
    let mut rows: Vec<_> = values
        .into_iter()
        .map(|(spelling, rows)| Glossary {
            spelling,
            rows: rows.as_array().cloned().unwrap_or_default(),
        })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.spelling.chars().count()));
    rows
}
fn default_glossary() -> &'static Vec<Glossary> {
    static ITEMS: OnceLock<Vec<Glossary>> = OnceLock::new();
    ITEMS.get_or_init(|| glossary_items(crate::import_catalog::language()))
}
pub fn glossary(text: &str, category: &str, vocabulary: Option<&Value>) -> Value {
    let custom = vocabulary.map(glossary_items);
    glossary_terms(
        text,
        category,
        custom.as_ref().unwrap_or_else(|| default_glossary()),
    )
}
fn glossary_terms(text: &str, category: &str, items: &[Glossary]) -> Value {
    let domain = if ["construction", "production"].contains(&category) {
        Some(category)
    } else {
        None
    };
    let mut result = vec![];
    let mut seen = HashSet::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let valid_before = cursor == 0 || !text.as_bytes()[cursor - 1].is_ascii_alphanumeric();
        let found = valid_before
            .then(|| {
                items.iter().find(|item| {
                    text[cursor..].starts_with(&item.spelling)
                        && text
                            .as_bytes()
                            .get(cursor + item.spelling.len())
                            .is_none_or(|b| !b.is_ascii_alphanumeric())
                })
            })
            .flatten();
        if let Some(item) = found {
            let scoped: Vec<_> = item
                .rows
                .iter()
                .filter(|r| domain.is_some_and(|d| r["domain"] == d))
                .collect();
            let candidates = if scoped.is_empty() {
                item.rows.iter().collect::<Vec<_>>()
            } else {
                scoped
            };
            let meanings: HashSet<_> = candidates
                .iter()
                .map(|v| v["meaning"].to_string())
                .collect();
            if meanings.len() == 1
                && let Some(entry) = candidates.first()
            {
                let code = entry["code"].as_str().unwrap_or("");
                if seen.insert(code.to_owned()) {
                    result.push((*entry).clone());
                    if result.len() == 24 {
                        break;
                    }
                }
            }
            cursor += item.spelling.len();
        } else {
            cursor += text[cursor..].chars().next().unwrap().len_utf8();
        }
    }
    json!(result)
}
fn folder<'a>(tech: &'a Value, folders: &Value) -> Option<&'a str> {
    tech["folders"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|f| {
            f["id"]
                .as_str()
                .filter(|id| folders.get(*id).is_some() && truth(&f["layout"]))
        })
}
pub fn enrich(args: &Value) -> Result<Value> {
    let custom_glossary = args.get("vocabulary").map(glossary_items);
    let glossary_entries = custom_glossary
        .as_ref()
        .unwrap_or_else(|| default_glossary());
    enrich_view(View {
        guide: &args["guide"],
        trees: &args["trees"],
        technologies: &args["technologies"],
        labels: &args["labels"],
        folders: &args["folders"],
        continuous: &args["continuous"],
        glossary_entries,
    })
}
struct View<'a> {
    guide: &'a Value,
    trees: &'a Value,
    technologies: &'a Value,
    labels: &'a Value,
    folders: &'a Value,
    continuous: &'a Value,
    glossary_entries: &'a [Glossary],
}
pub struct Presentation {
    source: std::sync::Arc<Value>,
    glossary: Vec<Glossary>,
}
impl Presentation {
    pub fn new(source: std::sync::Arc<Value>, vocabulary: &Value) -> Self {
        Self {
            source,
            glossary: glossary_items(vocabulary),
        }
    }
    pub fn enrich(&self, guide: &Value) -> Result<Value> {
        let source = &self.source;
        let mut guide = enrich_view(View {
            guide,
            trees: &source["trees"],
            technologies: &source["technologies"]["technologies"],
            labels: &source["labels"],
            folders: &source["technologies"]["folders"],
            continuous: &source["continuous_focuses"],
            glossary_entries: &self.glossary,
        })?;
        guide["catalog_country"] = json!(
            source["settings"]["country"]
                .as_str()
                .unwrap_or("")
                .to_uppercase()
        );
        guide["demo"] = source["demo"].clone();
        project_costs(guide, &source["operation_entities"])
    }
}
fn enrich_view(view: View<'_>) -> Result<Value> {
    let View {
        guide,
        trees,
        technologies,
        labels,
        folders,
        continuous,
        glossary_entries,
    } = view;
    let mut guide = guide_schema::normalize(guide)?;
    let tree_default = guide["tree_id"].as_str().map(str::to_owned);
    let country = guide["country"].as_str().unwrap_or("").to_owned();
    let mut warnings = guide["association_warnings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for track in guide["tracks"].as_object_mut().unwrap().values_mut() {
        for step in track.as_array_mut().unwrap() {
            let kind = step["kind"].as_str().unwrap_or("").to_owned();
            let target = step["target"].as_str().unwrap_or("").to_owned();
            if kind == "focus" {
                let mut tree_id = step["action"]["entity"]["scope"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .or(tree_default.clone());
                if tree_id.as_ref().is_none_or(String::is_empty) {
                    let matches: Vec<_> = trees
                        .as_object()
                        .into_iter()
                        .flatten()
                        .filter(|(_, t)| t["focuses"].get(&target).is_some())
                        .map(|(id, _)| id.clone())
                        .collect();
                    if matches.len() == 1 {
                        tree_id = Some(matches[0].clone());
                    }
                }
                let tree = tree_id
                    .as_ref()
                    .and_then(|id| trees.get(id))
                    .unwrap_or(&Value::Null);
                let palette = continuous.get(&target);
                let focus = palette.or_else(|| tree["focuses"].get(&target));
                step["catalog_match"] = json!(focus.is_some());
                if let Some(focus) = focus {
                    if palette.is_some() {
                        if step["action"].get("params").is_none() {
                            step["action"]["params"] = json!({});
                        }
                        step["action"]["params"]["focus_mode"] = json!("continuous");
                        step["name"] = focus["name"].clone();
                        step["continuous"] = json!(true);
                        step["has_conditions"] = focus["has_conditions"].clone();
                        step["prerequisites"] = json!([]);
                        step.as_object_mut().unwrap().remove("estimated_days");
                        step.as_object_mut().unwrap().remove("href");
                        continue;
                    }
                    if step["action"]["entity"].get("scope").is_none() {
                        step["action"]["entity"]["scope"] = json!(tree_id);
                    }
                    step["name"] = focus["name"].clone();
                    step["estimated_days"] =
                        json!((focus["cost"].as_f64().unwrap_or(10.) * 7.).round_ties_even());
                    step["prerequisites"] = focus["prerequisites"].clone();
                    step["href"] = json!(format!(
                        "/?tree={}&focus={}",
                        quote(tree_id.as_deref().unwrap_or("")),
                        quote(&target)
                    ));
                    if truth(&focus["criteria"]) {
                        let mut mutex: BTreeSet<String> = focus["mutually_exclusive"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect();
                        for (id, other) in tree["focuses"].as_object().into_iter().flatten() {
                            if other["mutually_exclusive"]
                                .as_array()
                                .is_some_and(|a| a.contains(&json!(target)))
                            {
                                mutex.insert(id.clone());
                            }
                        }
                        let mut excluded = mutex.clone();
                        loop {
                            let mut changed = false;
                            for (id, other) in tree["focuses"].as_object().into_iter().flatten() {
                                if !excluded.contains(id)
                                    && other["prerequisites"].as_array().into_iter().flatten().any(
                                        |g| {
                                            g.as_array().is_some_and(|a| {
                                                !a.is_empty()
                                                    && a.iter().all(|p| {
                                                        p.as_str()
                                                            .is_some_and(|id| excluded.contains(id))
                                                    })
                                            })
                                        },
                                    )
                                {
                                    excluded.insert(id.clone());
                                    changed = true;
                                }
                            }
                            if !changed {
                                break;
                            }
                        }
                        step["focus_criteria"] = focus["criteria"].clone();
                        step["focus_criteria"]["prerequisites"] = focus["prerequisites"].clone();
                        step["focus_criteria"]["mutex"] = json!(mutex);
                        step["focus_criteria"]["conflicts"] = json!(excluded);
                    }
                } else {
                    warnings.push(json!(format!(
                        "Focus non trovato nel catalogo compatibile: {target}"
                    )));
                }
            } else if kind == "technology" {
                if truth(&step["research_choice"]) {
                    let mut choices = Map::new();
                    let ids: BTreeSet<_> = step["research_choice"]["branches"]
                        .as_object()
                        .into_iter()
                        .flat_map(|m| m.values())
                        .flat_map(|v| v.as_array().into_iter().flatten())
                        .filter_map(Value::as_str)
                        .collect();
                    for identity in ids {
                        if let Some(option) = technologies.get(identity) {
                            let destination = folder(option, folders);
                            choices.insert(identity.to_owned(),json!({"id":identity,"name":focus_text::localise(labels[identity].as_str().unwrap_or(option["name"].as_str().unwrap_or(identity)),labels),"folder":destination,"folder_name":destination.and_then(|id|folders[id]["name"].as_str()),"href":destination.map(|id|format!("/technologies?folder={}&tech={}",quote(id),quote(identity)))}));
                        }
                    }
                    step["choice_entities"] = json!(choices);
                }
                let tech = technologies.get(&target);
                step["catalog_match"] = json!(tech.is_some());
                if let Some(tech) = tech {
                    let equipment = tech["unlocks"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|u| u["kind"] == "Equipaggiamento")
                        .and_then(|u| u["id"].as_str())
                        .unwrap_or("");
                    let names = [
                        format!("{country}_{target}"),
                        format!("{country}_{equipment}"),
                        target.clone(),
                        equipment.into(),
                    ];
                    let name = names
                        .iter()
                        .find_map(|key| labels.get(key).and_then(Value::as_str))
                        .unwrap_or(tech["name"].as_str().unwrap_or(&target));
                    let destination = folder(tech, folders);
                    step["name"] = json!(focus_text::localise(name, labels));
                    step["year"] = tech["start_year"].clone();
                    step["folder"] = json!(destination);
                    step["folder_name"] = destination
                        .and_then(|id| folders.get(id))
                        .map(|f| f["name"].clone())
                        .unwrap_or(Value::Null);
                    step["has_conditions"] =
                        tech.get("has_conditions").cloned().unwrap_or(json!(false));
                    step["special_project_tech"] =
                        tech.get("special_project").cloned().unwrap_or(json!(false));
                    if let Some(destination) = destination {
                        step["href"] = json!(format!(
                            "/technologies?folder={}&tech={}",
                            quote(destination),
                            quote(&target)
                        ));
                    }
                } else if !((truth(&step["relative_research"])
                    || truth(&step["relative_research_unresolved"]))
                    && target.starts_with("unbound-up:")
                    || truth(&step["research_choice"]) && target.starts_with("unbound-choice:"))
                {
                    warnings.push(json!(format!(
                        "Tecnologia non trovata nel catalogo compatibile: {target}"
                    )));
                }
            } else if ["gate", "project"].contains(&kind.as_str()) {
                step["verification"] = json!(if truth(&step["completion"]) {
                    "automatic"
                } else if truth(&step["started_when"]) {
                    "start_only"
                } else {
                    "manual"
                });
                if let Some(label) = labels[&target].as_str() {
                    step["name"] = json!(focus_text::localise(label, labels));
                }
            }
            if step.get("name").is_none() {
                step["name"] = step
                    .get("title")
                    .cloned()
                    .unwrap_or(json!(if target.is_empty() { &kind } else { &target }));
            }
            if ![Some("select_focus"), Some("research"), Some("free")]
                .contains(&step["action"]["kind"].as_str())
            {
                let params = &step["action"]["params"];
                let instructions = params["instructions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n");
                step["operational_glossary"] = glossary_terms(
                    &instructions,
                    params["category"].as_str().unwrap_or(""),
                    glossary_entries,
                );
            }
            if truth(&step["action"]["entity"]) {
                step["entity"] = json!({"schema_version":1,"ref":step["action"]["entity"],"name":step["name"],"properties":{"catalog_match":step["catalog_match"]}});
            }
        }
    }
    guide["catalog_warnings"] = json!(warnings);
    Ok(guide)
}
pub fn project_costs(mut guide: Value, catalog: &Value) -> Result<Value> {
    let mut projects: HashMap<&str, Vec<&Value>> = HashMap::new();
    for entry in catalog.as_array().ok_or("Missing operation catalog")? {
        if entry["kind"] == "project"
            && let Some(id) = entry["id"].as_str()
        {
            projects.entry(id).or_default().push(entry);
        }
    }
    for row in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
        .flat_map(|v| v.as_array_mut().into_iter().flatten())
    {
        if row["action"]["kind"] != "special_project" || row["tracking"] == "reminder" {
            continue;
        }
        if let Some(entries) = row["action"]["entity"]["id"]
            .as_str()
            .and_then(|id| projects.get(id))
            && entries.len() == 1
            && entries[0]["resource_costs"].is_object()
        {
            row["resource_costs"] = entries[0]["resource_costs"].clone();
        }
    }
    Ok(guide)
}
