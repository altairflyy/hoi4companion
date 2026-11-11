use hoi4companion_storage::{progress::{Store,Request},decision_history};
use serde_json::{Value,json};
const CAMPAIGN:&str="hoi4:11111111-1111-1111-1111-111111111111:12345678";
fn raw(store:&mut Store,op:&str,args:Value)->Value {
    store.request(Request{schema_version:1,op:op.into(),args,ids:vec![]}).unwrap()
}
fn call(store:&mut Store,op:&str,args:Value)->Value {
    let r=raw(store,op,args);if r["changed"]==true {raw(store,"accept",json!({}));}r["value"].clone()
}
fn bind(store:&mut Store,country:&str,session:&str,tick:u64) {
    call(store,"bind_campaign",json!({"campaign_id":CAMPAIGN,"country":country,"session_id":session,"tick":tick}));
}
fn state(country:&str,session:&str,phase:Option<&str>)->Value {
    let mut facts=json!({"campaign.id":{"status":"observed","value":CAMPAIGN,"source":"native_campaign"},"country":{"status":"observed","value":country,"source":"process_memory"}});
    for f in ["available","active","executed","completed"] {
        let value=if phase==Some(f) || f=="executed" && phase.is_some_and(|p|p!="available") {json!(["test:country:GER"])}else{json!([])};
        facts[format!("decisions.targeted.{f}")]=json!({"status":"calculated","source":"native_targeted_decision_adapter","value":value});
    }
    for f in ["active","executed","completed"] {facts[format!("decisions.{f}")]=json!({"status":"calculated","source":"native_decision_membership_adapter","value":[]});}
    json!({"schema_version":1,"usable":true,"session_id":session,"facts":facts})
}
fn observe(store:&mut Store,phase:Option<&str>,tick:u64) {
    bind(store,"NOR","s",tick);call(store,"observe_decisions",json!({"state":state("NOR","s",phase)}));
}
fn history(store:&mut Store,country:&str)->Value {call(store,"decision_history",json!({"campaign_id":CAMPAIGN,"country":country}))}
fn bounds(store:&mut Store)->(u64,u64) {let h=history(store,"NOR");let e=&h["entries"]["test:country:GER"];(e["executions_lower_bound"].as_u64().unwrap(),e["completions_lower_bound"].as_u64().unwrap())}
#[test]
fn paused_duplicates_and_ordered_same_tick_cycles_count_lower_bounds() {
    let mut s=Store::default();observe(&mut s,Some("active"),100);observe(&mut s,Some("active"),100);
    observe(&mut s,Some("completed"),100);observe(&mut s,Some("completed"),100);assert_eq!(bounds(&mut s),(1,1));
    observe(&mut s,Some("available"),100);observe(&mut s,Some("available"),100);
    observe(&mut s,Some("active"),100);observe(&mut s,Some("completed"),100);assert_eq!(bounds(&mut s),(2,2));
}
#[test]
fn disappearance_cancellation_and_unknown_never_invent_completion_or_repeat() {
    let mut s=Store::default();observe(&mut s,Some("executed"),100);observe(&mut s,None,101);observe(&mut s,Some("executed"),102);
    assert_eq!(bounds(&mut s),(1,0));observe(&mut s,Some("available"),103);
    let mut unknown=state("NOR","s",None);unknown["facts"]["decisions.targeted.executed"]["source"]=json!("unqualified");
    call(&mut s,"observe_decisions",json!({"state":unknown}));observe(&mut s,Some("active"),104);assert_eq!(bounds(&mut s),(1,0));
    assert_eq!(history(&mut s,"NOR")["coverage"]["gaps_observed"],true);
}
#[test]
fn session_restart_preserves_positive_history_and_breaks_transition_arms() {
    let mut s=Store::default();observe(&mut s,Some("completed"),100);observe(&mut s,Some("available"),101);
    let snapshot=call(&mut s,"snapshot",json!({}));let mut restored=Store::default();call(&mut restored,"load",json!({"snapshot":snapshot}));
    bind(&mut restored,"NOR","new-session",102);call(&mut restored,"observe_decisions",json!({"state":state("NOR","new-session",Some("active"))}));
    assert_eq!(bounds(&mut restored),(1,1));
}
#[test]
fn campaign_country_switch_and_rewind_are_isolated_without_selected_guide() {
    let mut s=Store::default();observe(&mut s,Some("completed"),100);bind(&mut s,"GER","s",101);
    assert!(history(&mut s,"GER")["entries"].as_object().unwrap().is_empty());
    bind(&mut s,"NOR","s",102);assert_eq!(bounds(&mut s),(1,1));
    bind(&mut s,"NOR","s",50);assert!(history(&mut s,"NOR")["entries"].as_object().unwrap().is_empty());
    assert!(call(&mut s,"snapshot",json!({}))["runs"].as_object().unwrap().is_empty());
}
#[test]
fn transaction_rollback_reverts_ledger_and_binding() {
    let mut s=Store::default();observe(&mut s,Some("active"),100);let before=call(&mut s,"snapshot",json!({}));
    let changed=raw(&mut s,"observe_decisions",json!({"state":state("NOR","s",Some("completed"))}));assert_eq!(changed["changed"],true);
    raw(&mut s,"rollback",json!({}));assert_eq!(call(&mut s,"snapshot",json!({})),before);
    raw(&mut s,"bind_campaign",json!({"campaign_id":CAMPAIGN,"country":"NOR","session_id":"s","tick":50}));raw(&mut s,"rollback",json!({}));
    assert_eq!(call(&mut s,"snapshot",json!({})),before);
}
#[test]
fn old_snapshot_migration_does_not_reconstruct_pre_observation_history() {
    let mut s=Store::default();let old=json!({"schema_version":1,"runs":{},"active":{},"campaign_active":{}});
    call(&mut s,"load",json!({"snapshot":old}));assert_eq!(call(&mut s,"snapshot",json!({})),old);
    observe(&mut s,None,100);let h=history(&mut s,"NOR");let facts=decision_history::facts(&h).unwrap();
    assert!(facts.get("decisions.history.executions_lower_bound:never_seen").is_none());assert_eq!(h["coverage"]["pre_observation_history"],"unknown");
}
#[test]
fn ledger_facts_keep_targets_current_memberships_and_missing_counts_separate() {
    let mut s=Store::default();observe(&mut s,Some("completed"),100);observe(&mut s,None,101);
    let facts=decision_history::facts(&history(&mut s,"NOR")).unwrap();
    assert_eq!(facts["decisions.targeted.history.executed"]["value"],json!(["test:country:GER"]));
    assert_eq!(facts["decisions.history.executions_lower_bound:test:country:GER"]["value"],1);
    assert!(!facts.contains_key("decisions.targeted.executed"));assert!(!facts.contains_key("decisions.history.executions_lower_bound:test:country:SWE"));
}
#[test]
fn advancing_unchanged_sample_is_noop_but_rewind_uses_latest_binding_tick() {
    let mut s=Store::default();observe(&mut s,Some("active"),100);let before=call(&mut s,"snapshot",json!({}));
    observe(&mut s,Some("active"),200);assert_eq!(call(&mut s,"snapshot",json!({})),before);
    bind(&mut s,"NOR","s",150);assert!(history(&mut s,"NOR")["entries"].as_object().unwrap().is_empty());
}
#[test]
fn unavailable_sample_and_country_switch_break_armed_cycles() {
    let mut s=Store::default();observe(&mut s,Some("completed"),100);observe(&mut s,Some("available"),101);
    call(&mut s,"observe_decisions",json!({"state":{"usable":false,"facts":{}}}));observe(&mut s,Some("active"),102);assert_eq!(bounds(&mut s),(1,1));
    observe(&mut s,Some("available"),103);bind(&mut s,"GER","s",104);bind(&mut s,"NOR","s",105);
    call(&mut s,"observe_decisions",json!({"state":state("NOR","s",Some("active"))}));assert_eq!(bounds(&mut s),(1,1));
}
