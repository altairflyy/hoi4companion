//! Observed current parser contracts; these are not assertions of ideal behavior.
use hoi4companion_backend::request;
use serde_json::{Value, json};
fn fixture() -> Value { serde_json::from_str(include_str!("../../../../tests/fixtures/guide-benchmark-contract-cases.json")).unwrap() }
fn guide(text: &str) -> Value {
    json!({"id":"bench","dependency_policy":"explicit","applies_to":{"country":"GER"},"tracks":{"operations":[{"key":"word-0001","tracking":"reminder","action":{"kind":"manual","params":{"instructions":[text]}}}]}})
}
fn promote(guide: &Value) -> Value {
    request(0,"import_promote_operations",json!({"guide":guide,"entries":[],"library":{}})).unwrap()
}
#[test]
fn split_is_not_a_claim_of_live_monitoring_and_unknown_residual_is_kept() {
    let result = promote(&guide("Create agency\nUnrecognized instruction"));
    let rows = result["guide"]["tracks"]["operations"].as_array().unwrap();
    assert_eq!(rows.len(),2);
    assert_eq!(rows[0]["key"],"word-0001");
    assert_eq!(rows[1]["key"],"word-0001--operation-2");
    assert_eq!(rows[1]["tracking"],"reminder");
    let bound = request(0,"import_bind_operations",json!({"guide":result["guide"],"entries":[],"library":{}})).unwrap();
    let report = bound["reports"].as_array().expect("binding reports");
    assert!(report.iter().any(|r| r["key"]=="word-0001--operation-2" && r["monitoring"]=="reminder"));
}
#[test]
fn timing_compilation_requires_all_clauses_and_uniform_operator() {
    let timing = |text| request(0,"import_timing",json!({"text":text})).unwrap();
    assert_eq!(timing("at 150 PP and at 20 AE")["op"],"all");
    assert!(timing("at 150 PP and 20 AE").is_null());
    assert!(timing("at 150 PP and at 20 AE or at 30 NE").is_null());
}
#[test]
fn heading_scope_currently_extends_to_following_lines_and_keys_follow_order() {
    let result = promote(&guide("When at war:\nCreate agency\nCreate agency"));
    let rows=result["guide"]["tracks"]["operations"].as_array().unwrap();
    assert_eq!(rows.len(),2);
    assert_eq!(rows[0]["key"],"word-0001");
    assert_eq!(rows[1]["key"],"word-0001--operation-2");
    assert_eq!(rows[0]["action"]["params"]["timing"],"When at war");
    assert_eq!(rows[1]["action"]["params"]["timing"],"When at war");
}
#[test]
fn minimized_country_and_unknown_name_probes() {
    let f=fixture();
    for case in f["foreign_cases"].as_array().unwrap() {
        let result=request(0,"import_foreign_commands",json!({"text":case["text"],"entries":[],"country_names":f["country_names"]})).unwrap();
        if case["expected_target"].is_null() { assert!(result.is_null()); }
        else { assert_eq!(result[0]["params"]["target_country"],case["expected_target"]); }
    }
    for case in f["named_cases"].as_array().unwrap() {
        let result=promote(&guide(case["text"].as_str().unwrap()));
        assert_eq!(result["changes"].as_array().unwrap().len(),case["expected_promoted"].as_u64().unwrap() as usize);
    }
}

