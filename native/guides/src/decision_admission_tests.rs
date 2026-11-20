//! Unit-only import admission contracts. No user data, progress store or game process.
use crate::{import_catalog::Index, import_preview, recipient};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Data derived from third-party guides is local only (`.local/guide-corpus`, or
/// the folder in HOI4_GUIDE_CORPUS) and never committed: without it the test is skipped.
fn corpus(name: &str) -> Option<Value> {
    let dir = std::env::var_os("HOI4_GUIDE_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/guide-corpus"));
    match std::fs::read(dir.join(name)) {
        Ok(bytes) => Some(serde_json::from_slice(&bytes).unwrap()),
        Err(_) => {
            eprintln!("local guide corpus absent ({name}): skipped");
            None
        }
    }
}
fn cases() -> Option<Value> {
    corpus("guide-binding-probe-cases-v1.json")
}
fn index(entries: &Value) -> Index {
    Index::new(&json!({"entries":entries})).unwrap()
}
fn input(text: &str, country: &str, params: Value) -> (Value, Value) {
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    let source = json!({"id":format!("src-{}",&digest[..16]),"type":"file","title":"Source","text":text,"content_sha256":digest});
    let guide = json!({"id":"admission-test","title":"Admission test","dependency_policy":"explicit","applies_to":{"country":country},"sources":[source],"tracks":{"operations":[{"key":"word-0001","tracking":"reminder","evidence":[{"source_id":source["id"],"basis":"explicit","quote":text}],"action":{"kind":"manual","params":params}}]}});
    (guide, json!([source]))
}
fn preview(guide: &Value, sources: &Value, entries: &Value) -> Value {
    import_preview::preview(guide, sources, &index(entries), &json!({}), &Value::Null)
}
fn step(report: &Value) -> &Value {
    &report["guide"]["tracks"]["operations"][0]
}

#[test]
fn decision_admission_weak_english_soviet_and_french_names_need_revision_without_replacement() {
    let Some(cases) = cases() else { return };
    for c in cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["expected_shadow_status"] == "requires_review")
    {
        let (guide, sources) = input(
            c["query"].as_str().unwrap(),
            c["country"].as_str().unwrap(),
            json!({"instructions":[c["query"]]}),
        );
        let r = preview(&guide, &sources, &c["entries"]);
        assert_eq!(r["valid"], false, "{}: {}", c["query"], r);
        assert_eq!(step(&r)["action"]["params"]["decision_id"], c["saved_id"]);
        assert_eq!(r["decision_validation"][0]["status"], "ambiguous");
        assert_eq!(r["decision_validation"][0]["ownership"], "unknown");
        assert_eq!(
            step(&r)["evidence"],
            guide["tracks"]["operations"][0]["evidence"]
        );
        assert_eq!(step(&r)["key"], "word-0001");
    }
}

#[test]
fn decision_admission_inference_survives_serialization_and_repreview() {
    let Some(cases) = cases() else { return };
    let c = &cases["cases"][0];
    let (guide, sources) = input(
        c["query"].as_str().unwrap(),
        "ENG",
        json!({"instructions":[c["query"]]}),
    );
    let mut r = preview(&guide, &sources, &c["entries"]);
    for _ in 0..3 {
        let mut compiled: Value =
            serde_json::from_str(&serde_json::to_string(&r["guide"]).unwrap()).unwrap();
        compiled["reviewed"] = json!(true);
        r = preview(&compiled, &sources, &c["entries"]);
        assert_eq!(r["valid"], false);
        let p = &step(&r)["action"]["params"]["decision_resolution"];
        assert_eq!(p["origin"], "inferred");
        assert_eq!(p["query"], c["query"]);
        assert_eq!(r["decision_validation"][0]["status"], "ambiguous");
    }
}

#[test]
fn decision_admission_legacy_ids_are_unknown_not_historical_explicit_choices() {
    let Some(cases) = cases() else { return };
    let c = &cases["cases"][0];
    let (mut guide, sources) = input("We shall…", "ENG", json!({"decision_id":c["saved_id"]}));
    guide["tracks"]["operations"][0]
        .as_object_mut()
        .unwrap()
        .remove("tracking");
    let r = preview(&guide, &sources, &c["entries"]);
    assert_eq!(r["valid"], true, "{r}");
    assert_eq!(
        step(&r)["action"]["params"]["decision_resolution"]["origin"],
        "legacy_unknown"
    );
    assert_eq!(r["decision_validation"][0]["ownership"], "unknown");
    assert!(!r["warnings"].as_array().unwrap().is_empty());
}

#[test]
fn decision_admission_explicit_and_reviewed_ids_are_preserved_but_not_proof_of_ownership() {
    let Some(cases) = cases() else { return };
    let c = &cases["cases"][0];
    for origin in ["explicit_id", "user_reviewed"] {
        let (mut guide, sources) = input(
            "We shall…",
            "ENG",
            json!({"decision_id":c["saved_id"],"decision_resolution":{"schema_version":1,"id":c["saved_id"],"origin":origin,"query":"We shall…"}}),
        );
        guide["tracks"]["operations"][0]
            .as_object_mut()
            .unwrap()
            .remove("tracking");
        let r = preview(&guide, &sources, &c["entries"]);
        assert_eq!(r["valid"], true, "{r}");
        assert_eq!(step(&r)["action"]["params"]["decision_id"], c["saved_id"]);
        assert_eq!(r["decision_validation"][0]["ownership"], "unknown");
        assert_eq!(
            step(&r)["action"]["params"]["decision_resolution"]["origin"],
            origin
        );
    }
}

#[test]
fn decision_admission_foreign_mines_carlist_abbreviations_and_exact_names_remain_usable() {
    let Some(cases) = cases() else { return };
    for c in cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["expected_shadow_status"] != "requires_review")
    {
        let (guide, sources) = input(
            c["query"].as_str().unwrap(),
            c["country"].as_str().unwrap(),
            json!({"instructions":[c["query"]]}),
        );
        let r = preview(&guide, &sources, &c["entries"]);
        assert_eq!(r["valid"], true, "{}: {}", c["query"], r);
        assert_eq!(step(&r)["action"]["params"]["decision_id"], c["saved_id"]);
    }
    let c = &cases["cases"][0];
    let name = c["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "decision")
        .unwrap()["name"]
        .as_str()
        .unwrap()
        .to_owned();
    let (guide, sources) = input(&name, "ENG", json!({"instructions":[name]}));
    assert_eq!(preview(&guide, &sources, &c["entries"])["valid"], true);
}

#[test]
fn decision_admission_known_tag_conflict_is_rejected_without_rewriting_id() {
    let (mut guide, sources) = input("Decision", "ENG", json!({"decision_id":"foreign"}));
    guide["tracks"]["operations"][0]
        .as_object_mut()
        .unwrap()
        .remove("tracking");
    let entries = json!([{"kind":"decision","id":"foreign","name":"Decision","country_condition":{"op":"all","args":[{"op":"tag","value":"SOV"},{"op":"unknown"}]}}]);
    let r = preview(&guide, &sources, &entries);
    assert_eq!(r["valid"], false);
    assert_eq!(r["decision_validation"][0]["ownership"], "incompatible");
    assert_eq!(r["decision_validation"][0]["status"], "rejected");
    assert_eq!(step(&r)["action"]["params"]["decision_id"], "foreign");
}

#[test]
fn decision_admission_changed_id_cannot_reuse_old_inference_provenance() {
    let Some(cases) = cases() else { return };
    let c = &cases["cases"][0];
    let (mut guide, sources) = input(
        "We shall…",
        "ENG",
        json!({"decision_id":c["saved_id"],"decision_resolution":{"schema_version":1,"id":"different","origin":"inferred","query":"We shall…"}}),
    );
    guide["tracks"]["operations"][0]
        .as_object_mut()
        .unwrap()
        .remove("tracking");
    let r = preview(&guide, &sources, &c["entries"]);
    assert_eq!(r["valid"], false);
    assert_eq!(
        r["decision_validation"][0]["reason"],
        "invalid_decision_provenance"
    );
}

#[test]
fn decision_admission_recipient_already_retains_allowed_original_tag_and_nested_logic() {
    let parsed=serde_json::to_value(hoi4companion_parser::parse(b"allowed = { OR = { original_tag = SOV AND = { tag = ENG NOT = { original_tag = GER } } } } available = { controls_state = 219 }").unwrap()).unwrap();
    let condition = recipient::political(&[&parsed]);
    assert!(condition.to_string().contains("original_tag"));
    assert!(condition.to_string().contains("unknown"));
    assert!(condition.to_string().contains("not"));
}

#[test]
fn decision_admission_nested_ownership_keeps_unknown_and_original_country_separate() {
    use crate::decision_admission::ownership;
    let parsed=serde_json::to_value(hoi4companion_parser::parse(b"allowed = { OR = { original_tag = SOV AND = { tag = ENG NOT = { original_tag = GER } } } }").unwrap()).unwrap();
    let tree = recipient::political(&[&parsed]);
    assert_eq!(ownership(&tree, Some("ENG"), None, 0), None);
    assert_eq!(ownership(&tree, Some("ENG"), Some("ENG"), 0), Some(true));
    assert_eq!(ownership(&tree, Some("SOV"), Some("ENG"), 0), Some(false));
    assert_eq!(ownership(&tree, Some("ENG"), Some("GER"), 0), Some(false));
    assert_eq!(ownership(&tree, None, Some("SOV"), 0), Some(true));
    assert_eq!(ownership(&tree, None, None, 0), None);
    let scoped = serde_json::to_value(
        hoi4companion_parser::parse(b"allowed = { SOV = { tag = SOV } }").unwrap(),
    )
    .unwrap();
    assert_eq!(
        ownership(
            &recipient::political(&[&scoped]),
            Some("ENG"),
            Some("ENG"),
            0
        ),
        None
    );
    let uncertain = json!({"op":"any","args":[{"op":"tag","value":"GER"},{"op":"unknown"}]});
    assert_eq!(ownership(&uncertain, Some("ENG"), None, 0), None);
    assert_eq!(
        ownership(&json!({"op":"not","arg":uncertain}), Some("ENG"), None, 0),
        None
    );
}

#[test]
fn decision_admission_cost_and_timing_suffixes_cannot_hide_a_weak_match() {
    let Some(cases) = cases() else { return };
    let c = &cases["cases"][0];
    for text in [
        "We shall… (50PP)",
        "ASAP: We shall… (50PP)",
        "We shall… (after war)",
    ] {
        let (guide, sources) = input(text, "ENG", json!({"instructions":[text]}));
        let r = preview(&guide, &sources, &c["entries"]);
        assert_eq!(r["valid"], false, "{r}");
        assert_eq!(r["decision_validation"][0]["status"], "ambiguous");
        assert_eq!(
            step(&r)["action"]["params"]["decision_resolution"]["query"],
            "We shall…"
        );
    }
}

#[test]
fn decision_admission_absent_context_is_unknown_and_target_country_is_not_actor() {
    let entries = json!([{"kind":"decision","id":"foreign","name":"Decision","country_condition":{"op":"original_tag","value":"GER"}}]);
    let (mut guide, sources) = input(
        "Decision",
        "ENG",
        json!({"decision_name":"Decision","target_country":"GER"}),
    );
    guide["tracks"]["operations"][0]
        .as_object_mut()
        .unwrap()
        .remove("tracking");
    guide["applies_to"] = json!({});
    let r = preview(&guide, &sources, &entries);
    assert_eq!(r["valid"], true, "{r}");
    assert_eq!(r["decision_validation"][0]["ownership"], "unknown");
    assert_eq!(step(&r)["action"]["params"]["target_country"], "GER");
}

#[test]
fn decision_admission_authoring_explicit_reference_is_recorded_at_its_actual_origin() {
    let (guide, sources) = input("Decision", "ENG", json!({}));
    let document = json!({"format":"hoi4-guide-authoring","authoring_schema_version":1,"sources":sources,"guide":{"id":"authored","title":"Authored","applies_to":{"country":"ENG"},"steps":[{"key":"d","type":"decision","target":{"id":"foreign"},"evidence":guide["tracks"]["operations"][0]["evidence"]}]}});
    let compiled = crate::authoring::compile(&document).unwrap();
    assert_eq!(
        compiled["guide"]["tracks"]["operations"][0]["action"]["params"]["decision_resolution"]["origin"],
        "explicit_id"
    );
}

#[test]
#[ignore = "requires HOI4_BINDING_SNAPSHOT frozen catalog; never a live game test"]
fn decision_admission_frozen_corpus_preserves_bindings_and_reports_new_import_impact() {
    let path = std::env::var("HOI4_BINDING_SNAPSHOT").expect("Set the frozen snapshot path");
    let snapshot: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        snapshot["benchmark_input"]["sha256"],
        "78c3f15d0632df92e28d818c5d6abd6698841a9302e556252a94467a4e76a901"
    );
    let mut args = snapshot["catalog"].clone();
    args["vocabulary"] = crate::import_catalog::language().clone();
    let index = Index::new(&args).unwrap();
    let mut decisions = 0;
    for item in snapshot["guides"].as_array().unwrap() {
        let original = &item["definition"];
        let mut guide = original.clone();
        let ids = crate::decision_admission::existing_ids(&guide);
        let audit = crate::decision_admission::validate(&mut guide, &ids, &index).unwrap();
        assert!(
            audit["errors"].as_array().unwrap().is_empty(),
            "{}: {}",
            item["id"],
            audit
        );
        decisions += audit["resolutions"].as_array().unwrap().len();
        // Admission adds provenance only: no key, action, condition, source or saved meaning changes.
        for lane in guide["tracks"].as_object_mut().unwrap().values_mut() {
            for row in lane.as_array_mut().unwrap() {
                if let Some(p) = row["action"]
                    .get_mut("params")
                    .and_then(Value::as_object_mut)
                {
                    p.remove("decision_resolution");
                }
            }
        }
        assert_eq!(&guide, original, "{}", item["id"]);
    }
    assert_eq!(decisions, 182);
    let Some(frozen) = corpus("guide-binding-safety-audit-v1.json") else { return };
    let mut changed = vec![];
    let mut unchanged = 0;
    for c in frozen["decision_shadow_policy"]["cases"]
        .as_array()
        .unwrap()
    {
        let (mut guide, sources) = input(
            c["source_text"].as_str().unwrap(),
            c["country"].as_str().unwrap(),
            json!({"decision_name":c["query"]}),
        );
        guide["tracks"]["operations"][0]
            .as_object_mut()
            .unwrap()
            .remove("tracking");
        let r = import_preview::preview(&guide, &sources, &index, &json!({}), &Value::Null);
        assert_eq!(
            step(&r)["action"]["params"]["decision_id"],
            c["saved_id"],
            "{}",
            c["query"]
        );
        let status = r["decision_validation"][0]["status"].as_str().unwrap();
        assert_ne!(status, "rejected", "{}: {}", c["query"], r);
        if status == "ambiguous" {
            changed.push((c["guide_id"].clone(), c["step_key"].clone()));
        } else {
            unchanged += 1;
            assert_eq!(r["valid"], true, "{}: {}", c["query"], r);
        }
    }
    let expected = frozen["decision_shadow_policy"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["narrow_shadow_admission"]["status"] == "requires_review")
        .map(|c| (c["guide_id"].clone(), c["step_key"].clone()))
        .collect::<Vec<_>>();
    assert_eq!(changed, expected);
    assert_eq!(unchanged, 178);
    println!(
        "Frozen corpus: 50 definitions preserved; 182 legacy decision IDs unchanged; new-name replay: {} ambiguous, 0 rejected, {unchanged} unchanged. Cases: {changed:?}",
        changed.len()
    );
}

#[test]
fn decision_admission_conditional_execution_cannot_prove_future_country_identity() {
    let (mut guide, sources) = input("Decision", "ENG", json!({"decision_id":"foreign"}));
    let row = &mut guide["tracks"]["operations"][0];
    row.as_object_mut().unwrap().remove("tracking");
    row["when"] = json!({"op":"fact","key":"country.tag","test":"eq","value":"GER"});
    let condition = row["when"].clone();
    let entries = json!([{"kind":"decision","id":"foreign","name":"Decision","country_condition":{"op":"tag","value":"GER"}}]);
    let r = preview(&guide, &sources, &entries);
    assert_eq!(r["valid"], true, "{r}");
    assert_eq!(r["decision_validation"][0]["ownership"], "unknown");
    assert_eq!(step(&r)["when"], condition);
}
