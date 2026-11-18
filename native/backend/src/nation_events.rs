//! Events a nation can receive, for the suite's events page. Only the
//! installed game's catalog (receivers from `event_reach`): an event whose
//! callers cannot be traced (on_actions, every_/random_ scopes, news) is never
//! claimed for a nation, only listed as "possible" when its own trigger names
//! that nation. Read-only; no live state.
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};

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
pub use crate::event_routes::guide_focuses;
use crate::event_routes::chains;

/// `{country, events:[...], possible:[...]}`: each event with its title,
/// description, type, timing, options (with political power cost) and what
/// fires it (focus/decision/event, named). With a guide's focuses, each event
/// also names the guide focuses that lead to it (`guide_focuses`, empty when
/// none does) and says whether the guide can see it (`guide_reach`).
/// `historical`: focuses the AI takes on its historical path
/// (catalog source "historical_focuses").
pub fn list(entries: &Value, country: &str, guide: &[String], historical: &Value) -> Value {
    let historical = crate::event_routes::historical_set(historical);
    let rows: &[Value] = entries.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if country.len() != 3 {
        return json!({"country":country,"events":[],"possible":[],"reason":"country_unknown"});
    }
    let chains = chains(rows, country);
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
    let empty = BTreeSet::new();
    let event = |row: &Value| {
        let node = format!("event:{}", s(&row["id"]));
        let roots = chains.roots.get(&node).unwrap_or(&empty);
        // "guide": a guide focus leads to it. "anyway": the nation meets it with
        // no focus (on_actions, timed, history, its own free decisions).
        // "historical": another nation's focus on the AI's historical path
        // leads to it (e.g. GER's Molotov-Ribbentrop pact). "other_branch":
        // only this nation's focuses outside the guide lead to it. "foreign":
        // other nations' focuses or decisions off the historical path.
        let own = format!("{country}_");
        let reach = if guide.iter().any(|f| roots.contains(f)) {
            "guide"
        } else if chains.free.contains(&node) {
            "anyway"
        } else if roots.iter().any(|f| !f.starts_with(&own) && historical.contains(f.as_str())) {
            "historical"
        } else if !roots.is_empty() && roots.iter().all(|f| f.starts_with(&own)) {
            "other_branch"
        } else {
            "foreign"
        };
        json!({"id":row["id"],"title":name(row),"description":row.get("description").cloned().unwrap_or(Value::Null),
            "event_type":row["event_type"],"triggered_only":row["triggered_only"],"timed":row["timed"],
            "options":options.get(s(&row["id"])).cloned().unwrap_or_default(),
            // Every nation the scripts send it to: one entry means exclusive to that nation.
            "receivers":row["receivers"].as_array().cloned().unwrap_or_default(),
            // Guide focuses that lead to it, in the guide's order.
            "guide_focuses":guide.iter().filter(|f| roots.contains(*f)).map(|f| source(&format!("focus:{f}"))).collect::<Vec<_>>(),
            // Position in the guide of the first focus that leads to it (null when none).
            "guide_order":guide.iter().position(|f| roots.contains(f)),
            "guide_reach":reach,
            // For an event only focuses outside the guide lead to: some of them.
            "branch_focuses":if reach == "other_branch" { roots.iter().take(3).map(|f| source(&format!("focus:{f}"))).collect::<Vec<_>>() } else { vec![] },
            // For an event of the historical path: the other nations' historical focuses behind it.
            "historical_focuses":if reach == "historical" { roots.iter().filter(|f| !f.starts_with(&own) && historical.contains(f.as_str())).take(3).map(|f| source(&format!("focus:{f}"))).collect::<Vec<_>>() } else { vec![] },
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
        let result = list(&entries, "SOV", &[], &Value::Null);
        assert_eq!(result["events"].as_array().unwrap().len(), 1);
        let purge = &result["events"][0];
        assert_eq!(purge["title"], "Purga");
        assert_eq!(purge["options"], json!([{"id":"sov.1.a","name":"Fallo","political_power_cost":25}]));
        assert_eq!(purge["triggered_by"], json!([{"kind":"focus","id":"SOV_purge","name":"La Grande Purga"},{"kind":"event","id":"sov.0","name":"sov.0"}]));
        assert_eq!(purge["receivers"], json!(["SOV"]));
        assert_eq!(readable_id("CZE_soviet_alliance"), "CZE · soviet alliance");
        assert_eq!(result["possible"].as_array().unwrap().iter().map(|e| e["id"].clone()).collect::<Vec<_>>(), [json!("any.1")]);
        assert_eq!(list(&entries, "", &[], &Value::Null)["reason"], "country_unknown");
    }

    #[test]
    fn guide_focuses_lead_to_events_through_answers_of_other_nations() {
        let entries = json!([
            {"kind":"focus","id":"SOV_pact","name":"Pact"},
            // Guide focus asks Germany; Germany's answer comes back to SOV.
            {"kind":"event","id":"ger.ask","name":"Soviet offer","receivers":["GER"],"triggered_by":["focus:SOV_pact"]},
            {"kind":"event","id":"sov.yes","name":"Germany agrees","receivers":["SOV"],"triggered_by":["event:ger.ask"]},
            {"kind":"event","id":"sov.loop","name":"Loop","receivers":["SOV"],"triggered_by":["event:sov.yes","event:sov.loop"]},
            {"kind":"event","id":"sov.other","name":"Other branch","receivers":["SOV"],"triggered_by":["focus:SOV_other"]},
            {"kind":"event","id":"sov.both","name":"Also on_action","receivers":["SOV"],"triggered_by":["focus:SOV_other","event:sov.pulse"]},
            {"kind":"event","id":"sov.pulse","name":"Monthly","receivers":["SOV"],"triggered_by":[]},
            {"kind":"event","id":"sov.cze","name":"Czech offer","receivers":["SOV"],"triggered_by":["focus:CZE_offer"]},
            {"kind":"event","id":"sov.hist","name":"Historical Czech move","receivers":["SOV"],"triggered_by":["focus:CZE_historic"]},
            {"kind":"decision","id":"GER_ask","triggered_by":[]},
            {"kind":"event","id":"sov.ger","name":"German request","receivers":["SOV"],"triggered_by":["decision:GER_ask"]},
            {"kind":"event","id":"ita.ai","name":"Italian AI move","receivers":["ITA"],"triggered_by":[]},
            {"kind":"event","id":"sov.ita","name":"Italian answer","receivers":["SOV"],"triggered_by":["event:ita.ai"]},
            {"kind":"decision","id":"SOV_own","triggered_by":[]},
            {"kind":"event","id":"sov.own","name":"Own choice","receivers":["SOV"],"triggered_by":["decision:SOV_own"]}]);
        let definition = json!({"tracks":{"focus":[
            {"action":{"kind":"select_focus","entity":{"kind":"focus","id":"SOV_pact"}}},
            {"action":{"kind":"research","entity":{"kind":"technology","id":"tech"}}},
            {"action":{"kind":"select_focus","entity":{"kind":"focus","id":"SOV_pact"}}}]}});
        let guide = guide_focuses(&definition);
        assert_eq!(guide, ["SOV_pact"]);
        let result = list(&entries, "SOV", &guide, &json!(["CZE_historic"]));
        let by_id = |id: &str| result["events"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap().clone();
        assert_eq!(by_id("sov.yes")["guide_focuses"], json!([{"kind":"focus","id":"SOV_pact","name":"Pact"}]));
        assert_eq!(by_id("sov.loop")["guide_focuses"].as_array().unwrap().len(), 1, "cycles end");
        assert_eq!(by_id("sov.other")["guide_focuses"], json!([]), "focus outside the guide");
        assert_eq!(by_id("sov.yes")["guide_order"], 0);
        assert_eq!(by_id("sov.other")["guide_order"], Value::Null);
        let reach = |id: &str| by_id(id)["guide_reach"].as_str().unwrap().to_owned();
        assert_eq!(reach("sov.yes"), "guide");
        assert_eq!(reach("sov.other"), "other_branch", "only a focus outside the guide leads to it");
        assert_eq!(by_id("sov.other")["branch_focuses"][0]["id"], "SOV_other");
        assert_eq!(reach("sov.both"), "anyway", "a monthly event can fire it too");
        assert_eq!(reach("sov.pulse"), "anyway", "nothing traced calls it");
        assert_eq!(reach("sov.cze"), "foreign", "another nation's focus sends it");
        assert_eq!(reach("sov.ger"), "foreign", "another nation's decision sends it");
        assert_eq!(reach("sov.ita"), "foreign", "an event only another nation receives starts it");
        assert_eq!(reach("sov.own"), "anyway", "own decision, always available");
        assert_eq!(reach("sov.hist"), "historical", "another nation's historical AI focus sends it");
        assert_eq!(by_id("sov.hist")["historical_focuses"][0]["id"], "CZE_historic");
        assert_eq!(result["guide_focus_count"], 1);
    }

    #[test]
    fn guide_focuses_lead_through_decisions_and_required_focuses() {
        let entries = json!([
            {"kind":"decision","id":"SOV_mission","triggered_by":["focus:SOV_purge"]},
            {"kind":"event","id":"sov.mission","receivers":["SOV"],"triggered_by":["decision:SOV_mission"]},
            {"kind":"event","id":"sov.timed","receivers":["SOV"],"triggered_by":[],"enabled_by":["focus:SOV_purge"]}]);
        let result = list(&entries, "SOV", &["SOV_purge".to_owned()], &Value::Null);
        for event in result["events"].as_array().unwrap() {
            assert_eq!(event["guide_order"], 0, "{}", event["id"]);
        }
    }
}
