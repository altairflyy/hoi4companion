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
