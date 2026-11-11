//! Pure prepared-v1 adapter. Definition validation is still at the incoming
//! guide boundary; this module never invents dependencies or strategy.
use crate::{domain_context, production_context, state};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, String>;
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn list(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn rows(guide: &Value) -> Vec<&Value> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(list)
        .collect()
}
fn fact_value<'a>(state: &'a Value, key: &str) -> Option<&'a Value> {
    let item = &state["facts"][key];
    if state["usable"] == true
        && ["observed", "calculated"].contains(&text(&item["status"]))
        && !text(&item["source"]).is_empty()
        && !item["value"].is_null()
    {
        item.get("value")
    } else {
        None
    }
}
fn checks_in(node: &Value, keys: &mut BTreeSet<String>) {
    if node["op"] == "check" {
        keys.insert(text(&node["key"]).into());
    }
    for child in list(&node["args"]) {
        checks_in(child, keys)
    }
    if let Some(arg) = node.get("arg") {
        checks_in(arg, keys)
    }
}
fn manual_keys(guide: &Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for step in rows(guide) {
        if ["gate", "project"].contains(&text(&step["kind"])) && step["tracking"] != "reminder" {
            keys.insert(text(&step["key"]).into());
        }
        for field in [
            "when",
            "priority_when",
            "skip_when",
            "completion",
            "started_when",
        ] {
            if let Some(node) = step.get(field) {
                checks_in(node, &mut keys)
            }
        }
        for field in ["gate", "available_gate"] {
            if let Some(value) = step.get(field) {
                keys.insert(text(value).into());
            }
        }
    }
    for milestone in list(&guide["milestones"]) {
        keys.insert(text(&milestone["key"]).into());
    }
    for phase in list(&guide["phases"]) {
        if let Some(node) = phase.get("when") {
            checks_in(node, &mut keys)
        }
    }
    keys
}
fn valid_campaign(value: &Value) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    if value.len() != 50 || !value.starts_with("hoi4:") || value.as_bytes()[41] != b':' {
        return false;
    }
    let uuid = &value.as_bytes()[5..41];
    uuid.iter().enumerate().all(|(i, b)| {
        if [8, 13, 18, 23].contains(&i) {
            *b == b'-'
        } else {
            b.is_ascii_digit() || (b'a'..=b'f').contains(b)
        }
    }) && uuid.iter().any(|b| *b != b'0' && *b != b'-')
        && value.as_bytes()[42..]
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
}
fn nonblank(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        !s.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
            .is_empty()
    })
}
fn progress(guide: &Value, state: &Value, value: &Value) -> Result<Value> {
    let empty = |status: &str| json!([status, null, {}, {}]);
    if value.is_null() {
        return Ok(empty("none"));
    }
    let checks = value.get("checks").cloned().unwrap_or_else(|| json!({}));
    let started = value.get("started").cloned().unwrap_or_else(|| json!({}));
    if !value.is_object()
        || !value["schema_version"].is_number()
        || value["schema_version"].as_f64() != Some(1.)
        || !value["guide_id"].is_string()
        || !nonblank(&value["run_id"])
        || text(&value["run_id"]).chars().count() > 160
        || !checks.is_object()
    {
        return Err("Avanzamento della partita non valido".into());
    }
    if checks
        .as_object()
        .unwrap()
        .values()
        .any(|v| !v.is_boolean())
    {
        return Err("Conferme della partita non valide".into());
    }
    if started
        .as_object()
        .is_none_or(|m| m.values().any(|v| !v.is_boolean()))
    {
        return Err("Avvii della partita non validi".into());
    }
    let observations = value
        .get("campaign_conditions")
        .cloned()
        .unwrap_or_else(|| json!({}));
    domain_context::validate_observations(&observations)?;
    let binding = &value["session_id"];
    if !binding.is_null() && !nonblank(binding) {
        return Err("Sessione della partita non valida".into());
    }
    if text(&value["guide_id"]) != guide.get("id").and_then(Value::as_str).unwrap_or("") {
        return Ok(empty("wrong_guide"));
    }
    let campaign = &value["campaign_id"];
    if state::truth(&value["campaign_required"]) && campaign.is_null() {
        return Ok(empty("campaign_unverified"));
    }
    if !campaign.is_null() {
        if !valid_campaign(campaign) {
            return Err("Campagna della partita non valida".into());
        }
        let observed = &state["facts"]["campaign.id"];
        if state["usable"] != true
            || observed["status"] != "observed"
            || text(&observed["source"]).is_empty()
        {
            return Ok(empty("campaign_unverified"));
        }
        if observed["value"] != *campaign || fact_value(state, "country") != value.get("country") {
            return Ok(empty("wrong_campaign"));
        }
    }
    if !binding.is_null() {
        if state["usable"] != true {
            return Ok(empty("session_unverified"));
        }
        if *binding != state["session_id"] {
            return Ok(empty("wrong_session"));
        }
    }
    let allowed = manual_keys(guide);
    let operations = rows(guide)
        .into_iter()
        .filter(|s| {
            ["manual", "special_project"].contains(&text(&s["action"]["kind"]))
                && s["tracking"] != "reminder"
        })
        .map(|s| text(&s["key"]))
        .collect::<BTreeSet<_>>();
    let checks = checks
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| allowed.contains(*k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<serde_json::Map<_, _>>();
    let started = started
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| operations.contains(k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<serde_json::Map<_, _>>();
    Ok(json!(["accepted", value["run_id"], checks, started]))
}
pub(crate) fn decision_history_predicate(node: &Value) -> Option<Value> {
    let key=match text(&node["key"]) {
        "decisions.executed"=>"decisions.history.executed", "decisions.completed"=>"decisions.history.completed",
        "decisions.targeted.executed"=>"decisions.targeted.history.executed", "decisions.targeted.completed"=>"decisions.targeted.history.completed", _=>return None,
    };
    Some(json!({"op":"fact","key":key,"test":"contains","value":node["value"]}))
}
pub(crate) fn foreign_predicate(params: &Value) -> Option<Value> {
    // Existing durable milestones also retain positively observed decision
    // evidence; the generated predicate includes the exact optional target.
    if params["decision_id"].is_string() {
        let node=&params["political_binding"]["completion"];
        if node["op"]=="fact" && node["test"]=="contains" &&
            (["decisions.executed","decisions.completed"].contains(&text(&node["key"])) && node["value"]==params["decision_id"] ||
             ["decisions.targeted.executed","decisions.targeted.completed"].contains(&text(&node["key"])) && crate::state::decisions::valid_fact(text(&node["key"]),&json!([node["value"]]))) {
            return Some(node.clone());
        }
        return None;
    }
    let target = text(&params["target_country"]);
    if target.len() != 3
        || !target
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return None;
    }
    let op = text(&params["operation_id"]);
    let fact =
        |key: &str, value: String| json!({"op":"fact","key":key,"test":"contains","value":value});
    let relation = &crate::political_binding::foreign_tasks()["relation_actions"][op];
    if relation.is_object() && ["outgoing", "incoming"].contains(&text(&relation["direction"]))
        && relation["present"].is_boolean()
    {
        let key=format!("diplomacy.{}",text(&relation["direction"]));
        let selected=format!("{}:{target}",text(&relation["kind"]));
        if state::valid_fact(&key,&json!([selected])) {
            let node=fact(&key,selected);
            let node=if relation["present"] == true {node} else {json!({"op":"not","arg":node})};
            return Some(json!({"op":"all","args":[fact("diplomacy.countries",target.into()),node]}));
        }
    }
    if op == "assign_operative_mission"
        && [
            "build_intel_network",
            "quiet_network",
            "counter_intelligence",
            "root_out_resistance",
            "boost_ideology",
            "control_trade",
            "diplomatic_pressure",
            "propaganda",
        ]
        .contains(&text(&params["mission_id"]))
    {
        return Some(
            json!({"op":"all","args":[fact("intelligence.countries",target.into()),fact("intelligence.active_missions",format!("{}:{target}",text(&params["mission_id"])))]}),
        );
    }
    if [
        "improve_relations",
        "non_aggression_pact",
        "stop_improving_relations",
    ]
    .contains(&op)
    {
        let member = fact(
            if op == "non_aggression_pact" {
                "diplomacy.non_aggression_pacts"
            } else {
                "diplomacy.improving_relations"
            },
            target.into(),
        );
        let member = if op == "stop_improving_relations" {
            json!({"op":"not","arg":member})
        } else {
            member
        };
        return Some(json!({"op":"all","args":[fact("diplomacy.countries",target.into()),member]}));
    }
    None
}
pub(crate) fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), canonical(v)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        v => v.clone(),
    }
}
#[cfg(test)]
mod foreign_coverage_tests {
    use super::*;
    #[test]
    fn every_declared_relation_action_has_a_typed_directional_predicate() {
        let actions=crate::political_binding::foreign_tasks()["relation_actions"].as_object().unwrap();
        for (id, action) in actions {
            let predicate=foreign_predicate(&json!({"operation_id":id,"target_country":"OTH"})).unwrap();
            assert_eq!(predicate["args"][0],json!({"op":"fact","key":"diplomacy.countries","test":"contains","value":"OTH"}));
            let node=if action["present"] == true {&predicate["args"][1]} else {&predicate["args"][1]["arg"]};
            assert_eq!(node["key"],format!("diplomacy.{}",action["direction"].as_str().unwrap()));
            assert_eq!(node["value"],format!("{}:OTH",action["kind"].as_str().unwrap()));
        }
        assert!(foreign_predicate(&json!({"operation_id":"embargo","target_country":"missing"})).is_none());
        assert!(foreign_predicate(&json!({"operation_id":"unrecognized","target_country":"OTH"})).is_none());
    }
}
pub(crate) fn foreign_token(key: &Value, node: &Value) -> String {
    let json = canonical(&json!([key, node])).to_string();
    let mut ascii = String::new();
    for c in json.chars() {
        if c.is_ascii() {
            ascii.push(c)
        } else {
            let mut buffer = [0; 2];
            for unit in c.encode_utf16(&mut buffer) {
                ascii.push_str(&format!("\\u{unit:04x}"))
            }
        }
    }
    format!("foreign:{:x}", Sha256::digest(ascii.as_bytes()))
}
pub fn prepare(guide: &Value, input: &Value, record: &Value) -> Result<Value> {
    let state = state::normalize(input);
    let country = fact_value(&state, "country");
    let tree = fact_value(&state, "focus.tree");
    let scope = &guide["applies_to"];
    let mode = if state["usable"] != true {
        "preview"
    } else if state::truth(&scope["country"])
        && country.is_some()
        && country != scope.get("country")
    {
        "wrong_country"
    } else if state::truth(&scope["tree_id"]) && tree.is_some() && tree != scope.get("tree_id") {
        "wrong_tree"
    } else if state::truth(&scope["country"]) && country.is_none()
        || state::truth(&scope["tree_id"]) && tree.is_none()
    {
        "preview"
    } else {
        "live"
    };
    let result = progress(guide, &state, record)?;
    let status = text(&result[0]);
    let baselines = if record.is_null() {
        json!({})
    } else {
        record
            .get("baselines")
            .cloned()
            .unwrap_or_else(|| json!({}))
    };
    domain_context::validate_baselines(&baselines)?;
    let (checks, started) = if ["wrong_country", "wrong_tree"].contains(&mode) {
        (json!({}), json!({}))
    } else {
        (result[2].clone(), result[3].clone())
    };
    let context = if mode == "live" {
        state.clone()
    } else {
        json!({"schema_version":1,"usable":false,"session_id":null,"facts":{}})
    };
    let observations = if status == "accepted" {
        record
            .get("campaign_conditions")
            .cloned()
            .unwrap_or_else(|| json!({}))
    } else {
        Value::Null
    };
    let context = domain_context::campaign_context(&context, &observations)?;
    let mut context=context;
    if status=="accepted" && mode=="live" && let Some(history)=record.get("decision_history") {
        hoi4companion_storage::decision_history::validate(history)?;
        let tick=&state["facts"]["campaign.tick"];
        if history["campaign_id"]==record["campaign_id"] && history["country"]==record["country"] && history["session_id"]==state["session_id"]
            && tick["status"]=="observed" && tick["source"]=="native_game_tick" && tick["value"].as_u64().is_some_and(|t|t>=history["last_tick"].as_u64().unwrap()) {
            context["facts"].as_object_mut().ok_or("Invalid history state")?.extend(hoi4companion_storage::decision_history::facts(history)?);
        }
    }
    let baseline_context = if status == "accepted" {
        baselines
    } else {
        json!({})
    };
    let construction = domain_context::construction_context(guide, &context, &baseline_context)?;
    let production = production_context::context(guide, &construction[0], &baseline_context);
    let mut selectors = json!({});
    let mut foreign = json!({});
    let mut explicit = vec![];
    for step in rows(guide) {
        if step.get("depends_on").is_some() || step.get("depends_on_started").is_some() {
            explicit.push(step["key"].clone())
        }
        let params = &step["action"]["params"];
        let key = text(&step["key"]);
        if let Some(predicate) = foreign_predicate(params) {
            foreign[key] = json!(foreign_token(&step["key"], &predicate));
        }
        if state::truth(&params["production_revision"]) {
            let default = [params.clone()];
            let targets = params
                .get("production_targets")
                .map(list)
                .unwrap_or(&default);
            selectors[key]=json!(targets.iter().map(|p|production_context::selector_for(&json!({"action":{"params":p}})).map(|s|json!({"identity":production_context::selector_identity(&s),"category":s["category"]}))).collect::<Vec<_>>());
        }
    }
    let payload = json!({"schema_version":1,"state":production[0],"original_state":state,"progress_status":status,"run_id":result[1],"checks":checks,"started":started,"explicit_dependencies":explicit,"construction":construction[1],"production":production[1],"research_bindings":if status=="accepted"&&record["research_bindings"].is_object(){record["research_bindings"].clone()}else{json!({})},"research_options":if status=="accepted"{record.get("research_options").cloned().unwrap_or_else(||json!({}))}else{json!({})},"foreign_tokens":foreign,"selectors":selectors});
    let mut definition = json!({});
    for field in ["id", "applies_to", "dependency_policy", "phases", "tracks"] {
        if let Some(v) = guide.get(field) {
            definition[field] = v.clone()
        }
    }
    if definition.get("id").is_none() {
        definition["id"] = json!("")
    }
    for steps in definition["tracks"]
        .as_object_mut()
        .ok_or("Invalid guide tracks")?
        .values_mut()
    {
        for step in steps.as_array_mut().ok_or("Invalid guide track")? {
            let step = step.as_object_mut().ok_or("Invalid guide row")?;
            for field in ["evidence", "title", "name", "note"] {
                step.remove(field);
            }
            let action = step
                .get_mut("action")
                .and_then(Value::as_object_mut)
                .ok_or("Invalid guide action")?;
            let params = action
                .entry("params")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("Invalid action parameters")?;
            for field in [
                "instructions",
                "source_action",
                "reference_images",
                "design_reference",
            ] {
                params.remove(field);
            }
        }
    }
    Ok(json!([definition, payload]))
}
