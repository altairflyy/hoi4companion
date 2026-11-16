//! Run-to-definition pinning: archives, legacy compatibility, restart,
//! multiple definitions and fail-closed behavior, through the real
//! application and SQLite store (temporary directories outside the repo).
#![cfg(windows)]
use hoi4companion_backend::application::Application;
use hoi4companion_storage::request as storage;
use serde_json::{Value, json};

const KEY: &str = "word-0001--operation-2";
fn guide(title: &str) -> Value {
    json!({"id":"identity","dependency_policy":"explicit","tracks":{"operations":[
        {"key":KEY,"title":title,"action":{"kind":"manual"},"completion":{"op":"check","key":KEY}}]}})
}
fn step<'a>(result: &'a Value, key: &str) -> &'a Value {
    result["steps"].as_array().unwrap().iter().find(|s| s["key"] == key).unwrap()
}

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("hoi4-pinning-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn db(&self) -> std::path::PathBuf {
        self.0.join("progress.sqlite3")
    }
    fn files(&self) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
    fn load(&self) -> Value {
        storage(json!({"schema_version":1,"op":"load","path":self.db()})).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        assert!(self.0.file_name().unwrap().to_string_lossy().starts_with("hoi4-pinning-"));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn open(temp: &Temp, title: &str) -> Application {
    Application::open(&json!({"path":temp.db(),"guides":[guide(title)]})).unwrap()
}
fn check(app: &mut Application) -> Value {
    let current = app.progress(Some("identity"), None, None).unwrap();
    app.progress(Some("identity"), None, Some(&json!({"run_id":current["run_id"],
        "expected_run_id":current["run_id"],"expected_revision":current["run_revision"],
        "checks":{KEY:true}}))).unwrap()
}

#[test]
fn archive_identity_is_deterministic_and_published_once() {
    let (a, b) = (Temp::new(), Temp::new());
    let first = check(&mut open(&a, "Same meaning"));
    let second = check(&mut open(&b, "Same meaning"));
    assert_eq!(first["definition_ref"], second["definition_ref"]);
    let id = first["definition_ref"]["id"].as_str().unwrap().to_owned();
    let stored = a.load();
    assert_eq!(stored["version"], 2);
    assert_eq!(stored["definitions"][&id], b.load()["definitions"][&id], "byte-identical archives");
    // Restart and further writes reuse, never duplicate or rewrite, the archive.
    let mut again = open(&a, "Same meaning");
    again.progress(Some("identity"), None, Some(&json!({}))).unwrap();
    let reloaded = a.load();
    assert_eq!(reloaded["definitions"].as_object().unwrap().len(), 1);
    assert_eq!(reloaded["definitions"][&id], stored["definitions"][&id]);
    assert!(a.files().iter().all(|f| !f.contains("v1-backup")), "new databases need no backup");
}

#[test]
fn two_runs_on_different_definitions_each_keep_their_own_meaning() {
    let temp = Temp::new();
    let mut app = open(&temp, "Version one");
    let old = check(&mut app);
    app.put_guide(&guide("Version two")).unwrap();
    let new = app.progress(Some("identity"), None, Some(&json!({}))).unwrap();
    assert_ne!(old["definition_ref"], new["definition_ref"]);
    drop(app);
    // After a restart with yet another current definition, selection switches
    // runs and each run keeps its own archived definition and progress.
    let mut app = open(&temp, "Version three");
    let current = app.projection(Some("identity"), false).unwrap();
    assert_eq!(current["guide"]["tracks"]["operations"][0]["title"], "Version two");
    assert_eq!(current["progress"]["checks"], json!({}));
    app.progress(Some("identity"), None, Some(&json!({"run_id":old["run_id"]}))).unwrap();
    let selected = app.projection(Some("identity"), false).unwrap();
    assert_eq!(selected["progress"]["run_id"], old["run_id"]);
    assert_eq!(selected["guide"]["tracks"]["operations"][0]["title"], "Version one");
    assert_eq!(selected["definition"]["id"], old["definition_ref"]["id"]);
    assert_eq!(selected["progress"]["checks"], json!({KEY:true}));
    assert_eq!(temp.load()["definitions"].as_object().unwrap().len(), 2);
}

#[test]
fn legacy_v1_progress_is_preserved_reported_and_backed_up_before_upgrade() {
    let temp = Temp::new();
    let path = temp.db();
    storage(json!({"schema_version":1,"op":"init","path":path})).unwrap();
    let legacy = json!({"schema_version":1,"runs":{"old-run":{"schema_version":1,"guide_id":"identity",
        "run_id":"old-run","session_id":null,"checks":{KEY:true,"word-0001--note-9":true},
        "started":{},"run_revision":4,"research_history":{"r":"tech"}}},
        "active":{"identity":"old-run"},"campaign_active":{}});
    storage(json!({"schema_version":1,"op":"save","path":path,"expected_revision":0,"payload":legacy.to_string()})).unwrap();
    let original = std::fs::read(&path).unwrap();
    {
        let mut app = open(&temp, "Current meaning");
        let packet = app.projection(Some("identity"), false).unwrap();
        assert_eq!(packet["definition"]["status"], "legacy_unresolved");
        assert_eq!(packet["definition"]["observed_definition_id"], Value::Null);
        assert_eq!(packet["definition"]["undeclared_checks"], json!(["word-0001--note-9"]));
        assert_eq!(packet["progress"]["checks"], legacy["runs"]["old-run"]["checks"]);
        assert_eq!(packet["progress"]["research_history"], json!({"r":"tech"}));
        // Restored runs without a campaign stay unverifiable offline (existing rule).
        assert_eq!(step(&packet["evaluation"], KEY)["status"], "unverifiable");
    }
    assert_eq!(std::fs::read(&path).unwrap(), original, "opening and reading never write");
    assert_eq!(temp.files(), vec!["progress.sqlite3"]);
    let current_id;
    {
        let mut app = open(&temp, "Current meaning");
        let current = app.progress(Some("identity"), None, None).unwrap();
        // The legacy run keeps undeclared historical keys; it is not rewritten.
        let mut checks = current["checks"].clone();
        checks.as_object_mut().unwrap().remove("word-0001--note-9");
        app.progress(Some("identity"), None, Some(&json!({"run_id":"old-run","expected_definition_id":null,"checks":checks}))).unwrap();
        let packet = app.projection(Some("identity"), false).unwrap();
        current_id = packet["definition"]["current_definition_id"].clone();
        assert_eq!(packet["definition"]["status"], "legacy_unresolved");
        assert_eq!(packet["definition"]["observed_definition_id"], current_id);
        assert_eq!(packet["definition"]["changed_since_observed"], false);
        assert_eq!(packet["progress"]["run_id"], "old-run");
    }
    let names = temp.files();
    assert!(names.contains(&"progress.sqlite3.v1-backup-r1.sqlite3".to_owned()), "{names:?}");
    let backup = storage(json!({"schema_version":1,"op":"load","path":temp.0.join("progress.sqlite3.v1-backup-r1.sqlite3")})).unwrap();
    assert_eq!(backup["version"], 1);
    assert_eq!(serde_json::from_str::<Value>(backup["payload"].as_str().unwrap()).unwrap(), legacy);
    assert_eq!(temp.load()["version"], 2);
    // A later definition change is detected and reported, never applied as a pin.
    let mut app = open(&temp, "Changed meaning");
    let packet = app.projection(Some("identity"), false).unwrap();
    assert_eq!(packet["definition"]["status"], "legacy_unresolved");
    assert_eq!(packet["definition"]["observed_definition_id"], current_id);
    assert_eq!(packet["definition"]["changed_since_observed"], true);
    assert_eq!(packet["progress"]["run_id"], "old-run");
}

#[test]
fn corrupt_archive_and_stale_writer_fail_closed_without_partial_state() {
    let temp = Temp::new();
    let saved = check(&mut open(&temp, "Original"));
    let id = saved["definition_ref"]["id"].as_str().unwrap().to_owned();
    // Two owners of the same database: the stale one cannot commit, and its
    // in-memory transaction is rolled back.
    let mut first = open(&temp, "Original");
    let mut second = open(&temp, "Original");
    first.progress(Some("identity"), None, Some(&json!({}))).unwrap();
    let before = second.progress(Some("identity"), None, None).unwrap();
    assert!(second.progress(Some("identity"), None, Some(&json!({}))).is_err());
    assert_eq!(second.progress(Some("identity"), None, None).unwrap()["run_id"], before["run_id"]);
    drop((first, second));
    // Forge a different archive under a new valid-format ID referenced by the
    // run: storage accepts the bytes, the backend verifies and refuses them.
    let state = temp.load();
    let mut payload: Value = serde_json::from_str(state["payload"].as_str().unwrap()).unwrap();
    let forged = format!("gd1_{}", "0".repeat(64));
    for row in payload["runs"].as_object_mut().unwrap().values_mut() {
        if row["definition_ref"]["id"] == id.as_str() {
            row["definition_ref"]["id"] = json!(forged);
        }
    }
    let active = payload["active"]["identity"].as_str().unwrap().to_owned();
    payload["runs"][&active]["definition_ref"]["id"] = json!(forged);
    storage(json!({"schema_version":1,"op":"save","path":temp.db(),"expected_revision":state["revision"],
        "version":2,"definitions":{forged.clone():state["definitions"][&id]},"payload":payload.to_string()})).unwrap();
    let mut app = open(&temp, "Original");
    let error = app.projection(Some("identity"), false).unwrap_err();
    assert!(error.contains("corrupt"), "{error}");
    let current = app.progress(Some("identity"), None, None).unwrap();
    let write = json!({"run_id":current["run_id"],"checks":{KEY:true}});
    assert!(app.progress(Some("identity"), None, Some(&write)).is_err(), "no writes against an unverifiable definition");
}

#[test]
fn updating_the_guide_moves_open_checklists_onto_the_new_definition() {
    let temp = Temp::new();
    let manual = |key: &str, title: &str| json!({"key":key,"title":title,"action":{"kind":"manual"},"completion":{"op":"check","key":key}});
    let open_with = |steps: Value| {
        Application::open(&json!({"path":temp.db(),"upgrade_without_catalog":true,
            "guides":[{"id":"identity","dependency_policy":"explicit","tracks":{"operations":steps}}]})).unwrap()
    };
    let reworded = "word-0002--operation-1";
    let mut app = open_with(json!([manual(KEY, "Keep"), manual(reworded, "Old wording")]));
    let current = app.progress(Some("identity"), None, None).unwrap();
    let old = app.progress(Some("identity"), None, Some(&json!({"run_id":current["run_id"],
        "expected_run_id":current["run_id"],"checks":{KEY:true,reworded:true}}))).unwrap();
    assert_eq!(old["definition_ref"]["status"], "pinned");
    drop(app);
    let updated = json!([manual(KEY, "Keep"), manual(reworded, "New wording"), manual("word-0003", "Added")]);
    for _restart in 0..2 {
        let mut app = open_with(updated.clone());
        let packet = app.projection(Some("identity"), false).unwrap();
        assert_eq!(packet["definition"]["status"], "pinned");
        assert_eq!(packet["definition"]["newer_definition_available"], false);
        assert_ne!(packet["definition"]["id"], old["definition_ref"]["id"]);
        assert_eq!(packet["progress"]["run_id"], old["run_id"], "same checklist, not a new one");
        assert_eq!(packet["guide"]["tracks"]["operations"][1]["title"], "New wording");
        // Same instruction keeps its confirmation; changed wording is not assumed done.
        assert_eq!(packet["progress"]["checks"], json!({KEY:true}));
        let previous = &packet["progress"]["previous_definitions"];
        assert_eq!(previous.as_array().unwrap().len(), 1);
        assert_eq!(previous[0]["definition_ref"], old["definition_ref"]);
        assert_eq!(previous[0]["checks"], json!({KEY:true,reworded:true}));
    }
    assert_eq!(temp.load()["definitions"].as_object().unwrap().len(), 2, "old archive kept");
}

#[test]
fn legacy_checklists_also_move_onto_the_current_definition() {
    let temp = Temp::new();
    let path = temp.db();
    storage(json!({"schema_version":1,"op":"init","path":path})).unwrap();
    let legacy = json!({"schema_version":1,"runs":{"old-run":{"schema_version":1,"guide_id":"identity",
        "run_id":"old-run","session_id":null,"checks":{KEY:true,"word-0001--note-9":true},
        "started":{},"run_revision":4}},"active":{"identity":"old-run"},"campaign_active":{}});
    storage(json!({"schema_version":1,"op":"save","path":path,"expected_revision":0,"payload":legacy.to_string()})).unwrap();
    let mut app = Application::open(&json!({"path":path,"upgrade_without_catalog":true,"guides":[guide("Current meaning")]})).unwrap();
    let packet = app.projection(Some("identity"), false).unwrap();
    assert_eq!(packet["definition"]["status"], "pinned");
    assert_eq!(packet["progress"]["run_id"], "old-run");
    assert_eq!(packet["progress"]["checks"], json!({KEY:true}), "undeclared historical key not carried");
    assert_eq!(packet["progress"]["previous_definitions"][0]["checks"], legacy["runs"]["old-run"]["checks"]);
    assert!(temp.files().contains(&"progress.sqlite3.v1-backup-r1.sqlite3".to_owned()));
}
