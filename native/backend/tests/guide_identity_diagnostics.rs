//! Diagnostic only: observed positional reuse is unsafe; the policy assertions
//! below describe a migration admission requirement, not an implemented guard.
use hoi4companion_backend::request;
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/guide-identity-diagnostics.json")).unwrap()
}
fn guide(text: &str, f: &Value) -> Value {
    json!({"id":"identity-diagnostic","dependency_policy":"explicit",
        "applies_to":{"country":"GER"},
        "sources":[{"id":f["source_id"],"type":"file","text":text}],
        "tracks":{"operations":[{"key":f["parent_key"],"tracking":"reminder",
            "details":{"source_action":{"source_id":f["source_id"],"key":f["parent_key"],"ordinal":1}},
            "evidence":[{"source_id":f["source_id"],"quote":text}],
            "action":{"kind":"manual","params":{"instructions":[text]}}}]}})
}
fn promote(g: &Value) -> Value {
    request(0,"import_promote_operations",json!({"guide":g,"entries":[],"library":{}})).unwrap()["guide"].clone()
}
fn find<'a>(guide: &'a Value, key: &Value) -> &'a Value {
    guide["tracks"]["operations"].as_array().unwrap().iter()
        .find(|step| step["key"] == *key).expect("missing key")
}
fn eval(g: &Value, state: &Value, progress: &Value) -> Value {
    let normalized = request(0,"guide_normalize",json!({"guide":g})).unwrap();
    request(0,"guide_evaluate",json!({"guide":normalized,"state":state,"progress":progress})).unwrap()
}
fn result_step<'a>(result: &'a Value, key: &str) -> &'a Value {
    result["steps"].as_array().unwrap().iter().find(|s| s["key"] == key).unwrap()
}

// Current behavior: same positional child key can denote a different instruction.
// The cloned evidence is parent-level provenance, not proof of child equivalence.
#[test]
fn observed_positional_reuse_keeps_parent_evidence_but_changes_child_meaning() {
    let f=fixture();
    let old=promote(&guide(f["old_text"].as_str().unwrap(),&f));
    let changed=promote(&guide(f["inserted_text"].as_str().unwrap(),&f));
    let old_child=find(&old,&f["old_child_key"]);
    let new_child=find(&changed,&f["old_child_key"]);
    assert_eq!(old_child["title"],f["old_child_text"]);
    assert_eq!(new_child["title"],f["replacement_child_text"]);
    assert_ne!(old_child["title"],new_child["title"]);
    let before_reorder=promote(&guide(f["reorder_before_text"].as_str().unwrap(),&f));
    let after_reorder=promote(&guide(f["reordered_text"].as_str().unwrap(),&f));
    assert_eq!(find(&before_reorder,&f["old_child_key"])["title"],f["old_child_text"]);
    assert_eq!(find(&after_reorder,&f["old_child_key"])["title"],f["replacement_child_text"]);
    for (g, source_text) in [(&old,&f["old_text"]),(&changed,&f["inserted_text"])] {
        let child=find(g,&f["old_child_key"]);
        assert_eq!(child["details"]["source_action"]["source_id"],f["source_id"]);
        assert_eq!(child["details"]["source_action"]["key"],f["parent_key"]);
        assert_eq!(child["evidence"][0]["source_id"],f["source_id"]);
        assert_eq!(child["evidence"][0]["quote"],*source_text);
    }
}

// Current evaluator trusts the supplied key; no admission/migration layer exists.
// Clearing the unproven mapping illustrates the required conservative outcome.
#[test]
fn observed_key_based_evaluation_and_conservative_unmapped_progress() {
    let f=fixture();
    let child=f["old_child_key"].as_str().unwrap();
    let manual = json!({"id":"identity-diagnostic","dependency_policy":"explicit","tracks":{"operations":[
        {"key":child,"action":{"kind":"manual"}}]}});
    let state=json!({"usable":true});
    let mut progress=json!({"schema_version":1,"guide_id":"identity-diagnostic","run_id":"old","checks":{}});
    progress["checks"][child]=json!(true);
    let unsafe_result=eval(&manual,&state,&progress);
    assert_eq!(result_step(&unsafe_result,child)["status"],"completed");
    // Desired migration policy: a positional key alone cannot establish identity.
    let old=promote(&guide(f["old_text"].as_str().unwrap(),&f));
    let changed=promote(&guide(f["inserted_text"].as_str().unwrap(),&f));
    assert_ne!(find(&old,&f["old_child_key"])["title"],find(&changed,&f["old_child_key"])["title"]);
    progress["checks"]=json!({});
    assert_eq!(result_step(&eval(&manual,&state,&progress),child)["status"],"unverifiable");
}

#[test]
fn observed_parent_check_does_not_confirm_child_and_runs_are_separate_inputs() {
    let f=fixture(); let parent=f["parent_key"].as_str().unwrap(); let child=f["old_child_key"].as_str().unwrap();
    let guide=json!({"id":"identity-diagnostic","dependency_policy":"explicit","tracks":{"operations":[
        {"key":parent,"action":{"kind":"manual"}},{"key":child,"action":{"kind":"manual"}}]}});
    let state=json!({"usable":true});
    let mut progress=json!({"schema_version":1,"guide_id":"identity-diagnostic","run_id":"old","checks":{}});
    progress["checks"][parent]=json!(true);
    let result=eval(&guide,&state,&progress);
    assert_eq!(result_step(&result,parent)["status"],"completed");
    assert_eq!(result_step(&result,child)["status"],"unverifiable");
    progress["run_id"]=json!("new"); progress["checks"]=json!({});
    assert_eq!(result_step(&eval(&guide,&state,&progress),parent)["status"],"unverifiable");
}

#[test]
fn observed_campaign_mismatch_rejects_old_checks() {
    let f=fixture(); let parent=f["parent_key"].as_str().unwrap();
    let guide=json!({"id":"identity-diagnostic","tracks":{"operations":[{"key":parent,"action":{"kind":"manual"}}]}});
    let mut state=json!({"usable":true,"sample":{"country":"GER","session_id":"s",
        "campaign_identity":{"status":"observed","source":"native_game_unique_id_and_seed","id":f["campaign_a"]}}});
    let mut progress=json!({"schema_version":1,"guide_id":"identity-diagnostic","run_id":"old","session_id":"s",
        "campaign_id":f["campaign_a"],"country":"GER","checks":{}});
    progress["checks"][parent]=json!(true);
    assert_eq!(eval(&guide,&state,&progress)["progress_status"],"accepted");
    state["sample"]["campaign_identity"]["id"]=f["campaign_b"].clone();
    let result=eval(&guide,&state,&progress);
    assert_eq!(result["progress_status"],"wrong_campaign");
    assert_eq!(result_step(&result,parent)["status"],"unverifiable");
}