//! Characterization, not a versioning implementation. Tests named `current_*`
//! deliberately expose gaps that must change when definition pinning ships.
#![cfg(windows)]
use hoi4companion_backend::{application::Application, request};
use serde_json::{Value, json};

struct Runs(u64);
impl Runs {
    fn open() -> Self {
        Self(
            request(0, "runs_open", json!({}))
                .unwrap()
                .as_u64()
                .unwrap(),
        )
    }
    fn call(&self, op: &str, args: Value) -> Result<Value, String> {
        request(
            0,
            "runs_call",
            json!({"handle":self.0,"operation":op,"args":args}),
        )
    }
    fn get(&self) -> Value {
        self.call("get", json!({"guide_id":"identity"})).unwrap()
    }
    fn seed(&self) {
        self.call("ensure", json!({"guide_id":"identity","checks":{}}))
            .unwrap();
    }
    fn replace(&self, row: Value) {
        let id = row["run_id"].as_str().unwrap().to_owned();
        self.call("observe_replace", json!({"records":{id:row},"value":true}))
            .unwrap();
    }
}
impl Drop for Runs {
    fn drop(&mut self) {
        let _ = request(0, "runs_release", json!({"handle":self.0}));
    }
}
fn guide(title: &str) -> Value {
    json!({"id":"identity","dependency_policy":"explicit","tracks":{"operations":[
        {"key":"word-0001--operation-2","title":title,"action":{"kind":"manual"},
         "completion":{"op":"check","key":"word-0001--operation-2"}}]}})
}
fn evaluate(guide: &Value, progress: &Value) -> Value {
    let guide = request(0, "guide_normalize", json!({"guide":guide})).unwrap();
    request(
        0,
        "guide_evaluate",
        json!({"guide":guide,"state":{"usable":true},"progress":progress}),
    )
    .unwrap()
}
fn step<'a>(result: &'a Value, key: &str) -> &'a Value {
    result["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == key)
        .unwrap()
}

// The four tests below replaced `current_*` gap characterizations when
// definition pinning shipped (2026-11-15); each asserts the safety contract.
#[test]
fn pinned_run_keeps_archived_meaning_after_registry_replacement() {
    // Internal registry replacement, NOT a claim that HTTP admits duplicate IDs.
    let mut app = Application::open(&json!({"guides":[guide("Old instruction")]})).unwrap();
    let initial = app.progress(Some("identity"), None, None).unwrap();
    let pinned = initial["definition_ref"].clone();
    assert_eq!(pinned["status"], "pinned");
    let update = json!({"run_id":initial["run_id"],"expected_run_id":initial["run_id"],
        "expected_revision":initial["run_revision"],"expected_definition_id":pinned["id"],
        "checks":{"word-0001--operation-2":true}});
    let saved = app.progress(Some("identity"), None, Some(&update)).unwrap();
    let before = app.projection(Some("identity"), false).unwrap();
    app.put_guide(&guide("Different instruction with reused key"))
        .unwrap();
    let after = app.projection(Some("identity"), false).unwrap();
    assert_eq!(before["definition_revision"], after["definition_revision"]);
    assert_eq!(after["guide"]["tracks"]["operations"][0]["title"], "Old instruction");
    assert_eq!(after["definition"]["id"], pinned["id"]);
    assert_eq!(after["definition"]["newer_definition_available"], true);
    assert_eq!(after["progress"]["run_id"], saved["run_id"]);
    assert_eq!(after["progress"]["run_revision"], saved["run_revision"]);
    assert_eq!(after["progress"]["checks"], saved["checks"]);
    assert_eq!(
        step(&after["evaluation"], "word-0001--operation-2")["status"],
        "completed"
    );
    // Definition CAS rejects a writer that believes in another definition.
    let stale = json!({"run_id":saved["run_id"],"expected_run_id":saved["run_id"],
        "expected_revision":saved["run_revision"],"expected_definition_id":"gd1_other","checks":saved["checks"]});
    assert!(app.progress(Some("identity"), None, Some(&stale)).unwrap_err().contains("definition"));
    // Only a new checklist uses the replacement; the old checks do not follow.
    let fresh = app.progress(Some("identity"), None, Some(&json!({}))).unwrap();
    assert_ne!(fresh["definition_ref"], pinned);
    assert_eq!(fresh["checks"], json!({}));
    let now = app.projection(Some("identity"), false).unwrap();
    assert_eq!(now["guide"]["tracks"]["operations"][0]["title"], "Different instruction with reused key");
}

#[test]
fn display_metadata_changes_reach_only_new_runs() {
    let mut app = Application::open(&json!({"guides":[guide("Instruction")]})).unwrap();
    let before = app.projection(Some("identity"), false).unwrap();
    let mut changed = guide("Instruction");
    changed["title"] = json!("Display translation only");
    app.put_guide(&changed).unwrap();
    let after = app.projection(Some("identity"), false).unwrap();
    assert_eq!(before["definition_revision"], after["definition_revision"]);
    assert_eq!(before["progress"]["run_id"], after["progress"]["run_id"]);
    app.progress(Some("identity"), None, Some(&json!({}))).unwrap();
    let fresh = app.projection(Some("identity"), false).unwrap();
    assert_ne!(before["definition_revision"], fresh["definition_revision"]);
}

#[test]
fn definition_ref_is_typed_and_immutable_in_run_storage() {
    let runs = Runs::open();
    runs.seed();
    let mut row = runs.get();
    row["definition_ref"] = json!({"id":"proposed-A","format":1});
    assert!(runs.call("observe_replace", json!({"records":{row["run_id"].as_str().unwrap():row},"value":true})).is_err());
    let pin = json!({"status":"pinned","id":format!("gd1_{}", "a".repeat(64)),"archive_format":1});
    let mut snapshot = runs.call("snapshot", json!({})).unwrap();
    let id = runs.get()["run_id"].as_str().unwrap().to_owned();
    snapshot["runs"][&id]["definition_ref"] = pin.clone();
    let restored = Runs::open();
    restored.call("load", json!({"snapshot":snapshot})).unwrap();
    restored
        .call(
            "update",
            json!({"guide_id":"identity","checks":{"x":true},"started":{"x":true}}),
        )
        .unwrap();
    let mut current = restored.get();
    assert_eq!(current["definition_ref"], pin);
    current["definition_ref"] = json!(42);
    assert!(restored.call("observe_replace", json!({"records":{id.clone():current},"value":true})).is_err());
    assert_eq!(restored.get()["definition_ref"], pin);
    assert_eq!(runs.get()["checks"], json!({})); // Independent stores, no shared user data.
}

#[test]
fn rewind_keeps_old_record_and_inherits_its_pin() {
    let runs = Runs::open();
    runs.seed();
    let binding = |tick| {
        json!({"campaign_id":"hoi4:11111111-1111-4111-8111-111111111111:00000001",
        "country":"ENG","session_id":"fixture","tick":tick})
    };
    runs.call("bind_campaign", binding(100)).unwrap();
    let pin = json!({"status":"pinned","id":format!("gd1_{}", "a".repeat(64)),"archive_format":1});
    let mut row = runs.get();
    row["checks"] = json!({"old-child":true});
    row["research_bindings"] = json!({"old-research":"tech-A"});
    runs.replace(row);
    // Pins are set at creation, never by an observer: restore one via snapshot.
    let mut snapshot = runs.call("snapshot", json!({})).unwrap();
    let key = runs.get()["run_id"].as_str().unwrap().to_owned();
    snapshot["runs"][&key]["definition_ref"] = pin.clone();
    runs.call("load", json!({"snapshot":snapshot})).unwrap();
    runs.call("bind_campaign", binding(100)).unwrap();
    let old = runs.get();
    runs.call("bind_campaign", binding(50)).unwrap();
    let new = runs.get();
    assert_ne!(old["run_id"], new["run_id"]);
    assert_eq!(new["branch_reason"], "timeline_rewind");
    assert_eq!(new["definition_ref"], pin);
    assert_eq!(new["checks"], json!({}));
    assert_eq!(new["research_history"]["old-research"], "tech-A");
    let records = runs.call("records", json!({})).unwrap();
    assert_eq!(records[old["run_id"].as_str().unwrap()], old);
}

#[test]
fn current_storage_rejects_unknown_root_versions_without_mutating_snapshot() {
    let runs = Runs::open();
    runs.seed();
    let original = runs.call("snapshot", json!({})).unwrap();
    let mut changed = original.clone();
    changed["guide_definitions"] = json!({});
    assert!(runs.call("load", json!({"snapshot":changed})).is_err());
    assert_eq!(runs.call("snapshot", json!({})).unwrap(), original);
    let mut version_two = original.clone();
    version_two["schema_version"] = json!(2);
    assert!(runs.call("load", json!({"snapshot":version_two})).is_err());
    assert_eq!(runs.call("snapshot", json!({})).unwrap(), original);
}

#[test]
fn sqlite_reload_keeps_run_data_and_its_archived_definition() {
    struct OwnedTemp(std::path::PathBuf);
    impl Drop for OwnedTemp {
        fn drop(&mut self) {
            // Only a directory uniquely created by this test, outside the repo.
            let root = std::env::temp_dir();
            assert_eq!(self.0.parent(), Some(root.as_path()));
            assert!(
                self.0
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("hoi4-identity-")
            );
            std::fs::remove_dir_all(&self.0).expect("owned test directory cleanup");
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp = OwnedTemp(
        std::env::temp_dir().join(format!("hoi4-identity-{}-{nonce}", std::process::id())),
    );
    std::fs::create_dir(&temp.0).unwrap();
    let path = temp.0.join("progress.sqlite3");
    let saved;
    {
        let mut app =
            Application::open(&json!({"path":path,"guides":[guide("Original meaning")]})).unwrap();
        let before = app.progress(Some("identity"), None, None).unwrap();
        saved = app
            .progress(
                Some("identity"),
                None,
                Some(&json!({"run_id":before["run_id"],
            "checks":{"word-0001--operation-2":true},"started":{"word-0001--operation-2":true}})),
            )
            .unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    {
        let mut app =
            Application::open(&json!({"path":path,"guides":[guide("Reloaded different meaning")]}))
                .unwrap();
        let after = app.projection(Some("identity"), false).unwrap();
        for key in ["run_id", "checks", "started", "run_revision"] {
            assert_eq!(after["progress"][key], saved[key]);
        }
        assert_eq!(
            after["guide"]["tracks"]["operations"][0]["title"],
            "Original meaning"
        );
        assert_eq!(after["definition"]["status"], "pinned");
        assert_eq!(after["definition"]["newer_definition_available"], true);
    }
    // Reading/evaluating after a restart writes nothing.
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn proposed_occurrences_preserve_unicode_quotes_scope_and_duplicate_identity() {
    let f: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/guide-definition-identity-cases.json"
    ))
    .unwrap();
    let text = f["source"]["text"].as_str().unwrap();
    assert_eq!(text.len(), 59);
    let occurrences = f["children"].as_array().unwrap();
    for occurrence in std::iter::once(&f["header"]).chain(occurrences) {
        let start = occurrence["start_utf8"].as_u64().unwrap() as usize;
        let end = occurrence["end_utf8"].as_u64().unwrap() as usize;
        assert_eq!(text.get(start..end), occurrence["quote"].as_str());
    }
    assert_eq!(occurrences[0]["quote"], occurrences[1]["quote"]);
    assert_ne!(occurrences[0]["id"], occurrences[1]["id"]);
    assert_ne!(
        occurrences[0]["occurrence_id"],
        occurrences[1]["occurrence_id"]
    );
    let child = |o: &Value| {
        json!({"key":o["id"],"title":o["quote"],"when":f["header"]["condition"],
        "source_occurrence":o,"action":{"kind":"manual"}})
    };
    let first = occurrences[0]["id"].as_str().unwrap();
    let second = occurrences[1]["id"].as_str().unwrap();
    let mut checks = json!({});
    checks[first] = json!(true);
    checks[f["block_id"].as_str().unwrap()] = json!(true);
    let progress =
        json!({"schema_version":1,"guide_id":"identity","run_id":"fixture","checks":checks});
    let state = json!({"usable":true,"facts":{"country.at_war":{"status":"observed","source":"fixture","value":true}}});
    for children in [
        json!([child(&occurrences[0]), child(&occurrences[1])]),
        json!([{"key":"step_33333333","action":{"kind":"manual"}},child(&occurrences[1]),child(&occurrences[0])]),
    ] {
        let normalized = request(0,"guide_normalize",json!({"guide":{"id":"identity","dependency_policy":"explicit","tracks":{"operations":children}}})).unwrap();
        let result = request(
            0,
            "guide_evaluate",
            json!({"guide":normalized,"state":state,"progress":progress}),
        )
        .unwrap();
        assert_eq!(step(&result, first)["status"], "completed");
        assert_eq!(step(&result, second)["status"], "unverifiable");
        let stored = normalized["tracks"]["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == second)
            .unwrap();
        assert_eq!(stored["source_occurrence"], occurrences[1]);
        assert_eq!(stored["when"], f["header"]["condition"]);
    }
}

#[test]
fn descriptive_understanding_is_not_automatic_completion_or_definition_enforcement() {
    let g = json!({"id":"identity","definition_id":"proposed-A","dependency_policy":"explicit","tracks":{"operations":[
        {"key":"understood","tracking":"reminder","interpretation":{"status":"understood","verification":"unavailable"},
         "action":{"kind":"manual","params":{"instructions":["Arrange a defensive line"]}}}]}});
    let progress = json!({"schema_version":1,"guide_id":"identity","run_id":"fixture","definition_id":"proposed-B","checks":{"understood":true}});
    let result = evaluate(&g, &progress);
    assert_ne!(step(&result, "understood")["status"], "completed");
    // Today unknown definition metadata is not enforced at the core boundary.
    // A future application admission gate is required; this does not implement it.
    assert_ne!(result["progress_status"], "wrong_definition");
    let mut invalid = g.clone();
    invalid["tracks"]["operations"]
        .as_array_mut()
        .unwrap()
        .push(json!({"key":"successor","depends_on":["understood"],"action":{"kind":"manual"}}));
    assert!(request(0, "guide_normalize", json!({"guide":invalid})).is_err());
    // A manual action with an explicit, currently unobserved fact predicate can
    // be a dependency. Its predicate remains unknown, even with a stray check.
    invalid["tracks"]["operations"][0]
        .as_object_mut()
        .unwrap()
        .remove("tracking");
    invalid["tracks"]["operations"][0]["completion"] =
        json!({"op":"fact","key":"fixture.defensive_line_ready","test":"eq","value":true});
    let unknown = evaluate(&invalid, &progress);
    assert_eq!(
        step(&unknown, "understood")["completion"]["value"],
        Value::Null
    );
    assert_ne!(step(&unknown, "successor")["status"], "completed");
}
