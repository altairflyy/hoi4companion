//! Startup answers before every guide is prepared: the active guide is
//! prepared first, the others afterwards. A guide still being prepared is
//! reported as such and is never evaluated (so never shown as done).
use hoi4companion_backend::http::{Service, serve_service};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn package() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/hoi4companion")
}

fn guide(id: &str, title: &str, default: bool) -> Value {
    json!({"id":id,"title":title,"default":default,
        "tracks":{"operations":[{"key":"note","title":"Note","action":{"kind":"manual"}}]}})
}

fn get(port: u16, target: &str) -> (u16, Value) {
    let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    write!(socket, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").unwrap();
    let mut response = Vec::new();
    socket.read_to_end(&mut response).unwrap();
    let response = String::from_utf8_lossy(&response).into_owned();
    let status = response.split(' ').nth(1).unwrap().parse().unwrap();
    let body = response.split_once("\r\n\r\n").unwrap().1;
    (status, serde_json::from_str(body).unwrap())
}

#[test]
fn the_server_answers_for_the_active_guide_before_the_other_guides_are_prepared() {
    let root = std::env::temp_dir().join(format!("hoi4-startup-preparation-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("documents")).unwrap();
    let package = package();
    let config = json!({
        "path": root.join("progress.sqlite3"),
        "selection_path": root.join("selection.json"),
        "web_root": package.join("web"),
        "reference_library": root.join("references"),
        "guide_directory": root.join("guides"),
        "association_directory": root.join("associations"),
        // Cold compiled cache: no cache root, every guide is prepared.
        "catalog": {"settings":{"game_dir":"","docs_dir":root.join("documents"),"country":"AUS"},
            "demo_file": package.join("examples/demo_focus.txt"),
            "demo_labels": package.join("examples/demo_labels.json")},
        // Test hook: the other guides wait for `prepare_pending_guides`.
        "guide_preparation": "manual",
        "guides": [guide("first", "First", false), guide("active", "Active", true), guide("third", "Third", false)],
    });
    let service = Service::open(&config).unwrap();
    let ready = root.join("ready.json");
    let server = {
        let (service, ready) = (service.clone(), ready.clone());
        std::thread::spawn(move || serve_service(service, 0, Some(ready)))
    };
    let started = Instant::now();
    while !ready.exists() {
        assert!(started.elapsed() < Duration::from_secs(30), "the server never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    let port = serde_json::from_slice::<Value>(&std::fs::read(&ready).unwrap()).unwrap()["port"]
        .as_u64().unwrap() as u16;

    let (status, active) = get(port, "/api/guide/projection?id=active&live=0");
    assert_eq!(status, 200, "{active}");
    assert_eq!(active["engine"], "rust-guide-core");
    assert!(active["evaluation"].is_object());
    assert_eq!(active["guide"]["id"], "active");

    for id in ["first", "third"] {
        let (status, pending) = get(port, &format!("/api/guide/projection?id={id}&live=0"));
        assert_eq!(status, 400, "a guide still being prepared is never evaluated: {pending}");
        assert!(pending.get("evaluation").is_none());
        assert_eq!(pending["error"], "Guide still being prepared: try again in a few seconds", "{pending}");
    }
    let (status, guides) = get(port, "/api/guides");
    assert_eq!(status, 200, "{guides}");
    let listed = |guides: &Value, id: &str| guides.to_string().contains(&format!("\"id\":\"{id}\""));
    assert!(listed(&guides, "active"));
    assert!(!listed(&guides, "first"), "a guide still being prepared cannot be picked: {guides}");

    service.prepare_pending_guides();
    for id in ["first", "third"] {
        let (status, prepared) = get(port, &format!("/api/guide/projection?id={id}&live=0"));
        assert_eq!(status, 200, "{prepared}");
        assert_eq!(prepared["engine"], "rust-guide-core");
        assert!(prepared["evaluation"].is_object());
    }
    let (_, guides) = get(port, "/api/guides");
    assert!(listed(&guides, "first") && listed(&guides, "third"), "{guides}");

    service.close();
    let _ = TcpStream::connect(("127.0.0.1", port));
    server.join().unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
