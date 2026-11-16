//! Opt-in (HOI4_REAL_CATALOG=1): compile installed guides with the REAL game
//! catalog (read-only game files) under contract 1 (registry) and the current
//! contract for new checklists, and compare what becomes live-monitored.
#![cfg(windows)]
use hoi4companion_backend::request;
use serde_json::{Value, json};
use std::collections::BTreeMap;

struct Catalog(u64);
impl Drop for Catalog {
    fn drop(&mut self) {
        let _ = request(0, "catalog_release", json!({"handle":self.0}));
    }
}
fn steps(definition: &Value) -> Vec<Value> {
    definition["tracks"].as_object().into_iter().flat_map(|m| m.values()).flat_map(|v| v.as_array().cloned().unwrap_or_default()).collect()
}
fn automatic(step: &Value) -> bool {
    step["completion"].is_object() || step["started_when"].is_object()
}

#[test]
fn real_catalog_contract_comparison() {
    if std::env::var("HOI4_REAL_CATALOG").as_deref() != Ok("1") {
        return eprintln!("real catalog comparison skipped (set HOI4_REAL_CATALOG=1)");
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = root.join(".local");
    let settings: Value = serde_json::from_slice(&std::fs::read(data.join("settings.json")).unwrap()).unwrap();
    let guides = request(0, "guide_repository", json!({"directories":[data.join("guides")],"association_directory":data.join("guide-associations")})).unwrap();
    let library = request(0, "reference_library_manifest", json!({"root":data.join("reference-library")})).unwrap();
    let catalog = Catalog(request(0, "catalog_open", json!({"settings":settings,"vocabulary_path":root.join("src/hoi4companion/guide_language.json")})).unwrap().as_u64().unwrap());
    let entities = request(0, "catalog_entities", json!({"handle":catalog.0})).unwrap();
    let found: Vec<String> = entities.as_array().into_iter().flatten().filter(|e| e["id"] == "GER_mobilize_the_selbschutz")
        .map(|e| e.to_string().chars().take(600).collect()).collect();
    eprintln!("catalog entity GER_mobilize_the_selbschutz: {found:?} (entities shape: {})", entities.to_string().chars().take(200).collect::<String>());
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();
    for guide in guides["guides"].as_array().unwrap() {
        let old = request(0, "catalog_runtime_guide", json!({"handle":catalog.0,"guide":guide,"library":library})).unwrap()["definition"].clone();
        let new = request(0, "catalog_runtime_guide_new", json!({"handle":catalog.0,"guide":guide,"library":library})).unwrap();
        let (a, b) = (steps(&old), steps(&new));
        *totals.entry("steps_contract1").or_default() += a.len();
        *totals.entry("steps_new").or_default() += b.len();
        *totals.entry("automatic_contract1").or_default() += a.iter().filter(|s| automatic(s)).count();
        *totals.entry("automatic_new").or_default() += b.iter().filter(|s| automatic(s)).count();
        *totals.entry("reminders_contract1").or_default() += a.iter().filter(|s| s["tracking"] == "reminder").count();
        *totals.entry("reminders_new").or_default() += b.iter().filter(|s| s["tracking"] == "reminder").count();
        *totals.entry("live_conditions_new").or_default() += b.iter().filter(|s| s["action"]["params"]["decomposition"]["condition_status"] == "compiled").count();
        *totals.entry("withdrawn_decisions").or_default() += b.iter().filter(|s| s["action"]["params"]["binding_withdrawn"].is_object()).count();
        for (label, list) in [("contract1", &a), ("new", &b)] {
            let count = |key: &str| list.iter().filter(|s| !s["action"]["params"][key].is_null()).count();
            *totals.entry(if label == "new" { "events_grounded_new" } else { "events_grounded_contract1" }).or_default() += count("reference_resolution");
            *totals.entry(if label == "new" { "candidates_shown_new" } else { "candidates_shown_contract1" }).or_default() += count("reference_candidates");
            *totals.entry(if label == "new" { "withdrawn_new" } else { "withdrawn_contract1" }).or_default() += count("binding_withdrawn");
        }
        if guide["id"] == "word-67736ac669bdcbf1" {
            for s in a.iter().filter(|s| s["key"].as_str().is_some_and(|k| k.starts_with("word-0002") || k == "word-0008")) {
                eprintln!("SOV-C1 {} | targets={} | withdrawn={} | candidates={}", s["key"],
                    s["action"]["params"]["production_targets"].as_array().map(|t| t.iter().map(|x| format!("{}:{}", x["equipment_name"].as_str().unwrap_or(""), x["requested_factories"])).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
                    s["action"]["params"]["binding_withdrawn"]["id"], s["action"]["params"]["reference_candidates"].to_string().chars().take(300).collect::<String>());
            }
        }
        if guide["id"] == "word-67736ac669bdcbf1" {
            // Regressions from the user's review of the Soviet guide (existing checklists).
            let step = |k: &str| a.iter().find(|s| s["key"] == k).cloned();
            assert_eq!(step("word-0002").unwrap()["action"]["params"]["production_targets"].as_array().unwrap().len(), 7, "one production step");
            assert!(step("word-0002--operation-2").is_none(), "no split production memo");
            // The wrong German decision is gone; the reference is grounded to the
            // only answer the Soviet Union can receive.
            let mobilize = step("word-0008").unwrap();
            assert!(mobilize["action"]["params"]["decision_id"].is_null());
            assert_eq!(mobilize["action"]["params"]["event_option_id"], "bftb_turkey.4.a");
        }
        for s in a.iter().filter(|s| !s["action"]["params"]["reference_resolution"].is_null()) {
            eprintln!("GROUNDED {} {} -> {} «{}»", guide["applies_to"]["country"].as_str().unwrap_or(""), s["action"]["params"]["instructions"][0], s["action"]["params"]["reference_resolution"]["option_id"], s["action"]["params"]["reference_resolution"]["option_text"].as_str().unwrap_or(""));
        }
        let targets = |v: &[Value]| v.iter().map(|s| s["action"]["params"]["production_targets"].as_array().map_or(0, Vec::len)).sum::<usize>();
        *totals.entry("production_targets_contract1").or_default() += targets(&a);
        *totals.entry("production_targets_new").or_default() += targets(&b);
        if guide["id"] == "word-67736ac669bdcbf1" {
            let s8 = b.iter().find(|s| s["key"] == "word-0008").cloned().unwrap_or_default();
            eprintln!("SOV word-0008 when={} timing={} params={}", s8["when"], s8["action"]["params"]["timing"], s8["action"]["params"].to_string().chars().take(400).collect::<String>());
            for s in &b {
                let n: u64 = s["key"].as_str().unwrap()[5..9].parse().unwrap_or(99);
                if n <= 4 || [8, 9, 13].contains(&n) {
                    eprintln!("SOV {} | {} | tracking={} | completion={} | targets={} | withdrawn={}", s["key"], s["title"],
                        s["tracking"].as_str().unwrap_or("-"), s["completion"].to_string().chars().take(140).collect::<String>(),
                        s["action"]["params"]["production_targets"].as_array().map(|t| t.iter().map(|x| format!("{}:{}", x["equipment_name"].as_str().unwrap_or(""), x["requested_factories"])).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
                        s["action"]["params"]["binding_withdrawn"]["id"]);
                }
            }
        }
    }
    eprintln!("real catalog comparison: {totals:#?}");
}

#[test]
fn truncated_reference_survey() {
    if std::env::var("HOI4_REAL_CATALOG").as_deref() != Ok("1") {
        return eprintln!("truncated reference survey skipped (set HOI4_REAL_CATALOG=1)");
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = root.join(".local");
    let settings: Value = serde_json::from_slice(&std::fs::read(data.join("settings.json")).unwrap()).unwrap();
    let guides = request(0, "guide_repository", json!({"directories":[data.join("guides")],"association_directory":data.join("guide-associations")})).unwrap();
    let catalog = Catalog(request(0, "catalog_open", json!({"settings":settings,"vocabulary_path":root.join("src/hoi4companion/guide_language.json")})).unwrap().as_u64().unwrap());
    let mut buckets: BTreeMap<String, usize> = BTreeMap::new();
    let mut samples: Vec<String> = vec![];
    for guide in guides["guides"].as_array().unwrap() {
        let country = &guide["applies_to"]["country"];
        let scope = &guide["applies_to"]["tree_id"];
        for step in steps(guide) {
            let p = &step["action"]["params"];
            let Some(text) = p["instructions"][0].as_str().or(step["title"].as_str()) else { continue };
            let text = text.trim();
            if text.contains('\n') || !(text.ends_with('…') || text.ends_with("...")) || text.len() > 80 {
                continue;
            }
            let found = request(0, "catalog_reference_candidates", json!({"handle":catalog.0,"text":text,"country":country,"scope":scope})).unwrap();
            let all = found.as_array().unwrap();
            let compatible: Vec<&Value> = all.iter().filter(|c| c["compatible"] != false).collect();
            let event = request(0, "catalog_event_command", json!({"handle":catalog.0,"text":text,"country":country})).unwrap();
            *buckets.entry(format!("event_command_{}", if event.is_null() { "none" } else { "bound" })).or_default() += 1;
            let bound = ["decision_id", "advisor_id", "law_id", "idea_id"].iter().any(|k| p[*k].is_string()) || step["action"]["entity"]["id"].is_string();
            let bucket = format!("{}|{}", if bound { "bound" } else { "unbound" }, match compatible.len() { 0 => "0", 1 => "1", 2..=3 => "2-3", _ => "4+" });
            *buckets.entry(bucket).or_default() += 1;
            if samples.len() < 80 && (text.starts_with("Mobilize") || samples.len() < 60) {
                samples.push(format!("{} {} [{}] event_cmd={} bound={} -> {}", guide["applies_to"]["country"].as_str().unwrap_or("?"), text,
                    p["decision_id"].as_str().or(step["action"]["entity"]["id"].as_str()).unwrap_or("-"), event["params"]["event_option_id"], bound,
                    compatible.iter().take(5).map(|c| format!("{}:{}({})", c["kind"].as_str().unwrap_or(""), c["id"].as_str().unwrap_or(""), c["name"].as_str().unwrap_or(""))).collect::<Vec<_>>().join(" | ")));
            }
        }
    }
    eprintln!("truncated references by binding / compatible candidates: {buckets:#?}");
    for s in samples {
        eprintln!("REF {s}");
    }
}

#[test]
fn reference_probe() {
    if std::env::var("HOI4_REAL_CATALOG").as_deref() != Ok("1") {
        return;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = root.join(".local");
    let settings: Value = serde_json::from_slice(&std::fs::read(data.join("settings.json")).unwrap()).unwrap();
    let catalog = Catalog(request(0, "catalog_open", json!({"settings":settings,"vocabulary_path":root.join("src/hoi4companion/guide_language.json")})).unwrap().as_u64().unwrap());
    let entities = request(0, "catalog_entities", json!({"handle":catalog.0})).unwrap();
    let rows = entities["entries"].as_array().unwrap();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for r in rows { *kinds.entry(r["kind"].as_str().unwrap_or("?").to_owned()).or_default() += 1; }
    eprintln!("PROBE kinds {kinds:?}");
    for kind in ["event", "event_option", "decision"] {
        if let Some(r) = rows.iter().find(|r| r["kind"] == kind) {
            eprintln!("PROBE sample {kind}: {}", r.to_string().chars().take(700).collect::<String>());
        }
    }
    for (text, country, scope) in [("We will seize…", "ITA", "italian_focus"), ("We will seize…", "GER", "german_focus"), ("Mobilize The…", "SOV", "soviet_focus")] {
        let found = request(0, "catalog_reference_candidates", json!({"handle":catalog.0,"text":text,"country":country,"scope":scope})).unwrap();
        for c in found.as_array().unwrap().iter().take(8) {
            let full = rows.iter().find(|r| r["id"] == c["id"] && r["kind"] == c["kind"]);
            let parent = c["event_id"].as_str().and_then(|e| rows.iter().find(|r| r["kind"] == "event" && r["id"] == e));
            eprintln!("PROBE {country} {text:?} -> {} {} | row={} | parent={}", c["kind"], c["id"],
                full.map(|r| r.to_string().chars().take(400).collect::<String>()).unwrap_or_default(),
                parent.map(|r| r.to_string().chars().take(400).collect::<String>()).unwrap_or_default());
        }
    }
}
