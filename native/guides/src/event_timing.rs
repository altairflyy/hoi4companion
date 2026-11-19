//! Earliest possible day of an event, as a true lower bound, and the day a
//! guide plans to reach one of its own focuses. Used to drop event answers a
//! guide line cannot mean: an event that cannot fire before the line's planned
//! day (`reference_candidates`). Read-only: game scripts and catalog rows only.
//!
//! The lower bound walks the event's traced callers (`triggered_by`, from
//! `event_reach`): a focus completes no earlier than its cost plus its cheapest
//! prerequisite chain, 7 days per cost unit (`FOCUS_POINT_DAYS` in the game's
//! defines), and a caller event or decision fires no earlier than what leads
//! to it. Anything not understood for certain gives no bound (`None`): an event
//! that also fires on its own (`mean_time_to_happen`, not `is_triggered_only`),
//! or that some script the trace does not read could fire (on_actions,
//! history, scripted effects, other block shapes: see `scan`), a decision
//! without a focus it certainly requires, a flag or unknown caller, a cycle.
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Days per focus cost unit (`FOCUS_POINT_DAYS = 7` in `common/defines`).
pub const FOCUS_POINT_DAYS: f64 = 7.0;
/// The fastest a nation could complete focuses relative to their written cost:
/// every computed day is divided by this factor so the bound stays a lower
/// bound. The game has no focus-speed modifier in its modifier list
/// (`documentation/modifiers_documentation.md`); a focus completes faster only
/// by script (`complete_national_focus`, `unlock_national_focus`,
/// `reduce_focus_completion_cost`, `bypass`, joint focuses), and those focuses
/// count as cost 0 here. The factor 1.5 (50 % faster than written) is a margin
/// for what remains unread: mods, scripted cost constants and engine rules.
pub const FOCUS_SPEED_FACTOR: f64 = 1.5;

/// Game script facts the catalog trace does not hold: events some script
/// outside the traced shapes mentions, and focuses a script can complete.
#[derive(Default, Debug)]
pub struct Scan {
    /// Events mentioned where the trace does not follow (they keep no bound).
    pub untraced: HashSet<String>,
    /// Focuses a script completes, unlocks or makes cheaper (cost 0 here).
    pub script_completed: HashSet<String>,
    /// Decisions a script activates (`activate_decision`, `activate_mission`,
    /// `activate_targeted_decision`): activation skips their `allowed`,
    /// `visible` and `available`, so their focus requirements bound nothing.
    pub activated: HashSet<String>,
}

/// Blocks whose content never runs as an effect: a mention there is no route.
const CONDITIONS: &[&str] = &["limit", "trigger", "allowed", "available", "visible", "ai_will_do", "ai_chance",
    "effect_tooltip", "custom_effect_tooltip", "custom_trigger_tooltip", "modifier", "mean_time_to_happen",
    "bypass", "cancel_if_invalid", "complete_tooltip", "target_trigger", "target_root_trigger"];
const COMPLETES: &[&str] = &["complete_national_focus", "unlock_national_focus", "reduce_focus_completion_cost"];
const ACTIVATES: &[&str] = &["activate_decision", "activate_mission", "activate_targeted_decision"];

fn tokens(data: &[u8]) -> Vec<&[u8]> {
    let mut out = vec![];
    let mut i = 0;
    while i < data.len() {
        let c = data[i];
        if c == b'#' {
            while i < data.len() && data[i] != b'\n' {
                i += 1;
            }
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'"' {
            let start = i;
            i += 1;
            while i < data.len() && data[i] != b'"' && data[i] != b'\n' {
                i += 1;
            }
            i = (i + 1).min(data.len());
            out.push(&data[start..i]);
        } else if matches!(c, b'{' | b'}' | b'=') {
            out.push(&data[i..i + 1]);
            i += 1;
        } else {
            let start = i;
            while i < data.len() && !data[i].is_ascii_whitespace() && !matches!(data[i], b'{' | b'}' | b'=' | b'#' | b'"') {
                i += 1;
            }
            out.push(&data[start..i]);
        }
    }
    out
}

/// Read one script file (`relative`: its path under the game root, lowercase,
/// "/" separators) into `scan`. An event mention counts as traced only in the
/// shapes `event_reach` follows: `country_event`/`news_event = ID` or `= { id
/// = ID }` inside a focus of a `focus_tree` or a `shared_focus`, inside a
/// decision (not a category file), or anywhere in an event file (its own
/// definition included). Mentions inside condition blocks are no route.
pub fn scan_text(relative: &str, data: &[u8], events: &HashSet<String>, scan: &mut Scan) {
    let dir = if relative.starts_with("events/") {
        "events"
    } else if relative.starts_with("common/national_focus/") {
        "focus"
    } else if relative.starts_with("common/decisions/") && !relative.starts_with("common/decisions/categories/") {
        "decision"
    } else {
        "other"
    };
    let tokens = tokens(data);
    let mut stack: Vec<String> = vec![];
    for (i, token) in tokens.iter().enumerate() {
        let text = String::from_utf8_lossy(token);
        match text.as_ref() {
            "{" => {
                let key = if i >= 2 && tokens[i - 1] == b"=" { String::from_utf8_lossy(tokens[i - 2]).into_owned() } else { String::new() };
                stack.push(key);
                continue;
            }
            "}" => {
                stack.pop();
                continue;
            }
            "=" => continue,
            _ => {}
        }
        // A key ("key = …") is no mention.
        if tokens.get(i + 1).is_some_and(|t| *t == b"=") && !(i >= 1 && tokens[i - 1] == b"=") {
            continue;
        }
        let key = if i >= 2 && tokens[i - 1] == b"=" { String::from_utf8_lossy(tokens[i - 2]).into_owned() } else { String::new() };
        // History blocks of every date count: a guide may start at a later bookmark.
        if ACTIVATES.contains(&key.as_str()) || stack.iter().any(|k| ACTIVATES.contains(&k.as_str())) {
            scan.activated.insert(text.trim_matches('"').to_owned());
        }
        if COMPLETES.contains(&key.as_str()) || stack.iter().any(|k| COMPLETES.contains(&k.as_str())) {
            scan.script_completed.insert(text.trim_matches('"').to_owned());
        }
        let name = text.trim_matches('"');
        if !events.contains(name) || stack.iter().any(|k| CONDITIONS.contains(&k.as_str())) {
            continue;
        }
        let top = stack.last().map(String::as_str).unwrap_or("");
        let call = matches!(key.as_str(), "country_event" | "news_event") || key == "id" && matches!(top, "country_event" | "news_event");
        let traced = call
            && match dir {
                "events" => true,
                "focus" => {
                    stack.first().is_some_and(|k| k == "shared_focus")
                        || stack.first().is_some_and(|k| k == "focus_tree") && stack.get(1).is_some_and(|k| k == "focus")
                }
                "decision" => stack.len() >= 2,
                _ => false,
            };
        if !traced {
            scan.untraced.insert(name.to_owned());
        }
    }
}

/// Scan the game's and mods' script folders (`common`, `events`, `history`).
pub fn scan(game: &Path, mods: &[PathBuf], events: &HashSet<String>) -> Result<Scan, String> {
    let mut scan = Scan::default();
    for folder in ["common", "events", "history"] {
        let files = crate::installation::collect(game, mods, folder, ".txt")?;
        for (relative, path) in files["files"].as_object().into_iter().flatten() {
            let Some(path) = path.as_str() else { continue };
            if let Ok(data) = std::fs::read(path) {
                scan_text(relative, &data, events, &mut scan);
            }
        }
    }
    Ok(scan)
}

/// Lower bounds over the catalog rows (focus `cost`/`prerequisites`, event
/// `triggered_by`/`callers_complete`, decision `required_focuses`).
pub struct Timing<'a> {
    focuses: HashMap<&'a str, Vec<&'a Value>>,
    events: HashMap<&'a str, Vec<&'a Value>>,
    decisions: HashMap<&'a str, Vec<&'a Value>>,
    focus_days: std::cell::RefCell<HashMap<String, f64>>,
}
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn truth(v: &Value) -> bool {
    v == true
}
impl<'a> Timing<'a> {
    pub fn new(rows: &'a [Value]) -> Self {
        let mut focuses: HashMap<&str, Vec<&Value>> = HashMap::new();
        let mut events: HashMap<&str, Vec<&Value>> = HashMap::new();
        let mut decisions: HashMap<&str, Vec<&Value>> = HashMap::new();
        for row in rows {
            let map = match s(&row["kind"]) {
                "focus" => &mut focuses,
                "event" => &mut events,
                "decision" => &mut decisions,
                _ => continue,
            };
            map.entry(s(&row["id"])).or_default().push(row);
        }
        Self { focuses, events, decisions, focus_days: Default::default() }
    }
    /// Days of focus work a focus needs at least (cost of the focus and of its
    /// cheapest prerequisite chain, 7 days per unit), before the speed factor.
    /// A focus a script or bypass can complete, or without a written cost,
    /// counts 0; an unknown focus counts 0.
    pub fn focus_work(&self, id: &str) -> f64 {
        self.focus_work_seen(id, &mut HashSet::new())
    }
    fn focus_work_seen(&self, id: &str, seen: &mut HashSet<String>) -> f64 {
        if let Some(days) = self.focus_days.borrow().get(id) {
            return *days;
        }
        if !seen.insert(id.to_owned()) {
            return 0.0;
        }
        let rows = self.focuses.get(id).map(Vec::as_slice).unwrap_or_default();
        let mut best: Option<f64> = None;
        for row in rows {
            let instant = truth(&row["bypass_possible"]) || truth(&row["external_completion_possible"]) || truth(&row["script_completion_possible"]);
            let own = if instant { 0.0 } else { row["cost"].as_f64().filter(|c| *c > 0.0).unwrap_or(0.0) * FOCUS_POINT_DAYS };
            let mut before: f64 = 0.0;
            for group in row["prerequisites"].as_array().into_iter().flatten() {
                let cheapest = group.as_array().into_iter().flatten().filter_map(Value::as_str)
                    .map(|p| self.focus_work_seen(p, seen)).fold(None, |m: Option<f64>, d| Some(m.map_or(d, |m| m.min(d))));
                before = before.max(cheapest.unwrap_or(0.0));
            }
            let total = before + own;
            best = Some(best.map_or(total, |b| b.min(total)));
        }
        seen.remove(id);
        let days = best.unwrap_or(0.0);
        self.focus_days.borrow_mut().insert(id.to_owned(), days);
        days
    }
    /// The earliest day (from the game start) an event can fire, as a lower
    /// bound (speed factor applied); `None` when any route is not understood.
    pub fn earliest_event(&self, id: &str) -> Option<f64> {
        self.event_work(id, &mut HashSet::new()).map(|d| d / FOCUS_SPEED_FACTOR)
    }
    fn event_work(&self, id: &str, seen: &mut HashSet<String>) -> Option<f64> {
        if !seen.insert(format!("event:{id}")) {
            return None;
        }
        let rows = self.events.get(id)?;
        let mut best: Option<f64> = None;
        for row in rows {
            let sources = row["triggered_by"].as_array()?;
            // Callers the trace may not hold (or the 64-entry cut) give no bound.
            if !truth(&row["callers_complete"]) || sources.is_empty() || sources.len() >= 64 {
                return None;
            }
            for source in sources {
                let day = self.source_work(s(source), seen)?;
                best = Some(best.map_or(day, |b: f64| b.min(day)));
            }
        }
        seen.remove(&format!("event:{id}"));
        best
    }
    fn source_work(&self, source: &str, seen: &mut HashSet<String>) -> Option<f64> {
        let (kind, id) = source.split_once(':')?;
        match kind {
            "focus" => Some(self.focus_work(id)),
            "event" => self.event_work(id, seen),
            "decision" => {
                // A decision is taken no earlier than the focuses it certainly
                // requires. A mission (its `timeout_effect` runs when `available`
                // fails) or a decision a script activates gives no bound.
                let rows = self.decisions.get(id)?;
                let mut best: Option<f64> = None;
                for row in rows {
                    if truth(&row["mission"]) || truth(&row["activated"]) {
                        return None;
                    }
                    let required: Vec<f64> = row["required_focuses"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|f| self.focus_work(f)).collect();
                    let day = required.into_iter().reduce(f64::max)?;
                    best = Some(best.map_or(day, |b: f64| b.min(day)));
                }
                best
            }
            _ => None,
        }
    }
    /// Written focus cost in days for a guide's plan (10 when unknown, the game's default).
    pub fn planned_cost(&self, id: &str) -> f64 {
        self.focuses.get(id).and_then(|rows| rows.iter().filter_map(|r| r["cost"].as_f64()).reduce(f64::max)).unwrap_or(10.0) * FOCUS_POINT_DAYS
    }
}

/// Day each focus of the guide's focus order is planned to complete if the
/// player follows the guide from the game start: the sum of the written costs
/// of the guide's focuses up to it. A plan, not a proof.
pub fn planned_days(guide_focuses: &[String], timing: &Timing) -> HashMap<String, f64> {
    let mut day = 0.0;
    let mut out = HashMap::new();
    for focus in guide_focuses {
        day += timing.planned_cost(focus);
        out.entry(focus.clone()).or_insert(day);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scanned(relative: &str, text: &str, events: &[&str]) -> Scan {
        let mut scan = Scan::default();
        let set = events.iter().map(|e| e.to_string()).collect();
        scan_text(relative, text.as_bytes(), &set, &mut scan);
        scan
    }

    #[test]
    fn only_the_traced_call_shapes_keep_an_event_traced() {
        let focus = "focus_tree = { focus = { id = TUR_a completion_reward = { ENG = { country_event = { id = tur.1 days = 3 } } } } }";
        assert!(scanned("common/national_focus/turkey.txt", focus, &["tur.1"]).untraced.is_empty());
        let joint = "joint_focus = { id = X completion_reward = { country_event = tur.1 } }";
        assert!(scanned("common/national_focus/joint.txt", joint, &["tur.1"]).untraced.contains("tur.1"));
        let event = "country_event = { id = tur.1 trigger = { has_country_flag = x } option = { name = a country_event = tur.2 } }";
        assert!(scanned("events/turkey.txt", event, &["tur.1", "tur.2"]).untraced.is_empty());
        // on_actions, scripted effects and an unknown shape keep no bound.
        let on_action = "on_actions = { on_startup = { effect = { country_event = tur.1 } } }";
        assert!(scanned("common/on_actions/x.txt", on_action, &["tur.1"]).untraced.contains("tur.1"));
        let random = "country_event = { id = tur.3 option = { random_events = { 100 = tur.1 } } }";
        assert!(scanned("events/turkey.txt", random, &["tur.1", "tur.3"]).untraced.contains("tur.1"));
        // A mention inside a condition is no route.
        let condition = "on_actions = { on_startup = { effect = { if = { limit = { has_event = tur.1 } } } } }";
        assert!(scanned("common/on_actions/x.txt", condition, &["tur.1"]).untraced.is_empty());
    }

    #[test]
    fn script_completed_focuses_and_activated_decisions_are_found() {
        let text = "complete_national_focus = TUR_a\ncomplete_national_focus = { focus = TUR_b use_side_message = yes }\nreduce_focus_completion_cost = { focus = TUR_c cost = 5 }";
        let scan = scanned("events/x.txt", text, &[]);
        assert!(["TUR_a", "TUR_b", "TUR_c"].iter().all(|f| scan.script_completed.contains(*f)));
        // A later bookmark counts too: the guide may start there.
        let history = "capital = 1\n1939.1.1 = { unlock_national_focus = TUR_late }";
        assert!(scanned("history/countries/TUR.txt", history, &[]).script_completed.contains("TUR_late"));
        let activate = "activate_mission = AST_m\nactivate_decision = { decision = AST_d }";
        let scan = scanned("common/national_focus/x.txt", activate, &[]);
        assert!(scan.activated.contains("AST_m") && scan.activated.contains("AST_d"));
    }

    fn rows() -> Vec<Value> {
        let focus = |id: &str, cost: f64, pre: Value| json!({"kind":"focus","id":id,"cost":cost,"prerequisites":pre,"bypass_possible":false,"external_completion_possible":false});
        vec![
            focus("TUR_a", 10.0, json!([])),
            focus("AST_a", 10.0, json!([])),
            focus("AST_b", 15.0, json!([["AST_a"]])),
            focus("CHL_a", 10.0, json!([])),
            focus("CHL_b", 10.0, json!([["CHL_a", "AST_b"]])),
            json!({"kind":"event","id":"tur.1","triggered_by":["focus:TUR_a"],"callers_complete":true}),
            json!({"kind":"event","id":"ast.1","triggered_by":["focus:AST_b"],"callers_complete":true}),
            json!({"kind":"event","id":"ast.2","triggered_by":["decision:AST_d"],"callers_complete":true}),
            json!({"kind":"decision","id":"AST_d","required_focuses":["AST_b"]}),
            json!({"kind":"event","id":"chl.1","triggered_by":["event:chl.0"],"callers_complete":true}),
            json!({"kind":"event","id":"chl.0","triggered_by":["focus:CHL_b"],"callers_complete":true}),
            json!({"kind":"event","id":"free.1","triggered_by":["focus:AST_b"],"callers_complete":false}),
            json!({"kind":"event","id":"flag.1","triggered_by":["flag:x"],"callers_complete":true}),
            json!({"kind":"event","id":"open.1","triggered_by":["decision:OPEN"],"callers_complete":true}),
            json!({"kind":"decision","id":"OPEN","required_focuses":[]}),
            json!({"kind":"event","id":"mission.1","triggered_by":["decision:AST_m"],"callers_complete":true}),
            json!({"kind":"decision","id":"AST_m","required_focuses":["AST_b"],"mission":true}),
            json!({"kind":"event","id":"active.1","triggered_by":["decision:AST_x"],"callers_complete":true}),
            json!({"kind":"decision","id":"AST_x","required_focuses":["AST_b"],"activated":true}),
        ]
    }

    #[test]
    fn earliest_day_follows_focus_chains_decisions_and_events_with_the_speed_factor() {
        let rows = rows();
        let timing = Timing::new(&rows);
        assert_eq!(timing.focus_work("AST_b"), 175.0);
        // The cheapest prerequisite of a group counts: CHL_a (70) beats AST_b (175).
        assert_eq!(timing.focus_work("CHL_b"), 140.0);
        assert_eq!(timing.earliest_event("tur.1"), Some(70.0 / FOCUS_SPEED_FACTOR));
        assert_eq!(timing.earliest_event("ast.1"), Some(175.0 / FOCUS_SPEED_FACTOR));
        assert_eq!(timing.earliest_event("ast.2"), Some(175.0 / FOCUS_SPEED_FACTOR));
        assert_eq!(timing.earliest_event("chl.1"), Some(140.0 / FOCUS_SPEED_FACTOR));
        // Unknown routes: untraced callers, a flag, a decision with no certain requirement.
        // A mission or an activated decision: its requirements bound nothing.
        for id in ["free.1", "flag.1", "open.1", "missing.1", "mission.1", "active.1"] {
            assert_eq!(timing.earliest_event(id), None, "{id}");
        }
    }

    #[test]
    fn a_focus_a_script_or_bypass_can_complete_costs_nothing() {
        let mut rows = rows();
        rows[1]["script_completion_possible"] = json!(true);
        let timing = Timing::new(&rows);
        assert_eq!(timing.focus_work("AST_b"), 105.0);
        rows[2]["bypass_possible"] = json!(true);
        let timing = Timing::new(&rows);
        assert_eq!(timing.focus_work("AST_b"), 0.0);
    }

    #[test]
    fn planned_days_sum_the_guide_focus_costs_in_order() {
        let rows = rows();
        let timing = Timing::new(&rows);
        let plan = planned_days(&["AST_a".into(), "AST_b".into(), "unknown_focus".into()], &timing);
        assert_eq!(plan["AST_a"], 70.0);
        assert_eq!(plan["AST_b"], 175.0);
        assert_eq!(plan["unknown_focus"], 245.0);
    }
}
