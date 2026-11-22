//! Offline timing of one guide's observation and evaluation on a projection
//! saved from a running app (`GET /api/guide/projection`: guide, state,
//! progress). HUD plan part 2 step 4b. Never commit the saved projection.
//! `HOI4_PROFILE_PROJECTION=<projection.json> node scripts/dev-native.cjs test guides --test profile_evaluation -- --ignored --nocapture`
use hoi4companion_guides::{guide_observe, guide_prepare, guide_runtime};
use hoi4companion_model::state;
use serde_json::{Value, json};
use std::time::Instant;

fn timed<T>(label: &str, runs: u32, mut work: impl FnMut() -> T) -> T {
    let mut result = None;
    let t = Instant::now();
    for _ in 0..runs {
        result = Some(work());
    }
    println!("{label:<32} {:8.1} ms", t.elapsed().as_secs_f64() * 1e3 / runs as f64);
    result.unwrap()
}

#[test]
#[ignore]
fn profile_guide_evaluation() {
    let Ok(path) = std::env::var("HOI4_PROFILE_PROJECTION") else { return };
    let projection: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let (guide, live, progress) = (&projection["guide"], &projection["state"], &projection["progress"]);
    println!("guide {} KB, state {} KB, progress {} KB", guide.to_string().len() / 1024, live.to_string().len() / 1024, progress.to_string().len() / 1024);
    let normalized = timed("state::normalize", 5, || state::normalize(live));
    println!("facts {}", normalized["facts"].as_object().map_or(0, |m| m.len()));
    timed("clone live", 5, || live.clone());
    timed("clone guide", 5, || guide.clone());
    timed("observe (no research)", 5, || guide_observe::observe(guide, live, progress, &json!({}), false).unwrap());
    timed("observe (research)", 5, || guide_observe::observe(guide, live, progress, &json!({}), true).unwrap());
    let prepared = timed("guide_prepare::prepare", 5, || guide_prepare::prepare(guide, live, progress).unwrap());
    timed("prepared definition to_string", 5, || prepared[0].to_string());
    timed("evaluate (first, compiles)", 1, || guide_runtime::evaluate(guide, live, progress).unwrap());
    timed("evaluate (cached graph)", 5, || guide_runtime::evaluate(guide, live, progress).unwrap());
}
