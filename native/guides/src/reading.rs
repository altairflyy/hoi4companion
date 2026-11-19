//! Reading layer of an operation step: presentation only. It never changes a
//! step's status, completion or progress. For every operation it records the
//! original text, the qualifiers written in it (ASAP, ALL, priority, PP cost,
//! notes) and the targets the binders already resolved, with localized names.
//! A composed title (sentence templates translated by the web UI) is given only
//! when the whole line was understood: a construction line the instruction
//! binder parsed in full, or a written-out foreign action whose text, without
//! its qualifiers, is exactly what the action parser read. Otherwise the title
//! is null and the UI keeps the original text with its glossary beside it.
use serde_json::{Value, json};
fn s(v: &Value) -> &str { v.as_str().unwrap_or("") }
fn re(pattern: &str) -> regex::Regex { crate::import_catalog::regex(pattern) }

/// Localized country name: the plain tag key of the UI localisation, never a
/// scripted or formatted text. "Ungheria (HUN)", or the tag alone.
fn country(tag: &str, labels: &Value) -> Value {
    let name = labels[tag].as_str().filter(|n| !n.trim().is_empty() && !n.contains(['[', '$', '§']));
    json!({"kind":"country","id":tag,"name":name.unwrap_or(tag)})
}
fn country_label(target: &Value) -> String {
    let (id, name) = (s(&target["id"]), s(&target["name"]));
    if name == id { id.to_owned() } else { format!("{name} ({id})") }
}
pub fn qualifiers(text: &str) -> Vec<Value> {
    let mut out = vec![];
    if re(r"\bASAP\b").is_match(text) { out.push(json!({"kind":"asap"})); }
    if re(r"\bALL\b").is_match(text) { out.push(json!({"kind":"all"})); }
    if re(r"(?i)\bprio(?:rity)?\b").is_match(text) { out.push(json!({"kind":"priority"})); }
    for hit in re(r"(?i)\(\s*(\d+)\s*PP\s*\)").captures_iter(text) {
        if let Ok(n) = hit[1].parse::<u64>() { out.push(json!({"kind":"political_power","value":n})); }
    }
    let note = re(r"(?i)^(?:(?:max|maximum|highest|top|lowest)\s+)?(?:prio|priority)$|^\d+\s*PP$");
    for hit in re(r"\(([^()]*)\)").captures_iter(text) {
        let inner = hit[1].trim();
        if !inner.is_empty() && !note.is_match(inner) { out.push(json!({"kind":"note","text":inner})); }
    }
    out
}
/// The line without the qualifiers that never change what is asked.
fn bare(text: &str) -> String {
    let text = re(r"(?i)^\s*ASAP\s*:?\s*").replace(text.trim(), "");
    let text = re(r"(?i)\s*[—–-]\s*ASAP\s*$").replace(&text, "");
    let text = re(r"(?i)(?:\s*\(\s*(?:(?:max|maximum|highest|top|lowest)\s+)?(?:prio|priority|\d+\s*PP)\s*\))+\s*$").replace(&text, "");
    let text = crate::reference_candidates::country_aliases(text.trim());
    re(r"\s+").replace_all(text.trim(), " ").to_lowercase()
}
fn action_template(action: &str) -> Option<&'static str> {
    Some(match action {
        "justify_war_goal" => "Justify a war goal on {0}",
        "annex" => "Annex {0}",
        "integrate" => "Integrate {0}",
        "invade" => "Invade {0}",
        "collaboration_government" => "Install a collaboration government in {0}",
        "improve_relations" => "Improve relations with {0}",
        "send_attache" => "Send an attaché to {0}",
        "guarantee_independence" => "Guarantee the independence of {0}",
        "declare_war" => "Declare war on {0}",
        "non_aggression_pact" => "Offer a non-aggression pact to {0}",
        "embargo" => "Embargo {0}",
        "lend_lease" => "Send lend-lease to {0}",
        "diplomatic_pressure" => "Apply diplomatic pressure on {0}",
        "build_intel_network" => "Build an intelligence network in {0}",
        _ => return None,
    })
}
fn construction(params: &Value, title: &str, targets: &mut Vec<Value>) -> Option<Value> {
    let rows = params["construction_targets"].as_array().filter(|a| !a.is_empty())?;
    let mut phases: Vec<(u64, Vec<String>, Vec<String>)> = vec![];
    for row in rows {
        let (building, state) = (s(&row["building_name"]), s(&row["state_name"]));
        if building.is_empty() || state.is_empty() { return None; }
        let building_target = json!({"kind":"building","id":row["building_id"],"name":building});
        let state_target = json!({"kind":"state","id":row["state_id"],"name":state});
        for t in [building_target, state_target] { if !targets.contains(&t) { targets.push(t); } }
        let order = row["order"].as_u64().unwrap_or(1);
        if phases.last().is_none_or(|p| p.0 != order) { phases.push((order, vec![], vec![])); }
        let phase = phases.last_mut().unwrap();
        if !phase.1.iter().any(|b| b == building) { phase.1.push(building.to_owned()); }
        if !phase.2.iter().any(|p| p == state) { phase.2.push(state.to_owned()); }
    }
    // The whole line must be the instruction binder's grammar, not only its targets.
    crate::construction_instructions::parse(title)?;
    // Later phases have their own template so translations never re-case words.
    let parts = phases.into_iter().enumerate().map(|(i, (_, b, p))| json!({"template":if i == 0 {"Build {0} in {1}"} else {"then build {0} in {1}"},"args":[b.join(", "),p.join(", ")]})).collect::<Vec<_>>();
    Some(json!({"parts":parts,"joiner":"then"}))
}
fn action(resolution: &Value, title: &str, labels: &Value, targets: &mut Vec<Value>) -> (Option<String>, Option<Value>) {
    let mut groups: Vec<(String, Vec<Value>)> = vec![];
    let mut add = |action: &str, tag: &str| {
        let target = country(tag, labels);
        if groups.last().is_none_or(|g| g.0 != action) { groups.push((action.to_owned(), vec![])); }
        groups.last_mut().unwrap().1.push(target);
    };
    match s(&resolution["status"]) {
        "not_observable" => for tag in resolution["targets"].as_array().into_iter().flatten() { add(s(&resolution["action"]), s(tag)); },
        "bound" => for item in resolution["actions"].as_array().into_iter().flatten() { add(s(&item["action"]), s(&item["target"])); },
        _ => return (None, None),
    }
    for (_, items) in &groups { for t in items { if !targets.contains(t) { targets.push(t.clone()); } } }
    let verb = (groups.len() == 1).then(|| groups[0].0.clone());
    let whole = !groups.is_empty() && !s(&resolution["text"]).is_empty() && bare(title) == bare(s(&resolution["text"]));
    let parts = groups.iter().map(|(a, items)| {
        action_template(a).map(|t| json!({"template":t,"args":[items.iter().map(country_label).collect::<Vec<_>>().join(", ")]}))
    }).collect::<Option<Vec<_>>>();
    (verb, parts.filter(|_| whole).map(|parts| json!({"parts":parts,"joiner":"and"})))
}
/// Reading of one operation step; null for focus and research steps, whose
/// names already come from the game catalog.
pub fn read(step: &Value, labels: &Value) -> Value {
    if ["select_focus", "research", "free"].contains(&s(&step["action"]["kind"])) { return Value::Null; }
    let title = s(&step["title"]);
    if title.trim().is_empty() { return Value::Null; }
    let params = &step["action"]["params"];
    let mut targets = vec![];
    let (verb, composed) = if params["category"] == "construction" {
        (Some("build".to_owned()), construction(params, title, &mut targets))
    } else if params["action_resolution"].is_object() {
        action(&params["action_resolution"], title, labels, &mut targets)
    } else {
        (None, None)
    };
    json!({"schema_version":1,"original":title,"understood":composed.is_some(),"verb":verb,
        "targets":targets,"qualifiers":qualifiers(title),"title":composed})
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Shared contract with the web UI (`tests/test_reading_contract.cjs`).
    #[test]
    fn contract_fixture_readings_match() {
        let fixture: Value = serde_json::from_str(include_str!("../../../tests/fixtures/reading-contract.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            assert_eq!(read(&case["step"], &fixture["labels"]), case["reading"], "{}", s(&case["name"]));
        }
    }
}
