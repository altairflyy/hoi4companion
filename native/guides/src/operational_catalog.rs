//! Installed operational identities, independent of guide wording and live state.
use crate::{
    focus_text, installation, recipient,
    script_document::{blocks, entries, read, scalar},
};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
type Result<T> = std::result::Result<T, String>;
pub struct Source<'a> {
    pub game: PathBuf,
    pub mods: Vec<PathBuf>,
    pub labels: &'a Value,
    pub reference: &'a Value,
    pub warnings: Vec<Value>,
}
impl<'a> Source<'a> {
    pub fn new(args: &'a Value) -> Result<Self> {
        Ok(Self {
            game: PathBuf::from(args["game"].as_str().ok_or("Missing game directory")?),
            mods: args["mods"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| {
                    p.as_str()
                        .map(PathBuf::from)
                        .ok_or("Invalid mod directory".into())
                })
                .collect::<Result<_>>()?,
            labels: &args["labels"],
            reference: &args["reference"],
            warnings: vec![],
        })
    }
    pub fn names(&self, key: &str) -> Value {
        let english =
            focus_text::localise(self.reference[key].as_str().unwrap_or(key), self.reference);
        let display =
            focus_text::localise(self.labels[key].as_str().unwrap_or(&english), self.labels);
        json!({"name":english,"localized_name":display,"aliases":if display!=english{vec![display]}else{vec![]}})
    }
    pub fn documents(&mut self, directory: &str, prefix: &str) -> Result<Vec<(PathBuf, Value)>> {
        let files = installation::collect(&self.game, &self.mods, directory, ".txt")?;
        self.warnings
            .extend(files["warnings"].as_array().into_iter().flatten().cloned());
        let mut result = vec![];
        for path in files["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(PathBuf::from)
        {
            match read(&path) {
                Ok(node) => result.push((path, node)),
                Err(e) => self.warnings.push(json!(format!(
                    "{prefix} ({}): {e}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ))),
            }
        }
        Ok(result)
    }
}
fn traits(node: &Value) -> Vec<Value> {
    blocks(node, "traits")
        .flat_map(entries)
        .filter_map(|(k, v)| (k.is_empty() && v.is_string()).then_some(v.clone()))
        .collect()
}
pub fn unique(values: impl IntoIterator<Item = Value>) -> Vec<Value> {
    let mut out = vec![];
    for v in values {
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}
/// Only catalog-qualified state buildings enter the regional live map. A
/// province-only building must never acquire a fabricated regional zero.
pub fn building_scopes(rows: &Value) -> Value {
    let mut scopes = std::collections::BTreeMap::<String, Option<String>>::new();
    for row in rows.as_array().into_iter().flatten().filter(|r| r["kind"] == "building") {
        let Some(id) = row["id"].as_str() else { continue };
        let scope = row["location_scope"].as_str().filter(|s| matches!(*s, "state" | "province")).map(str::to_owned);
        scopes.entry(id.to_owned()).and_modify(|old| {
            if *old != scope { *old = None; }
        }).or_insert(scope);
    }
    json!(scopes.into_iter().filter_map(|(id, scope)| scope.map(|s| (id, s))).collect::<std::collections::BTreeMap<_, _>>())
}

#[cfg(test)]
mod building_scope_tests {
    use super::*;
    #[test]
    fn conflicting_or_missing_catalog_scope_is_not_guessed() {
        let rows = json!([
            {"kind":"building","id":"a","location_scope":"state"},
            {"kind":"building","id":"a","location_scope":"province"},
            {"kind":"building","id":"b","location_scope":"province"},
            {"kind":"building","id":"b"},
            {"kind":"building","id":"c","location_scope":"province"},
            {"kind":"building","id":"c","location_scope":"province"}
        ]);
        assert_eq!(building_scopes(&rows), json!({"c":"province"}));
    }
}
/// Request only purchases actually declared by installed guides. An empty
/// catalog country list means unknown/global applicability, not a request to
/// price every installed idea. Native qualification still supplies the price.
pub fn political_cost_ids<'a>(rows:&Value,guides:impl IntoIterator<Item=&'a Value>)->Value {
    let mut catalog=std::collections::BTreeMap::<(&str,&str),Vec<&Value>>::new();
    for row in rows.as_array().into_iter().flatten() {
        if let (Some(kind),Some(id))=(row["kind"].as_str(),row["id"].as_str()) {
            catalog.entry((kind,id)).or_default().push(row);
        }
    }
    let mut targets=std::collections::BTreeMap::new();
    for guide in guides {
        let country=guide["applies_to"]["country"].as_str();
        for step in guide["tracks"].as_object().into_iter().flat_map(|m|m.values()).flat_map(|v|v.as_array().into_iter().flatten()) {
            let p=&step["action"]["params"];
            let selected=[("decision","decision_id",""),("law","law_id","law_group"),("idea","idea_id","idea_group"),("advisor","advisor_id","role")].into_iter().find_map(|(kind,key,group)|p[key].as_str().map(|id|(kind,id,group)));
            let Some((kind,id,group_key))=selected else {continue};
            let explicit=if kind=="decision" {Some("decision")}else{p[group_key].as_str().filter(|s|!s.is_empty())};
            let group=explicit.map(str::to_owned).or_else(|| {
                let groups=catalog.get(&(kind,id)).into_iter().flatten().filter(|r|country.is_none_or(|tag|r["countries"].as_array().is_none_or(|a|a.is_empty()||a.contains(&json!(tag)))))
                    .filter_map(|r|r[if kind=="advisor"{"slot"}else{"group"}].as_str()).collect::<std::collections::BTreeSet<_>>();
                (groups.len()==1).then(||groups.into_iter().next().unwrap().to_owned())
            });
            let Some(group)=group else {continue}; // Unresolved role remains unknown.
            if [id,group.as_str()].iter().any(|s|s.is_empty()||s.len()>160||!s.bytes().all(|c|c.is_ascii_alphanumeric()||b"_.-".contains(&c))) {continue;}
            let countries=country.map(|s|vec![s]).unwrap_or_default();
            targets.insert((country.unwrap_or("").to_owned(),group.clone(),id.to_owned()),json!({"id":id,"group":group,"countries":countries}));
        }
    }
    json!(targets.into_values().collect::<Vec<_>>())
}
#[cfg(test)]
mod cost_target_tests {
    use super::*;
    #[test]
    fn guide_purchases_do_not_expand_to_the_entire_unscoped_catalog() {
        let mut rows=(0..5000).map(|n|json!({"kind":"idea","id":format!("unrelated_{n}"),"group":"political_advisor","countries":[]})).collect::<Vec<_>>();
        rows.push(json!({"kind":"idea","id":"declared_idea","group":"industrial_concern","countries":[]}));
        let guide=json!({"applies_to":{"country":"NOR"},"tracks":{"operations":[
            {"action":{"params":{"law_id":"free_trade","law_group":"trade_laws"}}},
            {"action":{"params":{"idea_id":"declared_idea"}}},
            {"action":{"params":{"advisor_id":"NOR_advisor","role":"high_command"}}},
            {"action":{"params":{"decision_id":"NOR_decision"}}},
            {"action":{"params":{"law_id":"free_trade","law_group":"trade_laws"}}}
        ]}});
        let targets=political_cost_ids(&json!(rows),[&guide]);
        assert_eq!(targets.as_array().unwrap().len(),4);
        assert!(targets.as_array().unwrap().iter().all(|r|r["countries"]==json!(["NOR"])));
        assert!(targets.as_array().unwrap().contains(&json!({"id":"declared_idea","group":"industrial_concern","countries":["NOR"]})));
        assert!(!targets.to_string().contains("unrelated_"));
    }
    #[test]
    fn ambiguous_roles_remain_unrequested_and_guide_countries_remain_separate() {
        let rows=json!([{"kind":"advisor","id":"same","slot":"high_command","countries":[]},{"kind":"advisor","id":"same","slot":"political_advisor","countries":[]}]);
        let nor=json!({"applies_to":{"country":"NOR"},"tracks":{"operations":[{"action":{"params":{"advisor_id":"same"}}},{"action":{"params":{"decision_id":"same_decision"}}}]}});
        let ger=json!({"applies_to":{"country":"GER"},"tracks":{"operations":[{"action":{"params":{"decision_id":"same_decision"}}}]}});
        let targets=political_cost_ids(&rows,[&nor,&ger]);
        assert_eq!(targets.as_array().unwrap().len(),2);
        assert!(targets.as_array().unwrap().iter().all(|r|r["group"]=="decision"));
        assert_eq!(targets[0]["countries"],json!(["GER"]));assert_eq!(targets[1]["countries"],json!(["NOR"]));
    }
}
fn trait_names(source: &Source, ids: &[Value]) -> Vec<Value> {
    unique(ids.iter().flat_map(|id| {
        let names = source.names(id.as_str().unwrap_or(""));
        vec![
            id.clone(),
            names["name"].clone(),
            names["localized_name"].clone(),
        ]
    }))
}
fn countries(id: &str, path: &Path, condition: &Value) -> Vec<String> {
    if id.as_bytes().get(3) == Some(&b'_') && id.get(..3).is_some_and(recipient::tag) {
        return vec![id[..3].into()];
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    if recipient::tag(&stem) {
        return vec![stem.into_owned()];
    }
    recipient::origins(condition)
        .unwrap_or_default()
        .into_iter()
        .collect()
}
fn label_key<'a>(instance: &'a Value, node: &'a Value, id: &'a str) -> &'a str {
    crate::script_document::first(instance, "name")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            crate::script_document::first(node, "name")
                .and_then(Value::as_str)
                .unwrap_or(id)
        })
}
/// Focuses a decision certainly requires: `has_completed_focus` at the top of
/// its `allowed`, `visible` or `available` blocks or inside `AND` there; never
/// read through `OR`, `NOT`, `if` or another scope.
pub fn required_focuses(node: &Value) -> Vec<String> {
    fn walk(block: &Value, out: &mut Vec<String>, depth: usize) {
        for (key, value) in entries(block) {
            match key {
                "has_completed_focus" => {
                    if let Some(id) = value.as_str().filter(|id| !out.iter().any(|o| o == id)) {
                        out.push(id.to_owned());
                    }
                }
                "AND" if depth < 16 && value.get("entries").is_some() => walk(value, out, depth + 1),
                _ => {}
            }
        }
    }
    let mut out = vec![];
    for key in ["allowed", "visible", "available"] {
        for block in blocks(node, key) {
            walk(block, &mut out, 0);
        }
    }
    out
}
pub fn political(source: &mut Source) -> Result<Vec<Value>> {
    let mut rows = vec![];
    let metadata = [
        "allowed",
        "available",
        "visible",
        "default",
        "ai_will_do",
        "allowed_civil_war",
        "allowed_to_remove",
    ];
    for (path, root) in source.documents("common/ideas", "Catalogo politica non leggibile")? {
        for group in blocks(&root, "ideas") {
            for (group_id, definitions) in
                entries(group).filter(|(_, v)| v.get("entries").is_some())
            {
                let law = scalar(definitions, "law") == "yes";
                for (id, node) in entries(definitions)
                    .filter(|(k, v)| v.get("entries").is_some() && !metadata.contains(k))
                {
                    let ids = traits(node);
                    let condition = recipient::political(&[definitions, node]);
                    let mut row = source.names(id);
                    let aliases = unique(
                        row["aliases"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .cloned()
                            .chain(trait_names(source, &ids)),
                    );
                    row.as_object_mut().unwrap().extend(json!({"kind":if law{"law"}else{"idea"},"id":id,"group":group_id,"traits":ids,"countries":countries(id,&path,&condition),"country_condition":condition,"aliases":aliases,"operational_criteria":crate::focus_source_rules::operational(&[definitions,node])}).as_object().unwrap().clone());
                    rows.push(row);
                }
            }
        }
    }
    for (path, root) in source.documents("common/characters", "Catalogo politica non leggibile")? {
        for group in blocks(&root, "characters") {
            for (id, node) in entries(group).filter(|(_, v)| v.get("entries").is_some()) {
                for instance in std::iter::once(node).chain(blocks(node, "instance")) {
                    for advisor in blocks(instance, "advisor") {
                        let token = scalar(advisor, "idea_token");
                        let slot = scalar(advisor, "slot");
                        if token.is_empty() || slot.is_empty() {
                            continue;
                        }
                        let ids = traits(advisor);
                        let mut row = source.names(label_key(instance, node, id));
                        let aliases = unique(
                            row["aliases"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .cloned()
                                .chain([json!(id)])
                                .chain(trait_names(source, &ids)),
                        );
                        let nodes = if std::ptr::eq(instance, node) {
                            vec![node, advisor]
                        } else {
                            vec![node, instance, advisor]
                        };
                        row.as_object_mut().unwrap().extend(json!({"kind":"advisor","id":token,"character_id":id,"slot":slot,"traits":ids,"countries":countries(id,&path,&Value::Null),"country_condition":recipient::political(&nodes),"aliases":aliases,"operational_criteria":crate::focus_source_rules::operational(&nodes)}).as_object().unwrap().clone());
                        rows.push(row);
                    }
                    for role in ["field_marshal", "corps_commander", "navy_leader"] {
                        for commander in blocks(instance, role) {
                            let ids = traits(commander);
                            let mut row = source.names(label_key(instance, node, id));
                            let aliases = unique(
                                row["aliases"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .cloned()
                                    .chain(trait_names(source, &ids)),
                            );
                            let nodes = if std::ptr::eq(instance, node) {
                                vec![node, commander]
                            } else {
                                vec![node, instance, commander]
                            };
                            row.as_object_mut().unwrap().extend(json!({"kind":"commander","id":id,"character_id":id,"slot":role,"traits":ids,"countries":countries(id,&path,&Value::Null),"country_condition":recipient::political(&nodes),"aliases":aliases}).as_object().unwrap().clone());
                            rows.push(row);
                        }
                    }
                }
            }
        }
    }
    let event_docs = source.documents("events", "Catalogo politica non leggibile")?;
    for (_, root) in &event_docs {
        let root = root.clone();
        for (kind, event) in entries(&root).filter(|(k, v)| {
            [
                "country_event",
                "news_event",
                "state_event",
                "unit_leader_event",
            ]
            .contains(k)
                && v.get("entries").is_some()
        }) {
            let id = scalar(event, "id");
            if id.is_empty() {
                continue;
            }
            let title = scalar(event, "title");
            let mut row = source.names(if title.is_empty() { id } else { title });
            row.as_object_mut().unwrap().extend(json!({"kind":"event","id":id,"country_condition":recipient::event(kind,event),"identity_countries":recipient::identity(event)}).as_object().unwrap().clone());
            // For the events page: the localized description (first one), the
            // event type and whether it fires only when called or also on a timer.
            let desc = crate::script_document::first(event, "desc").and_then(|d| d.as_str().map(str::to_owned)
                .or_else(|| blocks(event, "desc").next().map(|b| scalar(b, "text").to_owned()))).unwrap_or_default();
            if !desc.is_empty() {
                let text = source.names(&desc);
                let text = text["localized_name"].as_str().filter(|t| *t != desc).unwrap_or("");
                if !text.is_empty() {
                    row["description"] = json!(text.chars().take(600).collect::<String>());
                }
            }
            row["event_type"] = json!(kind);
            row["triggered_only"] = json!(scalar(event, "is_triggered_only") == "yes");
            row["timed"] = json!(crate::script_document::first(event, "mean_time_to_happen").is_some());
            rows.push(row);
            for option in blocks(event, "option") {
                let key = scalar(option, "name");
                if key.is_empty() {
                    continue;
                }
                let mut row = source.names(key);
                // Political power the answer costs (add_political_power < 0 at the
                // option scope): lets an author note like "(25PP)" pick the answer.
                let cost: f64 = entries(option).filter(|(k, _)| *k == "add_political_power").filter_map(|(_, v)| v.as_str()?.parse::<f64>().ok()).filter(|v| *v < 0.0).map(|v| -v).sum();
                if cost > 0.0 {
                    row["political_power_cost"] = json!(cost);
                }
                row.as_object_mut().unwrap().extend(json!({"kind":"event_option","id":key,"event_id":id,"country_condition":recipient::combine("all",blocks(option,"trigger").map(|b|recipient::condition(b,"all",false,0)).collect())}).as_object().unwrap().clone());
                // Who must be human- or AI-played for the answer to show
                // ("DNZ = { is_ai = no }"): lets a single-player guide drop it.
                let players = recipient::players(&blocks(option, "trigger").collect::<Vec<_>>());
                if !players.is_null() {
                    row["player_requirement"] = players;
                }
                rows.push(row);
            }
        }
    }
    let mut categories = Map::new();
    for (_, root) in source.documents(
        "common/decisions/categories",
        "Catalogo politica non leggibile",
    )? {
        for (id, node) in entries(&root).filter(|(_, v)| v.get("entries").is_some()) {
            let condition = recipient::political(&[node]);
            let condition = if let Some(old) = categories.get(id) {
                recipient::combine("any", vec![old.clone(), condition])
            } else {
                condition
            };
            categories.insert(id.into(), condition);
        }
    }
    let decision_docs = source.documents("common/decisions", "Catalogo politica non leggibile")?;
    let mut decision_roots = vec![];
    for (path, root) in &decision_docs {
        let root = root.clone();
        if path
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase()
            .contains("common/decisions/categories/")
        {
            continue;
        }
        decision_roots.push(root.clone());
        for (category, group) in entries(&root).filter(|(_, v)| v.get("entries").is_some()) {
            for (id, node) in
                entries(group).filter(|(k, v)| v.get("entries").is_some() && !metadata.contains(k))
            {
                let mut condition = recipient::political(&[group, node]);
                if let Some(parent) = categories.get(category) {
                    condition = recipient::combine("all", vec![parent.clone(), condition]);
                }
                let mut row = source.names(id);
                let targeted = ["targets", "target_array", "target_trigger", "target_root_trigger"]
                    .iter().any(|k| crate::script_document::first(node,k).is_some()) || scalar(node,"state_target") == "yes";
                row.as_object_mut().unwrap().extend(json!({"kind":"decision","id":id,"group":category,"country_condition":condition,"targeted":targeted,"operational_criteria":crate::focus_source_rules::operational(&[group,node])}).as_object().unwrap().clone());
                // Focuses the decision certainly needs: bounds when it can be taken.
                row["required_focuses"] = json!(required_focuses(node));
                row["mission"] = json!(crate::script_document::first(node, "days_mission_timeout").is_some());
                // Real cost, so an author note ("(50PP)", "(25 CP)") can be
                // checked: the numeric `cost` (political power), and the
                // command power the completion effect removes.
                if let Ok(cost) = scalar(node, "cost").parse::<f64>()
                    && cost > 0.0
                {
                    row["political_power_cost"] = json!(cost);
                }
                let command: f64 = blocks(node, "complete_effect")
                    .flat_map(|effect| entries(effect).filter(|(k, _)| *k == "add_command_power").filter_map(|(_, v)| v.as_str()?.parse::<f64>().ok()))
                    .filter(|v| *v < 0.0)
                    .map(|v| -v)
                    .sum();
                if command > 0.0 {
                    row["command_power_cost"] = json!(command);
                }
                rows.push(row);
            }
        }
    }
    // Which nations can receive each event (focus/decision/event effects).
    let focus_roots: Vec<Value> = source
        .documents("common/national_focus", "Catalogo politica non leggibile")?
        .into_iter()
        .map(|(_, r)| r)
        .collect();
    let event_roots: Vec<Value> = event_docs.iter().map(|(_, r)| r.clone()).collect();
    let effect_roots: Vec<Value> = source
        .documents("common/scripted_effects", "Catalogo politica non leggibile")?
        .into_iter()
        .map(|(_, r)| r)
        .collect();
    let reach = crate::event_reach::compute(&focus_roots, &decision_roots, &event_roots, &effect_roots);
    let listed = |set: Option<&std::collections::BTreeSet<String>>| json!(set.map(|s| s.iter().take(64).cloned().collect::<Vec<_>>()).unwrap_or_default());
    for row in rows.iter_mut().filter(|r| r["kind"] == "decision") {
        // What makes it available: focuses (activating or required), decisions, events.
        row["triggered_by"] = listed(reach.unlocks.get(&format!("decision:{}", row["id"].as_str().unwrap_or(""))));
    }
    for row in rows.iter_mut().filter(|r| r["kind"] == "event") {
        let id = row["id"].as_str().unwrap_or("").to_owned();
        row["enabled_by"] = listed(reach.requires.get(&id));
        row["receivers"] = json!(reach.receivers.get(&id).map(|s| s.iter().cloned().collect::<Vec<_>>()).unwrap_or_default());
        row["receivers_unknown"] = json!(reach.unknown.contains(&id));
        row["triggered_by"] = json!(reach.sources.get(&id).map(|s| s.iter().take(64).cloned().collect::<Vec<_>>()).unwrap_or_default());
    }
    let mut keys = std::collections::HashMap::new();
    let mut out: Vec<Value> = vec![];
    for row in rows {
        let key = (
            row["kind"].clone(),
            row["id"].clone(),
            row["slot"].clone(),
            row["event_id"].clone(),
        );
        let key = serde_json::to_string(&key).map_err(|e| e.to_string())?;
        if let Some(&i) = keys.get(&key) {
            let old: &mut Value = &mut out[i];
            if row["targeted"] == true { old["targeted"] = json!(true); }
            // An event defined twice fires on its own if either definition does.
            if row["kind"] == "event" {
                if row["triggered_only"] == false { old["triggered_only"] = json!(false); }
                if row["timed"] == true { old["timed"] = json!(true); }
            }
            if row.get("country_condition").is_some() {
                old["country_condition"] = recipient::combine(
                    "any",
                    vec![
                        old["country_condition"].clone(),
                        row["country_condition"].clone(),
                    ],
                );
            }
            for field in ["aliases", "traits", "countries"] {
                if let Some(values) = row[field].as_array() {
                    old[field] = json!(unique(
                        old[field]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .cloned()
                            .chain(values.iter().cloned())
                    ));
                }
            }
        } else {
            keys.insert(key, out.len());
            out.push(row);
        }
    }
    Ok(out)
}
pub fn doctrines(source: &mut Source) -> Result<Vec<Value>> {
    let mut rows = vec![];
    for (path, root) in source.documents("common/doctrines", "Doctrine catalog unreadable")? {
        if !path.components().any(|c| {
            ["grand_doctrines", "subdoctrines"].contains(&c.as_os_str().to_str().unwrap_or(""))
        }) {
            continue;
        }
        for (id, node) in entries(&root)
            .filter(|(_, v)| v.get("entries").is_some() && !scalar(v, "name").is_empty())
        {
            let (currency, domain) = match scalar(node, "xp_type") {
                "army" => ("army_experience", "land"),
                "navy" => ("navy_experience", "naval"),
                "air" => ("air_experience", "air"),
                _ => continue,
            };
            let mut row = source.names(scalar(node, "name"));
            row.as_object_mut().unwrap().extend(json!({"kind":"doctrine","id":id,"domain":domain,"currency":currency,"track":crate::script_document::first(node,"track").and_then(Value::as_str).unwrap_or("grand")}).as_object().unwrap().clone());
            rows.push(row);
        }
    }
    Ok(rows)
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    let mut source = Source::new(args)?;
    if op == "source_neighbors" {
        return Ok(json!({"neighbors":crate::map_topology::neighbors(&source.game,&source.mods)?}));
    }
    let rows = match op {
        "source_political" => political(&mut source)?,
        "source_doctrines" => doctrines(&mut source)?,
        "source_operations" => operations(&mut source)?,
        _ => return Err("Unknown operational source".into()),
    };
    Ok(json!({"rows":rows,"warnings":source.warnings}))
}

struct Labels<'a> {
    source: &'a Source<'a>,
    countries: Map<String, Value>,
}
impl<'a> Labels<'a> {
    fn new(source: &'a Source<'a>) -> Self {
        let mut countries = Map::new();
        for texts in [source.reference, source.labels] {
            for (key, text) in texts.as_object().into_iter().flatten() {
                if key.as_bytes().get(3) == Some(&b'_')
                    && key.get(..3).is_some_and(recipient::tag)
                    && key.len() > 4
                    && let Some(text) = text.as_str()
                {
                    countries
                        .entry(key[4..].to_owned())
                        .or_insert_with(|| json!([]))
                        .as_array_mut()
                        .unwrap()
                        .push(json!(focus_text::localise(text, texts)));
                }
            }
        }
        Self { source, countries }
    }
    fn names(&self, key: &str) -> Value {
        let mut row = self.source.names(key);
        let english = &row["name"];
        row["aliases"] = json!(unique(
            std::iter::once(row["localized_name"].clone())
                .chain(
                    self.countries
                        .get(key)
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .cloned()
                )
                .filter(|v| v != english)
        ));
        row
    }
}
fn id(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok().filter(|n| *n > 0)
}
fn geography(state: &Value, labels: &Labels) -> Value {
    let provinces: std::collections::BTreeSet<_> = blocks(state, "provinces")
        .flat_map(entries)
        .filter_map(|(k, v)| k.is_empty().then(|| id(v.as_str().unwrap_or(""))).flatten())
        .collect();
    fn cities(
        node: &Value,
        provinces: &std::collections::BTreeSet<u64>,
        labels: &Labels,
        result: &mut std::collections::BTreeMap<u64, Value>,
        depth: usize,
    ) {
        if depth > 128 {
            return;
        }
        for (key, value) in entries(node) {
            if key == "victory_points" && value.get("entries").is_some() {
                let items: Vec<_> = entries(value)
                    .filter(|(k, _)| k.is_empty())
                    .map(|(_, v)| v)
                    .collect();
                if items.len() == 2
                    && let Some(province) = id(items[0].as_str().unwrap_or(""))
                    && provinces.contains(&province)
                {
                    let key = format!("VICTORY_POINTS_{province}");
                    let mut label = labels.names(&key);
                    let points = items[1].as_str().and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite());
                    if label["name"] != key
                        || label["aliases"].as_array().is_some_and(|a| !a.is_empty())
                    {
                        label["province_id"] = json!(province);
                        if let Some(points) = points {
                            label["victory_points"] = json!(points);
                        }
                        result.insert(province, label);
                    }
                }
            } else if value.get("entries").is_some() {
                cities(value, provinces, labels, result, depth + 1);
            }
        }
    }
    let mut result = std::collections::BTreeMap::new();
    for history in blocks(state, "history") {
        cities(history, &provinces, labels, &mut result, 0);
    }
    json!({"province_ids":provinces,"cities":result.into_values().collect::<Vec<_>>()})
}
/// Owner and cores written directly in a state's `history` block: the game's
/// start setup only. Dated sub-blocks (later bookmarks) and live ownership are
/// ignored, so these fields never describe the current campaign.
fn start_control(state: &Value) -> Map<String, Value> {
    let tag = |v: &str| (v.len() == 3 && v.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| v.to_ascii_uppercase());
    let mut owners = std::collections::BTreeSet::new();
    let mut cores = std::collections::BTreeSet::new();
    for history in blocks(state, "history") {
        for (key, value) in entries(history) {
            let Some(value) = value.as_str().and_then(tag) else { continue };
            match key {
                "owner" => { owners.insert(value); }
                "add_core_of" => { cores.insert(value); }
                "remove_core_of" => { cores.remove(&value); }
                _ => {}
            }
        }
    }
    let mut result = Map::new();
    // Two different start owners are a source conflict: none is claimed.
    if owners.len() == 1 {
        result.insert("start_owner".into(), json!(owners.first()));
    }
    if !cores.is_empty() {
        result.insert("start_cores".into(), json!(cores));
    }
    result
}
#[cfg(test)]
mod start_control_tests {
    use super::*;
    fn node(items: Value) -> Value { json!({"entries": items}) }
    #[test]
    fn only_undated_history_owner_and_cores_are_read() {
        let history = node(json!([["owner","ROM"],["add_core_of","ROM"],["add_core_of","hun"],["add_core_of","BUL"],["remove_core_of","BUL"],["1939.1.1",node(json!([["owner","GER"]]))]]));
        let state = node(json!([["id","46"],["history",history]]));
        assert_eq!(Value::Object(start_control(&state)), json!({"start_owner":"ROM","start_cores":["HUN","ROM"]}));
        let conflict = node(json!([["history",node(json!([["owner","ROM"],["owner","BUL"]]))]]));
        assert!(start_control(&conflict).is_empty());
    }
}
fn family<'a>(identifier: &'a str, definitions: &'a Map<String, Value>) -> Option<&'a str> {
    let mut identifier = identifier;
    let mut seen = std::collections::HashSet::new();
    while let Some(node) = definitions.get(identifier) {
        if !seen.insert(identifier) {
            return None;
        }
        let parent = scalar(node, "archetype");
        if parent.is_empty() {
            return Some(identifier);
        }
        identifier = parent;
    }
    None
}
fn interface_category(identifier: &str, definitions: &Map<String, Value>) -> Value {
    let mut identifier = identifier;
    let mut seen = std::collections::HashSet::new();
    while let Some(node) = definitions.get(identifier) {
        if !seen.insert(identifier) {
            break;
        }
        let category = scalar(node, "interface_category");
        if !category.is_empty() {
            return json!(category);
        }
        identifier = scalar(node, "archetype");
    }
    Value::Null
}
fn project_costs(node: &Value) -> Value {
    let cost: Vec<_> = blocks(node, "breakthrough_cost").collect();
    if cost.len() != 1 {
        return Value::Null;
    }
    let mut result = Map::new();
    for (key, value) in entries(cost[0]) {
        let target = format!("projects.points.{key}");
        let Some(value) = value
            .as_str()
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse::<u64>().ok())
        else {
            return Value::Null;
        };
        if key.is_empty() || result.insert(target, json!(value)).is_some() {
            return Value::Null;
        }
    }
    Value::Object(result)
}
pub fn operations(source: &mut Source) -> Result<Vec<Value>> {
    let mut rows = vec![];
    for (directory, kind) in [
        ("common/buildings", "building"),
        ("history/states", "state"),
    ] {
        let documents = source.documents(directory, "Catalogo operazioni non leggibile")?;
        let labels = Labels::new(source);
        for (_, root) in documents {
            if kind == "building" {
                for group in blocks(&root, "buildings") {
                    for (key, node) in entries(group).filter(|(_, v)| v.get("entries").is_some()) {
                        let mut row = labels.names(key);
                        row["kind"] = json!(kind);
                        row["id"] = json!(key);
                        let specializations=blocks(node,"specialization").flat_map(entries).filter_map(|(_,v)|v.as_str().map(str::to_owned)).collect::<Vec<_>>();
                        if !specializations.is_empty() {row["specializations"]=json!(specializations);}
                        let caps: Vec<_> = blocks(node, "level_cap").collect();
                        let province = caps.iter().any(|c| crate::script_document::first(c, "province_max").is_some());
                        let state = caps.iter().any(|c| crate::script_document::first(c, "state_max").is_some());
                        // The game definitions say province_max makes the
                        // building provincial, even with a state-wide cap.
                        let scope = match (state, province) {
                            (_, true) => Some("province"),
                            (true, false) => Some("state"),
                            _ => None,
                        };
                        if let Some(scope) = scope {
                            row["location_scope"] = json!(scope);
                        }
                        row["shares_slots"] = json!(caps.iter().any(|c| scalar(c,"shares_slots")=="yes"));
                        if let Some(max) = caps.iter().find_map(|c| {
                            let v = scalar(c, if province {"province_max"} else {"state_max"});
                            (!v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
                                .then(|| v.parse::<u64>().ok())
                                .flatten()
                        }) {
                            row["max_level"] = json!(max);
                        }
                        rows.push(row);
                    }
                }
            } else {
                for state in blocks(&root, "state") {
                    if let Some(identifier) = id(scalar(state, "id")) {
                        let fallback = format!("STATE_{}", scalar(state, "id"));
                        let name = crate::script_document::first(state, "name")
                            .and_then(Value::as_str)
                            .unwrap_or(&fallback);
                        let mut row = labels.names(name);
                        row.as_object_mut().unwrap().extend(json!({"kind":kind,"id":identifier.to_string(),"geography":geography(state,&labels)}).as_object().unwrap().clone());
                        row.as_object_mut().unwrap().extend(start_control(state));
                        rows.push(row);
                    }
                }
            }
        }
    }
    let anchors = crate::map_anchors::anchors(&source.game,&source.mods).unwrap_or_else(|e| {
        source.warnings.push(json!(format!("Map anchors unavailable: {e}")));
        Default::default()
    });
    let cities = crate::map_anchors::city_points(&source.game,&source.mods).unwrap_or_else(|e| {
        source.warnings.push(json!(format!("City map positions unavailable: {e}")));
        Default::default()
    });
    for row in rows.iter_mut().filter(|r|r["kind"]=="state") {
        let building=row["id"].as_str().and_then(|id|id.parse::<u64>().ok()).and_then(|id|anchors.get(&id));
        if let Some((point,province))=crate::map_anchors::state_anchor(&row["geography"]["cities"],&cities,building) {
            row["geography"]["map_anchor"]=point;
            if let Some(province)=province {row["geography"]["map_anchor_province"]=json!(province);}
        }
    }
    let neighbors = match crate::map_topology::neighbors(&source.game, &source.mods) {
        Ok(value) => value,
        Err(e) => {
            source
                .warnings
                .push(json!(format!("Province topology unavailable: {e}")));
            json!({})
        }
    };
    if neighbors.as_object().is_some_and(|m| !m.is_empty()) {
        for row in rows.iter_mut().filter(|r| r["kind"] == "state") {
            row["geography"]["province_neighbors"] = Value::Object(
                row["geography"]["province_ids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| {
                        let key = p.to_string();
                        neighbors.get(&key).map(|v| (key, v.clone()))
                    })
                    .collect(),
            );
        }
    }
    let mut definitions = Map::new();
    let mut duplicates = vec![];
    for (_, root) in source.documents(
        "common/units/equipment",
        "Catalogo equipaggiamenti non leggibile",
    )? {
        for group in blocks(&root, "equipments") {
            definitions.extend(
                entries(group)
                    .filter(|(_, v)| v.get("entries").is_some())
                    .map(|(k, v)| (k.into(), v.clone())),
            );
        }
        duplicates.extend(
            blocks(&root, "duplicate_archetypes")
                .flat_map(entries)
                .filter(|(_, v)| v.get("entries").is_some())
                .map(|(k, v)| (k.to_owned(), v.clone())),
        );
    }
    for (key, node) in duplicates {
        if !definitions.contains_key(scalar(&node, "archetype")) {
            continue;
        }
        definitions
            .entry(key.clone())
            .or_insert_with(|| json!({"entries":[["is_archetype","yes"]]}));
        for block in blocks(&node, "variant_name") {
            for (identifier, label) in entries(block).filter(|(_, v)| v.is_string()) {
                definitions.entry(identifier.to_owned()).or_insert_with(
                    || json!({"entries":[["archetype",key],["derived_variant_name",label]]}),
                );
            }
        }
    }
    let mut country_aliases: Map<String, Value> = Map::new();
    for texts in [source.reference, source.labels] {
        for (key, text) in texts.as_object().into_iter().flatten() {
            if key.len() < 5
                || key.as_bytes().get(3) != Some(&b'_')
                || !key.get(..3).is_some_and(recipient::tag)
            {
                continue;
            }
            let mut identifier = &key[4..];
            if !definitions.contains_key(identifier) && identifier.ends_with("_short") {
                identifier = &identifier[..identifier.len() - 6];
            }
            if definitions.contains_key(identifier)
                && let Some(text) = text.as_str()
            {
                country_aliases
                    .entry(identifier.to_owned())
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
                    .unwrap()
                    .entry(key[..3].to_owned())
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .unwrap()
                    .push(json!(focus_text::localise(text, texts)));
            }
        }
    }
    let mut members: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for (key, node) in &definitions {
        if let Some(root) = family(key, &definitions)
            && scalar(node, "is_archetype") != "yes"
        {
            members.entry(root).or_default().push(key);
        }
    }
    let labels = Labels::new(source);
    for (key, node) in &definitions {
        let Some(root) = family(key, &definitions) else {
            continue;
        };
        let mut row = labels.names(key);
        let mut aliases = row["aliases"].as_array().unwrap().clone();
        for labelkey in [
            scalar(node, "derived_variant_name").to_owned(),
            format!("{key}_short"),
        ] {
            for texts in [source.reference, source.labels] {
                if let Some(text) = texts[&labelkey].as_str() {
                    aliases.push(json!(focus_text::localise(text, texts)));
                }
            }
        }
        aliases.push(json!(
            key.strip_suffix("_equipment")
                .unwrap_or(key)
                .replace('_', " ")
        ));
        aliases.push(json!(format!("{}s", row["name"].as_str().unwrap_or(""))));
        let archetype = scalar(node, "is_archetype") == "yes";
        let mut equipment_ids = if archetype {
            members.get(root).cloned().unwrap_or_default()
        } else {
            vec![key.as_str()]
        };
        if archetype {
            equipment_ids.sort();
        }
        let country_aliases: Map<_, _> = country_aliases
            .get(key)
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!(unique(v.as_array().into_iter().flatten().cloned())),
                )
            })
            .collect();
        row.as_object_mut().unwrap().extend(json!({"kind":"equipment","id":key,"family_id":root,"production_ui_category":interface_category(key,&definitions),"equipment_ids":equipment_ids,"is_archetype":archetype,"country_aliases":country_aliases,"aliases":unique(aliases)}).as_object().unwrap().clone());
        rows.push(row);
    }
    let documents = source.documents(
        "common/special_projects/projects",
        "Catalogo progetti non leggibile",
    )?;
    let labels = Labels::new(source);
    for (_, root) in documents {
        for (key, node) in entries(&root).filter(|(_, v)| v.get("entries").is_some()) {
            let mut row = labels.names(key);
            row.as_object_mut().unwrap().extend(json!({"kind":"project","id":key,"specialization":scalar(node,"specialization"),"resource_costs":project_costs(node),"operational_criteria":crate::focus_source_rules::operational(&[node])}).as_object().unwrap().clone());
            rows.push(row);
        }
    }
    rows.extend(political(source)?);
    rows.extend(doctrines(source)?);
    Ok(rows)
}
