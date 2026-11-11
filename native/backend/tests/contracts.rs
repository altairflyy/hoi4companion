//! Frozen wire examples are data contracts shared with the web tests, not a
//! second implementation. These tests call the production Rust boundary.
use hoi4companion_backend::request;
use serde_json::{Value,json};
fn cases(name: &str) -> Vec<Value> {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    serde_json::from_slice(&std::fs::read(root.join(format!("{name}.json"))).unwrap()).unwrap()
}
fn normalize(guide: &Value) -> Value {request(0,"guide_normalize",json!({"guide":guide})).unwrap()}
fn evaluate(row: &Value) -> Value {
    request(0,"guide_evaluate",json!({"guide":normalize(&row["guide"]),"state":row["state"],"progress":row["progress"]})).unwrap_or_else(|error|panic!("{}: {error}",row["name"]))
}
#[test]
fn schema_and_focus_sequence_validation() {
    for fixture in ["guide-schema","focus-sequence-contract"] {
        for row in cases(fixture) {
            if let Some(raw)=row["raw_json"].as_str() {
                assert!(serde_json::from_str::<Value>(raw).is_err(),"{}",row["name"]);
                assert_eq!(row["valid"],false);continue;
            }
            let result=request(0,"guide_normalize",json!({"guide":row["guide"]}));
            assert_eq!(result.is_ok(),row["valid"]==true,"{}",row["name"]);
            if let Some(expected)=row.get("normalized") {assert_eq!(result.unwrap(),*expected,"{}",row["name"]);}
        }
    }
}
#[test]
fn observed_calculated_and_unknown_state_contract() {
    for row in ["framework-state", "live-extended-state"].into_iter().flat_map(cases) {
        let state=request(0,"state_normalize",json!({"value":row["input"]})).unwrap();
        for (key,expected) in row["expected"].as_object().unwrap() {
            let missing=json!({"status":"unknown","value":null,"source":""});
            let actual=state["facts"].get(key).unwrap_or(&missing);
            for (field,value) in expected.as_object().unwrap() {assert_eq!(actual[field],*value,"{} {key}.{field}",row["name"]);}
        }
    }
}
#[test]
fn source_operational_requirements_match_browser_and_preserve_explicit_conditions() {
    for row in cases("operational-readiness") {
        let result=evaluate(&row);
        assert_eq!(result["steps"][0]["action_readiness"]["value"],row["readiness"],"{}",row["name"]);
        assert_eq!(result["steps"][0]["action_readiness"]["requirements_declared"],true,"Source predicates must gate both HUD admission stages");
    }
}
#[test]
fn effective_political_costs_preserve_thresholds_and_unknown_native_targets() {
    for row in cases("political-costs") {
        let value=evaluate(&row);let readiness=&value["steps"][0]["action_readiness"];
        assert_eq!(readiness["value"],row["expected"]["readiness"],"{}",row["name"]);
        for (actual,expected) in [("cost","cost"),("effective_cost","effective")] {
            let value=&readiness["political_power"][actual];let expected=&row["expected"][expected];
            if expected.is_null() {assert!(value.is_null(),"{} {actual}",row["name"]);}
            else {assert!(value.is_number(),"{} {actual}",row["name"]);assert_eq!(value.as_f64(),expected.as_f64(),"{} {actual}",row["name"]);}
        }
    }
}
#[test]
fn canonical_political_costs_require_admitted_calculation_provenance() {
    for (status,source) in [("calculated","catalog_base"),("observed","native_political_purchase_formula")] {
        let value=json!({"schema_version":1,"usable":true,"facts":{"politics.costs":{"status":status,"source":source,"value":{"trade_laws/free_trade":{"currency":"political_power","base":150,"effective":120,"factor":0.8}}}}});
        let state=request(0,"state_normalize",json!({"value":value})).unwrap();
        assert_eq!(state["facts"]["politics.costs"]["status"],"unknown");
    }
}
#[test]
fn targeted_decision_binding_and_observed_history_preserve_campaign_identity() {
    let step=json!({"key":"d","action":{"kind":"manual","params":{"decision_id":"test","target_state":143}}});
    let entries=json!([{"kind":"decision","id":"test","name":"Test","targeted":true}]);
    let bound=request(0,"import_bind_political",json!({"step":step,"entries":entries,"country":"TST"})).unwrap()["step"].clone();
    assert_eq!(bound["completion"]["args"][0]["value"],"test:state:143");
    let guide=json!({"id":"targeted","dependency_policy":"explicit","tracks":{"operations":[bound]}});
    let state=json!({"schema_version":1,"usable":true,"session_id":"session","facts":{"country":{"status":"observed","source":"fixture","value":"TST"},"decisions.targeted.executed":{"status":"calculated","source":"native_targeted_decision_adapter","value":["test:state:143"]}}});
    let record=json!({"schema_version":1,"guide_id":"targeted","run_id":"run","session_id":"session","country":"TST","checks":{}});
    let first=request(0,"guide_observe",json!({"guide":guide,"state":state,"record":record,"runs":{}})).unwrap();
    assert_eq!(first["changed"],true);assert!(first["record"]["campaign_conditions"]["milestones"].as_object().is_some_and(|m|m.len()==1));
    let second=request(0,"guide_observe",json!({"guide":guide,"state":state,"record":first["record"],"runs":{}})).unwrap();assert_eq!(second["changed"],false);
    let mut gone=state.clone();gone["facts"]["decisions.targeted.executed"]["value"]=json!([]);
    assert_eq!(request(0,"guide_evaluate",json!({"guide":guide,"state":gone,"progress":first["record"]})).unwrap()["steps"][0]["status"],"completed");
    gone["session_id"]=json!("different");assert_ne!(request(0,"guide_evaluate",json!({"guide":guide,"state":gone,"progress":first["record"]})).unwrap()["steps"][0]["status"],"completed");
    let explicit=json!({"schema_version":1,"available":{"op":"const","value":true},"branch":{"op":"const","value":true}});
    let mut with_source=step.clone();with_source["action"]["params"]["operational_criteria"]=explicit.clone();
    let mut source_entries=entries.clone();source_entries[0]["operational_criteria"]=explicit.clone();source_entries[0]["operational_criteria"]["available"]["value"]=json!(false);
    let attached=request(0,"import_bind_political",json!({"step":with_source,"entries":source_entries,"country":"TST"})).unwrap()["step"].clone();
    assert_eq!(attached["action"]["params"]["operational_criteria"]["available"]["args"][1]["value"],false);
    let refreshed=request(0,"import_bind_political",json!({"step":attached,"entries":entries,"country":"TST"})).unwrap()["step"].clone();
    assert_eq!(refreshed["action"]["params"]["operational_criteria"],explicit,"source refresh restores explicit criteria rather than retaining obsolete generated rules");
}
#[test]
fn runtime_order_dependencies_conditions_campaigns_and_parallel_lanes() {
    for row in cases("guide-runtime") {
        let value=evaluate(&row);let expected=&row["expected"];
        let steps=value["steps"].as_array().unwrap();
        for (field,projection) in [("statuses","status"),("keys","key")] {
            if expected.get(field).is_some() {assert_eq!(json!(steps.iter().map(|r|r[projection].clone()).collect::<Vec<_>>()),expected[field],"{} {field}",row["name"]);}
        }
        for field in ["mode","progress_status"] {if expected.get(field).is_some(){assert_eq!(value[field],expected[field],"{} {field}",row["name"]);}}
        if let Some(lanes)=expected["parallel"].as_object() {for (lane,steps) in lanes{assert_eq!(value["parallel"][lane]["steps"],*steps,"{} parallel {lane}",row["name"]);}}
        for (field,projection) in [("selection_values","value"),("selection_sources","source")] {
            if expected.get(field).is_some(){assert_eq!(json!(steps.iter().map(|r|r["selection"][projection].clone()).collect::<Vec<_>>()),expected[field],"{} {field}",row["name"]);}
        }
    }
}
#[test]
fn construction_production_and_fortification_progress() {
    for fixture in ["construction-progress","construction-extended","production-progress","fortifications"] {
        for row in cases(fixture) {let value=evaluate(&row);assert_eq!(value["steps"][0]["status"],row["expected"],"{fixture} {}",row["name"]);}
    }
}
#[test]
fn project_start_political_sequence_and_resource_gates() {
    for row in cases("action-sequences") {
        let value=evaluate(&row);
        assert_eq!(json!(value["steps"].as_array().unwrap().iter().map(|r|r["status"].clone()).collect::<Vec<_>>()),row["statuses"],"{}",row["name"]);
    }
}
#[test]
fn native_logistics_provenance_and_signed_balance_match_browser_contracts() {
 for row in cases("logistics") {let state=request(0,"state_normalize",json!({"value":row["input"]})).unwrap();let missing=json!({"status":"unknown","value":null,"source":""});let f=state["facts"].get(row["key"].as_str().unwrap()).unwrap_or(&missing);assert_eq!(f["status"],row["status"],"{}",row["name"]);assert_eq!(f["value"],row["value"],"{}",row["name"]);}
}
