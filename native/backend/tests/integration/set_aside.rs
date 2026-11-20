//! Steps the player sets aside: saved with the run through the progress API
//! and kept across restarts (temporary directories outside the repo). How
//! guide-core reads them: `guide_runtime` and guide-core tests.
#![cfg(windows)]
use hoi4companion_backend::application::Application;
use serde_json::{Value, json};

fn guide() -> Value {
    json!({"id":"aside","tracks":{"operations":[
        {"key":"a","title":"First","action":{"kind":"manual"}},
        {"key":"b","title":"Second","action":{"kind":"manual"}},
        {"key":"r","title":"Method","tracking":"reminder","action":{"kind":"manual"}}]}})
}
fn update(app: &mut Application, body: Value) -> Result<Value, String> {
    let current = app.progress(Some("aside"), None, None).unwrap();
    let mut body = body;
    body["run_id"] = current["run_id"].clone();
    body["expected_revision"] = current["run_revision"].clone();
    app.progress(Some("aside"), None, Some(&body))
}

#[test]
fn set_aside_steps_are_saved_filtered_and_kept_across_restarts() {
    let dir = std::env::temp_dir().join(format!("hoi4-aside-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&dir).unwrap();
    let db = dir.join("progress.sqlite3");
    {
        let mut app = Application::open(&json!({"path":db,"guides":[guide()]})).unwrap();
        // Unknown keys, reminders and false entries are dropped, never stored.
        let saved = update(&mut app, json!({"set_aside":{"a":true,"ghost":true,"r":true,"b":false}})).unwrap();
        assert_eq!(saved["set_aside"], json!({"a":true}));
        assert!(update(&mut app, json!({"set_aside":{"a":"yes"}})).is_err());
        // Checks written alone keep the set-aside steps.
        assert_eq!(update(&mut app, json!({"checks":{"b":true}})).unwrap()["set_aside"], json!({"a":true}));
    }
    let mut app = Application::open(&json!({"path":db,"guides":[guide()]})).unwrap();
    assert_eq!(app.progress(Some("aside"), None, None).unwrap()["set_aside"], json!({"a":true}));
    // Resuming clears it.
    assert_eq!(update(&mut app, json!({"set_aside":{}})).unwrap()["set_aside"], json!({}));
    drop(app);
    let _ = std::fs::remove_dir_all(&dir);
}
