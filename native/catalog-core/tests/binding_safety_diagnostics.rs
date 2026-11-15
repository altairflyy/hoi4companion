//! Reproduce the existing resolver's unsafe confidence evidence. These tests
//! intentionally do not implement or claim a production admission fix.
use hoi4companion_catalog_core::Catalog;
use serde_json::{Value,json};

fn catalog(entries: &Value) -> Catalog {
    Catalog::compile(serde_json::from_value(json!({"schema_version":1,"entries":entries})).unwrap()).unwrap()
}
fn matches(catalog: &Catalog,query: &str,country: &str,kind: &str,explicit: bool) -> Vec<String> {
    let result=catalog.query(serde_json::from_value(json!({"schema_version":1,"op":"matches","query":query,"country":country,"kinds":[kind],"explicit":explicit})).unwrap()).unwrap();
    result.as_array().unwrap().iter().map(|i| catalog.entries()[i.as_u64().unwrap() as usize]["id"].as_str().unwrap().to_owned()).collect()
}
#[test]
fn current_catalog_uniqueness_does_not_disambiguate_decisions_from_event_answers() {
    let fixture:Value=serde_json::from_str(include_str!("../../../tests/fixtures/guide-binding-probe-cases-v1.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap(){
        let c=catalog(&case["entries"]);let query=case["query"].as_str().unwrap();let country=case["country"].as_str().unwrap();
        let ids=matches(&c,query,country,"decision",false);
        assert_eq!(serde_json::to_value(ids).unwrap(),case["observed_legacy_decision_ids"],"{}",query);
        let options=matches(&c,query,country,"event_option",false);
        if case["expected_shadow_status"]=="requires_review"{assert!(!options.is_empty(),"Rival must be visible to diagnostic for {query}");}
        else {assert!(options.is_empty(),"Controls must not accidentally acquire a rival: {query}");}
        // Explicit native IDs remain distinct from weak source-name inference.
        let id=case["saved_id"].as_str().unwrap();assert_eq!(matches(&c,id,country,"decision",true),vec![id.to_owned()]);
    }
}

#[test]
fn current_country_is_not_original_country_and_foreign_prefixes_do_not_decide() {
    let c=catalog(&json!([
        {"kind":"decision","id":"FOREIGN_controlled_mines","name":"Repair mines","country_condition":{"op":"unknown"}},
        {"kind":"decision","id":"OTHER_original","name":"Original identity","country_condition":{"op":"original_tag","value":"SOV"}},
        {"kind":"decision","id":"restricted_current","name":"Current restriction","country_condition":{"op":"tag","value":"SOV"}}
    ]));
    assert_eq!(matches(&c,"Repair mines","NOR","decision",false),vec!["FOREIGN_controlled_mines"]);
    assert_eq!(matches(&c,"Original identity","ENG","decision",false),vec!["OTHER_original"]);
    assert!(matches(&c,"Current restriction","ENG","decision",false).is_empty());
}

#[test]
fn event_parent_false_excludes_option_but_unknown_parent_is_not_invented_false() {
    let c=catalog(&json!([
        {"kind":"event","id":"false_parent","name":"False parent","country_condition":{"op":"tag","value":"SOV"}},
        {"kind":"event_option","id":"false_parent.a","name":"Shared answer","event_id":"false_parent"},
        {"kind":"event","id":"unknown_parent","name":"Unknown parent","country_condition":{"op":"original_tag","value":"SOV"}},
        {"kind":"event_option","id":"unknown_parent.a","name":"Shared answer","event_id":"unknown_parent"}
    ]));
    assert_eq!(matches(&c,"Shared answer","ENG","event_option",false),vec!["unknown_parent.a"]);
    let wrong_parent=c.query(serde_json::from_value(json!({"schema_version":1,"op":"matches","query":"unknown_parent.a","country":"ENG","kinds":["event_option"],"filters":{"event_id":"false_parent"}})).unwrap()).unwrap();
    assert_eq!(wrong_parent,json!([]));
}
