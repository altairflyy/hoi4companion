//! Grounded trigger resolution (compiler contract 3) measured on the real
//! installed guides (read-only, opt-in by presence) and on development cases.
//! Holdout is not read here.
#![cfg(windows)]
use hoi4companion_backend::decomposition::decompose;
use hoi4companion_backend::entity_resolution::{guide_candidates, resolve, trigger_condition};
use hoi4companion_backend::request;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn unknown(_: &str) -> Result<Option<Value>, String> {
    Ok(None)
}
fn installed() -> Vec<Value> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/guides");
    let Ok(entries) = std::fs::read_dir(&dir) else { return vec![] };
    entries
        .filter_map(|e| {
            let path = e.ok()?.path();
            (path.extension()? == "json").then(|| {
                let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                request(0, "guide_normalize", json!({"guide":raw})).unwrap()
            })
        })
        .collect()
}

#[test]
fn installed_corpus_trigger_resolution_report() {
    let guides = installed();
    if guides.is_empty() {
        return eprintln!("installed corpus absent: skipped");
    }
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut samples: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for guide in &guides {
        let titles: BTreeMap<String, String> = guide["tracks"].as_object().unwrap().values().flat_map(|v| v.as_array().unwrap())
            .filter_map(|s| Some((s["action"]["entity"]["id"].as_str()?.to_owned(), s["title"].as_str().unwrap_or("").to_owned())))
            .collect();
        let out = decompose(guide, &unknown).unwrap();
        let mut seen = std::collections::HashSet::new();
        for row in out["guide"]["tracks"].as_object().unwrap().values().flat_map(|v| v.as_array().unwrap()) {
            let d = &row["action"]["params"]["decomposition"];
            if d.is_null() || d["trigger"].is_null() {
                continue;
            }
            *totals.entry(format!("children_{}", d["condition_status"].as_str().unwrap())).or_default() += 1;
            let t = &d["trigger"];
            // Count each distinct trigger text once per guide.
            if !seen.insert(t["text"].as_str().unwrap().to_owned()) {
                continue;
            }
            let status = t["resolution"]["status"].as_str().unwrap_or("timing_rules");
            let key = format!("triggers_{}_{}", status, t["verification"].as_str().unwrap_or(""));
            *totals.entry(key.clone()).or_default() += 1;
            let line = match status {
                "resolved" => format!("{} => {} [{}] ({})", t["text"], titles.get(t["resolution"]["entity"]["id"].as_str().unwrap()).cloned().unwrap_or_default(),
                    t["resolution"]["entity"]["id"].as_str().unwrap(), t["relation"].as_str().unwrap_or("")),
                "ambiguous" => format!("{} => {}", t["text"], t["resolution"]["candidates"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect::<Vec<_>>().join(" | ")),
                _ => t["text"].to_string(),
            };
            let bucket = samples.entry(status.to_owned()).or_default();
            if bucket.len() < 60 {
                bucket.push(line);
            }
        }
    }
    eprintln!("contract-3 trigger report: {totals:#?}");
    for (status, lines) in &samples {
        eprintln!("--- {status} ---");
        for line in lines {
            eprintln!("{line}");
        }
    }
    let resolved: usize = totals.iter().filter(|(k, _)| k.starts_with("triggers_resolved")).map(|(_, v)| *v).sum();
    assert!(resolved > 0, "some trigger references resolve to grounded entities");
}

#[test]
fn development_triggers_never_resolve_through_country_labels_or_unsupported_matches() {
    // Development split only. Agent-proposed trigger texts (no gold IDs) are
    // resolved against the case's own installed guide when available.
    let guides: BTreeMap<String, Value> = installed().into_iter().map(|g| (g["id"].as_str().unwrap().to_owned(), g)).collect();
    if guides.is_empty() {
        return eprintln!("installed corpus absent: skipped");
    }
    // Local only, never committed (see scripts/guide-corpus.cjs).
    let dir = std::env::var_os("HOI4_GUIDE_CORPUS").map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/guide-corpus"));
    let Ok(bytes) = std::fs::read(dir.join("guide-benchmark-development.json")) else {
        return eprintln!("local guide corpus absent: skipped");
    };
    let dev: Value = serde_json::from_slice(&bytes).unwrap();
    let mut table: BTreeMap<String, usize> = BTreeMap::new();
    for case in dev["cases"].as_array().unwrap() {
        let text = case["proposed_expected"]["trigger"]["text"].as_str().unwrap_or("");
        let Some(guide) = guides.get(case["guide_id"].as_str().unwrap()) else { continue };
        if text.is_empty() {
            continue;
        }
        let r = resolve(text, &guide_candidates(guide));
        let status = r["status"].as_str().unwrap().to_owned();
        *table.entry(format!("{status}/{}", trigger_condition(&r).2)).or_default() += 1;
        if status == "resolved" {
            // Every resolution must rest on a non-label word of the reference.
            let matched = r["matched_text"].as_str().unwrap();
            assert!(matched.split(' ').any(|w| !(w.len() <= 3 && w.chars().all(|c| c.is_ascii_uppercase()))), "{text} -> {matched}");
            eprintln!("dev resolved: {text:?} -> {} via {:?} ({})", r["entity"]["id"], matched, r["match"]);
        }
    }
    eprintln!("development trigger resolution: {table:#?}");
}
