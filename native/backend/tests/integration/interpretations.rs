//! The player's reading of a line followed by hand (UX R13, stage A), over
//! HTTP: saved apart from the guide, attached to the projection after the
//! evaluation, refused for bound or changed lines, removed on request. The
//! evaluation is identical before and after. Temporary directories only.
#![cfg(windows)]
use hoi4companion_backend::http::{Service, serve_service};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const GUIDE: &str = "lines";

fn guide() -> Value {
    let source = |n: u64, line: &str| json!({"source_action":{"source_id":"s","key":"k","ordinal":n},"category":"checkpoints","instructions":[line]});
    json!({"id":GUIDE,"title":"Lines","default":true,"applies_to":{"country":"AUS"},"tracks":{"operations":[
        {"key":"raw","title":"Def against YUG","action":{"kind":"manual","params":source(1,"Def against YUG")}},
        {"key":"note","title":"Save PP","tracking":"reminder","action":{"kind":"manual","params":source(2,"Save PP")}},
    ]}})
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
fn step<'a>(projection: &'a Value, key: &str) -> &'a Value {
    projection["guide"]["tracks"]["operations"].as_array().unwrap().iter().find(|s| s["key"] == key).unwrap()
}

#[test]
fn an_interpretation_is_presentation_only_and_follows_its_line() {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("hoi4-interpretations-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(root.join("documents")).unwrap();
    let package = package();
    let config = json!({
        "path": root.join("progress.sqlite3"),
        "selection_path": root.join("selection.json"),
        "web_root": package.join("web"),
        "reference_library": root.join("references"),
        "guide_directory": root.join("guides"),
        "association_directory": root.join("associations"),
        "catalog": {"settings":{"game_dir":"","docs_dir":root.join("documents"),"country":"AUS"},
            "demo_file": package.join("examples/demo_focus.txt"),
            "demo_labels": package.join("examples/demo_labels.json")},
        "guides": [guide()],
    });
    let service = Service::open(&config).unwrap();
    let ready = root.join("ready.json");
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

    let (status, listed) = http(port, "GET", &format!("/api/guide/interpretations?id={GUIDE}"), None);
    assert_eq!(status, 200, "{listed}");
    let keys: Vec<_> = listed["steps"].as_array().unwrap().iter().map(|s| s["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["raw"], "a reminder is not offered: {listed}");
    assert_eq!(listed["steps"][0]["verification"], "uninterpreted");
    let (_, before) = http(port, "GET", &format!("/api/guide/projection?id={GUIDE}&live=0"), None);

    let save = |interpretation: Value, expected: &str| http(port, "POST", "/api/guide/interpretations",
        Some(&json!({"guide_id":GUIDE,"key":"raw","expected":expected,"interpretation":interpretation})));
    let (status, refused) = save(json!({"category":"army"}), "Defend against YUG");
    assert_ne!(status, 200, "a changed line is refused: {refused}");
    let (status, refused) = save(json!({"category":"army","target":{"kind":"focus","id":"NOT_A_FOCUS"}}), "Def against YUG");
    assert_ne!(status, 200, "an unknown target is refused: {refused}");
    let (status, refused) = save(json!({"category":"army","target":{"kind":"focus","id":"DEMO_foundations","name":"Spoofed"}}), "Def against YUG");
    assert_ne!(status, 200, "the name comes from the catalog only: {refused}");
    let (status, refused) = http(port, "POST", "/api/guide/interpretations",
        Some(&json!({"guide_id":GUIDE,"key":"note","expected":"Save PP","interpretation":{"category":"politics"}})));
    assert_ne!(status, 200, "a reminder stays a reminder: {refused}");

    let (status, saved) = save(json!({"category":"army","target":{"kind":"focus","id":"DEMO_foundations"}}), "Def against YUG");
    assert_eq!(status, 200, "{saved}");
    let interpretation = &saved["steps"][0]["interpretation"];
    assert_eq!(interpretation["category"], "army");
    assert_eq!(interpretation["target"]["id"], "DEMO_foundations");
    assert!(interpretation["target"]["name"].as_str().is_some_and(|n| !n.is_empty() && n != "Spoofed"), "{interpretation}");
    assert_eq!(saved["steps"][0]["verification"], "manual", "interpreted, still confirmed by hand");
    assert_eq!(saved["steps"][0]["category"], "army");

    let (_, after) = http(port, "GET", &format!("/api/guide/projection?id={GUIDE}&live=0"), None);
    assert_eq!(step(&after, "raw")["interpretation"], *interpretation);
    assert_eq!(after["evaluation"], before["evaluation"], "the evaluation never changes");
    assert_eq!(after["definition_revision"], before["definition_revision"], "the definition never changes");
    assert!(step(&after, "note").get("interpretation").is_none());

    let (status, removed) = save(Value::Null, "Def against YUG");
    assert_eq!(status, 200, "{removed}");
    assert_eq!(removed["steps"][0]["interpretation"], Value::Null);
    assert_eq!(removed["steps"][0]["verification"], "uninterpreted");
    let (_, cleared) = http(port, "GET", &format!("/api/guide/projection?id={GUIDE}&live=0"), None);
    assert!(step(&cleared, "raw").get("interpretation").is_none());

    service.close();
    let _ = TcpStream::connect(("127.0.0.1", port));
    server.join().unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
