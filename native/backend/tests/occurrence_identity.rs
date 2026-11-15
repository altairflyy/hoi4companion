//! Stable operation identities and occurrence manifests against the frozen
//! corpus, the benchmark development/holdout cases (identity invariants only:
//! no semantic expectation is read or tuned) and the real application.
#![cfg(windows)]
use hoi4companion_backend::application::Application;
use hoi4companion_backend::occurrence_manifest::{build, correspondence, line_matches, shadow, validate};
use serde_json::{Value, json};
use std::collections::HashSet;

fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/fixtures/{name}.json"));
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn frozen_corpus_steps_get_unique_identities_and_preserve_order_and_lineage() {
    // 50 guides / 6,996 prepared steps from the frozen baseline identity snapshot.
    let baseline = fixture("guide-benchmark-baseline");
    let (mut all, mut steps, mut children, mut shared) = (HashSet::new(), 0, 0, 0);
    for guide in baseline["per_guide"].as_array().unwrap() {
        let mut tracks = serde_json::Map::new();
        for s in guide["step_identity"].as_array().unwrap() {
            // Evidence digest stands in for quote text; no source text is available.
            let step = json!({"key":s["key"],"evidence":[{"source_id":"frozen","quote":s["evidence_sha256"]}],
                "action":{"kind":"manual"},"semantics":s["semantics_sha256"]});
            tracks.entry(s["track"].as_str().unwrap()).or_insert_with(|| json!([])).as_array_mut().unwrap().push(step);
        }
        let definition = json!({"id":guide["id"],"sources":[],"tracks":tracks});
        let manifest = build(&definition).unwrap();
        validate(&manifest, &definition).unwrap();
        assert_eq!(build(&definition).unwrap(), manifest, "deterministic");
        let ops = manifest["operations"].as_array().unwrap();
        assert_eq!(ops.len(), guide["step_identity"].as_array().unwrap().len());
        let mut evidence = std::collections::HashMap::<String, usize>::new();
        for op in ops {
            assert!(all.insert(op["operation_id"].as_str().unwrap().to_owned()), "corpus-wide unique");
            if op["role"] != "step" {
                children += 1;
                let parent = op["parent_key"].as_str().unwrap();
                assert!(ops.iter().any(|o| o["step_key"] == parent), "parent present: {parent}");
            }
        }
        for s in guide["step_identity"].as_array().unwrap() {
            *evidence.entry(s["evidence_sha256"].as_str().unwrap().to_owned()).or_default() += 1;
        }
        shared += evidence.values().filter(|n| **n > 1).map(|n| n - 1).sum::<usize>();
        // Order is kept per track, as a separate list.
        for (track, ids) in manifest["order"].as_object().unwrap() {
            let keys: Vec<_> = ids.as_array().unwrap().iter()
                .map(|id| ops.iter().find(|o| o["operation_id"] == *id).unwrap()["step_key"].clone()).collect();
            let expected: Vec<_> = guide["step_identity"].as_array().unwrap().iter()
                .filter(|s| s["track"] == track.as_str()).map(|s| s["key"].clone()).collect();
            assert_eq!(keys, expected);
        }
        steps += ops.len();
    }
    assert_eq!(steps, 6996);
    assert!(children >= 1000, "promoted children/notes: {children}");
    assert!(shared > 0, "steps sharing identical evidence still get distinct IDs");
    eprintln!("frozen corpus: steps={steps} children={children} shared_evidence_repeats={shared}");
}

#[test]
fn development_and_holdout_lines_get_exact_spans_or_stay_unlocated() {
    // Identity only: each case's block is placed in a document between its
    // neighbours; its line must slice back byte-exactly or be reported as not
    // located. Proposed annotations are never read.
    for split in ["guide-benchmark-development", "guide-benchmark-holdout"] {
        let (mut located, mut other, mut total) = (0, 0, 0);
        for case in fixture(split)["cases"].as_array().unwrap() {
            let ctx = &case["context"];
            let block = ctx["block"].as_str().unwrap_or("");
            let line = case["text"].as_str().unwrap();
            if block.is_empty() {
                continue;
            }
            total += 1;
            let source = format!("{}\n{block}\n{}\n", ctx["previous"].as_str().unwrap_or(""), ctx["next"].as_str().unwrap_or(""));
            let parent = json!({"key":"word-0001","tracking":"reminder","evidence":[{"source_id":"s","quote":block}],
                "action":{"kind":"manual","params":{"instructions":[block]}}});
            let child = json!({"key":"word-0001--operation-2","tracking":"reminder","evidence":[{"source_id":"s","quote":block}],
                "action":{"kind":"manual","params":{"instructions":[line]}}});
            let definition = json!({"id":"case","sources":[{"id":"s","type":"file","text":source}],"tracks":{"operations":[parent,child]}});
            let manifest = build(&definition).unwrap();
            validate(&manifest, &definition).unwrap();
            let op = &manifest["operations"][1];
            if op["span_status"] == "located" {
                let (s, e) = (op["start_utf8"].as_u64().unwrap() as usize, op["end_utf8"].as_u64().unwrap() as usize);
                assert_eq!(&source[s..e], line);
                assert_eq!(line_matches(block, line).len(), 1);
                located += 1;
            } else {
                assert!(op["start_utf8"].is_null());
                other += 1;
            }
            // Shadow children of the block are distinct and exact when located.
            let proposals = shadow(&definition, &manifest).unwrap();
            for c in proposals["containers"].as_array().unwrap() {
                let ids: HashSet<_> = c["children"].as_array().unwrap().iter().map(|k| k["proposed_id"].clone()).collect();
                assert_eq!(ids.len(), c["children"].as_array().unwrap().len());
                for k in c["children"].as_array().unwrap() {
                    if let Some(s) = k["span"]["start_utf8"].as_u64() {
                        assert_eq!(&source[s as usize..k["span"]["end_utf8"].as_u64().unwrap() as usize], k["quote"].as_str().unwrap());
                    }
                }
            }
        }
        assert!(total > 0 && located > 0);
        eprintln!("{split}: cases={total} located={located} not_located={other}");
    }
}

#[test]
fn installed_corpus_manifests_validate_when_available() {
    // Read-only, opt-in by presence: the user's installed (persisted) guides.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/guides");
    let Ok(entries) = std::fs::read_dir(&dir) else { return eprintln!("installed corpus absent: skipped") };
    let (mut guides, mut ops, mut located, mut ambiguous, mut containers, mut proposals) = (0, 0, 0, 0, 0, 0);
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let definition = hoi4companion_backend::request(0, "guide_normalize", json!({"guide":raw})).unwrap();
        let manifest = build(&definition).unwrap();
        validate(&manifest, &definition).unwrap();
        assert_eq!(build(&definition).unwrap(), manifest);
        for op in manifest["operations"].as_array().unwrap() {
            ops += 1;
            located += (op["span_status"] == "located") as usize;
            ambiguous += (op["span_status"] == "ambiguous") as usize;
        }
        let s = shadow(&definition, &manifest).unwrap();
        containers += s["container_count"].as_u64().unwrap() as usize;
        proposals += s["containers"].as_array().unwrap().iter().map(|c| c["children"].as_array().unwrap().len()).sum::<usize>();
        guides += 1;
    }
    eprintln!("installed corpus: guides={guides} operations={ops} located={located} ambiguous={ambiguous} containers={containers} shadow_children={proposals}");
    assert!(guides == 0 || located * 2 > ops, "most operations locate exactly");
}

fn guide(lines: &[&str]) -> Value {
    let block = lines.join("\n");
    let mut steps = vec![json!({"key":"word-0001","tracking":"reminder","title":"Container",
        "evidence":[{"source_id":"src-a","basis":"explicit","quote":block}],"action":{"kind":"manual","params":{"instructions":[block]}}})];
    steps.push(json!({"key":"word-0002","title":"Check","evidence":[{"source_id":"src-a","basis":"explicit","quote":"Check"}],
        "action":{"kind":"manual"},"completion":{"op":"check","key":"word-0002"}}));
    json!({"id":"identity","dependency_policy":"explicit","sources":[{"id":"src-a","type":"file","text":format!("{block}\nCheck\n")}],
        "tracks":{"operations":steps}})
}
struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn pinned_runs_keep_operation_identities_across_restart_and_reimport() {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let temp = Temp(std::env::temp_dir().join(format!("hoi4-manifest-{}-{nonce}", std::process::id())));
    std::fs::create_dir(&temp.0).unwrap();
    let path = temp.0.join("progress.sqlite3");
    let original = guide(&["When at war:", "Rafforza la difesa ⚑", "Rafforza la difesa ⚑"]);
    let first;
    {
        let mut app = Application::open(&json!({"path":path,"guides":[original.clone()]})).unwrap();
        let run = app.progress(Some("identity"), None, None).unwrap();
        app.progress(Some("identity"), None, Some(&json!({"run_id":run["run_id"],"checks":{"word-0002":true}}))).unwrap();
        first = app.manifest(Some("identity"), None).unwrap();
        assert_eq!(first["definition"]["manifest"], "bound");
        assert_eq!(first["definition"]["compiler_contract"], 3);
        // Contract 2: the container became two line children; nothing left to shadow.
        assert_eq!(first["definition"]["operations"], 3);
        assert_eq!(first["shadow_decomposition"]["container_count"], 0);
        let ops = first["manifest"]["operations"].as_array().unwrap();
        let keys: Vec<_> = ops.iter().map(|o| o["step_key"].as_str().unwrap()).collect();
        assert_eq!(keys, ["word-0001--line-2", "word-0001--line-3", "word-0002"]);
        assert_ne!(ops[0]["operation_id"], ops[1]["operation_id"]);
        assert_eq!((ops[0]["span_status"].clone(), ops[1]["span_status"].clone()), (json!("located"), json!("located")));
        assert_ne!(ops[0]["start_utf8"], ops[1]["start_utf8"], "repeated text, distinct occurrences");
        // Independent application, same input: identical identities.
        let mut other = Application::open(&json!({"guides":[original.clone()]})).unwrap();
        assert_eq!(other.manifest(Some("identity"), None).unwrap()["manifest"], first["manifest"]);
    }
    // Restart after a re-preparation of the SAME source whose container step
    // changed (e.g. a new decomposition/binding): only the untouched step maps.
    let mut changed = original.clone();
    changed["tracks"]["operations"][0]["action"]["params"]["category"] = json!("politics");
    let mut app = Application::open(&json!({"path":path,"guides":[changed]})).unwrap();
    let after = app.manifest(Some("identity"), None).unwrap();
    assert_eq!(after["manifest"], first["manifest"], "pinned run keeps its archived identities");
    let map = &after["correspondence_to_current"];
    assert_eq!(map["word-0002"]["status"], "same", "unchanged operation maps");
    assert_ne!(map["word-0001--line-2"]["status"], "same", "changed children never map");
    let packet = app.projection(Some("identity"), false).unwrap();
    assert_eq!(packet["progress"]["checks"], json!({"word-0002":true}));
    assert_eq!(packet["definition"]["manifest"], "bound");
    // A new checklist binds to the new definition's identities only.
    app.progress(Some("identity"), None, Some(&json!({}))).unwrap();
    let fresh = app.manifest(Some("identity"), None).unwrap();
    assert_ne!(fresh["manifest"]["operations"][0]["operation_id"], first["manifest"]["operations"][0]["operation_id"]);
    assert_eq!(fresh["manifest"]["operations"][2]["operation_id"], first["manifest"]["operations"][2]["operation_id"]);
    assert!(fresh["correspondence_to_current"].is_null());
    assert_ne!(correspondence(&first["manifest"], &fresh["manifest"])["word-0001--line-2"]["status"], "same");
    // A revised SOURCE is a new identity scope: nothing maps automatically,
    // even byte-identical steps (explicit reviewed mapping is future work).
    let revised = guide(&["When at war:", "Rafforza la difesa ⚑", "Costruisci fabbriche"]);
    let revised = hoi4companion_backend::request(0, "guide_normalize", json!({"guide":revised})).unwrap();
    let map = correspondence(&first["manifest"], &build(&revised).unwrap());
    assert!(map.as_object().unwrap().values().all(|v| v["status"] != "same"), "{map}");
}
