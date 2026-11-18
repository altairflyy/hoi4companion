//! Which focuses lead to an event, from the installed game's catalog rows
//! (`triggered_by`, `enabled_by`, `receivers` from `event_reach`). Shared by
//! the suite's events page (`nation_events`) and the grounding of author event
//! answers (`reference_candidates`), so both classify an event the same way.
//! Only what the scripts trace; read-only, no live state.
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
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

/// How events are reached. Links: a focus fires an event (directly or through
/// a scripted effect) or makes a decision available; a decision or event fires
/// the next event (other nations' answers included); an event's own trigger
/// requires a focus. Only what the scripts trace.
pub struct Chains {
    /// Every focus that leads to a node ("event:ID", "decision:ID").
    pub roots: HashMap<String, BTreeSet<String>>,
    /// Nodes the played nation meets without any focus: an event nothing traced
    /// calls (on_actions, timed, history), a decision always available, and
    /// what they fire. Another nation's decisions and the uncalled events only
    /// other nations receive do not start chains here.
    pub free: HashSet<String>,
}
/// The nation tag an id starts with ("GER_befriend_poland" is GER's).
fn tag_of(id: &str) -> Option<&str> {
    id.split_once('_').map(|(tag, _)| tag).filter(|t| t.len() == 3 && t.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
}
pub fn chains(rows: &[Value], country: &str) -> Chains {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut nodes = vec![];
    let mut foreign = HashSet::new();
    let mut has_parent = HashSet::new();
    for row in rows.iter().filter(|r| r["kind"] == "event" || r["kind"] == "decision") {
        let node = format!("{}:{}", s(&row["kind"]), s(&row["id"]));
        let required = row["enabled_by"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned);
        let sources = row["triggered_by"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned);
        for source in sources.chain(required) {
            children.entry(source).or_default().push(node.clone());
            has_parent.insert(node.clone());
        }
        let id = s(&row["id"]);
        let others_only = row["kind"] == "event"
            && row["receivers"].as_array().is_some_and(|r| !r.is_empty() && !r.iter().any(|t| t == country));
        if others_only || row["kind"] == "decision" && tag_of(id).is_some_and(|t| t != country) {
            foreign.insert(node.clone());
        }
        nodes.push(node);
    }
    let mut roots: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut work: Vec<(String, String)> = children.iter().filter_map(|(k, v)| k.strip_prefix("focus:").map(|f| (f.to_owned(), v)))
        .flat_map(|(f, v)| v.iter().map(move |c| (c.clone(), f.clone()))).collect();
    while let Some((node, root)) = work.pop() {
        if !roots.entry(node.clone()).or_default().insert(root.clone()) {
            continue;
        }
        work.extend(children.get(&node).into_iter().flatten().map(|c| (c.clone(), root.clone())));
    }
    // Free starts: catalog nodes without a traced parent, and callers the catalog does not know.
    let known: HashSet<&String> = nodes.iter().collect();
    let mut stack: Vec<String> = nodes.iter().filter(|n| !has_parent.contains(*n) && !foreign.contains(*n)).cloned()
        .chain(children.keys().filter(|k| !k.starts_with("focus:") && !known.contains(k)).cloned()).collect();
    let mut free = HashSet::new();
    while let Some(node) = stack.pop() {
        if free.insert(node.clone()) {
            stack.extend(children.get(&node).into_iter().flatten().cloned());
        }
    }
    Chains { roots, free }
}
impl Chains {
    /// Focuses leading to an event that another nation takes on its AI
    /// historical path (`historical`: catalog source "historical_focuses").
    pub fn historical_roots<'a>(&'a self, event_id: &str, country: &str, historical: &'a HashSet<&str>) -> impl Iterator<Item = &'a String> {
        let own = format!("{country}_");
        self.roots.get(&format!("event:{event_id}")).into_iter().flatten()
            .filter(move |f| !f.starts_with(&own) && historical.contains(f.as_str()))
    }
    /// Whether a focus the guide picks leads to the event.
    pub fn from_guide(&self, event_id: &str, guide: &[String]) -> bool {
        self.roots.get(&format!("event:{event_id}")).is_some_and(|roots| guide.iter().any(|f| roots.contains(f)))
    }
}

/// Focus ids of the catalog source "historical_focuses".
pub fn historical_set(historical: &Value) -> HashSet<&str> {
    historical.as_array().into_iter().flatten().filter_map(Value::as_str).collect()
}
