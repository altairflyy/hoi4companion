//! Offline timing of the progress store on a copy of a real database.
//! `HOI4_PROFILE_PROGRESS=<copy.sqlite3> node scripts/dev-native.cjs test guides --test profile_progress -- --ignored --nocapture`
use hoi4companion_guides::progress_store::Runs;
use serde_json::json;
use std::time::Instant;

#[test]
#[ignore]
fn profile_progress_store() {
    let Ok(path) = std::env::var("HOI4_PROFILE_PROGRESS") else { return };
    let t = Instant::now();
    let mut runs = Runs::open(Some(path.into())).unwrap();
    println!("open {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    let metadata = runs.call("metadata", json!({})).unwrap();
    println!("metadata {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let ids: Vec<String> = metadata.as_object().unwrap().keys().cloned().collect();
    let t = Instant::now();
    for id in &ids {
        runs.call("get", json!({"guide_id":id})).unwrap();
    }
    println!("get x{} {:.1} ms", ids.len(), t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    runs.call("records", json!({})).unwrap();
    println!("records {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let id = &ids[0];
    let record = runs.call("get", json!({"guide_id":id})).unwrap();
    for i in 0..3 {
        let mut checks = record["checks"].clone();
        checks[format!("__profile_{i}")] = json!(true);
        let t = Instant::now();
        runs.call("update", json!({"guide_id":id,"checks":checks})).unwrap();
        println!("update+persist {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    }
}
