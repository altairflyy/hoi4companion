//! Answers the player confirmed on the HUD for ambiguous event lines: stored
//! with the progress save, applied to every run of the guide on a clone of its
//! definition (never in the archive), undone and removed. Real application and
//! SQLite store in temporary directories outside the repository.
#![cfg(windows)]
use hoi4companion_backend::application::Application;
use hoi4companion_backend::http::{Service, serve_service};
use hoi4companion_storage::request as storage;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const GUIDE: &str = "answers";

fn options() -> Value {
    json!([{"id":"E.a","event_id":"E","compatible":true},{"id":"E.b","event_id":"E","compatible":true}])
}
fn line(key: &str, title: &str, quote: &str) -> Value {
    json!({"key":key,"kind":"gate","title":title,"tracking":"reminder","action":{"kind":"manual","params":{
        "instructions":[title],"reference_candidates":{"status":"ambiguous","text":quote,"candidates":options(),"event_options":options()}}}})
}
fn guide() -> Value {
    json!({"id":GUIDE,"title":"Answers","applies_to":{"country":"GER"},"tracks":{"operations":[
        line("single", "Mobilize The…", "Mobilize The…"),
        line("pair", "Absolutely Not… + There is a…", "Absolutely Not…"),
    ]}})
}
/// Catalog entries used while no game catalog is loaded (test hook).
fn answer_index() -> Value {
    json!({"entries":[
        {"kind":"country","id":"GER","name":"Germany"},
        {"kind":"event","id":"E","name":"E","receivers":["GER"],"receivers_unknown":false},
        {"kind":"event_option","id":"E.a","event_id":"E","name":"Alpha"},
        {"kind":"event_option","id":"E.b","event_id":"E","name":"Beta"},
    ]})
}

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("hoi4-answers-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn db(&self) -> PathBuf {
        self.0.join("progress.sqlite3")
    }
    fn load(&self) -> Value {
        storage(json!({"schema_version":1,"op":"load","path":self.db()})).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        assert!(self.0.file_name().unwrap().to_string_lossy().starts_with("hoi4-answers-"));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn open(temp: &Temp) -> Application {
    Application::open(&json!({"path":temp.db(),"upgrade_without_catalog":true,"guides":[guide()],"answer_index":answer_index()})).unwrap()
}
fn answers(app: &mut Application, update: Value) -> Result<Value, String> {
    app.request("app_answers", &json!({"guide_id":GUIDE,"update":update}))
}
fn confirm(app: &mut Application, key: &str, quote: &str, option: &str) -> Result<Value, String> {
    answers(app, json!({"op":"confirm","key":key,"quote":quote,"event_id":"E","option_id":option}))
}
fn rows(app: &mut Application) -> Vec<Value> {
    app.request("app_answers", &json!({"guide_id":GUIDE})).unwrap()["answers"].as_array().unwrap().clone()
}
fn step(app: &mut Application, key: &str) -> Value {
    let packet = app.projection(Some(GUIDE), false).unwrap();
    packet["guide"]["tracks"]["operations"].as_array().unwrap().iter().find(|s| s["key"] == key).unwrap().clone()
}
fn checks(app: &mut Application) -> Value {
    app.progress(Some(GUIDE), None, None).unwrap()["checks"].clone()
}

#[test]
fn confirm_stores_the_row_and_checks_a_single_reference_line() {
    let temp = Temp::new();
    let mut app = open(&temp);
    let result = confirm(&mut app, "single", "Mobilize The…", "E.b").unwrap();
    assert_eq!(result["previous"], Value::Null);
    assert_eq!(result["checked"], true);
    assert_eq!(checks(&mut app), json!({"single":true}));
    let stored = temp.load()["answers"].clone();
    assert_eq!(stored.as_array().unwrap().len(), 1);
    let row = &stored[0];
    assert_eq!((&row["guide_id"], &row["step_key"], &row["quote"]), (&json!(GUIDE), &json!("single"), &json!("Mobilize The…")));
    assert_eq!((&row["event_id"], &row["option_id"]), (&json!("E"), &json!("E.b")));
    assert!(row["confirmed_at"].as_str().is_some_and(|t| t.len() >= 19 && t.contains('T')), "{row}");
    let bound = step(&mut app, "single");
    assert!(bound.get("tracking").is_none(), "{bound}");
    assert_eq!(bound["action"]["params"]["reference_resolution"]["basis"], "user_confirmed");
    assert_eq!(rows(&mut app), stored.as_array().unwrap().clone());
}

#[test]
fn confirm_on_two_reference_line_stores_but_does_not_check() {
    let temp = Temp::new();
    let mut app = open(&temp);
    let result = confirm(&mut app, "pair", "Absolutely Not…", "E.a").unwrap();
    assert_eq!(result, json!({"previous":null,"checked":false}));
    assert_eq!(checks(&mut app), json!({}));
    assert_eq!(temp.load()["answers"].as_array().unwrap().len(), 1);
    let pair = step(&mut app, "pair");
    assert_eq!(pair["tracking"], "reminder");
    assert_eq!(pair["action"]["params"]["reference_candidates"]["confirmed"], json!({"event_id":"E","option_id":"E.a","basis":"user_confirmed"}));
    // A quote or option that is not this line's never stores anything.
    assert!(confirm(&mut app, "pair", "Other…", "E.a").is_err());
    assert!(confirm(&mut app, "pair", "Absolutely Not…", "E.zzz").is_err());
    assert_eq!(temp.load()["answers"].as_array().unwrap().len(), 1);
}

#[test]
fn restore_brings_back_the_replaced_row_and_unchecks() {
    let temp = Temp::new();
    let mut app = open(&temp);
    confirm(&mut app, "single", "Mobilize The…", "E.a").unwrap();
    let first = temp.load()["answers"][0].clone();
    let second = confirm(&mut app, "single", "Mobilize The…", "E.b").unwrap();
    assert_eq!(second["previous"], first, "the replaced row is returned for Undo");
    assert_eq!(temp.load()["answers"][0]["option_id"], "E.b");
    answers(&mut app, json!({"op":"restore","key":"single","quote":"Mobilize The…","previous":second["previous"]})).unwrap();
    assert_eq!(temp.load()["answers"], json!([first]));
    assert_eq!(checks(&mut app), json!({}));
    // Undo of a first confirmation deletes the row.
    answers(&mut app, json!({"op":"restore","key":"single","quote":"Mobilize The…","previous":null})).unwrap();
    assert_eq!(temp.load()["answers"], json!([]));
    assert_eq!(step(&mut app, "single")["tracking"], "reminder");
}

#[test]
fn confirmation_applies_to_a_new_run_of_the_same_guide() {
    let temp = Temp::new();
    {
        let mut app = open(&temp);
        confirm(&mut app, "single", "Mobilize The…", "E.b").unwrap();
    }
    // After a restart the rows come back with the progress.
    let mut app = open(&temp);
    assert_eq!(rows(&mut app).len(), 1);
    let new = app.progress(Some(GUIDE), None, Some(&json!({}))).unwrap();
    assert_eq!(new["checks"], json!({}), "a new campaign starts unchecked");
    let bound = step(&mut app, "single");
    assert!(bound.get("tracking").is_none(), "{bound}");
    assert_eq!(bound["action"]["params"]["reference_candidates"]["confirmed"]["option_id"], "E.b");
    // The bound line is a manual check in the new run.
    let checked = app.request("app_checks", &json!({"guide_id":GUIDE,"update":{"key":"single","checked":true}})).unwrap();
    assert_eq!(checked["checks"], json!({"single":true}));
}

#[test]
fn archive_payload_never_contains_confirmed() {
    let temp = Temp::new();
    let mut app = open(&temp);
    confirm(&mut app, "single", "Mobilize The…", "E.b").unwrap();
    confirm(&mut app, "pair", "Absolutely Not…", "E.a").unwrap();
    assert_eq!(step(&mut app, "pair")["action"]["params"]["reference_candidates"]["confirmed"]["option_id"], "E.a");
    let stored = temp.load();
    let definitions = stored["definitions"].as_object().unwrap();
    assert!(!definitions.is_empty());
    for payload in definitions.values() {
        let payload = payload.as_str().unwrap();
        assert!(!payload.contains("user_confirmed") && !payload.contains("\"confirmed\""), "archive holds a confirmation");
    }
    // The registry definition stays clean too.
    let registry = app.request("app_definition", &json!({"guide_id":GUIDE})).unwrap();
    assert!(!registry.to_string().contains("user_confirmed"));
}

#[test]
fn remove_deletes_the_row_and_the_line_is_ambiguous_again() {
    let temp = Temp::new();
    let mut app = open(&temp);
    confirm(&mut app, "single", "Mobilize The…", "E.b").unwrap();
    answers(&mut app, json!({"op":"remove","key":"single","quote":"Mobilize The…"})).unwrap();
    assert_eq!(temp.load()["answers"], json!([]));
    assert!(rows(&mut app).is_empty());
    let line = step(&mut app, "single");
    assert_eq!(line["tracking"], "reminder");
    assert!(line["action"]["params"]["reference_candidates"].get("confirmed").is_none());
}

fn package() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/hoi4companion")
}
fn http(port: u16, method: &str, target: &str, body: Option<&Value>) -> (u16, Value) {
    let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    let body = body.map(Value::to_string).unwrap_or_default();
    write!(socket, "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut response = Vec::new();
    socket.read_to_end(&mut response).unwrap();
    let response = String::from_utf8_lossy(&response).into_owned();
    let status = response.split(' ').nth(1).unwrap().parse().unwrap();
    (status, serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap())
}

#[test]
fn confirm_refused_when_no_visible_candidate_matches() {
    let temp = Temp::new();
    let package = package();
    let mut active = guide();
    active["default"] = json!(true);
    let config = json!({
        "path": temp.db(),
        "selection_path": temp.0.join("selection.json"),
        "web_root": package.join("web"),
        "reference_library": temp.0.join("references"),
        "guide_directory": temp.0.join("guides"),
        "association_directory": temp.0.join("associations"),
        "catalog": {"settings":{"game_dir":"","docs_dir":temp.0.join("documents"),"country":"GER"},
            "demo_file": package.join("examples/demo_focus.txt"),
            "demo_labels": package.join("examples/demo_labels.json")},
        "guides": [active],
    });
    std::fs::create_dir_all(temp.0.join("documents")).unwrap();
    let service = Service::open(&config).unwrap();
    let ready = temp.0.join("ready.json");
    let server = {
        let (service, ready) = (service.clone(), ready.clone());
        std::thread::spawn(move || serve_service(service, 0, Some(ready)))
    };
    let started = Instant::now();
    while !ready.exists() {
        assert!(started.elapsed() < Duration::from_secs(60), "the server never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    let port = serde_json::from_slice::<Value>(&std::fs::read(&ready).unwrap()).unwrap()["port"].as_u64().unwrap() as u16;
    let request = json!({"schema_version":1,"kind":"confirm_answer","token":1,"guide_id":GUIDE,"key":"single",
        "quote":"Mobilize The…","event_id":"E","option_id":"E.b"});
    // No game is observed, so no frame proposes a visible candidate.
    let (status, refused) = http(port, "POST", "/api/overlay", Some(&request));
    assert_ne!(status, 200, "{refused}");
    assert_eq!(refused["error"], "The event answer is no longer shown: nothing was confirmed");
    let (status, listed) = http(port, "GET", &format!("/api/guide/answers?id={GUIDE}"), None);
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["answers"], json!([]));
    let (_, progress) = http(port, "GET", &format!("/api/guide/progress?id={GUIDE}"), None);
    assert_eq!(progress["checks"], json!({}));
    service.close();
    let _ = TcpStream::connect(("127.0.0.1", port));
    server.join().unwrap().unwrap();
}
