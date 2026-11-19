//! Which nations can actually receive each event, from the installed game
//! scripts (read-only). Events rarely say who gets them; the effects that
//! fire them do. Sources: national focuses (owner = the nations of their
//! focus tree), decisions (owner = tags in their allowed/visible/available
//! blocks, or, for one allowed to nobody, the nations whose `activate_*` call
//! makes it available) and, transitively, event options/immediate effects
//! (owner = the receivers of that event). A call inside `TAG = { ... }` targets that tag;
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
        calls(body, "effect", &Receiver::Owner, &mut out, 0, self, None);
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
    id.starts_with("decision:") || id.starts_with("flag:") || id.starts_with("tooltip:")
}
/// Key under `unlocks`: a decision shown by a tooltip counts like an activated one there.
fn unlock_key(id: String) -> String {
    id.strip_prefix("tooltip:").map(str::to_owned).unwrap_or(id)
}

/// `from`: the tags a targeted decision's `FROM` (its target) certainly is one
/// of (`target_tags`); a `FROM` block then sends to each of them. Elsewhere,
/// or under another scope, `FROM` stays an unknown receiver.
fn calls(node: &Value, parent: &str, receiver: &Receiver, out: &mut Vec<(String, Receiver)>, depth: usize, effects: &Effects, from: Option<&BTreeSet<String>>) {
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
                    // A tooltip only shows the decision: it unlocks it for the events page but activates nothing.
                    let tooltip = if key == "unlock_decision_tooltip" { "tooltip:" } else { "" };
                    out.push((format!("{tooltip}decision:{id}"), receiver.clone()));
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
        if key == "FROM"
            && let Some(tags) = from
        {
            for tag in tags {
                calls(value, key, &Receiver::Tag(tag.clone()), out, depth + 1, effects, None);
            }
            continue;
        }
        // Control keywords are case-insensitive in the game files (`IF`, `ELSE_IF`, `ElSE`, `Hidden_effect`).
        let lower = key.to_ascii_lowercase();
        let next = if is_tag(key) {
            Receiver::Tag(key.to_owned())
        } else if key.chars().all(|c| c.is_ascii_digit()) {
            // random_list weights keep the scope; a bare number is a state scope.
            if parent.eq_ignore_ascii_case("random_list") { receiver.clone() } else { Receiver::Unknown }
        } else if matches!(key, "ROOT" | "AND" | "OR")
            || matches!(lower.as_str(), "if" | "else_if" | "else" | "hidden_effect" | "random_list"
            | "complete_effect" | "remove_effect" | "timeout_effect" | "cancel_effect" | "completion_reward"
            | "select_effect" | "option" | "immediate" | "effect")
        {
            receiver.clone()
        } else {
            Receiver::Unknown
        };
        // Only a block that keeps the scope keeps the meaning of FROM.
        let keep = if next == *receiver && !is_tag(key) { from } else { None };
        calls(value, key, &next, out, depth + 1, effects, keep);
    }
}
/// The tags a targeted decision's target (`FROM`) certainly is one of: a
/// `target_trigger` whose top level holds `FROM = { tag = X }` or `FROM = {
/// OR = { tag = X tag = Y } }` (other conditions there only narrow the list;
/// several such lists intersect). None when not understood for certain: no
/// such list, a list under `OR`, `NOT` or `if`, or a state target.
pub fn target_tags(decision: &Value) -> Option<BTreeSet<String>> {
    if scalar(decision, "state_target") == "yes" {
        return None;
    }
    let mut set: Option<BTreeSet<String>> = None;
    for block in blocks(decision, "target_trigger") {
        for (_, target) in entries(block).filter(|(k, v)| *k == "FROM" && v.get("entries").is_some()) {
            for (key, value) in entries(target) {
                let found: Option<BTreeSet<String>> = match key {
                    "tag" => value.as_str().filter(|t| is_tag(t)).map(|t| BTreeSet::from([t.to_owned()])),
                    "OR" => {
                        let listed: Vec<Option<&str>> =
                            entries(value).map(|(k, v)| if k == "tag" { v.as_str().filter(|t| is_tag(t)) } else { None }).collect();
                        (!listed.is_empty() && listed.iter().all(Option::is_some)).then(|| listed.into_iter().flatten().map(str::to_owned).collect())
                    }
                    _ => None,
                };
                if let Some(found) = found {
                    set = Some(match set {
                        Some(old) => old.intersection(&found).cloned().collect(),
                        None => found,
                    });
                }
            }
        }
    }
    set.filter(|s| !s.is_empty())
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
    // Decisions no nation is allowed to take by itself (`allowed = { always = no }`,
    // no tag): only an `activate_*` call makes them available, so their owners
    // are the nations those calls target. Each is an internal "decision:ID" node
    // of the event chains below; one no traced caller activates stays unknown.
    let mut activated: HashSet<String> = HashSet::new();
    let mut edges: Vec<(String, String, Receiver)> = vec![];
    let mut gated: HashSet<String> = HashSet::new();
    for root in decision_docs {
        for (_, group) in entries(root).filter(|(_, v)| v.get("entries").is_some()) {
            for (decision_id, decision) in entries(group).filter(|(_, v)| v.get("entries").is_some()) {
                let mut owners = BTreeSet::new();
                for key in ["allowed", "visible", "available"] {
                    for block in blocks(decision, key) {
                        tags(block, &mut owners, 0);
                    }
                }
                let never = blocks(decision, "allowed").any(|b| entries(b).any(|(k, v)| k == "always" && v.as_str() == Some("no")));
                if never && owners.is_empty() {
                    gated.insert(format!("decision:{decision_id}"));
                }
            }
        }
    }
    let gated = &gated;
    // Nations a country flag is set on: a named tag, or the owners of the
    // focus/decision; (flag, event) when an event sets it on its own receiver.
    let mut flag_tags: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut flag_events: Vec<(String, String)> = vec![];
    // Flags each event's own trigger requires, before they are resolved.
    let mut event_flags: HashMap<String, Vec<String>> = HashMap::new();
    let flag_owner = |flag: &str, to: &Receiver, owners: Option<&BTreeSet<String>>, flag_tags: &mut HashMap<String, BTreeSet<String>>| {
        if !flag.starts_with("flag:") {
            return;
        }
        match to {
            Receiver::Tag(t) => {
                flag_tags.entry(flag.to_owned()).or_default().insert(t.clone());
            }
            Receiver::Owner => flag_tags.entry(flag.to_owned()).or_default().extend(owners.into_iter().flatten().cloned()),
            Receiver::Unknown => {}
        }
    };
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
                calls(focus, "focus", &Receiver::Owner, &mut out, 0, effects, None);
                let source = format!("focus:{}", scalar(focus, "id"));
                for (id, to) in out {
                    if gate(&id) {
                        flag_owner(&id, &to, owners.as_ref(), &mut flag_tags);
                        if gated.contains(&id) {
                            activated.insert(id.clone());
                            seed(id.clone(), to.clone(), owners.as_ref(), &mut reach);
                        }
                        reach.unlocks.entry(unlock_key(id)).or_default().insert(source.clone());
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
                let node = format!("decision:{decision_id}");
                let mut out = vec![];
                let target = target_tags(decision);
                calls(decision, "decision", &Receiver::Owner, &mut out, 0, effects, target.as_ref());
                for (id, to) in out {
                    if gate(&id) {
                        flag_owner(&id, &to, Some(&owners), &mut flag_tags);
                        if gated.contains(&id) {
                            activated.insert(id.clone());
                            if gated.contains(&node) {
                                edges.push((node.clone(), id.clone(), to.clone()));
                            } else {
                                seed(id.clone(), to.clone(), Some(&owners), &mut reach);
                            }
                        }
                        reach.unlocks.entry(unlock_key(id)).or_default().insert(node.clone());
                        continue;
                    }
                    reach.sources.entry(id.clone()).or_default().insert(node.clone());
                    if gated.contains(&node) {
                        edges.push((node.clone(), id, to));
                    } else {
                        seed(id, to, Some(&owners), &mut reach);
                    }
                }
            }
        }
    }
    // Event chains: an event's effects reach the receivers of that event.
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
            let flags: Vec<String> = needs.iter().filter(|k| k.starts_with("flag:")).cloned().collect();
            if !flags.is_empty() {
                event_flags.insert(id.clone(), flags);
            }
            if !needs.is_empty() {
                reach.requires.entry(id.clone()).or_default().extend(needs);
            }
            let mut out = vec![];
            calls(event, "event", &Receiver::Owner, &mut out, 0, effects, None);
            let (unlocked, out): (Vec<_>, Vec<_>) = out.into_iter().partition(|(to, _)| gate(to));
            for (unlocked, to) in unlocked {
                if unlocked.starts_with("flag:") {
                    if to == Receiver::Owner {
                        flag_events.push((unlocked.clone(), id.clone()));
                    } else {
                        flag_owner(&unlocked, &to, None, &mut flag_tags);
                    }
                }
                if gated.contains(&unlocked) {
                    activated.insert(unlocked.clone());
                    edges.push((id.clone(), unlocked.clone(), to));
                }
                reach.unlocks.entry(unlock_key(unlocked)).or_default().insert(format!("event:{id}"));
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
    reach.unknown.extend(gated.iter().filter(|d| !activated.contains(*d)).cloned());
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
            // A decision allowed to nobody that an event with no traced
            // receiver activates (on_action, timed event) may reach any nation.
            for (from, to, r) in &edges {
                if *r == Receiver::Owner && gated.contains(to) && !reach.receivers.contains_key(from) && !reach.unknown.contains(from) {
                    changed |= reach.unknown.insert(to.clone());
                }
            }
            if !changed {
                break;
            }
        }
    }
    reach.receivers.retain(|k, _| !k.starts_with("decision:"));
    reach.unknown.retain(|k| !k.starts_with("decision:"));
    // An event nothing calls (timed, mean_time_to_happen) whose trigger needs
    // a country flag reaches the nations that flag is set on.
    for (flag, event) in &flag_events {
        let set = reach.receivers.get(event).cloned().unwrap_or_default();
        flag_tags.entry(flag.clone()).or_default().extend(set);
    }
    for (event, flags) in &event_flags {
        if reach.receivers.contains_key(event) || reach.unknown.contains(event) {
            continue;
        }
        let set: BTreeSet<String> = flags.iter().flat_map(|f| flag_tags.get(f).into_iter().flatten().cloned()).collect();
        if !set.is_empty() {
            reach.receivers.insert(event.clone(), set);
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
            // Spain sets a flag on SOV; a timed event nothing calls needs it.
            ("country_event", node(vec![("id", json!("spr.gold")), ("option", node(vec![
                ("SOV", node(vec![("set_country_flag", json!("SPR_gold_flag_sov"))]))]))])),
            ("country_event", node(vec![("id", json!("spr.seize")),
                ("trigger", node(vec![("has_country_flag", json!("SPR_gold_flag_sov"))])),
                ("mean_time_to_happen", node(vec![("days", json!("30"))]))])),
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
        assert_eq!(set(&r.receivers, "spr.seize"), ["SOV"], "a timed event reaches the nation its flag is set on");
        assert_eq!(set(&r.requires, "spr.seize"), ["event:spr.gold"]);
        assert_eq!(set(&r.receivers, "sov.trial"), ["SOV"], "flag set by a SOV focus");
    }

    #[test]
    fn a_targeted_decision_sends_events_to_the_tags_its_target_trigger_lists() {
        // ETH.txt: `ETH_host_country_decision` targets `FROM = { OR = { tag = ENG ... } }`
        // and its complete_effect sends BBA_italo_ethiopian_war_events.100 to FROM.
        let target = |trigger: Value| node(vec![("allowed", node(vec![("tag", json!("ETH"))])), ("target_trigger", trigger),
            ("complete_effect", node(vec![("FROM", node(vec![("country_event", node(vec![("id", json!("eth.host"))]))]))]))]);
        let tags = node(vec![("FROM", node(vec![("OR", node(vec![("tag", json!("ENG")), ("tag", json!("JAP"))]))]))]);
        let one = node(vec![("FROM", node(vec![("tag", json!("SWE")), ("has_war", json!("yes"))]))]);
        let open = node(vec![("FROM", node(vec![("is_major", json!("yes"))]))]);
        let either = node(vec![("OR", node(vec![("FROM", node(vec![("tag", json!("ENG"))])), ("always", json!("yes"))]))]);
        let mut state = target(node(vec![("FROM", node(vec![("tag", json!("ENG"))]))]));
        state["entries"].as_array_mut().unwrap().push(json!(["state_target", "yes"]));
        let decision = |id: &str, body: Value| (id.to_owned(), body);
        let rows: Vec<(String, Value)> = vec![decision("ETH_host", target(tags.clone())), decision("SWE_one", target(one)),
            decision("ETH_open", target(open)), decision("ETH_either", target(either)), decision("ETH_state", state)];
        for (id, body) in rows {
            let decisions = node(vec![("cat", node(vec![(id.as_str(), body)]))]);
            let r = compute(&[], &[decisions], &[], &[]);
            let set = r.receivers.get("eth.host").cloned().unwrap_or_default().into_iter().collect::<Vec<_>>();
            match id.as_str() {
                "ETH_host" => assert_eq!((set, r.unknown.contains("eth.host")), (vec!["ENG".to_owned(), "JAP".to_owned()], false)),
                "SWE_one" => assert_eq!((set, r.unknown.contains("eth.host")), (vec!["SWE".to_owned()], false), "other conditions only narrow"),
                _ => assert!(r.unknown.contains("eth.host"), "{id}: a target not restricted to tags for certain stays unknown"),
            }
        }
        // FROM outside a targeted decision (an event's sender) stays unknown.
        let events = node(vec![("country_event", node(vec![("id", json!("x.1")), ("option", node(vec![("FROM", node(vec![("country_event", json!("x.2"))]))]))]))]);
        assert!(compute(&[], &[], &[events], &[]).unknown.contains("x.2"));
    }

    #[test]
    fn upper_case_control_blocks_keep_the_scope() {
        // hungary_wuw.txt: `IF = { limit = {...} country_event = { id = WW_hungary.30 } }`.
        let tree = node(vec![("focus_tree", node(vec![
            ("country", node(vec![("modifier", node(vec![("tag", json!("HUN"))]))])),
            ("focus", node(vec![("id", json!("HUN_sanction")), ("completion_reward", node(vec![
                ("IF", node(vec![("limit", node(vec![("always", json!("yes"))])), ("country_event", node(vec![("id", json!("hun.30"))]))])),
                ("ELSE_IF", node(vec![("Hidden_effect", node(vec![("country_event", json!("hun.31"))]))])),
                ("ElSE", node(vec![("country_event", json!("hun.32"))])),
            ]))])),
        ]))]);
        let r = compute(&[tree], &[], &[], &[]);
        for id in ["hun.30", "hun.31", "hun.32"] {
            assert_eq!(r.receivers.get(id).cloned().unwrap_or_default().into_iter().collect::<Vec<_>>(), ["HUN"], "{id}");
            assert!(!r.unknown.contains(id), "{id}");
        }
    }

    #[test]
    fn a_mission_allowed_to_nobody_reaches_the_nations_that_activate_it() {
        // GER.txt: `GER_fourth_silesian_uprising_mission` has `allowed = { always = no }`
        // and is activated by event wtt_germany.1 (fired by a GER focus); its
        // timeout_effect fires wuw_GER_civil_war.5.
        let tree = node(vec![("focus_tree", node(vec![
            ("country", node(vec![("modifier", node(vec![("tag", json!("GER"))]))])),
            ("focus", node(vec![("id", json!("GER_civil_war")), ("completion_reward", node(vec![("country_event", node(vec![("id", json!("ger.1"))]))]))])),
        ]))]);
        let decisions = node(vec![("cat", node(vec![
            ("GER_uprising_mission", node(vec![("allowed", node(vec![("always", json!("no"))])),
                ("timeout_effect", node(vec![("country_event", node(vec![("id", json!("ger.civil.5"))]))]))])),
            // Activated only under a scope the files do not resolve: unknown.
            ("GER_mefo_mission", node(vec![("allowed", node(vec![("always", json!("no"))])),
                ("timeout_effect", node(vec![("country_event", json!("ger.mefo"))]))])),
            // Never activated by any traced caller: unknown, as before.
            ("GER_orphan_mission", node(vec![("allowed", node(vec![("always", json!("no"))])),
                ("timeout_effect", node(vec![("country_event", json!("ger.orphan"))]))])),
            // Allowed to every nation (no tag, not `always = no`): stays unknown.
            ("open_decision", node(vec![("complete_effect", node(vec![("country_event", json!("any.open"))]))])),
            ("open_activator", node(vec![("allowed", node(vec![("tag", json!("ITA"))])),
                ("complete_effect", node(vec![("activate_decision", json!("open_decision"))]))])),
        ]))]);
        let events = node(vec![("country_event", node(vec![("id", json!("ger.1")), ("option", node(vec![
            ("IF", node(vec![("activate_mission", json!("GER_uprising_mission"))])),
            ("random_other_country", node(vec![("activate_mission", json!("GER_mefo_mission"))])),
        ]))]))]);
        let r = compute(&[tree], &[decisions], &[events], &[]);
        let set = |id: &str| r.receivers.get(id).cloned().unwrap_or_default().into_iter().collect::<Vec<_>>();
        assert_eq!(set("ger.civil.5"), ["GER"]);
        assert!(!r.unknown.contains("ger.civil.5"));
        assert!(r.unknown.contains("ger.mefo") && set("ger.mefo").is_empty());
        assert!(r.unknown.contains("ger.orphan") && set("ger.orphan").is_empty());
        assert!(r.unknown.contains("any.open"), "a decision open to every nation stays unknown");
        assert_eq!(r.sources["ger.civil.5"].iter().collect::<Vec<_>>(), ["decision:GER_uprising_mission"]);
        assert!(r.receivers.keys().chain(r.unknown.iter()).all(|k| !k.starts_with("decision:")), "decision nodes stay internal");
    }

    #[test]
    fn a_mission_also_activated_by_an_untraced_event_or_only_by_a_tooltip_stays_unknown() {
        let tree = node(vec![("focus_tree", node(vec![
            ("country", node(vec![("modifier", node(vec![("tag", json!("GER"))]))])),
            ("focus", node(vec![("id", json!("GER_a")), ("completion_reward", node(vec![
                ("activate_mission", json!("GER_shared_mission")),
                ("unlock_decision_tooltip", json!("GER_tooltip_mission")),
            ]))])),
        ]))]);
        let decisions = node(vec![("cat", node(vec![
            ("GER_shared_mission", node(vec![("allowed", node(vec![("always", json!("no"))])),
                ("timeout_effect", node(vec![("country_event", json!("ger.shared"))]))])),
            ("GER_tooltip_mission", node(vec![("allowed", node(vec![("always", json!("no"))])),
                ("timeout_effect", node(vec![("country_event", json!("ger.tooltip"))]))])),
        ]))]);
        // Nothing traced fires this event (an on_action or a timed event), yet it activates the mission.
        let events = node(vec![("country_event", node(vec![("id", json!("any.untraced")), ("option", node(vec![
            ("activate_mission", json!("GER_shared_mission"))]))]))]);
        let r = compute(&[tree], &[decisions], &[events], &[]);
        assert!(r.unknown.contains("ger.shared"), "an untraced activator keeps the receivers open");
        assert!(r.unknown.contains("ger.tooltip"), "a tooltip does not activate the decision");
        assert_eq!(r.unlocks["decision:GER_tooltip_mission"].iter().collect::<Vec<_>>(), ["focus:GER_a"], "the tooltip still unlocks it for the events page");
    }
}
