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
