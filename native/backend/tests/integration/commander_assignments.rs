use hoi4companion_backend::request;
use serde_json::{Value,json};
fn bind(params:Value)->Value {
    request(0,"import_bind_political",json!({"country":"TST","step":{"key":"assign","action":{"kind":"manual","params":params}},"entries":[{"kind":"commander","id":"admiral","name":"Admiral","slot":"navy_leader","countries":["TST"]}]})).unwrap()
}
#[test]
fn commander_binding_requires_exact_role_and_preserves_declared_targets() {
    let base=json!({"commander_id":"admiral","role":"navy_leader"});
    let assigned=bind(base.clone());assert_eq!(assigned["step"]["completion"]["key"],"politics.commanders.assigned");assert_eq!(assigned["step"]["completion"]["value"],"navy_leader:admiral");
    let mut exact=base.clone();exact["assignment_target"]=json!({"kind":"fleet","instance_id":"3d00000003000000"});
    assert_eq!(bind(exact)["step"]["completion"]["value"],"navy_leader:admiral:fleet:3d00000003000000");
    for extra in [json!({"fleet_name":"Northern Fleet"}),json!({"target":"Northern Fleet"}),json!({"army_id":"same"}),json!({"assignment_target":{"kind":"army","instance_id":"3500000003000000"}})] {
        let mut p=base.clone();p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());let v=bind(p);assert_eq!(v["reason"],"commander_target_unresolved");assert!(v["step"].get("completion").is_none());
    }
    let mut wrong=base.clone();wrong["role"]=json!("corps_commander");assert!(bind(wrong)["step"].get("completion").is_none());
    // Refreshing a previous generic binding with a named target removes the
    // generated completion instead of retaining its weaker previous condition.
    let mut step=assigned["step"].clone();step["action"]["params"]["fleet_name"]=json!("Named fleet");
    let refreshed=request(0,"import_bind_political",json!({"country":"TST","step":step,"entries":[{"kind":"commander","id":"admiral","name":"Admiral","slot":"navy_leader","countries":["TST"]}]})).unwrap();assert!(refreshed["step"].get("completion").is_none());
}
#[test]
fn native_commander_state_keeps_observed_empty_distinct_from_roster_and_bad_role() {
    let row=json!({"commander_id":"admiral","role":"navy_leader","target_kind":"fleet","instance_id":"3d00000003000000","name":"Fleet"});
    let mut input=json!({"usable":true,"sample":{"country":"TST","country_overview":{"country":"TST","politics":{"status":"partial","commander_assignments":{"schema_version":1,"status":"observed","source":"native_reciprocal_commander_assignments","items":[row]}}}}});
    let normalize=|v:&Value| request(0,"state_normalize",json!({"value":v})).unwrap();
    assert_eq!(normalize(&input)["facts"]["politics.commanders.assigned"]["value"],json!(["navy_leader:admiral"]));
    input["sample"]["country_overview"]["politics"]["commander_assignments"]["items"][0]["role"]=json!("field_marshal");assert_eq!(normalize(&input)["facts"]["politics.commanders.assigned"]["status"],"unknown");
    input["sample"]["country_overview"]["politics"]["commander_assignments"]["items"]=json!([]);assert_eq!(normalize(&input)["facts"]["politics.commanders.assigned"]["value"],json!([]));
    input["sample"]["country_overview"]["politics"]["commander_assignments"]["status"]=json!("unavailable");assert_eq!(normalize(&input)["facts"]["politics.commanders.assigned"]["status"],"unknown");
}
