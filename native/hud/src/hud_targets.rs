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

/// The event answer of a guide line with several possible answers
/// (`reference_candidates.event_options`) that the player sees right now,
/// proposed for highlight with its own basis (`resolution`
/// "visible_candidate"), never as a proven binding and never as completion
/// (the evaluation alone decides the step's status). Only when: the qualified
/// popup reader observes exactly one visible popup answer among the line's
/// candidates; that answer's text starts with the guide's words; the line is
/// the current one of its track (not done, every earlier non-reminder step
/// done) and the next focus anchor after it is not done.
fn visible_candidate(guide: &Value, row: &Value, item: &Value, statuses: &HashMap<&str, &str>, events: &Value, index: &Index, country: &Value) -> Option<Value> {
    let params = &row["action"]["params"];
    let candidates = &params["reference_candidates"];
    let options = a(&candidates["event_options"]);
    let text = s(&candidates["text"]);
    if options.is_empty() || text.is_empty() || state::truth(&params["event_option_id"]) || events["status"] != "observed" {
        return None;
    }
    let status = s(&item["status"]);
    if !["ready", "active", "unverifiable"].contains(&status) {
        return None;
    }
    let key = s(&row["key"]);
    let steps = guide["tracks"].as_object()?.values().map(a).find(|steps| steps.iter().any(|r| r["key"] == row["key"]))?;
    let position = steps.iter().position(|r| r["key"] == row["key"])?;
    let done = |r: &Value| matches!(statuses.get(s(&r["key"])), Some(&"completed") | Some(&"not_applicable"));
    if steps[..position].iter().any(|r| r["tracking"] != "reminder" && !done(r)) {
        return None;
    }
    let anchor = |r: &Value| {
        r["action"]["entity"]["kind"] == "focus"
            || r["action"]["params"]["trigger_resolution"]["status"] == "compiled" && r["action"]["params"]["trigger_resolution"]["entity"]["kind"] == "focus"
    };
    // The window stays open only while the anchor is known to be open: an
    // unverifiable or unevaluated anchor may already be done.
    if steps[position + 1..].iter().find(|r| anchor(r))
        .is_some_and(|r| !matches!(statuses.get(s(&r["key"])), Some(&"ready") | Some(&"active") | Some(&"waiting") | Some(&"blocked")))
    {
        return None;
    }
    // Candidate answers on screen now: (event, option, popup instance).
    let mut shown: Vec<(&str, &str, Vec<&str>)> = vec![];
    for option in options {
        let (event, id) = (s(&option["event_id"]), s(&option["id"]));
        let instances: Vec<&str> = a(&events["visible"]).iter()
            .filter(|e| e["event_id"] == event && a(&e["option_ids"]).iter().any(|o| o == id))
            .filter_map(|e| e["instance_id"].as_str()).collect();
        if !instances.is_empty() && !shown.iter().any(|(e, o, _)| *e == event && *o == id) {
            shown.push((event, id, instances));
        }
    }
    let [(event, id, instances)] = shown.as_slice() else { return None };
    let rows = index.rows();
    let answers: Vec<&Value> = rows.iter().filter(|r| r["kind"] == "event_option" && r["event_id"] == *event && r["id"] == *id).collect();
    let names = labels(answers.iter().flat_map(|r| [r["localized_name"].clone(), r["name"].clone()].into_iter().chain(a(&r["aliases"]).iter().cloned())));
    if answers.is_empty() || !names.iter().any(|n| crate::reference_candidates::answer_matches(text, s(n))) {
        return None;
    }
    let titles = labels(rows.iter().filter(|r| r["kind"] == "event" && r["id"] == *event).flat_map(|r| [r["localized_name"].clone(), r["name"].clone()]));
    Some(json!({"key":key,"panel":"event","identity":format!("{event}:{id}"),"event_id":event,"option_id":id,"labels":names,
        "event_titles":titles,"event_title":titles.first().cloned().unwrap_or(json!(event)),"title":names.first().cloned().unwrap_or(json!(id)),
        "resolution":"visible_candidate","status":status,"country":country,"watch":false,"instances":instances,"live_visible":true,"ocr_only":false}))
}

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
    let statuses: HashMap<&str, &str> = a(&runtime["steps"]).iter().map(|r| (s(&r["key"]), s(&r["status"]))).collect();
    for item in items {
        let key = s(&item["key"]);
        let Some(row) = rows.get(key).copied() else {
            continue;
        };
        let status = s(&item["status"]);
        let reason = s(&item["reason"]);
        let action = &row["action"];
        let params = &action["params"];
        if let Some(proposal) = visible_candidate(guide, row, item, &statuses, events, index, country) {
            if seen.insert(("event".into(), s(&proposal["identity"]).into())) {
                result.push(proposal);
            }
            continue;
        }
        if row["tracking"] != "reminder" && params["category"] == "production" {
            if (["ready", "active", "completed"].contains(&status)
                || status == "not_applicable" && reason == "production_superseded"
                || status == "unverifiable" && reason == "manual_confirmation_required"
                // Its own targets are observed even while an earlier manual step
                // ("Finish all but cancel extra") is unconfirmed.
                || status == "unverifiable" && reason == "dependency_unknown")
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
        if row["tracking"] == "reminder" && !((prepare_events || events["status"] != "observed") && bindings.get(key).is_some())
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
            // Without a qualified event reader the answer is still proposed, but
            // only exact on-screen text (event title and answer) can place it.
            let ocr_only = events["status"] != "observed";
            if !prepare_events && !live_visible && !ocr_only {
                continue;
            }
            let mut proposal = binding.clone();
            for(k,v)in json!({"status":status,"country":country,"watch":prepare_events,"instances":visible.iter().filter_map(|e|e["instance_id"].as_str()).collect::<Vec<_>>(),"live_visible":live_visible,"ocr_only":ocr_only&&!live_visible}).as_object().unwrap(){proposal[k]=v.clone();}
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
    // Every proposal carries the role that picks its HUD colour; `tone` stays.
    for p in &mut result {
        let row = rows.get(s(if p["guide_step"].is_string() { &p["guide_step"] } else { &p["key"] })).copied();
        let params = row.map(|r| &r["action"]["params"]);
        let role = if p["resolution"] == "visible_candidate" {
            "ambiguous"
        } else if p["tone"] == "choice" {
            "choice"
        } else if p["status"] == "active" && p["panel"] != "event" {
            "active"
        } else {
            "next"
        };
        p["role"] = json!(role);
        if role == "ambiguous" {
            p["reference_text"] = params.map_or(Value::Null, |v| v["reference_candidates"]["text"].clone());
        }
        if p["panel"] == "event" && params.is_some_and(|v| v["reference_resolution"]["basis"] == "user_confirmed") {
            p["confirmed_by_user"] = json!(true);
        }
    }
    Ok(json!(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A guide line with several possible event answers, between a done step
    /// and the focus anchor that follows it, and the live popup reader.
    fn fixture() -> (Value, Value, Index, Value) {
        let index = Index::new(&json!({"entries":[
            {"kind":"country","id":"ENG","name":"United Kingdom"},
            {"kind":"event","id":"eth.100","name":"A Request from Ethiopia"},
            {"kind":"event_option","id":"eth.100.b","event_id":"eth.100","name":"We cannot allow this"},
            {"kind":"event_option","id":"eth.100.a","event_id":"eth.100","name":"Of course"},
            {"kind":"event","id":"hol.4","name":"Dutch question"},
            {"kind":"event_option","id":"hol.4.b","event_id":"hol.4","name":"We cannot help them"},
        ]})).unwrap();
        let options = json!([{"id":"eth.100.b","event_id":"eth.100","compatible":true},{"id":"hol.4.b","event_id":"hol.4","compatible":true}]);
        let guide = json!({"id":"g","applies_to":{"country":"ENG"},"tracks":{"operations":[
            {"key":"a","title":"Done","action":{"kind":"manual","params":{"instructions":["Done"]}}},
            {"key":"b","title":"We cannot…","tracking":"reminder","action":{"kind":"manual","params":{"instructions":["We cannot…"],
                "reference_candidates":{"status":"ambiguous","text":"We cannot…","candidates":options,"event_options":options}}}},
            {"key":"c","action":{"kind":"select_focus","entity":{"kind":"focus","id":"ENG_organize"}}},
        ]}});
        let runtime = json!({"guide_id":"g","mode":"live","country":"ENG","steps":[
            {"key":"a","status":"completed"},{"key":"b","status":"ready","reason":"reminder_available"},{"key":"c","status":"ready"}]});
        let live = json!({"usable":true,"sample":{"country":"ENG","country_overview":{"country":"ENG","events":{"status":"observed",
            "visible":[{"event_id":"eth.100","instance_id":"i1","option_ids":["eth.100.a","eth.100.b"]}]}}}});
        (guide, runtime, index, live)
    }
    fn events(result: &Value) -> Vec<Value> {
        result.as_array().unwrap().iter().filter(|p| p["panel"] == "event").cloned().collect()
    }

    #[test]
    fn a_visible_popup_highlights_the_only_candidate_answer_of_the_current_line() {
        let (guide, runtime, index, live) = fixture();
        let found = events(&targets(&guide, &runtime, &index, &live, false, &Value::Null).unwrap());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["identity"], "eth.100:eth.100.b");
        assert_eq!(found[0]["key"], "b");
        // Its own basis: a candidate the popup shows, not a proven binding.
        assert_eq!(found[0]["resolution"], "visible_candidate");
        assert_eq!(found[0]["instances"], json!(["i1"]));
        // Only a highlight: the step's status is the evaluation's, never completed here.
        assert_eq!(found[0]["status"], "ready");
        assert!(found[0].get("completion").is_none());
    }

    #[test]
    fn no_highlight_without_every_condition() {
        let (guide, runtime, index, live) = fixture();
        let none = |guide: &Value, runtime: &Value, live: &Value| events(&targets(guide, runtime, &index, live, false, &Value::Null).unwrap()).is_empty();
        // An earlier step not done: the line is not the current one.
        let mut r = runtime.clone();
        r["steps"][0]["status"] = json!("ready");
        assert!(none(&guide, &r, &live), "previous step open");
        // The next focus anchor already done: the line's window is over.
        let mut r = runtime.clone();
        r["steps"][2]["status"] = json!("completed");
        assert!(none(&guide, &r, &live), "anchor done");
        // An anchor whose status is unknown may be done already.
        for unknown in [json!("unverifiable"), json!("not_applicable"), Value::Null] {
            let mut r = runtime.clone();
            r["steps"][2]["status"] = unknown.clone();
            assert!(none(&guide, &r, &live), "anchor {unknown}");
        }
        // The line itself done.
        let mut r = runtime.clone();
        r["steps"][1]["status"] = json!("completed");
        assert!(none(&guide, &r, &live), "line done");
        // Two candidates of the line visible at once.
        let mut l = live.clone();
        l["sample"]["country_overview"]["events"]["visible"].as_array_mut().unwrap()
            .push(json!({"event_id":"hol.4","instance_id":"i2","option_ids":["hol.4.b"]}));
        assert!(none(&guide, &runtime, &l), "two candidates visible");
        // The popup's options do not include the candidate answer.
        let mut l = live.clone();
        l["sample"]["country_overview"]["events"]["visible"][0]["option_ids"] = json!(["eth.100.a"]);
        assert!(none(&guide, &runtime, &l), "answer not shown");
        // Unknown or unqualified popup reading.
        let mut l = live.clone();
        l["sample"]["country_overview"]["events"]["status"] = json!("unavailable");
        assert!(none(&guide, &runtime, &l), "reader not observed");
        // The answer's text does not start with the guide's words.
        let mut g = guide.clone();
        g["tracks"]["operations"][1]["action"]["params"]["reference_candidates"]["text"] = json!("We shall…");
        assert!(none(&g, &runtime, &live), "text differs");
    }

    #[test]
    fn visible_candidate_has_role_ambiguous_and_reference_text() {
        let (guide, runtime, index, live) = fixture();
        let found = events(&targets(&guide, &runtime, &index, &live, false, &Value::Null).unwrap());
        assert_eq!(found[0]["resolution"], "visible_candidate");
        assert_eq!(found[0]["role"], "ambiguous");
        assert_eq!(found[0]["reference_text"], "We cannot…");
        assert!(found[0].get("confirmed_by_user").is_none());
    }

    #[test]
    fn user_confirmed_binding_is_next_and_flagged() {
        let (mut guide, runtime, index, live) = fixture();
        guide["tracks"]["operations"][1]["action"]["params"] = json!({"event_id":"eth.100","event_option_id":"eth.100.b",
            "reference_resolution":{"basis":"user_confirmed"},
            "reference_candidates":{"status":"ambiguous","text":"We cannot…","confirmed":{"event_id":"eth.100","id":"eth.100.b"}}});
        guide["tracks"]["operations"][1]["tracking"] = Value::Null;
        let plan = json!({"guide_id":"g","bindings":{"b":{"key":"b","panel":"event","identity":"eth.100:eth.100.b","event_id":"eth.100","option_id":"eth.100.b",
            "labels":["We cannot allow this"],"event_titles":[],"event_title":"eth.100","resolution":"catalog_identity","title":"We cannot allow this"}}});
        let found = events(&targets(&guide, &runtime, &index, &live, false, &plan).unwrap());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["role"], "next");
        assert_eq!(found[0]["confirmed_by_user"], true);
        assert_eq!(found[0]["resolution"], "catalog_identity");
    }

    #[test]
    fn choice_and_active_roles_match_previous_tone() {
        let (guide, mut runtime, index, live) = fixture();
        // An optional focus is a choice, an active focus is active, a ready one is next.
        let mut g = guide.clone();
        g["tracks"]["operations"][2]["action"]["params"] = json!({"focus_optional":true});
        let index = Index::new(&json!({"entries":[{"kind":"country","id":"ENG","name":"United Kingdom"},
            {"kind":"focus","id":"ENG_organize","name":"Organize the army","scope":null}]})).unwrap();
        let role = |g: &Value, r: &Value| {
            let all = targets(g, r, &index, &live, false, &Value::Null).unwrap();
            all.as_array().unwrap().iter().find(|p| p["panel"] == "focus").map(|p| (p["role"].clone(), p.get("tone").cloned()))
        };
        runtime["parallel"] = json!({"focus":{"steps":["c"]}});
        assert_eq!(role(&g, &runtime), Some((json!("choice"), Some(json!("choice")))));
        assert_eq!(role(&guide, &runtime), Some((json!("next"), None)));
        runtime["steps"][2]["status"] = json!("active");
        assert_eq!(role(&guide, &runtime), Some((json!("active"), None)));
    }
}
