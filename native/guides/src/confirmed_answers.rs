//! Answers the player confirmed on the HUD for an ambiguous event line
//! (`progress_store` keeps them). Applied to a clone of a run's definition,
//! never to an archive or a prepared-guide cache.

use crate::import_catalog::Index;
use serde_json::{Value, json};

/// Apply stored confirmations to `guide`. A row is matched by `key ==
/// step_key` and `reference_candidates.text == quote`; the option must still
/// be one of its `event_options`. The confirmation is always recorded
/// (`reference_candidates.confirmed`); the line becomes a bound step only when
/// it names exactly one reference and the event binding succeeds. Completion
/// stays manual: no `completion` fact is attached.
pub fn apply(guide: &mut Value, answers: &[Value], index: &Index, country: &Value) -> Result<(), String> {
    let Some(tracks) = guide["tracks"].as_object_mut() else { return Ok(()) };
    for row in tracks.values_mut().filter_map(Value::as_array_mut).flatten() {
        for answer in answers {
            apply_one(row, answer, index, country)?;
        }
    }
    Ok(())
}

fn apply_one(row: &mut Value, answer: &Value, index: &Index, country: &Value) -> Result<(), String> {
    let (Some(key), Some(quote), Some(event_id), Some(option_id)) =
        (answer["step_key"].as_str(), answer["quote"].as_str(), answer["event_id"].as_str(), answer["option_id"].as_str())
    else {
        return Ok(());
    };
    let candidates = &row["action"]["params"]["reference_candidates"];
    if row["key"] != key || candidates["text"] != quote {
        return Ok(());
    }
    let listed = candidates["event_options"]
        .as_array()
        .is_some_and(|options| options.iter().any(|o| o["id"] == option_id && o["event_id"] == event_id));
    if !listed {
        return Ok(());
    }
    row["action"]["params"]["reference_candidates"]["confirmed"] = json!({"event_id":event_id,"option_id":option_id,"basis":"user_confirmed"});
    // Only a line holding exactly one reference can be checked as one step.
    let single = crate::reference_candidates::first_line(row)
        .is_some_and(|(line, _)| crate::reference_candidates::references(&line).len() == 1);
    if !single {
        return Ok(());
    }
    let option = json!({"id":option_id,"event_id":event_id});
    let Some(result) = crate::reference_candidates::bind(&option, index, country)? else { return Ok(()) };
    let step = &result["step"];
    let mut params = step["action"]["params"].clone();
    for keep in ["instructions", "source_action", "trigger_resolution", "reference_candidates"] {
        if !row["action"]["params"][keep].is_null() {
            params[keep] = row["action"]["params"][keep].clone();
        }
    }
    params["reference_resolution"] = json!({"text":quote,"basis":"user_confirmed","kind":"event_option","id":option_id,"event_id":event_id,"option_id":option_id});
    row["action"]["params"] = params;
    if let Some(map) = row.as_object_mut() {
        map.remove("tracking");
    }
    if !step["started_when"].is_null() {
        row["started_when"] = step["started_when"].clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn index() -> Index {
        Index::new(&json!({"entries":[
            {"kind":"country","id":"GER","name":"Germany"},
            {"kind":"event","id":"E","name":"E","receivers":["GER"],"receivers_unknown":false},
            {"kind":"event_option","id":"E.b","event_id":"E","name":"Beta"},
            {"kind":"event_option","id":"E.a","event_id":"E","name":"Alpha"},
        ]})).unwrap()
    }
    fn row(line: &str, text: &str) -> Value {
        json!({"key":"s1","kind":"gate","title":line,"tracking":"reminder","action":{"kind":"manual","params":{"instructions":[line],
            "reference_candidates":{"status":"ambiguous","text":text,"event_options":[
                {"id":"E.a","event_id":"E","compatible":true},{"id":"E.b","event_id":"E","compatible":true}]}}}})
    }
    fn guide(row: Value) -> Value {
        json!({"applies_to":{"country":"GER"},"tracks":{"operations":[row]}})
    }
    fn answer(key: &str, quote: &str, option: &str) -> Value {
        json!({"guide_id":"g","step_key":key,"quote":quote,"event_id":"E","option_id":option,"confirmed_at":"2026-11-19T10:00:00Z"})
    }
    fn first(guide: &Value) -> &Value {
        &guide["tracks"]["operations"][0]
    }
    #[test]
    fn single_reference_line_becomes_a_bound_checkable_step() {
        let mut g = guide(row("Mobilize The…", "Mobilize The…"));
        apply(&mut g, &[answer("s1", "Mobilize The…", "E.b")], &index(), &json!("GER")).unwrap();
        let r = first(&g);
        assert!(r.get("tracking").is_none());
        assert_eq!(r["kind"], "gate");
        assert_eq!(r["started_when"], json!({"op":"fact","key":"events.visible_options","test":"contains","value":"E:E.b"}));
        assert!(r.get("completion").is_none());
        assert_eq!(r["action"]["params"]["event_option_id"], "E.b");
        assert_eq!(r["action"]["params"]["reference_resolution"]["basis"], "user_confirmed");
        assert_eq!(r["action"]["params"]["reference_candidates"]["confirmed"]["option_id"], "E.b");
    }
    #[test]
    fn two_reference_line_records_but_stays_a_reminder() {
        let mut g = guide(row("Absolutely Not… + There is a…", "Absolutely Not…"));
        apply(&mut g, &[answer("s1", "Absolutely Not…", "E.a")], &index(), &json!("GER")).unwrap();
        let r = first(&g);
        assert_eq!(r["tracking"], "reminder");
        assert!(r.get("started_when").is_none());
        assert!(r["action"]["params"]["event_option_id"].is_null());
        assert_eq!(r["action"]["params"]["reference_candidates"]["confirmed"], json!({"event_id":"E","option_id":"E.a","basis":"user_confirmed"}));
    }
    #[test]
    fn quote_mismatch_is_not_applied() {
        let mut g = guide(row("Mobilize The…", "Mobilize The…"));
        let before = g.clone();
        apply(&mut g, &[answer("s1", "Other…", "E.b")], &index(), &json!("GER")).unwrap();
        assert_eq!(g, before);
    }
    #[test]
    fn option_no_longer_a_candidate_is_not_applied() {
        let mut g = guide(row("Mobilize The…", "Mobilize The…"));
        let before = g.clone();
        apply(&mut g, &[answer("s1", "Mobilize The…", "E.gone")], &index(), &json!("GER")).unwrap();
        assert_eq!(g, before);
    }
    #[test]
    fn other_guide_rows_are_ignored() {
        let mut g = guide(row("Mobilize The…", "Mobilize The…"));
        let before = g.clone();
        apply(&mut g, &[answer("s2", "Mobilize The…", "E.b")], &index(), &json!("GER")).unwrap();
        assert_eq!(g, before);
    }
}
