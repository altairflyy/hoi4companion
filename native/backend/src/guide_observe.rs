//! Qualified campaign observations, calculated before a single storage commit.
//! No clocks, acquisition, action execution or inferred campaign identities.
use crate::{domain_context, guide_prepare, guide_runtime, production_context, state};
use hoi4companion_guide_core::{Condition, State};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
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
fn truth(v: &Value) -> bool {
    crate::state::truth(v)
}
fn check(node: &Value, state: &Value) -> Result<Option<bool>> {
    let node: Condition = serde_json::from_value(node.clone()).map_err(|e| e.to_string())?;
    let state: State = serde_json::from_value(state.clone()).map_err(|e| e.to_string())?;
    Ok(node.evaluate(&state, &BTreeMap::new()))
}
fn admitted(evaluation: &Value, state: &Value, record: &Value) -> bool {
    evaluation["mode"] == "live"
        && evaluation["progress_status"] == "accepted"
        && truth(&record["session_id"])
        && record["session_id"] == state["session_id"]
}
fn war_value(state: &Value) -> Option<(&Value, bool)> {
    let item = &state["facts"]["country.diplomacy.at_war"];
    let country = &state["facts"]["country"];
    if state["usable"] != true
        || item["status"] != "observed"
        || !truth(&item["source"])
        || !["observed", "calculated"].contains(&text(&country["status"]))
        || !truth(&country["source"])
    {
        return None;
    }
    let tag = text(&country["value"]);
    if tag.len() != 3
        || !tag
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return None;
    }
    Some((&country["value"], item["value"].as_bool()?))
}
fn has_after_war(node: &Value) -> bool {
    if node["op"] == "fact" && node["key"] == "guide.campaign.after_war" {
        return true;
    }
    match node {
        Value::Object(m) => m.values().any(has_after_war),
        Value::Array(a) => a.iter().any(has_after_war),
        _ => false,
    }
}
fn uses_after_war(guide: &Value) -> bool {
    rows(guide).iter().any(|s| {
        [
            "when",
            "skip_when",
            "completion",
            "started_when",
            "priority_when",
            "requirements",
        ]
        .iter()
        .any(|f| has_after_war(&s[f]))
    }) || list(&guide["phases"])
        .iter()
        .any(|p| has_after_war(&p["when"]))
}
fn definitions(guide: &Value) -> Value {
    let mut result = json!({});
    let steps = rows(guide);
    for row in &steps {
        for milestone in list(&row["action"]["params"]["production_milestones"]) {
            result[text(&milestone["token"])] = milestone.clone();
        }
    }
    for row in steps {
        let Some(node) = guide_prepare::foreign_predicate(&row["action"]["params"]) else {
            continue;
        };
        if row["tracking"] == "reminder" {
            continue;
        }
        let token = guide_prepare::foreign_token(&row["key"], &node);
        if row["completion"]
            == json!({"op":"any","args":[node,{"op":"fact","key":"guide.campaign.milestones","test":"contains","value":token}]})
        {
            result[&token] = json!({"step":row["key"],"token":token,"condition":node,"foreign_interaction":true});
        }
    }
    result
}
fn campaign(guide: &Value, state: &Value, record: &mut Value, milestones: &Value) -> Result<()> {
    let evaluation = guide_runtime::evaluate(guide, state, record)?;
    if !admitted(&evaluation, state, record) {
        return Ok(());
    }
    let mut observed = record
        .get("campaign_conditions")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some((country, true)) = war_value(state) {
        observed["country"] = country.clone();
        observed["war_seen"] = json!(true);
    }
    let completed = list(&evaluation["steps"])
        .iter()
        .filter(|r| r["status"] == "completed")
        .map(|r| text(&r["key"]))
        .collect::<HashSet<_>>();
    let results = list(&evaluation["steps"])
        .iter()
        .map(|r| (text(&r["key"]), r))
        .collect::<BTreeMap<_, _>>();
    for (token, milestone) in milestones.as_object().into_iter().flatten() {
        if milestone["foreign_interaction"] == true {
            let step = rows(guide)
                .into_iter()
                .find(|r| r["key"] == milestone["step"])
                .ok_or("Missing foreign milestone step")?;
            let mut allowed = true;
            for field in ["when", "requirements"] {
                if let Some(node) = step.get(field)
                    && check(node, state)? != Some(true)
                {
                    allowed = false
                }
            }
            for phase in list(&guide["phases"]) {
                if phase["id"] == step["phase"]
                    && let Some(node) = phase.get("when")
                    && check(node, state)? != Some(true)
                {
                    allowed = false
                }
            }
            if let Some(node) = step.get("skip_when")
                && check(node, state)? != Some(false)
            {
                allowed = false
            }
            if list(&step["depends_on"]).iter().any(|dep| {
                results
                    .get(text(dep))
                    .is_none_or(|r| r["status"] != "completed")
            }) || list(&step["depends_on_started"]).iter().any(|dep| {
                results.get(text(dep)).is_none_or(|r| {
                    r["selection"]["value"] != true && r["completion"]["value"] != true
                })
            }) {
                allowed = false
            }
            if !allowed {
                continue;
            }
        }
        if completed.contains(text(&milestone["step"]))
            && check(&milestone["condition"], state)? == Some(true)
        {
            observed["country"] = evaluation["country"].clone();
            if observed.get("milestones").is_none() {
                observed["milestones"] = json!({})
            }
            observed["milestones"][token] = json!(true);
        }
    }
    if truth(&observed) && record.get("campaign_conditions") != Some(&observed) {
        record["campaign_conditions"] = observed
    }
    Ok(())
}
fn step_result<'a>(evaluation: &'a Value, key: &str) -> Result<&'a Value> {
    list(&evaluation["steps"])
        .iter()
        .find(|r| text(&r["key"]) == key)
        .ok_or_else(|| "Missing evaluated guide step".into())
}
fn research(guide: &Value, state: &Value, record: &mut Value, runs: &Value) -> Result<()> {
    let evaluation = guide_runtime::evaluate(guide, state, record)?;
    if !admitted(&evaluation, state, record) {
        return Ok(());
    }
    let mut values = vec![];
    for key in ["research.completed", "research.active"] {
        let f = &state["facts"][key];
        if f["status"] != "observed" || !truth(&f["source"]) || !f["value"].is_array() {
            return Ok(());
        }
        values.push(list(&f["value"]).to_vec());
    }
    let completed = &values[0];
    let before_research = record.clone();
    if record.get("research_bindings").is_none() {
        record["research_bindings"] = json!({})
    }
    let mut history = record
        .get("research_history")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if record["branch_reason"] == "timeline_rewind"
        && record.get("research_history").is_none()
        && truth(&record["campaign_id"])
    {
        for prior in runs.as_object().into_iter().flat_map(|m| m.values()) {
            if prior["run_id"] == record["run_id"] {
                break;
            }
            if ["guide_id", "campaign_id", "country"]
                .iter()
                .any(|k| prior[k] != record[k])
            {
                continue;
            }
            for field in ["research_history", "research_bindings"] {
                for (key, value) in prior[field].as_object().into_iter().flatten() {
                    if history.get(key).is_none() {
                        history[key] = value.clone()
                    }
                }
            }
        }
        if truth(&history) {
            record["research_history"] = history.clone()
        }
    }
    for step in rows(guide) {
        let key = text(&step["key"]);
        let chain = list(&step["relative_research"]["chain"]);
        let saved = &history[key];
        if chain.contains(saved)
            && record["research_bindings"]
                .get(key)
                .is_some_and(|v| *v != *saved)
        {
            record["research_bindings"]
                .as_object_mut()
                .unwrap()
                .remove(key);
        }
    }
    for lane in guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
    {
        let mut previous = vec![];
        for step in list(lane) {
            let key = text(&step["key"]);
            if truth(&step["research_choice"]) {
                if record["research_bindings"].get(key).is_none() {
                    let mut info = guide_runtime::choices(guide, state, record)?[key].clone();
                    if step["research_choice"]["mode"] == "once"
                        && truth(&info["options"])
                        && info["options"] != record["research_options"][key]
                    {
                        let assessed = guide_runtime::evaluate(guide, state, record)?;
                        if step_result(&assessed, key)?["status"] == "ready" {
                            if record.get("research_options").is_none() {
                                record["research_options"] = json!({})
                            }
                            record["research_options"][key] = info["options"].clone();
                            info = guide_runtime::choices(guide, state, record)?[key].clone();
                        }
                    }
                    if truth(&info["branch"]) && list(&info["options"]).len() == 1 {
                        let assessed = guide_runtime::evaluate(guide, state, record)?;
                        if step_result(&assessed, key)?["status"] != "ready" {
                            previous.push(step);
                            continue;
                        }
                        let mut trial = record.clone();
                        trial["research_bindings"][key] = info["options"][0].clone();
                        let assessed = guide_runtime::evaluate(guide, state, &trial)?;
                        let row = step_result(&assessed, key)?;
                        if ["ready", "active", "completed"].contains(&text(&row["status"])) {
                            record["research_bindings"][key] = info["options"][0].clone();
                        }
                    }
                }
                previous.push(step);
                continue;
            }
            let relative = &step["relative_research"];
            if !truth(relative) || record["research_bindings"].get(key).is_some() {
                if step["action"]["kind"] == "research" {
                    previous.push(step)
                }
                continue;
            }
            let chain = list(&relative["chain"]);
            let mut used = HashSet::new();
            let mut blocked = false;
            for item in &previous {
                let item_key = text(&item["key"]);
                if item["relative_research"]["chain"] == relative["chain"]
                    && record["research_bindings"].get(item_key).is_none()
                {
                    blocked = true;
                    break;
                }
                let ident = if truth(&item["relative_research"]) {
                    &record["research_bindings"][item_key]
                } else {
                    &item["action"]["entity"]["id"]
                };
                if chain.contains(ident) {
                    if !completed.contains(ident) {
                        blocked = true;
                        break;
                    }
                    used.insert(ident.to_string());
                }
            }
            if !blocked {
                let saved = &history[key];
                let target = if chain.contains(saved) {
                    Some(saved)
                } else {
                    chain
                        .iter()
                        .find(|v| !completed.contains(v) && !used.contains(&v.to_string()))
                };
                if let Some(target) = target.filter(|v| truth(v)) {
                    let mut trial = record.clone();
                    trial["research_bindings"][key] = target.clone();
                    let assessed = guide_runtime::evaluate(guide, state, &trial)?;
                    let row = step_result(&assessed, key)?;
                    if ["ready", "active"].contains(&text(&row["status"]))
                        || chain.contains(saved) && row["status"] == "completed"
                    {
                        record["research_bindings"][key] = target.clone();
                    }
                }
            }
            previous.push(step);
        }
    }
    if before_research.get("research_bindings").is_none()
        && record["research_bindings"]
            .as_object()
            .is_some_and(|m| m.is_empty())
    {
        let mut without_empty = record.clone();
        without_empty
            .as_object_mut()
            .unwrap()
            .remove("research_bindings");
        if without_empty == before_research {
            *record = before_research;
        }
    }
    Ok(())
}
fn candidates(guide: &Value, state: &Value, record: &mut Value, specs: &Value) -> Result<()> {
    let evaluation = guide_runtime::evaluate(guide, state, record)?;
    if !admitted(&evaluation, state, record)
        || !evaluation["country"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    {
        return Ok(());
    }
    let queued = &state["facts"]["construction.queued"];
    let known = ["observed", "calculated"].contains(&text(&queued["status"]))
        && truth(&queued["source"])
        && queued["value"].is_array();
    let statuses = list(&evaluation["steps"])
        .iter()
        .map(|r| (text(&r["key"]), text(&r["status"])))
        .collect::<BTreeMap<_, _>>();
    let mut occupied = HashSet::new();
    for (key, baseline) in record["baselines"].as_object().into_iter().flatten() {
        if !statuses
            .get(key.as_str())
            .is_some_and(|s| ["completed", "not_applicable"].contains(s))
        {
            occupied.insert(baseline["fact_key"].to_string());
        }
    }
    for row in list(&evaluation["steps"]) {
        let key = text(&row["key"]);
        let Some(spec) = specs.get(key) else { continue };
        if spec
            .get("queue_key")
            .is_some_and(|queue| !known || list(&queued["value"]).contains(queue))
            || occupied.contains(&spec["fact_key"].to_string())
            || !["prerequisites_satisfied", "manual_confirmation_required"]
                .contains(&text(&row["reason"]))
        {
            continue;
        }
        let current = if spec.get("queue_key").is_some() {
            let item = &state["facts"][text(&spec["fact_key"])];
            if state["usable"] == true
                && item["status"] == "observed"
                && truth(&item["source"])
                && item["value"]
                    .as_i64()
                    .is_some_and(|v| (0..=9007199254740991).contains(&v))
            {
                Some((item["value"].clone(), item["source"].clone()))
            } else {
                None
            }
        } else {
            let metrics = &row["production"];
            let requested = text(&spec["fact_key"]).starts_with("production_requested:");
            let value = &metrics[if requested { "requested" } else { "assigned" }];
            if value.as_i64().is_some() && (requested || metrics["requested"] == *value) {
                Some((value.clone(), json!("production_adapter")))
            } else {
                None
            }
        };
        let Some((value, source)) = current else {
            continue;
        };
        if record.get("baselines").is_none() {
            record["baselines"] = json!({})
        }
        record["baselines"][key] = json!({"fact_key":spec["fact_key"],"value":value,"quantity":spec["quantity"],"country":evaluation["country"],"source":source});
        occupied.insert(spec["fact_key"].to_string());
    }
    Ok(())
}
pub fn observe(
    guide: &Value,
    input: &Value,
    record: &Value,
    runs: &Value,
    allow_research: bool,
) -> Result<Value> {
    let state = state::normalize(input);
    let mut record = record.clone();
    let before = record.clone();
    let mut specs = json!({});
    let steps = rows(guide);
    for step in &steps {
        let key = text(&step["key"]);
        if record["baselines"].get(key).is_none()
            && let Some(spec) = domain_context::construction_spec(step)
                .or_else(|| production_context::relative_spec(step))
        {
            specs[key] = spec;
        }
    }
    let milestones = definitions(guide);
    let history = &record["campaign_conditions"];
    let needs_war = uses_after_war(guide) && history["war_seen"] != true;
    let needs_milestones = milestones
        .as_object()
        .into_iter()
        .flat_map(|m| m.keys())
        .any(|key| history["milestones"].get(key).is_none());
    if needs_war || needs_milestones {
        campaign(guide, &state, &mut record, &milestones)?
    }
    if allow_research
        && steps
            .iter()
            .any(|s| truth(&s["relative_research"]) || truth(&s["research_choice"]))
    {
        research(guide, &state, &mut record, runs)?
    }
    if truth(&specs) {
        candidates(guide, &state, &mut record, &specs)?
    }
    Ok(json!({"changed":record!=before,"record":record}))
}
