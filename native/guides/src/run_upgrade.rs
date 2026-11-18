//! Carries a checklist's manual progress from the definition it was evaluated
//! against to the current one, so updating the app updates open checklists.
//! A confirmation moves only where the new definition provably holds the same
//! instruction: same key and text, the same text elsewhere in the same Word
//! block, or the children a confirmed whole block was split into. Anything
//! else is left unconfirmed (never invented); the old progress is kept in the
//! run row by storage.
use crate::occurrence_manifest::{instruction, lineage};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};

fn steps(guide: &Value) -> impl Iterator<Item = &Value> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|v| v.as_array().into_iter().flatten())
}
fn fingerprint(step: &Value) -> String {
    let text = instruction(step)
        .or_else(|| step["title"].as_str())
        .unwrap_or("");
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}
/// New keys for one old key, before filtering by what the new run allows.
fn targets(key: &str, old: Option<&Value>, new: &Value, allowed: &HashSet<String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let new_steps: HashMap<&str, &Value> = steps(new).filter_map(|s| Some((s["key"].as_str()?, s))).collect();
    let Some(old_step) = old.and_then(|g| steps(g).find(|s| s["key"] == key)) else {
        // Original definition unavailable: only an identical key can carry over.
        if new_steps.contains_key(key) {
            out.insert(key.to_owned());
        }
        return out;
    };
    let print = fingerprint(old_step);
    if allowed.contains(key) && new_steps.get(key).is_some_and(|s| fingerprint(s) == print) {
        out.insert(key.to_owned());
        return out;
    }
    let (base, parent, _) = lineage(key);
    let same_text: Vec<&str> = new_steps
        .iter()
        .filter(|(k, s)| allowed.contains(**k) && lineage(k).0 == base && !print.is_empty() && fingerprint(s) == print)
        .map(|(k, _)| *k)
        .collect();
    if same_text.len() == 1 {
        out.insert(same_text[0].to_owned());
        return out;
    }
    // A confirmed whole block now decomposed (or no longer checkable as a
    // whole): its parts were done too.
    if parent.is_none() {
        out.extend(new_steps.keys().filter(|k| lineage(k).0 == base).map(|k| (*k).to_owned()));
    }
    out
}
fn carry(map: &Value, old: Option<&Value>, new: &Value, allowed: &HashSet<String>, moved: &mut Vec<Value>) -> Value {
    let mut out = Map::new();
    for (key, value) in map.as_object().into_iter().flatten() {
        if value != true {
            continue;
        }
        let to: Vec<String> = targets(key, old, new, allowed).into_iter().filter(|k| allowed.contains(k)).collect();
        for k in &to {
            out.insert(k.clone(), json!(true));
        }
        moved.push(json!({"from":key,"to":to}));
    }
    Value::Object(out)
}
/// (checks, started, report) translated onto `new`.
pub fn translate(
    old: Option<&Value>,
    new: &Value,
    checks: &Value,
    started: &Value,
    allowed_checks: &HashSet<String>,
    allowed_started: &HashSet<String>,
) -> (Value, Value, Value) {
    let mut moved = vec![];
    let c = carry(checks, old, new, allowed_checks, &mut moved);
    let s = carry(started, old, new, allowed_started, &mut moved);
    (c, s, Value::Array(moved))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn step(key: &str, text: &str) -> Value {
        json!({"key":key,"action":{"kind":"manual","params":{"instructions":[text]}}})
    }
    fn guide(steps: Vec<Value>) -> Value {
        json!({"tracks":{"operations":steps}})
    }
    fn set(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|k| (*k).to_owned()).collect()
    }
    #[test]
    fn progress_follows_the_same_instruction_and_split_blocks() {
        let old = guide(vec![
            step("word-0001", "Build forts"),
            step("word-0002", "Line A\nLine B"),
            step("word-0003--operation-2", "Mobilize The…"),
            step("word-0004", "Old text"),
            step("word-0005--operation-1", "Moved line"),
        ]);
        let new = guide(vec![
            step("word-0001", "Build  forts"),
            step("word-0002", "Line A\nLine B"),
            step("word-0002--line-1", "Line A"),
            step("word-0002--line-2", "Line B"),
            step("word-0003--operation-2", "Something else"),
            step("word-0004", "New text"),
            step("word-0005--line-3", "Moved line"),
        ]);
        let allowed = set(&["word-0001", "word-0002--line-1", "word-0002--line-2", "word-0003--operation-2", "word-0004", "word-0005--line-3"]);
        let checks = json!({"word-0001":true,"word-0002":true,"word-0003--operation-2":true,"word-0004":true,"word-0005--operation-1":true});
        let (c, s, _) = translate(Some(&old), &new, &checks, &json!({}), &allowed, &allowed);
        assert_eq!(c, json!({"word-0001":true,"word-0002--line-1":true,"word-0002--line-2":true,
            "word-0004":true,"word-0005--line-3":true}));
        assert_eq!(s, json!({}));
        // Without the original definition only identical keys carry over.
        let (c, _, _) = translate(None, &new, &json!({"word-0001":true,"gone":true}), &json!({}), &allowed, &allowed);
        assert_eq!(c, json!({"word-0001":true}));
    }
}
