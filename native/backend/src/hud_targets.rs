//! Presentation targets come only from admitted runtime actions and qualified
//! state. No strategy, clicks, inferred completions or cross-campaign reuse.
use crate::{hud_events, hud_production, import_catalog::Index, state};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
fn a(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
pub fn text_key(v: &str) -> String {
    hoi4companion_catalog_core::casefold(&hoi4companion_catalog_core::nfkd(v))
        .chars()
        .filter(|c| hoi4companion_catalog_core::unicode_alnum(*c))
        .collect()
}
fn labels(values: impl Iterator<Item = Value>) -> Vec<Value> {
    let mut out = vec![];
    for v in values.filter(|v| v.as_str().is_some_and(|s| text_key(s).chars().count() >= 4)) {
        if !out.contains(&v) {
            out.push(v)
        }
    }
    out
}
use crate::hud_geometry::resource_ready as ready;

pub fn targets(
    guide: &Value,
    runtime: &Value,
    index: &Index,
    live: &Value,
    prepare_events: bool,
    event_plan: &Value,
) -> Result<Value, String> {
    let empty = || Ok(json!([]));
    let sample = &live["sample"];
    if !state::truth(&live["usable"]) || !sample.is_object() {
        return empty();
    }
    let country = &sample["country"];
    let required = &guide["applies_to"]["country"];
    if runtime["guide_id"] != guide["id"]
        || runtime["mode"] != "live"
        || !state::truth(country)
        || runtime["country"] != *country
        || state::truth(required) && required != country
    {
        return empty();
    }
    if runtime["action_context"].is_object() {
        let normalized = state::normalize(live);
        let context = &runtime["action_context"];
        let session = &context["session_id"];
        let campaign = &context["campaign_id"];
        let observed = &normalized["facts"]["campaign.id"];
        if !session.is_null() && normalized["session_id"] != *session
            || !campaign.is_null()
                && (observed["status"] != "observed"
                    || !state::truth(&observed["source"])
                    || observed["value"] != *campaign)
        {
            return empty();
        }
    }
    let rows = guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(a)
        .map(|r| (s(&r["key"]), r))
        .collect::<HashMap<_, _>>();
    let native = runtime["parallel"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|v| {
            a(&v["steps"])
                .iter()
                .chain(a(&v["optional_steps"]))
                .chain(a(&v["alternative_steps"]))
        })
        .map(s)
        .collect::<HashSet<_>>();
    let alternatives = a(&runtime["parallel"]["focus"]["alternative_steps"])
        .iter()
        .map(s)
        .collect::<HashSet<_>>();
    let overview = &sample["country_overview"];
    let events = if overview["country"] == *country {
        &overview["events"]
    } else {
        &Value::Null
    };
    let generated = if event_plan["guide_id"] != guide["id"] || event_plan.is_null() {
        hud_events::plan(guide, index)?
    } else {
        event_plan.clone()
    };
    let bindings = &generated["bindings"];
    let mut needed = HashSet::new();
    for row in rows.values() {
        let action = &row["action"];
        let entity = &action["entity"];
        let params = &action["params"];
        if state::truth(&entity["kind"]) && state::truth(&entity["id"]) {
            needed.insert((s(&entity["kind"]), s(&entity["id"])));
        }
        for id in [&params["advisor_id"], &params["law_id"], &params["idea_id"]] {
            if state::truth(id) {
                for kind in ["law", "idea", "advisor"] {
                    needed.insert((kind, s(id)));
                }
            }
        }
    }
    for item in a(&runtime["steps"]) {
        if state::truth(&item["research_target"]) {
            needed.insert(("technology", s(&item["research_target"])));
        }
        for id in a(&item["research_options"]) {
            needed.insert(("technology", s(id)));
        }
    }
    let mut references = HashMap::<(&str, &str), Vec<&Value>>::new();
    for entry in index.rows() {
        let key = (s(&entry["kind"]), s(&entry["id"]));
        if needed.contains(&key) {
            references.entry(key).or_default().push(entry)
        }
    }
    let lookup = |kind: &str, id: &str| references.get(&(kind, id)).cloned().unwrap_or_default();
    let order = a(&runtime["parallel"]["research"]["steps"])
        .iter()
        .enumerate()
        .map(|(i, v)| (s(v), i))
        .collect::<HashMap<_, _>>();
    let mut items = a(&runtime["steps"]).iter().collect::<Vec<_>>();
    items.sort_by_key(|item| order.get(s(&item["key"])).copied().unwrap_or(order.len()));
    let mut result = vec![];
    let mut seen = HashSet::<(String, String)>::new();
    for item in items {
        let key = s(&item["key"]);
        let Some(row) = rows.get(key).copied() else {
            continue;
        };
        let status = s(&item["status"]);
        let reason = s(&item["reason"]);
        let action = &row["action"];
        let params = &action["params"];
        if row["tracking"] != "reminder" && params["category"] == "production" {
            if (["ready", "active", "completed"].contains(&status)
                || status == "not_applicable" && reason == "production_superseded"
                || status == "unverifiable" && reason == "manual_confirmation_required")
                && item["production_admitted"] != false
            {
                result.extend(hud_production::proposals(
                    row,
                    item,
                    index.rows(),
                    country,
                    live,
                ));
            }
            continue;
        }
        if row["tracking"] == "reminder" && !(prepare_events && bindings.get(key).is_some())
            || !["ready", "active", "unverifiable"].contains(&status)
            || status == "unverifiable" && reason != "manual_confirmation_required"
        {
            continue;
        }
        if !a(&item["research_options"]).is_empty() && native.contains(key) && status == "ready" {
            for id in a(&item["research_options"]) {
                let identity = s(id);
                let entries = lookup("technology", identity);
                if entries.len() != 1 || seen.contains(&("research".into(), identity.into())) {
                    continue;
                }
                let entry = entries[0];
                let info = &row["choice_entities"][identity];
                let names = labels(
                    [entry["localized_name"].clone(), entry["name"].clone()]
                        .into_iter()
                        .chain(a(&entry["aliases"]).iter().cloned()),
                );
                result.push(json!({"key":format!("{key}::choice:{identity}"),"guide_step":key,"panel":"research","identity":identity,"labels":names,"title":entry["name"],"status":status,"tone":if a(&item["research_options"]).len()>1||row["research_choice"]["mode"]=="once"{"choice"}else{"next"},"country":country,"folder":info["folder"],"folder_name":info["folder_name"]}));
                seen.insert(("research".into(), identity.into()));
            }
            continue;
        }
        let mut entity = action["entity"].clone();
        if action["kind"] == "research" && state::truth(&item["research_target"]) {
            entity["id"] = item["research_target"].clone();
        }
        if ["event_id", "event_name", "event_option_id", "event_option"]
            .iter()
            .any(|k| params.get(k).is_some())
        {
            let Some(binding) = bindings.get(key) else {
                continue;
            };
            let identity = s(&binding["identity"]);
            if seen.contains(&("event".into(), identity.into())) {
                continue;
            }
            let visible = if events["status"] == "observed" {
                a(&events["visible"])
                    .iter()
                    .filter(|e| e["event_id"] == binding["event_id"])
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };
            let live_visible = visible
                .iter()
                .any(|e| a(&e["option_ids"]).contains(&binding["option_id"]));
            if !prepare_events && !live_visible {
                continue;
            }
            let mut proposal = binding.clone();
            for(k,v)in json!({"status":status,"country":country,"watch":prepare_events,"instances":visible.iter().filter_map(|e|e["instance_id"].as_str()).collect::<Vec<_>>(),"live_visible":live_visible}).as_object().unwrap(){proposal[k]=v.clone();}
            result.push(proposal);
            seen.insert(("event".into(), identity.into()));
            continue;
        }
        let kind = s(&action["kind"]);
        let panel;
        let identity;
        let entries;
        let mut extra_names = vec![];
        if ["select_focus", "research"].contains(&kind) {
            if !native.contains(key) && status != "active" {
                continue;
            }
            panel = if kind == "select_focus" {
                "focus"
            } else {
                "research"
            };
            identity = s(&entity["id"]);
            entries = lookup(s(&entity["kind"]), identity)
                .into_iter()
                .filter(|e| !state::truth(&entity["scope"]) || e["scope"] == entity["scope"])
                .collect::<Vec<_>>();
        } else if kind == "special_project" {
            if status == "active" {
                continue;
            }
            panel = "projects";
            identity = s(&entity["id"]);
            entries = lookup("project", identity);
        } else if ["politics", "characters"].contains(&s(&params["category"])) {
            if item["key"] != runtime["action_sequence"]["political"] {
                continue;
            }
            panel = "politics";
            identity = s(params
                .get("advisor_id")
                .filter(|v| state::truth(v))
                .or_else(|| params.get("law_id").filter(|v| state::truth(v)))
                .unwrap_or(&params["idea_id"]));
            let ids = [&params["law_id"], &params["idea_id"], &params["advisor_id"]]
                .into_iter()
                .filter(|v| !v.is_null())
                .map(s)
                .collect::<HashSet<_>>();
            entries = ["law", "idea", "advisor"]
                .into_iter()
                .flat_map(|kind| ids.iter().flat_map(move |id| lookup(kind, id)))
                .filter(|e| {
                    (!state::truth(&params["role"])
                        || e.get("slot").unwrap_or(&e["group"]) == &params["role"])
                        && (!state::truth(&params["law_group"])
                            || e["group"] == params["law_group"])
                })
                .collect::<Vec<_>>();
            if state::truth(&params["person_name"]) {
                extra_names.push(params["person_name"].clone());
            }
        } else {
            continue;
        }
        if identity.is_empty() || seen.contains(&(panel.into(), identity.into())) {
            continue;
        }
        if ["politics", "projects"].contains(&panel) && !ready(&item["action_readiness"]) {
            continue;
        }
        if entries.len() > 1 || ["politics", "projects"].contains(&panel) && entries.len() != 1 {
            continue;
        }
        for entry in &entries {
            extra_names.extend([entry["localized_name"].clone(), entry["name"].clone()]);
            if !["politics", "projects"].contains(&panel) {
                extra_names.extend(a(&entry["aliases"]).iter().cloned());
            }
        }
        let names = labels(extra_names.into_iter());
        if names.is_empty() {
            continue;
        }
        seen.insert((panel.into(), identity.into()));
        let prefix = if alternatives.contains(key) {
            "Alternative · "
        } else if status == "ready"
            && item.get("focus_readiness").is_some()
            && item["focus_readiness"]["value"].is_null()
        {
            "Verify requirements · "
        } else {
            ""
        };
        let title = row
            .get("title")
            .filter(|v| state::truth(v))
            .or_else(|| row.get("name").filter(|v| state::truth(v)))
            .unwrap_or(&row["key"]);
        let mut proposal = json!({"key":key,"panel":panel,"identity":identity,"labels":names,"title":format!("{prefix}{}",s(title)),"status":status,"folder":row["folder"],"folder_name":row["folder_name"],"event_title":params["event_name"],"event_titles":[],"country":country,"scope":entity.get("scope").filter(|v|state::truth(v)).cloned().unwrap_or_else(||if panel=="focus"{guide["applies_to"]["tree_id"].clone()}else{Value::Null}),"quantity":item["production"].get("target").unwrap_or_else(||params.get("requested_factories").unwrap_or(&params["assigned_factories"]))});
        if alternatives.contains(key) {
            proposal["tone"] = json!("choice");
            proposal["recommendation"] = json!("alternative");
        } else if state::truth(&params["focus_optional"]) {
            proposal["tone"] = json!("choice");
        }
        if ["politics", "projects"].contains(&panel) {
            proposal["action_readiness"] = item["action_readiness"].clone();
            proposal["entity_kind"] = entries[0]["kind"].clone();
            proposal["category_id"] = params
                .get("law_group")
                .filter(|v| state::truth(v))
                .or_else(|| params.get("role").filter(|v| state::truth(v)))
                .unwrap_or(&entries[0]["group"])
                .clone();
            proposal["specialization"] = entries[0]["specialization"].clone();
        }
        result.push(proposal);
    }
    let mut choices = HashMap::<String, HashSet<String>>::new();
    for proposal in &result {
        if proposal["panel"] == "event" {
            let event = proposal["event_id"]
                .as_str()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| s(&proposal["identity"]).split(':').next().unwrap_or(""));
            choices
                .entry(event.into())
                .or_default()
                .insert(s(&proposal["identity"]).into());
        }
    }
    result.retain(|p| {
        p["panel"] != "event"
            || choices
                .get(
                    p["event_id"]
                        .as_str()
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| s(&p["identity"]).split(':').next().unwrap_or("")),
                )
                .is_some_and(|v| v.len() == 1)
    });
    Ok(json!(result))
}
