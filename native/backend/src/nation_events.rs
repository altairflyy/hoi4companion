//! Events a nation can receive, for the suite's events page. Only the
//! installed game's catalog (receivers from `event_reach`): an event whose
//! callers cannot be traced (on_actions, every_/random_ scopes, news) is never
//! claimed for a nation, only listed as "possible" when its own trigger names
//! that nation. Read-only; no live state.
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn name(row: &Value) -> Value {
    [&row["localized_name"], &row["name"], &row["id"]].into_iter().find(|v| !s(v).trim().is_empty()).cloned().unwrap_or(Value::Null)
}

fn readable_id(id: &str) -> String {
    match id.split_once('_') {
        Some((tag, rest)) if tag.len() == 3 && tag.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) => format!("{tag} · {}", rest.replace('_', " ")),
        _ => id.replace('_', " "),
    }
}
/// Focus ids a guide definition asks for (its `select_focus` steps), in order.
pub fn guide_focuses(definition: &Value) -> Vec<String> {
    let tracks: Vec<&Value> = match &definition["tracks"] {
        Value::Object(map) => map.values().collect(),
        Value::Array(list) => list.iter().map(|t| if t["steps"].is_array() { &t["steps"] } else { t }).collect(),
        _ => vec![],
    };
    let mut seen = HashSet::new();
    tracks.into_iter().flat_map(|steps| steps.as_array().into_iter().flatten())
        .filter(|step| step["action"]["entity"]["kind"] == "focus")
        .filter_map(|step| step["action"]["entity"]["id"].as_str())
        .filter(|id| seen.insert(id.to_owned()))
        .map(str::to_owned).collect()
}

/// For every event the guide's focuses lead to, the guide focuses at the start
/// of the chain. Links: a focus fires an event (directly or through a
/// scripted effect) or makes a decision available; a decision or event fires
/// the next event (other nations' answers included); an event whose own
/// trigger requires a focus. Only what the scripts trace.
fn guide_roots(rows: &[Value], guide: &[String]) -> HashMap<String, BTreeSet<String>> {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows.iter().filter(|r| r["kind"] == "event" || r["kind"] == "decision") {
        let node = format!("{}:{}", s(&row["kind"]), s(&row["id"]));
        let required = row["requires_focus"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|f| format!("focus:{f}"));
        let sources = row["triggered_by"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned);
        for source in sources.chain(required) {
            children.entry(source).or_default().push(node.clone());
        }
    }
    let mut roots: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut work: Vec<(String, String)> = vec![];
    for focus in guide {
        for child in children.get(&format!("focus:{focus}")).into_iter().flatten() {
            work.push((child.clone(), focus.clone()));
        }
    }
    while let Some((node, root)) = work.pop() {
        if !roots.entry(node.clone()).or_default().insert(root.clone()) {
            continue;
        }
        for child in children.get(&node).into_iter().flatten() {
            work.push((child.clone(), root.clone()));
        }
    }
    roots
}

/// `{country, events:[...], possible:[...]}`: each event with its title,
/// description, type, timing, options (with political power cost) and what
/// fires it (focus/decision/event, named). With a guide's focuses, each event
/// also names the guide focuses that lead to it (`guide_focuses`, empty when
/// none does).
pub fn list(entries: &Value, country: &str, guide: &[String]) -> Value {
    let rows: &[Value] = entries.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if country.len() != 3 {
        return json!({"country":country,"events":[],"possible":[],"reason":"country_unknown"});
    }
    let roots = guide_roots(rows, guide);
    let by_kind_id: HashMap<(&str, &str), &Value> = rows.iter().map(|r| ((s(&r["kind"]), s(&r["id"])), r)).collect();
    let mut options: HashMap<&str, Vec<Value>> = HashMap::new();
    for row in rows.iter().filter(|r| r["kind"] == "event_option") {
        let mut option = json!({"id":row["id"],"name":name(row)});
        if let Some(cost) = row.get("political_power_cost").filter(|v| v.is_number()) {
            option["political_power_cost"] = cost.clone();
        }
        options.entry(s(&row["event_id"])).or_default().push(option);
    }
    let source = |key: &str| {
        let (kind, id) = key.split_once(':').unwrap_or(("", key));
        // Unknown to the catalog (another nation's tree): "CZE_soviet_alliance" reads "CZE · soviet alliance".
        let found = by_kind_id.get(&(kind, id)).map(|r| name(r)).unwrap_or_else(|| json!(readable_id(id)));
        json!({"kind":kind,"id":id,"name":found})
    };
    let event = |row: &Value| {
        json!({"id":row["id"],"title":name(row),"description":row.get("description").cloned().unwrap_or(Value::Null),
            "event_type":row["event_type"],"triggered_only":row["triggered_only"],"timed":row["timed"],
            "options":options.get(s(&row["id"])).cloned().unwrap_or_default(),
            // Every nation the scripts send it to: one entry means exclusive to that nation.
            "receivers":row["receivers"].as_array().cloned().unwrap_or_default(),
            // Guide focuses that lead to it, in the guide's order.
            "guide_focuses":guide.iter().filter(|f| roots.get(&format!("event:{}", s(&row["id"]))).is_some_and(|r| r.contains(*f)))
                .map(|f| source(&format!("focus:{f}"))).collect::<Vec<_>>(),
            // Position in the guide of the first focus that leads to it (null when none).
            "guide_order":guide.iter().position(|f| roots.get(&format!("event:{}", s(&row["id"]))).is_some_and(|r| r.contains(f))),
            "triggered_by":row["triggered_by"].as_array().into_iter().flatten().filter_map(Value::as_str).map(source).collect::<Vec<_>>()})
    };
    let mut events = vec![];
    let mut possible = vec![];
    for row in rows.iter().filter(|r| r["kind"] == "event") {
        let receives = row["receivers"].as_array().is_some_and(|r| r.iter().any(|t| t == country));
        if receives {
            events.push(event(row));
        } else if row["receivers_unknown"] == true
            && row["identity_countries"].as_array().is_some_and(|c| c.iter().any(|t| t == country))
        {
            possible.push(event(row));
        }
    }
    let order = |a: &Value, b: &Value| s(&a["title"]).to_lowercase().cmp(&s(&b["title"]).to_lowercase());
    events.sort_by(order);
    possible.sort_by(order);
    json!({"country":country,"events":events,"possible":possible,"guide_focus_count":guide.len()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_nation_gets_its_traced_events_and_possible_ones_named_in_their_trigger() {
        let entries = json!([
            {"kind":"event","id":"sov.1","name":"Purge","localized_name":"Purga","description":"Il partito...","event_type":"country_event","triggered_only":true,"timed":false,
                "receivers":["SOV"],"receivers_unknown":false,"triggered_by":["focus:SOV_purge","event:sov.0"]},
            {"kind":"event_option","id":"sov.1.a","event_id":"sov.1","name":"Do it","localized_name":"Fallo","political_power_cost":25},
            {"kind":"event","id":"ger.1","name":"Anschluss","receivers":["GER"],"receivers_unknown":false},
            {"kind":"event","id":"any.1","name":"Border clash","receivers":[],"receivers_unknown":true,"identity_countries":["SOV","JAP"]},
            {"kind":"event","id":"any.2","name":"Elsewhere","receivers":[],"receivers_unknown":true,"identity_countries":["USA"]},
            {"kind":"focus","id":"SOV_purge","name":"The Great Purge","localized_name":"La Grande Purga"}]);
        let result = list(&entries, "SOV", &[]);
        assert_eq!(result["events"].as_array().unwrap().len(), 1);
        let purge = &result["events"][0];
        assert_eq!(purge["title"], "Purga");
        assert_eq!(purge["options"], json!([{"id":"sov.1.a","name":"Fallo","political_power_cost":25}]));
        assert_eq!(purge["triggered_by"], json!([{"kind":"focus","id":"SOV_purge","name":"La Grande Purga"},{"kind":"event","id":"sov.0","name":"sov.0"}]));
        assert_eq!(purge["receivers"], json!(["SOV"]));
        assert_eq!(readable_id("CZE_soviet_alliance"), "CZE · soviet alliance");
        assert_eq!(result["possible"].as_array().unwrap().iter().map(|e| e["id"].clone()).collect::<Vec<_>>(), [json!("any.1")]);
        assert_eq!(list(&entries, "", &[])["reason"], "country_unknown");
    }

    #[test]
    fn guide_focuses_lead_to_events_through_answers_of_other_nations() {
        let entries = json!([
            {"kind":"focus","id":"SOV_pact","name":"Pact"},
            // Guide focus asks Germany; Germany's answer comes back to SOV.
            {"kind":"event","id":"ger.ask","name":"Soviet offer","receivers":["GER"],"triggered_by":["focus:SOV_pact"]},
            {"kind":"event","id":"sov.yes","name":"Germany agrees","receivers":["SOV"],"triggered_by":["event:ger.ask"]},
            {"kind":"event","id":"sov.loop","name":"Loop","receivers":["SOV"],"triggered_by":["event:sov.yes","event:sov.loop"]},
            {"kind":"event","id":"sov.other","name":"Other branch","receivers":["SOV"],"triggered_by":["focus:SOV_other"]}]);
        let definition = json!({"tracks":{"focus":[
            {"action":{"kind":"select_focus","entity":{"kind":"focus","id":"SOV_pact"}}},
            {"action":{"kind":"research","entity":{"kind":"technology","id":"tech"}}},
            {"action":{"kind":"select_focus","entity":{"kind":"focus","id":"SOV_pact"}}}]}});
        let guide = guide_focuses(&definition);
        assert_eq!(guide, ["SOV_pact"]);
        let result = list(&entries, "SOV", &guide);
        let by_id = |id: &str| result["events"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap().clone();
        assert_eq!(by_id("sov.yes")["guide_focuses"], json!([{"kind":"focus","id":"SOV_pact","name":"Pact"}]));
        assert_eq!(by_id("sov.loop")["guide_focuses"].as_array().unwrap().len(), 1, "cycles end");
        assert_eq!(by_id("sov.other")["guide_focuses"], json!([]), "focus outside the guide");
        assert_eq!(by_id("sov.yes")["guide_order"], 0);
        assert_eq!(by_id("sov.other")["guide_order"], Value::Null);
        assert_eq!(result["guide_focus_count"], 1);
    }

    #[test]
    fn guide_focuses_lead_through_decisions_and_required_focuses() {
        let entries = json!([
            {"kind":"decision","id":"SOV_mission","triggered_by":["focus:SOV_purge"]},
            {"kind":"event","id":"sov.mission","receivers":["SOV"],"triggered_by":["decision:SOV_mission"]},
            {"kind":"event","id":"sov.timed","receivers":["SOV"],"triggered_by":[],"requires_focus":["SOV_purge"]}]);
        let result = list(&entries, "SOV", &["SOV_purge".to_owned()]);
        for event in result["events"].as_array().unwrap() {
            assert_eq!(event["guide_order"], 0, "{}", event["id"]);
        }
    }
}
