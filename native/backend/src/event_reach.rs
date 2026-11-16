//! Which nations can actually receive each event, from the installed game
//! scripts (read-only). Events rarely say who gets them; the effects that
//! fire them do. Sources: national focuses (owner = the nations of their
//! focus tree), decisions (owner = tags in their allowed/visible/available
//! blocks) and, transitively, event options/immediate effects (owner = the
//! receivers of that event). A call inside `TAG = { ... }` targets that tag;
//! any other scope change (every_/random_/FROM/states/...) or a news event
//! makes the receiver unknown. An event is filterable only when its known
//! receivers are complete (no unknown caller).
use crate::script_document::{blocks, entries, scalar};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
enum Receiver {
    Owner,
    Tag(String),
    Unknown,
}
#[derive(Default, Debug)]
pub struct Reach {
    pub receivers: HashMap<String, BTreeSet<String>>,
    pub unknown: HashSet<String>,
}

fn is_tag(key: &str) -> bool {
    key.len() == 3
        && key.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && key.chars().any(|c| c.is_ascii_uppercase())
        && !["AND", "NOT"].contains(&key)
}
/// Effect/condition keys that never execute effects.
const SKIP: &[&str] = &["limit", "trigger", "allowed", "available", "visible", "ai_will_do", "ai_chance",
    "effect_tooltip", "custom_effect_tooltip", "custom_trigger_tooltip", "modifier", "mean_time_to_happen",
    "is_triggered_only", "bypass", "cancel_if_invalid", "complete_tooltip", "target_trigger", "target_root_trigger"];

fn calls(node: &Value, parent: &str, receiver: &Receiver, out: &mut Vec<(String, Receiver)>, depth: usize) {
    if depth > 64 {
        return;
    }
    for (key, value) in entries(node) {
        if SKIP.contains(&key) {
            continue;
        }
        match key {
            "country_event" | "news_event" => {
                let id = value.as_str().map(str::to_owned).unwrap_or_else(|| scalar(value, "id").to_owned());
                if !id.is_empty() {
                    let to = if key == "news_event" { Receiver::Unknown } else { receiver.clone() };
                    out.push((id, to));
                }
                continue;
            }
            _ => {}
        }
        if value.get("entries").is_none() {
            continue;
        }
        let next = if is_tag(key) {
            Receiver::Tag(key.to_owned())
        } else if key.chars().all(|c| c.is_ascii_digit()) {
            // random_list weights keep the scope; a bare number is a state scope.
            if parent == "random_list" { receiver.clone() } else { Receiver::Unknown }
        } else if matches!(key, "ROOT" | "if" | "else_if" | "else" | "hidden_effect" | "random_list" | "AND" | "OR"
            | "complete_effect" | "remove_effect" | "timeout_effect" | "cancel_effect" | "completion_reward"
            | "select_effect" | "option" | "immediate" | "effect")
        {
            receiver.clone()
        } else {
            Receiver::Unknown
        };
        calls(value, key, &next, out, depth + 1);
    }
}
fn tags(node: &Value, out: &mut BTreeSet<String>, depth: usize) {
    if depth > 64 {
        return;
    }
    for (key, value) in entries(node) {
        if ["tag", "original_tag"].contains(&key)
            && let Some(t) = value.as_str().filter(|t| is_tag(t))
        {
            out.insert(t.to_owned());
        } else if value.get("entries").is_some() {
            tags(value, out, depth + 1);
        }
    }
}

/// `focus_docs`: national_focus roots; `decision_docs`: decision roots
/// (category -> decision); `event_docs`: event roots.
pub fn compute(focus_docs: &[Value], decision_docs: &[Value], event_docs: &[Value]) -> Reach {
    let mut reach = Reach::default();
    let seed = |id: String, to: Receiver, owners: Option<&BTreeSet<String>>, reach: &mut Reach| match to {
        Receiver::Tag(t) => {
            reach.receivers.entry(id).or_default().insert(t);
        }
        Receiver::Owner if owners.is_some_and(|o| !o.is_empty()) => {
            reach.receivers.entry(id).or_default().extend(owners.unwrap().iter().cloned());
        }
        _ => {
            reach.unknown.insert(id);
        }
    };
    for root in focus_docs {
        for (key, tree) in entries(root) {
            let owners = if key == "focus_tree" {
                let mut set = BTreeSet::new();
                for country in blocks(tree, "country") {
                    tags(country, &mut set, 0);
                }
                Some(set)
            } else {
                None
            };
            let focuses: Vec<&Value> = if key == "focus_tree" { blocks(tree, "focus").collect() } else if key == "shared_focus" { vec![tree] } else { vec![] };
            for focus in focuses {
                let mut out = vec![];
                calls(focus, "focus", &Receiver::Owner, &mut out, 0);
                for (id, to) in out {
                    seed(id, to, owners.as_ref(), &mut reach);
                }
            }
        }
    }
    for root in decision_docs {
        for (_, group) in entries(root).filter(|(_, v)| v.get("entries").is_some()) {
            for (_, decision) in entries(group).filter(|(_, v)| v.get("entries").is_some()) {
                let mut owners = BTreeSet::new();
                for key in ["allowed", "visible", "available"] {
                    for block in blocks(decision, key) {
                        tags(block, &mut owners, 0);
                    }
                }
                let mut out = vec![];
                calls(decision, "decision", &Receiver::Owner, &mut out, 0);
                for (id, to) in out {
                    seed(id, to, Some(&owners), &mut reach);
                }
            }
        }
    }
    // Event chains: an event's effects reach the receivers of that event.
    let mut edges: Vec<(String, String, Receiver)> = vec![];
    for root in event_docs {
        for (kind, event) in entries(root).filter(|(k, v)| ["country_event", "news_event"].contains(k) && v.get("entries").is_some()) {
            let id = scalar(event, "id").to_owned();
            if id.is_empty() {
                continue;
            }
            if kind == "news_event" {
                reach.unknown.insert(id.clone());
            }
            let mut out = vec![];
            calls(event, "event", &Receiver::Owner, &mut out, 0);
            edges.extend(out.into_iter().map(|(to, r)| (id.clone(), to, r)));
        }
    }
    for _ in 0..64 {
        let mut changed = false;
        for (from, to, r) in &edges {
            let unknown_from = reach.unknown.contains(from);
            match r {
                Receiver::Tag(t) => changed |= reach.receivers.entry(to.clone()).or_default().insert(t.clone()),
                Receiver::Unknown => changed |= reach.unknown.insert(to.clone()),
                Receiver::Owner => {
                    if unknown_from {
                        changed |= reach.unknown.insert(to.clone());
                    }
                    if let Some(set) = reach.receivers.get(from).cloned() {
                        let target = reach.receivers.entry(to.clone()).or_default();
                        for t in set {
                            changed |= target.insert(t);
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    reach
}
/// Filterable answer for one event: Some(true/false) only when its callers are
/// all known; None when unknown or never seen fired (e.g. on_actions).
pub fn receives(reach_receivers: &Value, reach_unknown: &Value, country: &str) -> Option<bool> {
    let list = reach_receivers.as_array()?;
    if reach_unknown == true || list.is_empty() {
        return None;
    }
    Some(list.iter().any(|c| c == country))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn node(pairs: Vec<(&str, Value)>) -> Value {
        json!({"entries":pairs.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>()})
    }
    #[test]
    fn receivers_follow_focus_trees_decisions_tags_and_event_chains() {
        let tree = node(vec![("focus_tree", node(vec![
            ("id", json!("soviet_focus")),
            ("country", node(vec![("modifier", node(vec![("tag", json!("SOV"))]))])),
            ("focus", node(vec![("id", json!("SOV_a")), ("completion_reward", node(vec![
                ("country_event", json!("sov.1")),
                ("ENG", node(vec![("country_event", node(vec![("id", json!("eng.5"))]))])),
                ("every_country", node(vec![("country_event", json!("any.1"))])),
            ]))])),
        ]))]);
        let decisions = node(vec![("cat", node(vec![("ITA_dec", node(vec![
            ("allowed", node(vec![("original_tag", json!("ITA"))])),
            ("complete_effect", node(vec![("random_list", node(vec![("50", node(vec![("country_event", json!("ita.2"))]))]))])),
        ]))]))]);
        let events = node(vec![
            ("country_event", node(vec![("id", json!("sov.1")), ("option", node(vec![("country_event", json!("sov.2"))]))])),
            ("country_event", node(vec![("id", json!("eng.5")), ("option", node(vec![("FROM", node(vec![("country_event", json!("back.1"))]))]))])),
        ]);
        let r = compute(&[tree], &[decisions], &[events]);
        let set = |id: &str| r.receivers.get(id).cloned().unwrap_or_default().into_iter().collect::<Vec<_>>();
        assert_eq!(set("sov.1"), ["SOV"]);
        assert_eq!(set("sov.2"), ["SOV"], "chain inherits the receiver");
        assert_eq!(set("eng.5"), ["ENG"], "TAG scope targets that nation");
        assert_eq!(set("ita.2"), ["ITA"], "random_list weight keeps the scope");
        assert!(r.unknown.contains("any.1") && r.unknown.contains("back.1"));
        assert_eq!(receives(&json!(["SOV"]), &json!(false), "SOV"), Some(true));
        assert_eq!(receives(&json!(["ENG"]), &json!(false), "SOV"), Some(false));
        assert_eq!(receives(&json!(["ENG"]), &json!(true), "SOV"), None);
        assert_eq!(receives(&json!([]), &json!(false), "SOV"), None);
    }
}
