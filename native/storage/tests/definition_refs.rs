//! Run-to-definition binding contracts of the progress state machine.
use hoi4companion_storage::progress::{Request, Store};
use serde_json::{Value, json};

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
const CAMPAIGN: &str = "hoi4:12345678-1234-1234-1234-123456789abc:deadbeef";
fn pin(c: char) -> Value {
    json!({"status":"pinned","id":format!("gd1_{}", c.to_string().repeat(64)),"archive_format":1})
}
fn call(store: &mut Store, op: &str, args: Value) -> Result<Value, String> {
    let mut ids = vec![];
    loop {
        let result = store.request(Request {
            schema_version: 1,
            op: op.into(),
            args: args.clone(),
            ids: ids.clone(),
        })?;
        if result["needs_id"] == true {
            ids.push(json!(format!("run{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))));
            continue;
        }
        if result["changed"] == true {
            store.request(Request { schema_version: 1, op: "accept".into(), args: Value::Null, ids: vec![] })?;
        }
        return Ok(result);
    }
}
fn legacy_snapshot() -> Value {
    json!({"schema_version":1,"runs":{"old":{"schema_version":1,"guide_id":"g","run_id":"old",
        "session_id":null,"checks":{"k":true},"started":{},"run_revision":3}},
        "active":{"g":"old"},"campaign_active":{}})
}

#[test]
fn new_runs_ask_for_and_pin_the_current_definition() {
    let mut store = Store::default();
    let asked = call(&mut store, "ensure", json!({"guide_id":"g","checks":{},"definitions":{}})).unwrap();
    assert_eq!(asked, json!({"needs_definition":"g"}), "nothing committed without a definition");
    assert!(call(&mut store, "get", json!({"guide_id":"g"})).is_err());
    let made = call(&mut store, "ensure", json!({"guide_id":"g","checks":{},"definitions":{"g":pin('a')}})).unwrap();
    assert_eq!(made["value"]["definition_ref"], pin('a'));
    // A new checklist takes the definition current at its creation.
    let next = call(&mut store, "create", json!({"guide_id":"g","definitions":{"g":pin('b')}})).unwrap();
    assert_eq!(next["value"]["definition_ref"], pin('b'));
    // Updates never change an existing pin, whatever is current.
    let updated = call(&mut store, "update", json!({"guide_id":"g","checks":{"x":true},"definitions":{"g":pin('c')}})).unwrap();
    assert_eq!(updated["value"]["definition_ref"], pin('b'));
    // Explicit null leaves a run unbound (generic/test callers).
    let mut plain = Store::default();
    let row = call(&mut plain, "ensure", json!({"guide_id":"g","checks":{},"definitions":{"g":null}})).unwrap();
    assert!(row["value"].get("definition_ref").is_none());
    assert!(call(&mut plain, "create", json!({"guide_id":"g","definitions":{"g":{"status":"pinned","id":"x","archive_format":1}}})).is_err());
}

#[test]
fn observers_cannot_rewrite_a_pin() {
    let mut store = Store::default();
    let row = call(&mut store, "ensure", json!({"guide_id":"g","checks":{},"definitions":{"g":pin('a')}})).unwrap()["value"].clone();
    let id = row["run_id"].as_str().unwrap().to_owned();
    for changed in [Some(pin('b')), None] {
        let mut forged = row.clone();
        forged["checks"] = json!({"k":true});
        match &changed {
            Some(v) => forged["definition_ref"] = v.clone(),
            None => { forged.as_object_mut().unwrap().remove("definition_ref"); }
        }
        assert!(call(&mut store, "observe_replace", json!({"records":{id.clone():forged},"value":true})).is_err());
    }
}

#[test]
fn legacy_runs_keep_progress_and_only_record_the_first_observed_definition() {
    let mut store = Store::default();
    call(&mut store, "load", json!({"snapshot":legacy_snapshot()})).unwrap();
    let read = call(&mut store, "get", json!({"guide_id":"g"})).unwrap()["value"].clone();
    assert!(read.get("definition_ref").is_none(), "reading never binds");
    let first = call(&mut store, "update", json!({"guide_id":"g","checks":{"k":true,"n":true},"definitions":{"g":pin('a')}})).unwrap();
    let marker = json!({"status":"legacy_unresolved","reason":"original_definition_unavailable","observed_definition_id":pin('a')["id"]});
    assert_eq!(first["value"]["definition_ref"], marker);
    assert_eq!(first["value"]["checks"], json!({"k":true,"n":true}));
    assert_eq!(first["value"]["run_id"], "old");
    let later = call(&mut store, "update", json!({"guide_id":"g","checks":{"k":true},"definitions":{"g":pin('b')}})).unwrap();
    assert_eq!(later["value"]["definition_ref"], marker, "observation evidence is immutable");
    // Older-format checks never become a pin either.
    let mut imported = Store::default();
    let row = call(&mut imported, "ensure", json!({"guide_id":"g","checks":{"k":true},"definitions":{"g":pin('a')}})).unwrap();
    assert_eq!(row["value"]["definition_ref"]["status"], "legacy_unresolved");
}

#[test]
fn rewinds_inherit_pins_and_legacy_status_never_the_current_definition() {
    for (initial, expected_status) in [(Some(pin('a')), "pinned"), (None, "legacy_unresolved")] {
        let mut store = Store::default();
        // The saved run already belongs to this campaign, as after a restart.
        let mut snapshot = legacy_snapshot();
        let old = &mut snapshot["runs"]["old"];
        old["campaign_id"] = json!(CAMPAIGN);
        old["country"] = json!("GER");
        old["last_tick"] = json!(100);
        if let Some(v) = &initial {
            old["definition_ref"] = v.clone();
        }
        snapshot["campaign_active"] = json!({"g":{json!([CAMPAIGN,"GER"]).to_string():"old"}});
        call(&mut store, "load", json!({"snapshot":snapshot})).unwrap();
        let defs = json!({"g":pin('f')});
        call(&mut store, "bind_campaign", json!({"campaign_id":CAMPAIGN,"country":"GER","session_id":"s","tick":100,"definitions":defs})).unwrap();
        let bound = call(&mut store, "get", json!({"guide_id":"g"})).unwrap()["value"].clone();
        call(&mut store, "bind_campaign", json!({"campaign_id":CAMPAIGN,"country":"GER","session_id":"s","tick":50,"definitions":defs})).unwrap();
        let rewound = call(&mut store, "get", json!({"guide_id":"g"})).unwrap()["value"].clone();
        assert_eq!(rewound["branch_reason"], "timeline_rewind");
        assert_ne!(rewound["run_id"], bound["run_id"]);
        assert_eq!(rewound["definition_ref"]["status"], expected_status);
        if let Some(v) = &initial {
            assert_eq!(rewound["definition_ref"], *v);
            assert_eq!(bound["definition_ref"], *v);
        } else {
            assert_ne!(rewound["definition_ref"]["status"], "pinned");
        }
    }
}

#[test]
fn malformed_definition_refs_are_rejected_on_load() {
    for bad in [json!("gd1_x"), json!({"status":"pinned","id":"gd1_x","archive_format":1}),
        json!({"status":"pinned","id":pin('a')["id"],"archive_format":2}),
        json!({"status":"legacy_unresolved"}), json!({"status":"other","reason":"x"})] {
        let mut snapshot = legacy_snapshot();
        snapshot["runs"]["old"]["definition_ref"] = bad;
        assert!(call(&mut Store::default(), "load", json!({"snapshot":snapshot})).is_err());
    }
}

#[test]
fn set_aside_steps_are_kept_with_the_run_and_left_out_of_metadata() {
    let mut store = Store::default();
    call(&mut store, "ensure", json!({"guide_id":"g","checks":{},"definitions":{"g":pin('a')}})).unwrap();
    let aside = call(&mut store, "update", json!({"guide_id":"g","checks":{},"set_aside":{"k":true},"definitions":{}})).unwrap();
    assert_eq!(aside["value"]["set_aside"], json!({"k":true}));
    // An update without the field keeps the set-aside steps.
    let kept = call(&mut store, "update", json!({"guide_id":"g","checks":{"x":true},"definitions":{}})).unwrap();
    assert_eq!(kept["value"]["set_aside"], json!({"k":true}));
    assert!(call(&mut store, "update", json!({"guide_id":"g","checks":{},"set_aside":{"k":"yes"},"definitions":{}})).is_err());
    let metadata = call(&mut store, "metadata", json!({})).unwrap();
    assert!(metadata["value"]["g"].get("set_aside").is_none());
    // Runs written before the field existed stay valid.
    let mut legacy = Store::default();
    call(&mut legacy, "load", json!({"snapshot":legacy_snapshot()})).unwrap();
    assert!(call(&mut legacy, "get", json!({"guide_id":"g"})).unwrap()["value"].get("set_aside").is_none());
}
