//! Compiler contract 2 (decomposition) semantics and activation safety:
//! development-set agreement (agent proposals, not human gold), opt-in final
//! holdout evaluation, the real installed corpus and run protection.
#![cfg(windows)]
use hoi4companion_backend::application::Application;
use hoi4companion_backend::decomposition::{Line, classify, decompose, mixed_header};
use hoi4companion_backend::{definition_archive, occurrence_manifest, request};
use hoi4companion_storage::request as storage;
use serde_json::{Value, json};
use std::collections::BTreeMap;

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
fn fixture(name: &str) -> Option<Value> {
    corpus(&format!("{name}.json"))
}
fn category(line: &str) -> &'static str {
    // Contract 3 splits `scope: action:` headers into a scope and a trackable step.
    let bare = line.trim().trim_start_matches(['-', '•']).trim();
    if mixed_header(bare).is_some() {
        return "header+action";
    }
    match classify(line) {
        Line::Blank => "blank",
        Line::TimingHeader(_) | Line::LeadHeader(_) => "header",
        Line::Step { class, .. } => class,
    }
}
fn unknown(_: &str) -> Result<Option<Value>, String> {
    Ok(None)
}
fn container(block: &str) -> Value {
    json!({"key":"word-0001","kind":"gate","tracking":"reminder","title":"Block",
        "evidence":[{"source_id":"s","basis":"explicit","quote":block}],
        "action":{"kind":"manual","params":{"instructions":[block]}}})
}
/// Confusion of proposed disposition -> contract-2 line category.
fn agreement(split: &str) -> Option<(BTreeMap<String, usize>, usize)> {
    let (mut table, mut unconditional) = (BTreeMap::new(), 0);
    for case in fixture(split)?["cases"].as_array().unwrap() {
        let proposed = case["proposed_expected"]["disposition"].as_str().unwrap();
        *table.entry(format!("{proposed} -> {}", category(case["text"].as_str().unwrap()))).or_default() += 1;
        // Structural safety on the whole block: after a timing header no child
        // may be unconditional.
        let block = case["context"]["block"].as_str().unwrap_or("");
        if block.lines().count() > 1 {
            let guide = json!({"id":"g","dependency_policy":"explicit","tracks":{"operations":[container(block)]}});
            let out = decompose(&guide, &unknown).unwrap();
            for row in out["guide"]["tracks"]["operations"].as_array().unwrap() {
                let d = &row["action"]["params"]["decomposition"];
                if !d["timing"].is_null() && row.get("when").is_none() {
                    unconditional += 1;
                }
            }
        }
    }
    Some((table, unconditional))
}

#[test]
fn development_semantics_agree_with_proposals_on_headers_and_never_drop_conditions() {
    // Development split only; proposals are agent annotations (not human gold).
    let Some((table, unconditional)) = agreement("guide-benchmark-development") else { return };
    eprintln!("development agreement: {table:#?}");
    let get = |k: &str| table.get(k).copied().unwrap_or(0);
    assert_eq!(unconditional, 0, "a timing-scoped line never becomes unconditional");
    assert_eq!(get("trigger_header -> header"), 13, "every proposed trigger header is a header, never an action");
    assert_eq!(get("actionable -> header"), 0, "no proposed action is swallowed as a header");
    assert_eq!(get("actionable -> advice"), 0, "no proposed action is downgraded to advice");
    // Advice detection is deliberately conservative (informational markers
    // only); proposed "ignore X" advice stays trackable rather than risking
    // the downgrade of real constraints.
    assert!(get("advice -> advice") >= 2, "advice recognized: {}", get("advice -> advice"));
    let actionable_kept = get("actionable -> manual") + get("actionable -> ambiguous") + get("actionable -> header+action");
    assert_eq!(actionable_kept, 299, "every proposed action stays trackable");
}

#[test]
fn holdout_final_evaluation_is_opt_in_and_unasserted() {
    // Holdout annotations are read only for the final report, never for tuning.
    if std::env::var("HOI4_HOLDOUT_FINAL").as_deref() != Ok("1") {
        return eprintln!("holdout final evaluation skipped (set HOI4_HOLDOUT_FINAL=1)");
    }
    let Some((table, unconditional)) = agreement("guide-benchmark-holdout") else { return };
    eprintln!("HOLDOUT final agreement: {table:#?}\nunconditional_after_timing={unconditional}");
}

#[test]
fn installed_corpus_decomposes_with_preserved_text_and_valid_definitions() {
    // Read-only, opt-in by presence; persisted guides, no catalog (conditions
    // without a catalog compile only through catalog-free rules or stay unknown).
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/guides");
    let Ok(entries) = std::fs::read_dir(&dir) else { return eprintln!("installed corpus absent: skipped") };
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut bump = |k: &str, n: usize| *totals.entry(k.to_owned()).or_default() += n;
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let guide = request(0, "guide_normalize", json!({"guide":raw})).unwrap();
        let out = decompose(&guide, &unknown).unwrap();
        let decomposed = &out["guide"];
        // Still a valid schema-v1 guide and a valid manifest.
        request(0, "guide_normalize", json!({"guide":decomposed})).unwrap();
        let manifest = occurrence_manifest::build(decomposed).unwrap();
        occurrence_manifest::validate(&manifest, decomposed).unwrap();
        // Every source instruction (multiline containers and, since contract 4,
        // single-line reminders) by key, to check preserved text.
        let originals: BTreeMap<String, String> = guide["tracks"].as_object().unwrap().values()
            .flat_map(|v| v.as_array().unwrap())
            .filter_map(|r| Some((r["key"].as_str()?.to_owned(), r["action"]["params"]["instructions"][0].as_str()?.to_owned())))
            .collect();
        let containers = guide["tracks"].as_object().unwrap().values().flat_map(|v| v.as_array().unwrap())
            .filter(|r| occurrence_manifest::is_container(r)).count();
        bump("containers", containers);
        for e in out["report"].as_array().unwrap() {
            bump(&format!("container_{}", e["status"].as_str().unwrap()), 1);
        }
        for row in decomposed["tracks"].as_object().unwrap().values().flat_map(|v| v.as_array().unwrap()) {
            let d = &row["action"]["params"]["decomposition"];
            if d.is_null() {
                continue;
            }
            bump("children", 1);
            bump(&format!("class_{}", d["status"].as_str().unwrap()), 1);
            bump(&format!("condition_{}", d["condition_status"].as_str().unwrap()), 1);
            if !d["lead"].is_null() { bump("lead_prefixed", 1); }
            if !d["context"].is_null() { bump("bullet_context", 1); }
            assert!(row.get("completion").is_none(), "no invented completion");
            assert_eq!(row["tracking"] == "reminder", d["status"] == "advice");
            if !d["timing"].is_null() {
                assert!(row.get("when").is_some(), "timing-scoped child has a condition");
            }
            // Original text preserved on its source line.
            let block = &originals[d["parent_key"].as_str().unwrap()];
            let line = block.split('\n').nth(d["line"].as_u64().unwrap() as usize - 1).unwrap();
            assert!(line.contains(row["action"]["params"]["instructions"][0].as_str().unwrap()));
        }
    }
    eprintln!("installed corpus contract-2: {totals:#?}");
    let get = |k: &str| totals.get(k).copied().unwrap_or(0);
    // Contract 4 also reports single-line reminders it converts.
    assert!(get("container_decomposed") + get("container_kept") >= get("containers"));
    assert!(get("children") > get("containers"));
}

fn guide(lines: &[&str]) -> Value {
    let block = lines.join("\n");
    json!({"id":"identity","dependency_policy":"explicit","sources":[{"id":"src-a","type":"file","text":format!("{block}\n")}],
        "tracks":{"operations":[{"key":"word-0001","tracking":"reminder","title":"Container","kind":"gate",
        "evidence":[{"source_id":"src-a","basis":"explicit","quote":block}],"action":{"kind":"manual","params":{"instructions":[block]}}}]}})
}
struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn activation_protects_legacy_and_contract1_runs_and_never_transfers_parent_progress() {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let temp = Temp(std::env::temp_dir().join(format!("hoi4-decompose-{}-{nonce}", std::process::id())));
    std::fs::create_dir(&temp.0).unwrap();
    let path = temp.0.join("progress.sqlite3");
    let source = guide(&["When at war:", "Train divisions", "(Optional: ignore deficits)"]);
    let registry = Application::open(&json!({"guides":[source.clone()]})).unwrap().request("app_definition", &json!({"guide_id":"identity"})).unwrap();
    // A run pinned to a contract-1 archive (as written before this milestone)
    // and a legacy run, both carrying a saved check on the container key.
    let mut envelope: Value = serde_json::from_str(&definition_archive::build("identity", &registry, &Value::Null).unwrap().payload).unwrap();
    envelope["compiler_contract"] = json!(1);
    envelope["occurrence_manifest"] = Value::Null;
    envelope["definition"] = registry.clone();
    envelope["definition"].as_object_mut().unwrap().remove("runtime_definition_revision");
    envelope["definition"].as_object_mut().unwrap().remove("runtime_engine");
    let old = definition_archive::seal(&envelope).unwrap();
    definition_archive::open(&old.id, &old.payload).unwrap();
    let row = |id: &str, pin: Option<&str>| {
        let mut r = json!({"schema_version":1,"guide_id":"identity","run_id":id,"session_id":null,
            "checks":{"word-0001":true},"started":{},"run_revision":1});
        if let Some(pin) = pin { r["definition_ref"] = definition_archive::pin(pin); }
        r
    };
    let snapshot = json!({"schema_version":1,"runs":{"pinned1":row("pinned1", Some(&old.id)),"legacy":row("legacy", None)},
        "active":{"identity":"pinned1"},"campaign_active":{}});
    storage(json!({"schema_version":1,"op":"init","path":path})).unwrap();
    storage(json!({"schema_version":1,"op":"save","path":path,"expected_revision":0,"version":2,
        "definitions":{old.id.clone():old.payload},"payload":snapshot.to_string()})).unwrap();

    let mut app = Application::open(&json!({"path":path,"guides":[source]})).unwrap();
    let steps = |packet: &Value| -> Vec<String> {
        packet["guide"]["tracks"]["operations"].as_array().unwrap().iter().map(|s| s["key"].as_str().unwrap().to_owned()).collect()
    };
    // Contract-1 pinned run: original container, original key, saved check intact.
    let pinned = app.projection(Some("identity"), false).unwrap();
    assert_eq!(pinned["definition"]["compiler_contract"], 1);
    assert_eq!(steps(&pinned), ["word-0001"]);
    assert_eq!(pinned["progress"]["checks"], json!({"word-0001":true}));
    // Legacy run: evaluated against the contract-1 registry, keys unchanged.
    app.progress(Some("identity"), None, Some(&json!({"run_id":"legacy"}))).unwrap();
    let legacy = app.projection(Some("identity"), false).unwrap();
    assert_eq!(legacy["definition"]["status"], "legacy_unresolved");
    assert_eq!(legacy["definition"]["compiler_contract"], 1);
    assert_eq!(steps(&legacy), ["word-0001"]);
    assert_eq!(legacy["progress"]["checks"], json!({"word-0001":true}));
    // New checklist: contract 2, children only, no inherited progress.
    let fresh = app.progress(Some("identity"), None, Some(&json!({}))).unwrap();
    let packet = app.projection(Some("identity"), false).unwrap();
    assert_eq!(packet["definition"]["compiler_contract"], definition_archive::COMPILER_CONTRACT);
    assert_eq!(steps(&packet), ["word-0001--line-2", "word-0001--line-3"]);
    assert_eq!(packet["progress"]["checks"], json!({}), "parent progress never transferred");
    let train = &packet["guide"]["tracks"]["operations"][0];
    assert!(train["when"]["key"].as_str().unwrap().starts_with("guide.conditions."), "header kept as unknown condition");
    assert_eq!(packet["guide"]["tracks"]["operations"][1]["tracking"], "reminder");
    // Children are evaluated by the Rust core like any manual gate.
    let evaluated: Vec<_> = packet["evaluation"]["steps"].as_array().unwrap().iter().map(|s| s["key"].clone()).collect();
    assert!(evaluated.contains(&json!("word-0001--line-2")));
    // The actionable child is manually checkable; advice and the old key are not.
    let write = |checks: Value| json!({"run_id":fresh["run_id"],"checks":checks});
    app.progress(Some("identity"), None, Some(&write(json!({"word-0001--line-2":true})))).unwrap();
    assert!(app.progress(Some("identity"), None, Some(&write(json!({"word-0001--line-3":true})))).is_err());
    assert!(app.progress(Some("identity"), None, Some(&write(json!({"word-0001":true})))).is_err());
    // The other runs are untouched.
    app.progress(Some("identity"), None, Some(&json!({"run_id":"pinned1"}))).unwrap();
    assert_eq!(app.projection(Some("identity"), false).unwrap()["progress"]["checks"], json!({"word-0001":true}));
}
