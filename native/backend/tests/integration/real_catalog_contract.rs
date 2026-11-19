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
fn real_catalog_sov_construction_monitoring() {
    if std::env::var("HOI4_REAL_CATALOG").as_deref()!=Ok("1") {return;}
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data=root.join(".local");
    let settings:Value=serde_json::from_slice(&std::fs::read(data.join("settings.json")).unwrap()).unwrap();
    let guides=request(0,"guide_repository",json!({"directories":[data.join("guides")],"association_directory":data.join("guide-associations")})).unwrap();
    let guide=guides["guides"].as_array().unwrap().iter().find(|g|g["id"]=="word-67736ac669bdcbf1").unwrap();
    let catalog=Catalog(request(0,"catalog_open",json!({"settings":settings,"vocabulary_path":root.join("src/hoi4companion/guide_language.json")})).unwrap().as_u64().unwrap());
    let compiled=request(0,"catalog_runtime_guide_new",json!({"handle":catalog.0,"guide":guide,"library":{}})).unwrap();
    let rows=steps(&compiled);
    let facility=rows.iter().find(|r|r["title"].as_str()==Some("Build: INFRA+Land Facility in Moscow")).expect("real facility instruction");
    let targets=facility["action"]["params"]["construction_targets"].as_array().expect("live targets");
    assert_eq!(targets.len(),2);
    assert_eq!(targets[0]["state_id"],219);
    assert_eq!(targets[0]["target_level"],5);
    assert_eq!(targets[1]["province_id"],6380);
    assert_eq!(targets[1]["target_level"],1);
    assert_eq!(facility["completion"]["op"],"all");
    let industry=rows.iter().find(|r|r["title"].as_str().is_some_and(|t|t.starts_with("Build: INFRA+MILs in:"))).expect("real industry instruction");
    assert_eq!(industry["action"]["params"]["construction_targets"].as_array().unwrap().len(),12);
    assert!(industry["started_when"].is_object());
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
            // The naval line of the same row is tracked production too.
            let navy: Vec<Value> = b.iter().filter(|s| s["title"].as_str().is_some_and(|t| t.starts_with("Navy: build convoys"))).cloned().collect();
            eprintln!("SOV navy steps: {}", navy.iter().map(|s| format!("{} targets={}", s["key"], s["action"]["params"]["production_targets"])).collect::<Vec<_>>().join(" | "));
            assert!(navy.iter().any(|s| s["action"]["params"]["production_targets"].as_array().is_some_and(|t| t.len() == 2)), "naval production is tracked");
            // The wrong German decision is gone; the reference is grounded to the
            // only answer the Soviet Union can receive.
            let mobilize = step("word-0008").unwrap();
            assert!(mobilize["action"]["params"]["decision_id"].is_null());
            assert_eq!(mobilize["action"]["params"]["event_option_id"], "bftb_turkey.4.a");
        }
        // Author abbreviations of event answers (cut mid-word, accents, notes,
        // answer before an action, after a timing lead-in) reach the real answer.
        for (country, line, option) in [("GER", "A New Bismarck…", "wuw_GER_civil_war.3.a"), ("AUS", "Osterr…", "AUS_political_events.1.b"),
            ("SOV", "No Deal…(50PP)", "germany.11.b"), ("SPR", "Which By God…: CW setup*,spy Madrid", "lar_spain.2.c"),
            ("SWE", "On event, wait 10 days, then: Clearly The Opposition…", "AAT_Sweden.1.c")] {
            if guide["applies_to"]["country"] == country {
                for s in a.iter().filter(|s| s["action"]["params"]["instructions"][0] == line) {
                    assert_eq!(s["action"]["params"]["event_option_id"], option, "{country} {line}");
                }
            }
        }
        // Abbreviated decisions (cost note checked against the real cost) and
        // focus triggers on existing single-line steps (precision checked by
        // hand against the game's names).
        for (country, line, key, id) in [("HUN", "Recall Kocs…(25 CP)", "decision_id", "HUN_recall_kocsard_janky_to_active_service"),
            ("ITA", "On Strengthen…: Liturgical…(70PP)", "decision_id", "ITA_bop_liturgical_reforms"),
            ("ITA", "Take Over Ministry Of Corp… (50PP)", "decision_id", "ITA_bop_take_over_ministry_of_corporations"),
            ("GER", "Exploit the… (30PP)", "decision_id", "GER_exploit_the_gombos_treaty"),
            ("GER", "On Coal… Develop Ersatz… (30PP)", "decision_id", "GER_ersatz_material")] {
            if guide["applies_to"]["country"] == country {
                for s in a.iter().filter(|s| s["action"]["params"]["instructions"][0] == line) {
                    assert_eq!(s["action"]["params"][key], id, "{country} {line}");
                }
            }
        }
        // Several actions plus other work ("& SPA war*"): the first action is
        // recognized, but one observed action never completes the step.
        if guide["applies_to"]["country"] == "FRA" {
            for s in a.iter().filter(|s| s["action"]["params"]["instructions"][0] == "On Intervention…: Allow Volunteer…+ Intervene On… (25PPx2) & SPA war*") {
                let p = &s["action"]["params"];
                assert_eq!(p["reference_resolution"]["id"], "FRA_allow_volunteer_work");
                assert_eq!(p["reference_resolution"]["tracked"], false);
                assert!(p["decision_id"].is_null() && s.get("completion").is_none());
            }
        }
        for (country, line, focus) in [("GER", "On Coal… Develop Ersatz… (30PP)", "GER_coal_liquefaction"), ("SWE", "On “Appeal for…”: pick all decisions except “aristocracy”", "SWE_appeal_for_support"),
            ("USA", "On Picking Reaffirm…:  Plan invasion of VEN from PUE (3 groups if possible)", "USA_reinforce_monroe_doctrine"), ("ITA", "On Depose…: collab ROM", "ITA_depose_mussolini")] {
            if guide["applies_to"]["country"] == country {
                for s in a.iter().filter(|s| s["action"]["params"]["instructions"][0] == line) {
                    assert_eq!(s["when"]["args"][1]["value"], focus, "{country} {line}");
                }
            }
        }
        // An abbreviation shared by several focuses ("On Strengthen…": five
        // Italian focuses) resolves only because exactly one of them is a step
        // of the same guide; the others stay listed as candidates.
        if guide["applies_to"]["country"] == "ITA" {
            for s in a.iter().filter(|s| s["action"]["params"]["instructions"][0] == "On Strengthen…: Liturgical…(70PP)") {
                let t = &s["action"]["params"]["trigger_resolution"];
                assert_eq!((t["status"].as_str(), t["disambiguation"].as_str()), (Some("compiled"), Some("unique_guide_step")));
                assert_eq!(t["entity"]["id"], "ITA_strengthen_the_papacy");
                assert_eq!(s["when"]["args"][1]["value"], "ITA_strengthen_the_papacy");
            }
        }
        // Two tied focuses that are both steps of the guide ("On Expand…done":
        // Expand Aviation Institutes, Expand the Agitprop) stay unknown.
        if guide["applies_to"]["country"] == "SOV" {
            for s in a.iter().filter(|s| s["action"]["params"]["instructions"][0].as_str().is_some_and(|l| l.starts_with("On Expand…done:"))) {
                assert_eq!(s["action"]["params"]["trigger_resolution"]["resolution_status"], "ambiguous");
                assert!(s["action"]["params"]["trigger_resolution"]["entity"].is_null());
            }
        }
        for s in a.iter().filter(|s| !s["action"]["params"]["reference_resolution"].is_null()) {
            eprintln!("GROUNDED {} {} -> {} «{}» basis={} also={}", guide["applies_to"]["country"].as_str().unwrap_or(""), s["action"]["params"]["instructions"][0], s["action"]["params"]["reference_resolution"]["option_id"], s["action"]["params"]["reference_resolution"]["option_text"].as_str().unwrap_or(""), s["action"]["params"]["reference_resolution"]["basis"], s["action"]["params"]["reference_resolution"]["also"]);
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

/// Contract-1 definitions of every guide (what `catalog_runtime_guide`
/// returns as `definition`), in input order. One catalog generation runs its
/// requests one at a time, so each worker opens its own generation and takes
/// the next guide; the results do not depend on the worker count.
/// `HOI4_SURVEY_WORKERS` sets the count (default: a quarter of the
/// cores, 1-3: each generation holds about 1.5 GB, and 4 measured slower).
fn compile_in_parallel(root: &std::path::Path, settings: &Value, library: &Value, guides: &[Value]) -> Vec<Value> {
    use std::sync::{Mutex, atomic::{AtomicUsize, Ordering}};
    let workers = std::env::var("HOI4_SURVEY_WORKERS").ok().and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get() / 4).clamp(1, 3))
        .clamp(1, guides.len().max(1));
    let next = AtomicUsize::new(0);
    let results = Mutex::new(vec![Value::Null; guides.len()]);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                let catalog = Catalog(request(0, "catalog_open", json!({"settings":settings,"vocabulary_path":root.join("src/hoi4companion/guide_language.json")})).unwrap().as_u64().unwrap());
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(guide) = guides.get(i) else { break };
                    let definition = request(0, "catalog_runtime_guide", json!({"handle":catalog.0,"guide":guide,"library":library})).unwrap()["definition"].clone();
                    results.lock().unwrap()[i] = definition;
                }
            });
        }
    });
    results.into_inner().unwrap()
}

/// Opt-in survey of every author reference that could be an event answer:
/// how each compiled step carries it and whether it is bound live.
#[test]
fn event_reference_survey() {
    if std::env::var("HOI4_REAL_CATALOG").as_deref() != Ok("1") {
        return eprintln!("event reference survey skipped (set HOI4_REAL_CATALOG=1)");
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = root.join(".local");
    let settings: Value = serde_json::from_slice(&std::fs::read(data.join("settings.json")).unwrap()).unwrap();
    let guides = request(0, "guide_repository", json!({"directories":[data.join("guides")],"association_directory":data.join("guide-associations")})).unwrap();
    let library = request(0, "reference_library_manifest", json!({"root":data.join("reference-library")})).unwrap();
    let list = guides["guides"].as_array().unwrap();
    let definitions = compile_in_parallel(&root, &settings, &library, list);
    // Traced receivers of every event (event_reach), to count the candidate
    // events of these lines that no traced caller delivers to a known nation.
    let catalog = Catalog(request(0, "catalog_open", json!({"settings":settings,"vocabulary_path":root.join("src/hoi4companion/guide_language.json")})).unwrap().as_u64().unwrap());
    let entities = request(0, "catalog_entities", json!({"handle":catalog.0})).unwrap();
    let reach: BTreeMap<String, (Value, Value)> = entities["entries"].as_array().unwrap().iter().filter(|r| r["kind"] == "event")
        .map(|r| (r["id"].as_str().unwrap_or("").to_owned(), (r["receivers"].clone(), r["receivers_unknown"].clone()))).collect();
    // Event answers whose own trigger names who must be human- or AI-played.
    let guarded: Vec<(String, &Value)> = entities["entries"].as_array().unwrap().iter()
        .filter(|r| r["kind"] == "event_option" && !r["player_requirement"].is_null())
        .flat_map(|r| [&r["name"], &r["localized_name"]].into_iter().filter_map(Value::as_str)
            .map(|n| hoi4companion_backend::reference_candidates::normalize(n)).filter(|n| !n.is_empty()).collect::<std::collections::BTreeSet<_>>()
            .into_iter().map(move |n| (n, r)))
        .collect();
    eprintln!("GUARDED options in catalog: {}", entities["entries"].as_array().unwrap().iter().filter(|r| r["kind"] == "event_option" && !r["player_requirement"].is_null()).count());
    // Catalog rows of the events named in HOI4_SURVEY_EVENTS (comma-separated), for hand checks.
    for id in std::env::var("HOI4_SURVEY_EVENTS").unwrap_or_default().split(',').filter(|s| !s.is_empty()) {
        for row in entities["entries"].as_array().unwrap().iter().filter(|r| (r["kind"] == "event" || r["kind"] == "decision") && r["id"] == id) {
            eprintln!("EVROW {id} kind={} receivers={} unknown={} earliest={} triggered_by={} callers_complete={} required_focuses={}", row["kind"], row["receivers"], row["receivers_unknown"],
                row["earliest_day"], row["triggered_by"], row["callers_complete"], row["required_focuses"]);
        }
    }
    let mut buckets: BTreeMap<String, usize> = BTreeMap::new();
    // Neighbouring guide events (IL-12 rule 4): for each candidate line, the
    // nearest bound event-answer lines of the same track, and which candidate
    // events a certain causal order with them rules out.
    let catalog_rows: Vec<Value> = entities["entries"].as_array().cloned().unwrap_or_default();
    let timing = hoi4companion_backend::event_timing::Timing::new(&catalog_rows);
    for (guide, compiled) in list.iter().zip(&definitions) {
        let country = guide["applies_to"]["country"].as_str().unwrap_or("?");
        for (track, steps) in compiled["tracks"].as_object().into_iter().flatten() {
            let steps = steps.as_array().cloned().unwrap_or_default();
            let bound: Vec<(usize, String)> = steps.iter().enumerate()
                .filter_map(|(i, s)| s["action"]["params"]["event_option_id"].as_str().and(s["action"]["params"]["event_id"].as_str()).map(|e| (i, e.to_owned()))).collect();
            for (i, s) in steps.iter().enumerate() {
                let Some(options) = s["action"]["params"]["reference_candidates"]["event_options"].as_array() else { continue };
                let before: Vec<&String> = bound.iter().rev().filter(|(j, _)| *j < i).take(2).map(|(_, e)| e).collect();
                let after: Vec<&String> = bound.iter().filter(|(j, _)| *j > i).take(2).map(|(_, e)| e).collect();
                *buckets.entry("neighbour candidate lines".into()).or_default() += 1;
                if !before.is_empty() || !after.is_empty() {
                    *buckets.entry("neighbour lines with a bound neighbour".into()).or_default() += 1;
                }
                let mut drops = vec![];
                let mut related = vec![];
                for o in options {
                    let c = o["event_id"].as_str().unwrap_or("");
                    let later = after.iter().find(|n| timing.only_after(c, n));
                    let earlier = before.iter().find(|p| timing.only_after(p, c));
                    if later.is_some() || earlier.is_some() {
                        drops.push(format!("{} (after {:?} / before {:?})", o["id"].as_str().unwrap_or(""), later, earlier));
                    }
                    let ns = c.split('.').next().unwrap_or("");
                    if before.iter().chain(after.iter()).any(|n| n.split('.').next() == Some(ns)) {
                        related.push(o["id"].as_str().unwrap_or("").to_owned());
                    }
                }
                *buckets.entry("neighbour dropped answers".into()).or_default() += drops.len();
                if !related.is_empty() {
                    *buckets.entry("neighbour lines with a same-namespace candidate".into()).or_default() += 1;
                }
                eprintln!("NEIGH {country} {track} {} | {} | options={} before={before:?} after={after:?} drops={drops:?} same_namespace={related:?}", s["key"],
                    s["action"]["params"]["instructions"][0].as_str().unwrap_or("").chars().take(60).collect::<String>(), options.len());
            }
        }
    }
    let mut candidate_events: BTreeMap<(String, String), &'static str> = BTreeMap::new();
    let only = std::env::var("HOI4_SURVEY_COUNTRY").ok();
    for (guide, compiled) in list.iter().zip(&definitions) {
        let country = guide["applies_to"]["country"].as_str().unwrap_or("?").to_owned();
        for s in steps(compiled) {
            let p = &s["action"]["params"];
            let text = p["instructions"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ⏎ ")).unwrap_or_default();
            if !(text.contains('…') || text.contains("...")) {
                continue;
            }
            for c in p["reference_candidates"]["candidates"].as_array().into_iter().flatten() {
                let event = if c["kind"] == "event" { c["id"].as_str() } else { c["event_id"].as_str() };
                let Some(event) = event.filter(|e| !e.is_empty() && ["event", "event_option"].contains(&c["kind"].as_str().unwrap_or(""))) else { continue };
                let (receivers, unknown) = reach.get(event).cloned().unwrap_or((json!([]), json!(false)));
                let traced = receivers.as_array().is_some_and(|r| !r.is_empty());
                let class = match (traced, unknown == true) { (false, _) => "no_receiver", (true, true) => "incomplete", (true, false) => "complete" };
                candidate_events.insert((country.clone(), event.to_owned()), class);
            }
            let state = if p["event_option_id"].is_string() { if p["reference_resolution"].is_object() { "event_grounded" } else { "event_bound" } }
                else if p["reference_resolution"].is_object() { "named_grounded" }
                else if p["reference_candidates"].is_object() { "candidates" }
                else if ["decision_id", "advisor_id", "law_id", "idea_id", "operation_id", "technology_id"].iter().any(|k| p[*k].is_string()) || s["action"]["entity"]["id"].is_string() { "other_bound" }
                else { "unbound" };
            *buckets.entry(state.into()).or_default() += 1;
            // Family of the line: 1 trigger "On X…: …", 3 several actions,
            // 2 a single abbreviated name. A trigger line also counts in the
            // family of its action part when that names references.
            use hoi4companion_backend::reference_candidates::{references, trigger};
            let whole = p["instructions"].as_array().filter(|a| a.len() == 1).and_then(|a| a[0].as_str()).unwrap_or("");
            // A block is read by its lead line (its trigger scopes the block).
            let block = whole.lines().filter(|l| !l.trim().is_empty()).count() > 1;
            let line = whole.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
            let header = line.strip_suffix(':').filter(|h| block && (h.contains('…') || h.contains("...")) && ["on ", "when ", "at ", "after ", "before ", "around ", "once ", "in ", "on:"].iter().any(|w| h.to_lowercase().starts_with(w))).map(|h| (h.trim().to_owned(), String::new()));
            let trig = trigger(line).filter(|(left, _)| left.contains('…') || left.contains("...")).or(header);
            let refs = references(line);
            // Event answers matching a truncated reference that a single-player
            // game where the human plays the guide's nation can never show.
            for r in refs.iter().filter(|r| r.text.contains('…') || r.text.contains("...")) {
                let q = hoi4companion_backend::reference_candidates::normalize(&hoi4companion_backend::reference_candidates::query(&r.text));
                for (name, row) in &guarded {
                    if name.starts_with(&q) && !hoi4companion_backend::reference_candidates::single_player(&row["player_requirement"], &country, reach.get(row["event_id"].as_str().unwrap_or("")).is_some_and(|(r, _)| r.as_array().is_some_and(|r| r.iter().any(|t| t == country.as_str())))) {
                        *buckets.entry("single-player dropped answers".into()).or_default() += 1;
                        eprintln!("SPDROP {country} {} | {} -> {} «{}» requirement={}", s["key"], r.text, row["id"].as_str().unwrap_or(""), row["name"].as_str().unwrap_or(""), row["player_requirement"]);
                    }
                }
            }
            let family = if refs.len() > 1 { "3_multi" } else if refs.iter().any(|r| r.text.contains('…') || r.text.contains("...")) { "2_named" } else { "other" };
            let bound = state != "unbound" && state != "candidates";
            let shown = if p["reference_resolution"]["tracked"] == false {
                if p["reference_resolution"]["scope"] == "line" { "recognized_untracked_line" } else { "recognized_block_lead" }
            } else if bound { "bound" } else { state };
            *buckets.entry(format!("family {family}{} {shown}", if block { "(block)" } else { "" })).or_default() += 1;
            if let Some((left, _)) = &trig {
                let t = &p["trigger_resolution"];
                let status = match t["status"].as_str() { Some("compiled") => "compiled".to_owned(), Some(_) => format!("unknown:{}/{}", t["verification"].as_str().unwrap_or(""), t["resolution_status"].as_str().unwrap_or("")), None => "none".to_owned() };
                *buckets.entry(format!("family 1_trigger{} {}", if block { "(block)" } else { "" }, status.split(':').next().unwrap_or(""))).or_default() += 1;
                *buckets.entry(format!("trigger reason {status}")).or_default() += 1;
                eprintln!("TRIG {country} {} [{status}] {left} -> {} {} when={}", s["key"], t["entity"]["id"].as_str().unwrap_or("-"), t["relation"].as_str().unwrap_or(""),
                    s["when"].to_string().chars().take(160).collect::<String>());
                if t["candidates"].is_array() {
                    let ids: Vec<String> = t["candidates"].as_array().into_iter().flatten()
                        .map(|c| format!("{}:{}({})", c["kind"].as_str().unwrap_or(""), c["id"].as_str().unwrap_or(""), c["source"].as_str().unwrap_or(""))).collect();
                    eprintln!("TRIGCAND {country} {} {} | {}", s["key"], t["disambiguation"].as_str().unwrap_or("-"), ids.join(", "));
                }
            }
            eprintln!("FAM {country} {} {family} block={block} trig={} [{state}] | {}", s["key"], trig.is_some(), line.chars().take(100).collect::<String>());
            if p["reference_resolution"].is_object() && !p["event_option_id"].is_string() || p["reference_resolution"]["also"].is_array() {
                eprintln!("NAMED {country} {} | {} -> {}:{} «{}» basis={} also={} open={}", s["key"], text.chars().take(120).collect::<String>(), p["reference_resolution"]["kind"].as_str().unwrap_or(""),
                    p["reference_resolution"]["id"].as_str().unwrap_or(""), p["reference_resolution"]["name"].as_str().unwrap_or(""), p["reference_resolution"]["basis"], p["reference_resolution"]["also"], p["reference_resolution"]["unresolved"]);
            }
            // The guide timeline (IL-11): answers dropped because their event
            // cannot fire by the line's planned day, and the bindings it left.
            let resolution = &p["reference_resolution"];
            let timelines = std::iter::once(resolution).chain(resolution["also"].as_array().into_iter().flatten())
                .filter(|r| r["basis"] == "guide_timeline").map(|r| (&r["timeline"], format!("bound {} «{}»", r["id"].as_str().unwrap_or(""), r["name"].as_str().unwrap_or(""))))
                .chain(p["reference_candidates"]["timeline"].is_object().then(|| (&p["reference_candidates"]["timeline"], format!("narrowed to {}", p["reference_candidates"]["candidates"].as_array().map_or(0, Vec::len)))))
                .collect::<Vec<_>>();
            for (record, outcome) in timelines {
                *buckets.entry(if outcome.starts_with("bound") { "guide_timeline bindings".into() } else { "guide_timeline narrowed lines".into() }).or_default() += 1;
                *buckets.entry("guide_timeline dropped answers".into()).or_default() += record["dropped"].as_array().map_or(0, Vec::len);
                eprintln!("TIMELINE {country} {} | {} | {outcome} | anchor={} planned={} earliest={} dropped={}", s["key"], text.chars().take(80).collect::<String>(),
                    record["anchor_focus"], record["planned_day"], record["earliest_day"], record["dropped"]);
                eprintln!("TIMELEFT {country} {} | {} | left={} received={} then={}", s["key"], text.chars().take(40).collect::<String>(), record["left"], record["left_received"], record["then"]);
            }
            // Literal words before other spellings (IL-12): answers set aside.
            let literals = std::iter::once(resolution).chain(resolution["also"].as_array().into_iter().flatten())
                .filter(|r| r["literal"].is_object()).map(|r| (&r["literal"], format!("bound {} «{}» basis={}", r["id"].as_str().unwrap_or(""), r["name"].as_str().unwrap_or(""), r["basis"])))
                .chain(p["reference_candidates"]["literal"].is_object().then(|| (&p["reference_candidates"]["literal"], format!("narrowed to {}", p["reference_candidates"]["candidates"].as_array().map_or(0, Vec::len)))))
                .collect::<Vec<_>>();
            for (record, outcome) in literals {
                *buckets.entry(if outcome.starts_with("bound") { "literal_text bindings".into() } else { "literal_text narrowed lines".into() }).or_default() += 1;
                *buckets.entry("literal_text dropped answers".into()).or_default() += record["dropped"].as_array().map_or(0, Vec::len);
                eprintln!("LITERAL {country} {} | {} | {outcome} | dropped={} then={}", s["key"], text.chars().take(80).collect::<String>(), record["dropped"], record["then"]);
            }
            if only.as_deref().is_none_or(|c| c == country) && state != "event_bound" && state != "other_bound" {
                eprintln!("EVREF {country} {} [{state}] kind={} | {} | cand={}", s["key"], s["action"]["kind"], text.chars().take(140).collect::<String>(),
                    p["reference_candidates"]["candidates"].as_array().map(|c| c.iter().take(3).map(|x| format!("{}:{}", x["kind"].as_str().unwrap_or(""), x["id"].as_str().unwrap_or(""))).collect::<Vec<_>>().join(",")).unwrap_or_default());
            }
        }
    }
    // Candidate events by tracing: "no_receiver" (no traced caller names a
    // nation), "incomplete" (some callers untraced), "complete".
    for ((country, event), class) in &candidate_events {
        eprintln!("CANDEV {country} {event} {class} receivers={} unknown={}", reach.get(event).map(|r| r.0.to_string()).unwrap_or_default(),
            reach.get(event).map(|r| r.1.to_string()).unwrap_or_default());
        *buckets.entry(format!("candidate events {class}")).or_default() += 1;
    }
    eprintln!("event reference survey: {buckets:#?}");
}

/// Written-out actions on existing single-line steps ("REL UK, FRA (40PP)",
/// "war on GRE", "Justify HUN"): what was bound or recognized, for review.
#[test]
fn written_action_survey() {
    if std::env::var("HOI4_REAL_CATALOG").as_deref() != Ok("1") {
        return eprintln!("written action survey skipped (set HOI4_REAL_CATALOG=1)");
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = root.join(".local");
    let settings: Value = serde_json::from_slice(&std::fs::read(data.join("settings.json")).unwrap()).unwrap();
    let guides = request(0, "guide_repository", json!({"directories":[data.join("guides")],"association_directory":data.join("guide-associations")})).unwrap();
    let library = request(0, "reference_library_manifest", json!({"root":data.join("reference-library")})).unwrap();
    let catalog = Catalog(request(0, "catalog_open", json!({"settings":settings,"vocabulary_path":root.join("src/hoi4companion/guide_language.json")})).unwrap().as_u64().unwrap());
    let mut buckets: BTreeMap<String, usize> = BTreeMap::new();
    let verbs = ["rel ", "improve relations", "justify", "annex", "integrate", "invade", "collab", "war on", "war with", "war against", "declare war", "guarantee", "embargo", "lend"];
    let written = |line: &str| {
        let action = hoi4companion_backend::reference_candidates::trigger(line).map(|t| t.1).unwrap_or_else(|| line.trim().trim_start_matches(|c: char| "-•*– ".contains(c)).to_owned());
        let action = action.to_lowercase();
        verbs.iter().any(|v| action.starts_with(v))
    };
    for guide in guides["guides"].as_array().unwrap() {
        let country = guide["applies_to"]["country"].as_str().unwrap_or("?").to_owned();
        let compiled = request(0, "catalog_runtime_guide", json!({"handle":catalog.0,"guide":guide,"library":library})).unwrap()["definition"].clone();
        for s in steps(&compiled) {
            let p = &s["action"]["params"];
            let line = p["instructions"].as_array().filter(|a| a.len() == 1).and_then(|a| a[0].as_str()).unwrap_or("");
            let act = &p["action_resolution"];
            if act.is_null() && !(s["action"]["kind"] == "manual" && !line.contains('\n') && written(line)) {
                continue;
            }
            let state = match act["status"].as_str() {
                Some("bound") if p["foreign_parts"].is_array() => "bound_several",
                Some("bound") => "bound_one",
                Some("not_observable") => "recognized_not_observable",
                _ if p["operation_id"].is_string() => "already_bound",
                _ => "not_understood",
            };
            *buckets.entry(state.into()).or_default() += 1;
            eprintln!("ACT {country} {} [{state}] {} | {}", s["key"], line.chars().take(110).collect::<String>(),
                if act.is_null() { String::new() } else { format!("{} {}", act["action"].as_str().unwrap_or(""), if act["actions"].is_array() { act["actions"].to_string() } else { act["targets"].to_string() }) });
        }
    }
    eprintln!("written action survey: {buckets:#?}");
}

#[test]
fn event_reference_probe() {
    if std::env::var("HOI4_REAL_CATALOG").as_deref() != Ok("1") {
        return;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = root.join(".local");
    let settings: Value = serde_json::from_slice(&std::fs::read(data.join("settings.json")).unwrap()).unwrap();
    let catalog = Catalog(request(0, "catalog_open", json!({"settings":settings,"vocabulary_path":root.join("src/hoi4companion/guide_language.json")})).unwrap().as_u64().unwrap());
    let entities = request(0, "catalog_entities", json!({"handle":catalog.0})).unwrap();
    let rows = entities["entries"].as_array().unwrap();
    for (text, country) in [("Hang them…", "GER"), ("Perhaps They…", "GER"), ("We have no need…", "GER"), ("A New Bismarck…", "GER"), ("No Deal…", "SOV"),
        ("Which By God…", "SPR"), ("They can not…", "HUN"), ("McArthur and...", "USA"), ("Absolutely Not…", "AUS"), ("We cannot…", "ENG"), ("We shall…", "ENG"),
        ("Refuse to scrap…", "USA"), ("They will not…", "GER"), ("We will acc…", "SOV"), ("Clearly The Opposition…", "SWE"), ("All Of Czechoslovakia…", "GER"), ("Osterr…", "AUS")] {
        let q = text.trim_end_matches('…').trim_end_matches("...").to_lowercase();
        let named: Vec<String> = rows.iter().filter(|r| ["event_option", "event"].contains(&r["kind"].as_str().unwrap_or(""))
            && r["name"].as_str().is_some_and(|n| n.to_lowercase().starts_with(&q)))
            .take(4).map(|r| format!("{}:{}«{}» own={} recv={}", r["kind"].as_str().unwrap_or(""), r["id"].as_str().unwrap_or(""), r["name"].as_str().unwrap_or(""),
                r["country_condition"].to_string().chars().take(60).collect::<String>(), r["receivers"])).collect();
        let found = request(0, "catalog_reference_candidates", json!({"handle":catalog.0,"text":text,"country":country,"scope":null})).unwrap();
        eprintln!("PROBE2 {country} {text:?} name-prefix={named:?} | candidates={}", found.as_array().unwrap().iter().take(4)
            .map(|c| format!("{}:{}({})", c["kind"].as_str().unwrap_or(""), c["id"].as_str().unwrap_or(""), c["compatible"])).collect::<Vec<_>>().join(" "));
    }
}
