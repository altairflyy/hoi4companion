//! Events a nation can receive, for the suite's events page. Only the
//! installed game's catalog (receivers from `event_reach`): an event whose
//! callers cannot be traced (on_actions, every_/random_ scopes, news) is never
//! claimed for a nation, only listed as "possible" when its own trigger names
//! that nation. Read-only; no live state.
use serde_json::{Value, json};
use std::collections::HashMap;

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
/// `{country, events:[...], possible:[...]}`: each event with its title,
/// description, type, timing, options (with political power cost) and what
/// fires it (focus/decision/event, named).
pub fn list(entries: &Value, country: &str) -> Value {
    let rows: &[Value] = entries.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if country.len() != 3 {
        return json!({"country":country,"events":[],"possible":[],"reason":"country_unknown"});
    }
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
    json!({"country":country,"events":events,"possible":possible})
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
        let result = list(&entries, "SOV");
        assert_eq!(result["events"].as_array().unwrap().len(), 1);
        let purge = &result["events"][0];
        assert_eq!(purge["title"], "Purga");
        assert_eq!(purge["options"], json!([{"id":"sov.1.a","name":"Fallo","political_power_cost":25}]));
        assert_eq!(purge["triggered_by"], json!([{"kind":"focus","id":"SOV_purge","name":"La Grande Purga"},{"kind":"event","id":"sov.0","name":"sov.0"}]));
        assert_eq!(purge["receivers"], json!(["SOV"]));
        assert_eq!(readable_id("CZE_soviet_alliance"), "CZE · soviet alliance");
        assert_eq!(result["possible"].as_array().unwrap().iter().map(|e| e["id"].clone()).collect::<Vec<_>>(), [json!("any.1")]);
        assert_eq!(list(&entries, "")["reason"], "country_unknown");
    }
}
