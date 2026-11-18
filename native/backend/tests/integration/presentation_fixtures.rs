//! Presentation fixtures for the browser tests. Each case in
//! `tests/fixtures/presentation-cases.json` is evaluated by the production Rust
//! boundary and its hand-written `expect` values are asserted here. The full
//! evaluations are then compared with `tests/fixtures/presentation-evaluations.json`,
//! which the JavaScript tests read instead of evaluating guides themselves.
//!
//! A difference fails the test. To accept a reviewed change, rerun this test
//! with `HOI4_UPDATE_PRESENTATION_FIXTURES=1` and review the file's git diff.
use hoi4companion_backend::request;
use serde_json::{Map, Value, json};
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn evaluate(case: &Value) -> Value {
    let name = &case["name"];
    let guide = request(0, "guide_normalize", json!({"guide":case["guide"]}))
        .unwrap_or_else(|error| panic!("{name}: guide rejected: {error}"));
    request(
        0,
        "guide_evaluate",
        json!({"guide":guide,"state":case["state"],"progress":case["progress"]}),
    )
    .unwrap_or_else(|error| panic!("{name}: evaluation failed: {error}"))
}

#[test]
fn presentation_evaluations_match_expectations_and_stored_fixture() {
    let cases: Vec<Value> =
        serde_json::from_slice(&std::fs::read(fixtures().join("presentation-cases.json")).unwrap())
            .unwrap();
    let mut generated = Map::new();
    for case in &cases {
        let name = case["name"].as_str().expect("case name");
        let value = evaluate(case);
        for (pointer, expected) in case["expect"].as_object().expect("expect object") {
            assert_eq!(
                value.pointer(pointer),
                Some(expected),
                "{name}: {pointer}"
            );
        }
        // Evaluation must be a pure function of its inputs.
        assert_eq!(evaluate(case), value, "{name}: repeated evaluation differs");
        assert!(
            generated.insert(name.to_owned(), value).is_none(),
            "duplicate case {name}"
        );
    }
    let text = serde_json::to_string_pretty(&Value::Object(generated.clone())).unwrap() + "\n";
    let path = fixtures().join("presentation-evaluations.json");
    if std::env::var_os("HOI4_UPDATE_PRESENTATION_FIXTURES").is_some() {
        std::fs::write(&path, text).unwrap();
        eprintln!("Rewrote {}; review its git diff before keeping it.", path.display());
        return;
    }
    let stored = std::fs::read_to_string(&path).unwrap_or_default();
    if stored != text {
        let previous: Map<String, Value> = serde_json::from_str(&stored).unwrap_or_default();
        let mut changed: Vec<&str> = generated
            .iter()
            .filter(|(name, value)| previous.get(*name) != Some(*value))
            .map(|(name, _)| name.as_str())
            .collect();
        changed.extend(
            previous
                .keys()
                .filter(|name| !generated.contains_key(*name))
                .map(String::as_str),
        );
        panic!(
            "Stored presentation evaluations differ from Rust for: {changed:?}. \
             If the change is intended, rerun with HOI4_UPDATE_PRESENTATION_FIXTURES=1 \
             and review the git diff of {}.",
            path.display()
        );
    }
}
