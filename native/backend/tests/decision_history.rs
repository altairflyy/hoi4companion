use hoi4companion_backend::request;
use serde_json::{Value,json};

struct TestDatabase(std::path::PathBuf);
impl TestDatabase {
    fn new() -> Self {
        let root = std::env::var_os("HOI4COMPANION_TEST_TEMP_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("hoi4companion-tests"));
        std::fs::create_dir_all(&root).unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        for attempt in 0..100 {
            let directory = root.join(format!("decision-history-{}-{nonce}-{attempt}", std::process::id()));
            match std::fs::create_dir(&directory) {
                Ok(()) => return Self(directory.join("history.sqlite3")),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("Cannot create owned test directory: {error}"),
            }
        }
        panic!("Cannot allocate unique test directory");
    }
}
impl Drop for TestDatabase {
    fn drop(&mut self) {
        // Delete only the database and SQLite sidecars in our exclusively
        // created directory; never recursively remove a caller-supplied root.
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut path = self.0.as_os_str().to_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(std::path::PathBuf::from(path));
        }
        let _ = std::fs::remove_dir(self.0.parent().unwrap());
    }
}
#[test]
fn campaign_decision_history_projection_matches_browser_contract() {
    let fixture:Value=serde_json::from_str(include_str!("../../../tests/fixtures/decision-history.json")).unwrap();
    for row in fixture["cases"].as_array().unwrap() {
        let mut guide=fixture["guide"].clone();let mut state=fixture["state"].clone();let mut progress=fixture["progress"].clone();
        for (source,target) in [("history_country","country"),("history_session","session_id")] {if let Some(v)=row.get(source){progress["decision_history"][target]=v.clone();}}
        if let Some(v)=row.get("executions") {progress["decision_history"]["entries"]["test:country:GER"]["executions_lower_bound"]=v.clone();}
        if let Some(v)=row.get("tick") {state["facts"]["campaign.tick"]["value"]=v.clone();}
        if row["missing_tick"]==true {state["facts"].as_object_mut().unwrap().remove("campaign.tick");}
        if let Some(v)=row.get("tick_source") {state["facts"]["campaign.tick"]["source"]=v.clone();}
        if row["empty"]==true {progress["decision_history"]["entries"]=json!({});}
        if let Some(v)=row.get("guide_id") {guide["id"]=v.clone();progress["guide_id"]=v.clone();}
        let guide=request(0,"guide_normalize",json!({"guide":guide})).unwrap();
        let result=request(0,"guide_evaluate",json!({"guide":guide,"state":state,"progress":progress})).unwrap();
        assert_eq!(result["steps"][0]["status"]=="completed",row["completed"],"{}",row["name"]);
        if row["empty"]==true || row["missing_tick"]==true || row.get("tick_source").is_some() || row.get("tick").is_some() {assert!(result["steps"][0]["completion"]["value"].is_null(),"{}",row["name"]);}
    }
}
#[test]
fn malformed_history_identifiers_match_browser_rejection() {
    let fixture:Value=serde_json::from_str(include_str!("../../../tests/fixtures/decision-history.json")).unwrap();
    for row in fixture["invalid_histories"].as_array().unwrap() {
        let mut progress=fixture["progress"].clone();
        let mut entry=progress["decision_history"]["entries"]["test:country:GER"].clone();
        entry["id"]=row["id"].clone();entry["target"]=row["target"].clone();
        progress["decision_history"]["entries"]=json!({});
        progress["decision_history"]["entries"][row["token"].as_str().unwrap()]=entry;
        let guide=request(0,"guide_normalize",json!({"guide":fixture["guide"]})).unwrap();
        let error=request(0,"guide_evaluate",json!({"guide":guide,"state":fixture["state"],"progress":progress})).unwrap_err();
        assert_eq!(error,"Storico decisioni campagna non valido","{}",row["name"]);
    }
}
#[test]
fn durable_history_survives_restart_and_disk_conflict_rolls_back_without_guide() {
    let fixture:Value=serde_json::from_str(include_str!("../../../tests/fixtures/decision-history.json")).unwrap();
    let database=TestDatabase::new();
    let path=&database.0;
    assert!(!path.exists(),"test must never overwrite existing state");
    let name=path.to_string_lossy().to_string();
    let handle=request(0,"runs_open",json!({"path":name})).unwrap();
    let call=|operation:&str,args:Value|request(0,"runs_call",json!({"handle":handle,"operation":operation,"args":args}));
    call("bind_campaign",json!({"campaign_id":fixture["progress"]["campaign_id"],"country":"NOR","session_id":"s","tick":100})).unwrap();
    let mut state=fixture["state"].clone();
    for (prefix,source,fields) in [("decisions","native_decision_membership_adapter",vec!["executed","active","completed"]),("decisions.targeted","native_targeted_decision_adapter",vec!["available","executed","active","completed"])] {
        for field in fields {state["facts"][format!("{prefix}.{field}")]=json!({"status":"calculated","source":source,"value":if prefix=="decisions.targeted" && field=="executed" {json!(["test:country:GER"])}else{json!([])}});}
    }
    call("observe_decisions",json!({"state":state})).unwrap();
    let before=call("snapshot",json!({})).unwrap();
    assert!(before["runs"].as_object().unwrap().is_empty());
    let reopened=request(0,"runs_open",json!({"path":name})).unwrap();
    assert_eq!(request(0,"runs_call",json!({"handle":reopened,"operation":"snapshot","args":{}})).unwrap(),before);
    request(0,"runs_release",json!({"handle":reopened})).unwrap();
    let revision=request(0,"runs_status",json!({"handle":handle})).unwrap()["revision"].clone();
    hoi4companion_storage::request(json!({"schema_version":1,"op":"save","path":name,"expected_revision":revision,"payload":before.to_string()})).unwrap();
    state["facts"]["decisions.targeted.completed"]["value"]=json!(["test:country:GER"]);
    assert!(call("observe_decisions",json!({"state":state})).is_err(),"stale SQLite owner must reject commit");
    assert_eq!(call("snapshot",json!({})).unwrap(),before,"failed commit must roll back ledger in memory");
    request(0,"runs_release",json!({"handle":handle})).unwrap();
}
