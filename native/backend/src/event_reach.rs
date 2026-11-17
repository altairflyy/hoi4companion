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
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

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
    /// What fires each event: "focus:ID", "decision:ID" or "event:ID".
    pub sources: HashMap<String, BTreeSet<String>>,
    /// What makes each decision available: "focus:ID" (activated by it or
    /// required by its conditions), "decision:ID" or "event:ID".
    pub unlocks: HashMap<String, BTreeSet<String>>,
    /// What an event's own trigger requires: "focus:ID" (`has_completed_focus`)
    /// or, for a required country flag, whatever sets it.
    pub requires: HashMap<String, BTreeSet<String>>,
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

/// Scripted effects by name, expanded where a focus, decision or event calls
/// them; each body's calls are computed once, relative to the caller's scope.
#[derive(Default)]
struct Effects<'a> {
    bodies: HashMap<&'a str, &'a Value>,
    memo: RefCell<HashMap<String, Rc<Vec<(String, Receiver)>>>>,
    active: RefCell<HashSet<String>>,
}
impl Effects<'_> {
    fn expand(&self, name: &str) -> Option<Rc<Vec<(String, Receiver)>>> {
        let body = self.bodies.get(name)?;
        if let Some(found) = self.memo.borrow().get(name) {
            return Some(found.clone());
        }
        // A recursive effect contributes nothing more while it is being expanded.
        if !self.active.borrow_mut().insert(name.to_owned()) {
            return Some(Rc::new(vec![]));
        }
        let mut out = vec![];
        calls(body, "effect", &Receiver::Owner, &mut out, 0, self);
        self.active.borrow_mut().remove(name);
        let out = Rc::new(out);
        self.memo.borrow_mut().insert(name.to_owned(), out.clone());
        Some(out)
    }
}
/// Decision a call makes available: `activate_decision = X` or `{ decision = X }`.
const ACTIVATES: &[&str] = &["activate_decision", "activate_mission", "activate_targeted_decision", "unlock_decision_tooltip"];

/// What a condition block requires, outside `NOT`: "focus:ID"
/// (`has_completed_focus`) and "flag:NAME" (`has_country_flag`).
fn conditions(node: &Value, out: &mut BTreeSet<String>, depth: usize) {
    if depth > 64 {
        return;
    }
    for (key, value) in entries(node) {
        let named = value.as_str().map(str::to_owned).unwrap_or_else(|| scalar(value, "flag").to_owned());
        match key {
            "has_completed_focus" if !named.is_empty() => {
                out.insert(format!("focus:{named}"));
            }
            "has_country_flag" if !named.is_empty() => {
                out.insert(format!("flag:{named}"));
            }
            "NOT" => {}
            _ if value.get("entries").is_some() => conditions(value, out, depth + 1),
            _ => {}
        }
    }
}
/// A call target that is not an event: a decision made available or a country flag set.
fn gate(id: &str) -> bool {
    id.starts_with("decision:") || id.starts_with("flag:")
}

fn calls(node: &Value, parent: &str, receiver: &Receiver, out: &mut Vec<(String, Receiver)>, depth: usize, effects: &Effects) {
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
            "set_country_flag" => {
                let id = value.as_str().map(str::to_owned).unwrap_or_else(|| scalar(value, "flag").to_owned());
                if !id.is_empty() {
                    out.push((format!("flag:{id}"), receiver.clone()));
                }
                continue;
            }
            _ if ACTIVATES.contains(&key) => {
                let id = value.as_str().map(str::to_owned).unwrap_or_else(|| scalar(value, "decision").to_owned());
                if !id.is_empty() {
                    out.push((format!("decision:{id}"), receiver.clone()));
                }
                continue;
            }
            _ => {}
        }
        if let Some(found) = effects.expand(key) {
            out.extend(found.iter().map(|(id, to)| (id.clone(), if *to == Receiver::Owner { receiver.clone() } else { to.clone() })));
            continue;
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
        calls(value, key, &next, out, depth + 1, effects);
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
pub fn compute(focus_docs: &[Value], decision_docs: &[Value], event_docs: &[Value], effect_docs: &[Value]) -> Reach {
    let mut reach = Reach::default();
    let mut effects = Effects::default();
    for root in effect_docs {
        for (name, body) in entries(root).filter(|(_, v)| v.get("entries").is_some()) {
            effects.bodies.insert(name, body);
        }
    }
    let effects = &effects;
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
                calls(focus, "focus", &Receiver::Owner, &mut out, 0, effects);
                let source = format!("focus:{}", scalar(focus, "id"));
                for (id, to) in out {
                    if gate(&id) {
                        reach.unlocks.entry(id).or_default().insert(source.clone());
                        continue;
                    }
                    reach.sources.entry(id.clone()).or_default().insert(source.clone());
                    seed(id, to, owners.as_ref(), &mut reach);
                }
            }
        }
    }
    for root in decision_docs {
        for (_, group) in entries(root).filter(|(_, v)| v.get("entries").is_some()) {
            for (decision_id, decision) in entries(group).filter(|(_, v)| v.get("entries").is_some()) {
                let mut owners = BTreeSet::new();
                let mut needs = BTreeSet::new();
                for key in ["allowed", "visible", "available"] {
                    for block in blocks(decision, key) {
                        tags(block, &mut owners, 0);
                        conditions(block, &mut needs, 0);
                    }
                }
                if !needs.is_empty() {
                    reach.unlocks.entry(format!("decision:{decision_id}")).or_default().extend(needs);
                }
                let mut out = vec![];
                calls(decision, "decision", &Receiver::Owner, &mut out, 0, effects);
                for (id, to) in out {
                    if gate(&id) {
                        reach.unlocks.entry(id).or_default().insert(format!("decision:{decision_id}"));
                        continue;
                    }
                    reach.sources.entry(id.clone()).or_default().insert(format!("decision:{decision_id}"));
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
            let mut needs = BTreeSet::new();
            for block in blocks(event, "trigger") {
                conditions(block, &mut needs, 0);
            }
            if !needs.is_empty() {
                reach.requires.entry(id.clone()).or_default().extend(needs);
            }
            let mut out = vec![];
            calls(event, "event", &Receiver::Owner, &mut out, 0, effects);
            let (unlocked, out): (Vec<_>, Vec<_>) = out.into_iter().partition(|(to, _)| gate(to));
            for (decision, _) in unlocked {
                reach.unlocks.entry(decision).or_default().insert(format!("event:{id}"));
            }
            for (to, _) in &out {
                reach.sources.entry(to.clone()).or_default().insert(format!("event:{id}"));
            }
            edges.extend(out.into_iter().map(|(to, r)| (id.clone(), to, r)));
        }
    }
    // A required flag stands for whatever sets it (focus, decision, event).
    let flags: HashMap<String, BTreeSet<String>> = reach.unlocks.iter().filter(|(k, _)| k.starts_with("flag:")).map(|(k, v)| (k.clone(), v.clone())).collect();
    let resolve = |set: &mut BTreeSet<String>| {
        let named: Vec<String> = set.iter().filter(|k| k.starts_with("flag:")).cloned().collect();
        for flag in named {
            set.remove(&flag);
            set.extend(flags.get(&flag).into_iter().flatten().cloned());
        }
    };
    reach.requires.values_mut().for_each(resolve);
    reach.unlocks.retain(|k, _| !k.starts_with("flag:"));
    reach.unlocks.values_mut().for_each(resolve);
    reach.requires.retain(|_, v| !v.is_empty());
    reach.unlocks.retain(|_, v| !v.is_empty());
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
        let r = compute(&[tree], &[decisions], &[events], &[]);
        let set = |id: &str| r.receivers.get(id).cloned().unwrap_or_default().into_iter().collect::<Vec<_>>();
        assert_eq!(set("sov.1"), ["SOV"]);
        assert_eq!(set("sov.2"), ["SOV"], "chain inherits the receiver");
        assert_eq!(set("eng.5"), ["ENG"], "TAG scope targets that nation");
        assert_eq!(set("ita.2"), ["ITA"], "random_list weight keeps the scope");
        assert!(r.unknown.contains("any.1") && r.unknown.contains("back.1"));
        let from = |id: &str| r.sources[id].iter().cloned().collect::<Vec<_>>();
        assert_eq!(from("sov.1"), ["focus:SOV_a"]);
        assert_eq!(from("sov.2"), ["event:sov.1"]);
        assert_eq!(from("ita.2"), ["decision:ITA_dec"]);
        assert_eq!(receives(&json!(["SOV"]), &json!(false), "SOV"), Some(true));
        assert_eq!(receives(&json!(["ENG"]), &json!(false), "SOV"), Some(false));
        assert_eq!(receives(&json!(["ENG"]), &json!(true), "SOV"), None);
        assert_eq!(receives(&json!([]), &json!(false), "SOV"), None);
    }

    #[test]
    fn focuses_reach_events_through_scripted_effects_decisions_and_triggers() {
        let tree = node(vec![("focus_tree", node(vec![
            ("country", node(vec![("modifier", node(vec![("tag", json!("SOV"))]))])),
            ("focus", node(vec![("id", json!("SOV_purge")), ("completion_reward", node(vec![
                ("SOV_purge_effect", json!("yes")),
                ("activate_mission", json!("SOV_mission")),
                ("set_country_flag", node(vec![("flag", json!("SOV_trial_flag")), ("days", json!("15"))])),
            ]))])),
        ]))]);
        // The effect calls itself: expansion stops instead of looping.
        let effects = node(vec![("SOV_purge_effect", node(vec![
            ("country_event", json!("sov.purge")), ("SOV_purge_effect", json!("yes"))]))]);
        let decisions = node(vec![("cat", node(vec![
            ("SOV_mission", node(vec![("complete_effect", node(vec![("country_event", json!("sov.mission"))]))])),
            ("SOV_gated", node(vec![("visible", node(vec![("has_completed_focus", json!("SOV_purge")),
                ("NOT", node(vec![("has_completed_focus", json!("SOV_other"))]))]))])),
        ]))]);
        let events = node(vec![
            ("country_event", node(vec![("id", json!("sov.timed")),
                ("trigger", node(vec![("has_completed_focus", json!("SOV_purge"))]))])),
            ("country_event", node(vec![("id", json!("sov.trial")),
                ("trigger", node(vec![("has_country_flag", json!("SOV_trial_flag")),
                    ("NOT", node(vec![("has_country_flag", json!("SOV_never"))]))]))])),
        ]);
        let r = compute(&[tree], &[decisions], &[events], &[effects]);
        let set = |m: &HashMap<String, BTreeSet<String>>, id: &str| m.get(id).cloned().unwrap_or_default().into_iter().collect::<Vec<_>>();
        assert_eq!(set(&r.sources, "sov.purge"), ["focus:SOV_purge"], "scripted effect expanded");
        assert_eq!(set(&r.receivers, "sov.purge"), ["SOV"]);
        assert_eq!(set(&r.unlocks, "decision:SOV_mission"), ["focus:SOV_purge"], "activated by the focus");
        assert_eq!(set(&r.unlocks, "decision:SOV_gated"), ["focus:SOV_purge"], "required focus, NOT excluded");
        assert_eq!(set(&r.sources, "sov.mission"), ["decision:SOV_mission"]);
        assert_eq!(set(&r.requires, "sov.timed"), ["focus:SOV_purge"]);
        assert_eq!(set(&r.requires, "sov.trial"), ["focus:SOV_purge"], "a required flag stands for what sets it");
        assert!(r.unlocks.keys().all(|k| !k.starts_with("flag:")));
    }
}
