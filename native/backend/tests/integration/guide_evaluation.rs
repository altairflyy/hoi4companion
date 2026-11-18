//! Guide evaluation rules asserted against the production Rust boundary.
//! These cases were previously checked only against the retired browser
//! evaluator; they now pin the behavior of the only evaluator that ships.
use hoi4companion_backend::request;
use serde_json::{Value, json};

fn try_eval(guide: &Value, state: &Value, progress: &Value) -> Result<Value, String> {
    let guide = request(0, "guide_normalize", json!({"guide":guide}))?;
    request(0, "guide_evaluate", json!({"guide":guide,"state":state,"progress":progress}))
}
fn eval(guide: &Value, state: &Value, progress: &Value) -> Value {
    try_eval(guide, state, progress).unwrap()
}
fn eval0(guide: &Value, state: &Value) -> Value {
    eval(guide, state, &Value::Null)
}
fn observed(value: Value) -> Value {
    json!({"status":"observed","source":"fixture","value":value})
}
fn statuses(result: &Value) -> Vec<String> {
    result["steps"].as_array().unwrap().iter().map(|s| s["status"].as_str().unwrap().to_owned()).collect()
}
fn step<'a>(result: &'a Value, key: &str) -> &'a Value {
    result["steps"].as_array().unwrap().iter().find(|s| s["key"] == key).unwrap_or_else(|| panic!("missing step {key}"))
}
fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/fixtures/{name}.json"));
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn normalized_state(wire: &Value) -> Value {
    request(0, "state_normalize", json!({"value":wire})).unwrap()
}

#[test]
fn region_ownership_and_control_require_matching_country_and_boolean_values() {
    let mut wire = json!({"usable":true,"sample":{"country":"SOV","country_overview":{"country":"SOV","states":{"status":"observed","items":[{"id":219,"owned":true,"controlled":false,"infrastructure":5}]}}}});
    let s = normalized_state(&wire);
    assert_eq!(s["facts"]["country.states.items.219.owned"]["value"], true);
    assert_eq!(s["facts"]["country.states.items.219.controlled"]["value"], false);
    wire["sample"]["country_overview"]["country"] = json!("GER");
    assert_ne!(normalized_state(&wire)["facts"]["country.states.items.219.owned"]["status"], "calculated");
    wire["sample"]["country_overview"]["country"] = json!("SOV");
    wire["sample"]["country_overview"]["states"]["items"][0]["owned"] = json!("true");
    assert_ne!(normalized_state(&wire)["facts"]["country.states.items.219.owned"]["status"], "calculated");
}

// ---- runtime core

#[test]
fn continuous_focuses_use_qualified_continuous_selection_and_never_native_completion() {
    let mut guide = json!({"id":"continuous","dependency_policy":"explicit","tracks":{"focus":[
        {"key":"suppress","action":{"kind":"select_focus","entity":{"kind":"focus","id":"continuous_restrict_freedom"},"params":{"focus_mode":"continuous"}}},
        {"key":"after_start","action":{"kind":"select_focus","entity":{"kind":"focus","id":"next"}},"depends_on_started":["suppress"]},
        {"key":"after_end","action":{"kind":"select_focus","entity":{"kind":"focus","id":"last"}},"depends_on":["suppress"]}]}});
    let mut state = json!({"schema_version":1,"usable":true,"facts":{"focus.active":observed(json!([])),"focus.completed":observed(json!([])),"focus.continuous":observed(json!(["continuous_restrict_freedom"]))}});
    let result = eval0(&guide, &state);
    assert_eq!(statuses(&result), ["active", "ready", "waiting"]);
    assert_eq!(result["steps"][0]["completion"]["value"], false);
    state["facts"].as_object_mut().unwrap().remove("focus.continuous");
    state["facts"]["focus.active"] = observed(json!(["continuous_restrict_freedom"]));
    assert!(eval0(&guide, &state)["steps"][0]["selection"]["value"].is_null());
    guide["tracks"]["focus"][0]["action"]["params"]["focus_mode"] = json!("invalid");
    assert!(try_eval(&guide, &state, &Value::Null).is_err());
}

#[test]
fn explicit_policy_removes_only_implicit_barriers() {
    let state = fixture("guide-runtime")[0]["state"].clone();
    let focus = |key: &str| json!({"key":key,"action":{"kind":"select_focus","entity":{"kind":"focus","id":key}}});
    let mut dependent = focus("dependent");
    dependent["depends_on"] = json!(["gate"]);
    let mut conditional = focus("conditional");
    conditional["when"] = json!({"op":"check","key":"unknown"});
    let guide = json!({"id":"generic","tracks":{"focus":[{"key":"gate","action":{"kind":"manual"}},focus("independent"),dependent,conditional]}});
    let implicit = eval0(&guide, &state);
    let mut explicit_guide = guide.clone();
    explicit_guide["dependency_policy"] = json!("explicit");
    let explicit = eval0(&explicit_guide, &state);
    assert_eq!(implicit["steps"][1]["reason"], "dependency_unknown");
    assert_eq!(explicit["steps"][1]["status"], "ready");
    assert_eq!(explicit["steps"][2]["reason"], "dependency_unknown");
    assert_eq!(explicit["steps"][3]["reason"], "condition_unknown");
    for bad in [json!(null), json!([]), json!({}), json!(1), json!("bad")] {
        let mut invalid = guide.clone();
        invalid["dependency_policy"] = bad.clone();
        assert!(try_eval(&invalid, &state, &Value::Null).is_err(), "policy {bad}");
    }
}

#[test]
fn shared_runtime_fixture_evaluation_is_repeatable() {
    for row in fixture("guide-runtime").as_array().unwrap() {
        let first = eval(&row["guide"], &row["state"], &row["progress"]);
        assert_eq!(eval(&row["guide"], &row["state"], &row["progress"]), first, "{}", row["name"]);
    }
}

#[test]
fn invalid_progress_is_rejected_and_large_dependency_chains_are_iterative() {
    let example = fixture("guide-runtime")[0].clone();
    for change in [json!({"schema_version":true}), json!({"schema_version":2}), json!({"checks":null}), json!({"checks":{"x":1}}),
        json!({"started":null}), json!({"started":[]}), json!({"started":{"x":1}}), json!({"run_id":" "}), json!({"session_id":""})] {
        let mut progress = example["progress"].clone();
        for (k, v) in change.as_object().unwrap() { progress[k] = v.clone(); }
        assert!(try_eval(&example["guide"], &example["state"], &progress).is_err(), "progress change {change}");
    }
    let mut rows: Vec<Value> = (0..2000).map(|i| json!({"key":i.to_string(),"action":{"kind":"select_focus","entity":{"kind":"focus","id":i.to_string()}},"depends_on":if i>0 {json!([(i-1).to_string()])} else {json!([])}})).collect();
    let mut state = example["state"].clone();
    state["facts"]["focus.completed"]["value"] = json!(rows[..1999].iter().map(|r| r["key"].clone()).collect::<Vec<_>>());
    let guide = json!({"id":"deep","tracks":{"focus":rows.clone()}});
    assert_eq!(eval0(&guide, &state)["steps"][1999]["status"], "ready");
    rows[0]["depends_on"] = json!(["1999"]);
    assert!(try_eval(&json!({"id":"deep","tracks":{"focus":rows}}), &state, &Value::Null).is_err());
}

#[test]
fn runs_with_the_same_step_keys_are_isolated_and_completion_rolls_back() {
    let cases = fixture("guide-runtime");
    let find = |name: &str| cases.as_array().unwrap().iter().find(|c| c["name"] == name).unwrap().clone();
    let sample = find("manual checkpoint is explicit evidence");
    assert_eq!(eval(&sample["guide"], &sample["state"], &sample["progress"])["steps"][0]["status"], "completed");
    let mut new_run = sample["progress"].clone();
    new_run["run_id"] = json!("new");
    new_run["checks"] = json!({});
    assert_eq!(eval(&sample["guide"], &sample["state"], &new_run)["steps"][0]["status"], "unverifiable");
    let complete = find("dependency completion unlocks other tracks");
    assert_eq!(eval(&complete["guide"], &complete["state"], &complete["progress"])["steps"][0]["status"], "completed");
    let earlier = &cases[0];
    assert_eq!(eval(&earlier["guide"], &earlier["state"], &earlier["progress"])["steps"][0]["status"], "ready");
}

#[test]
fn campaign_identity_controls_progress_acceptance() {
    let f = fixture("campaign-progress");
    for case in f["cases"].as_array().unwrap() {
        let mut state = f["state"].clone();
        for (k, v) in case["sample"].as_object().unwrap() { state["sample"][k] = v.clone(); }
        let result = eval(&f["guide"], &state, &f["progress"]);
        assert_eq!(result["progress_status"], case["progress_status"], "{}", case["name"]);
        assert_eq!(result["steps"][0]["status"], case["step_status"], "{}", case["name"]);
    }
}

#[test]
fn focus_criteria_evaluation_matches_recorded_results() {
    for item in fixture("focus-criteria").as_array().unwrap() {
        let result = eval0(&item["guide"], &item["state"]);
        assert_eq!(result, item["expected"], "{}", item["name"]);
    }
}

// ---- reminders

fn reminder(extra: Value) -> Value {
    let mut row = json!({"key":"method","action":{"kind":"manual"},"tracking":"reminder"});
    for (k, v) in extra.as_object().unwrap() { row[k] = v.clone(); }
    row
}
fn focus_step() -> Value {
    json!({"key":"focus","action":{"kind":"select_focus","entity":{"kind":"focus","id":"focus"}}})
}
fn reminder_live() -> Value {
    json!({"usable":true,"sample":{"country":"GER","completed_focuses":[],"current_focus":null,"researched_technologies":[],"research_slots":[]}})
}

#[test]
fn reminders_are_visible_instructions_without_confirmation_or_barriers() {
    let guide = json!({"id":"reminders","tracks":{"focus":[reminder(json!({})),focus_step()]}});
    let result = eval(&guide, &reminder_live(), &json!({"schema_version":1,"guide_id":"reminders","run_id":"r","checks":{"method":true},"started":{"method":true}}));
    let row = &result["steps"][0];
    assert_eq!(row["status"], "ready");
    assert_eq!(row["reason"], "reminder_available");
    assert_eq!(row["completion_basis"], "reminder");
    assert_eq!(row["tracking"], "reminder");
    assert!(row["completion"]["value"].is_null());
    assert_eq!(row["completion"]["reason"], "not_tracked");
    assert!(row["selection"]["value"].is_null());
    assert_eq!(result["ready"], json!(["focus"]));
    assert_eq!(result["tracks"]["focus"]["next"], "focus");
    assert_eq!(result["parallel"]["focus"]["steps"], json!(["focus"]));
}

#[test]
fn unknown_reminder_visibility_never_blocks_an_independent_recommendation() {
    let guide = json!({"id":"reminders","tracks":{"focus":[reminder(json!({"when":{"op":"fact","key":"missing","test":"eq","value":true}})),focus_step()]}});
    let result = eval0(&guide, &reminder_live());
    assert_eq!(result["steps"][0]["reason"], "condition_unknown");
    assert_eq!(result["parallel"]["focus"]["steps"], json!(["focus"]));
}

#[test]
fn reminder_may_depend_on_a_real_action() {
    let guide = json!({"id":"reminders","tracks":{"focus":[focus_step(),reminder(json!({"depends_on":["focus"]}))]}});
    let result = eval0(&guide, &reminder_live());
    assert_eq!(result["steps"][1]["status"], "waiting");
    assert_eq!(result["steps"][1]["blocked_by"], json!(["focus"]));
    let manual = json!({"id":"reminders","tracks":{"focus":[{"key":"manual","tracking":"action","action":{"kind":"manual"}}]}});
    assert_eq!(eval0(&manual, &reminder_live())["steps"][0]["reason"], "manual_confirmation_required");
}

// ---- research

fn relative_guide() -> Value {
    json!({"schema_version":1,"id":"g","title":"G","applies_to":{"country":"POL"},"tracks":{"research":[
        {"key":"r","action":{"kind":"research","entity":{"kind":"technology","id":"a"}},"relative_research":{"query":"Artillery UP","chain":["a","b","c"]}}]}})
}
fn relative_state() -> Value {
    json!({"schema_version":1,"usable":true,"session_id":"s","facts":{"country":observed(json!("POL")),"research.completed":observed(json!(["a","b"])),
        "research.active":observed(json!([])),"research.slots":observed(json!([{"technology_id":null}]))}})
}

#[test]
fn relative_research_requires_a_campaign_binding_and_retains_completed_target() {
    let (guide, state) = (relative_guide(), relative_state());
    let progress = json!({"schema_version":1,"guide_id":"g","run_id":"x","session_id":"s","checks":{},"research_bindings":{"r":"b"}});
    assert_eq!(eval(&guide, &state, &progress)["steps"][0]["status"], "completed");
    for (field, value) in [("research_bindings", json!({})), ("session_id", json!("other")), ("research_bindings", json!({"r":"unrelated"}))] {
        let mut changed = progress.clone();
        changed[field] = value;
        assert_eq!(eval(&guide, &state, &changed)["steps"][0]["status"], "unverifiable", "{field}");
    }
    let mut broken = guide.clone();
    broken["tracks"]["research"][0]["relative_research"]["chain"] = json!(["a","a"]);
    assert!(try_eval(&broken, &state, &Value::Null).is_err());
}

#[test]
fn waiting_upgrades_do_not_hide_independent_research() {
    let mut guide = relative_guide();
    let mut later = guide["tracks"]["research"][0].clone();
    later["key"] = json!("later");
    guide["tracks"]["research"].as_array_mut().unwrap().extend([later, json!({"key":"other","action":{"kind":"research","entity":{"kind":"technology","id":"d"}}})]);
    let mut state = relative_state();
    state["facts"]["research.completed"] = observed(json!(["a"]));
    state["facts"]["research.active"] = observed(json!(["b"]));
    let progress = json!({"schema_version":1,"guide_id":"g","run_id":"x","session_id":"s","checks":{},"research_bindings":{"r":"b"}});
    let result = eval(&guide, &state, &progress);
    assert_eq!(result["steps"][1]["status"], "waiting");
    assert_eq!(result["parallel"]["research"]["steps"], json!(["other"]));
    let mut future = progress.clone();
    future["research_bindings"] = json!({"r":"c"});
    assert_eq!(eval(&guide, &state, &future)["steps"][0]["status"], "waiting");
}

#[test]
fn finished_relative_chain_is_not_applicable_and_missing_evidence_stays_unknown() {
    let mut state = relative_state();
    state["facts"]["research.completed"] = observed(json!(["a","b","c"]));
    let progress = json!({"schema_version":1,"guide_id":"g","run_id":"x","session_id":"s","checks":{}});
    assert_eq!(eval(&relative_guide(), &state, &progress)["steps"][0]["status"], "not_applicable");
    state["facts"].as_object_mut().unwrap().remove("research.completed");
    assert_eq!(eval(&relative_guide(), &state, &progress)["steps"][0]["status"], "unverifiable");
}

#[test]
fn conflicting_or_foreign_session_research_facts_never_choose_a_branch() {
    let cases: Vec<Value> = serde_json::from_slice(&std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/presentation-cases.json")).unwrap()).unwrap();
    let open = cases.iter().find(|c| c["name"] == "research: open branch choice").unwrap();
    let mut conflicting = open["state"].clone();
    conflicting["facts"]["research.completed"] = observed(json!(["c1","d1"]));
    assert_eq!(eval(&open["guide"], &conflicting, &open["progress"])["parallel"]["research"]["steps"], json!([]));
    let mut other = open["progress"].clone();
    other["session_id"] = json!("other");
    assert_eq!(eval(&open["guide"], &open["state"], &other)["parallel"]["research"]["steps"], json!([]));
}

// ---- production

fn production_instruction(key: &str, n: u64, order: u64, when: Option<Value>) -> Value {
    let mut row = json!({"key":key,"title":key,"action":{"kind":"manual","params":{"category":"production","equipment_id":"sup","requested_factories":n,"production_revision":{"group":"setup","order":order},"production_selector":{"equipment_ids":["sup"],"variant_names":[],"category":"military"}}},"completion":{"op":"fact","key":format!("guide.production.{key}.requested"),"test":"eq","value":n}});
    if let Some(when) = when { row["when"] = when; }
    row
}
fn production_phases() -> Value {
    json!({"schema_version":1,"id":"phases","title":"Phased guide","dependency_policy":"explicit","applies_to":{"country":"TST"},"tracks":{"operations":[
        production_instruction("initial",1,0,None),
        production_instruction("current",7,1,Some(json!({"op":"fact","key":"focus.completed","test":"contains","value":"phase"}))),
        production_instruction("future",30,2,Some(json!({"op":"fact","key":"unknown.condition","test":"eq","value":true})))]}})
}
fn production_live() -> Value {
    json!({"schema_version":1,"usable":true,"facts":{"country":observed(json!("TST")),"focus.completed":observed(json!(["phase"])),"production.lines":observed(json!([{"id":"1","equipment":"sup","name":"Support","category":"military","requested_factories":7,"assigned_factories":3,"active_factories":3,"damaged_factories":0}]))}})
}
fn with_adoption(mut guide: Value) -> Value {
    let condition = json!({"op":"fact","key":"politics.advisors","test":"contains","value":"expert"});
    guide["tracks"]["adoptions"] = json!([{"key":"advisor","action":{"kind":"manual"},"completion":condition}]);
    guide["tracks"]["operations"][1]["action"]["params"]["production_milestones"] = json!([{"step":"advisor","token":"[\"advisor\",\"politics.advisors\",\"expert\"]","condition":condition}]);
    guide
}

#[test]
fn reached_production_phase_retires_old_layout_without_inventing_completion() {
    let result = eval0(&with_adoption(production_phases()), &production_live());
    assert_eq!(step(&result, "initial")["reason"], "production_superseded");
    assert!(step(&result, "initial")["completion"]["value"].is_null());
    assert_eq!(step(&result, "current")["reason"], "milestone_unknown");
    assert_eq!(step(&result, "current")["production_admitted"], false);
    let plain = eval0(&production_phases(), &production_live());
    assert!(plain["steps"][0]["completion"]["value"].is_null());
}

#[test]
fn production_phase_dependencies_stay_closed_without_phase_proof() {
    let mut guide = with_adoption(production_phases());
    guide["tracks"]["other"] = json!([{"key":"after","action":{"kind":"manual"},"depends_on":["current"]},{"key":"after-start","action":{"kind":"manual"},"depends_on_started":["current"]}]);
    let result = eval0(&guide, &production_live());
    assert_eq!(step(&result, "after")["reason"], "dependency_unknown");
    assert_eq!(step(&result, "after-start")["reason"], "dependency_unknown");
    let mut stale = production_live();
    stale["usable"] = json!(false);
    assert!(eval0(&guide, &stale)["steps"].as_array().unwrap().iter().all(|s| s.get("superseded_by").is_none()));
    let mut missing = production_live();
    missing["facts"].as_object_mut().unwrap().remove("focus.completed");
    assert!(step(&eval0(&guide, &missing), "initial").get("superseded_by").is_none());
    guide["tracks"]["operations"][1].as_object_mut().unwrap().remove("when");
    assert!(step(&eval0(&guide, &production_live()), "initial").get("superseded_by").is_none());
}

#[test]
fn explicit_dependencies_bind_production_revisions() {
    let mut guide = production_phases();
    guide["tracks"]["operations"][1]["depends_on"] = json!(["initial"]);
    let result = eval0(&guide, &production_live());
    assert_eq!(result["steps"][1]["status"], "waiting");
    assert!(result["steps"][0].get("superseded_by").is_none());
}

#[test]
fn unknown_revision_gates_cannot_release_cross_track_dependencies() {
    let mut guide = production_phases();
    guide["tracks"]["other"] = json!([{"key":"after","action":{"kind":"manual"},"depends_on":["current"]},{"key":"after-start","action":{"kind":"manual"},"depends_on_started":["current"]}]);
    let mut state = production_live();
    state["facts"].as_object_mut().unwrap().remove("focus.completed");
    let result = eval0(&guide, &state);
    assert!(step(&result, "current")["completion"]["value"].is_null());
    assert_eq!(step(&result, "current")["original_completion"]["value"], true);
    for key in ["after", "after-start"] { assert_eq!(step(&result, key)["reason"], "dependency_unknown"); }
}

fn remaining_line(id: &str, equipment: &str, requested: i64) -> Value {
    json!({"id":id,"equipment":equipment,"name":equipment,"category":"military","requested_factories":requested,"assigned_factories":0,"active_factories":0,"damaged_factories":0})
}
fn remaining_state(lines: Value, total: Option<i64>) -> Value {
    let mut state = json!({"schema_version":1,"usable":true,"facts":{"country":observed(json!("TST")),"production.lines":observed(lines)}});
    if let Some(total) = total { state["facts"]["country.production.effective_factories.military"] = observed(json!(total)); }
    state
}
fn remaining_guide() -> Value {
    json!({"schema_version":1,"id":"remaining","title":"Remaining factories","applies_to":{"country":"TST"},"tracks":{"operations":[{"key":"inf","action":{"kind":"manual","params":{"category":"production","quantity_mode":"remaining","production_category":"military","production_selector":{"equipment_ids":["inf"],"variant_names":[],"category":"military"}}},"completion":{"op":"fact","key":"guide.production.inf.remaining_allocated","test":"eq","value":true},"started_when":{"op":"fact","key":"guide.production.inf.started","test":"eq","value":true}}]}})
}

#[test]
fn remaining_production_follows_capacity_after_other_lines() {
    let guide = remaining_guide();
    let lines = json!([remaining_line("1","sup",3), remaining_line("2","inf",7)]);
    let row = &eval0(&guide, &remaining_state(lines.clone(), Some(10)))["steps"][0];
    assert_eq!(row["status"], "completed");
    assert_eq!(row["production"]["target"], 7);
    let row = &eval0(&guide, &remaining_state(lines, Some(11)))["steps"][0];
    assert_eq!(row["status"], "active");
    assert_eq!(row["production"]["target"], 8);
    let lines = json!([remaining_line("1","sup",3), remaining_line("2","inf",150)]);
    assert_eq!(eval0(&guide, &remaining_state(lines, Some(11)))["steps"][0]["status"], "completed");
}

#[test]
fn remaining_production_requires_unique_line_capacity_and_positive_request() {
    let guide = remaining_guide();
    for state in [remaining_state(json!([]), Some(0)), remaining_state(json!([remaining_line("1","inf",0)]), Some(0)),
        remaining_state(json!([remaining_line("1","inf",150), remaining_line("2","sup",1)]), Some(10)),
        remaining_state(json!([remaining_line("1","inf",150), remaining_line("2","inf",1)]), Some(10)),
        remaining_state(json!([remaining_line("1","inf",150)]), None), remaining_state(json!([remaining_line("1","inf",150)]), Some(-1))] {
        assert_ne!(eval0(&guide, &state)["steps"][0]["status"], "completed", "{state}");
    }
}

#[test]
fn requested_factories_include_pending_slots_and_stale_evidence_cannot_complete() {
    let guide = json!({"id":"generic","tracks":{"operations":[{"key":"order","action":{"kind":"manual","params":{"category":"production","requested_factories":10,"production_selector":{"category":null,"equipment_ids":["rifle"],"variant_names":[]}}},
        "started_when":{"op":"fact","key":"guide.production.order.started","test":"eq","value":true},"completion":{"op":"fact","key":"guide.production.order.requested","test":"eq","value":10}}]}});
    let mut state = json!({"usable":true,"sample":{"country":"TST","country_overview":{"country":"TST","production":{"status":"observed","lines":[{"id":"1","equipment":"rifle","name":"Rifle","category":"military","requested_factories":10,"assigned_factories":3,"active_factories":3,"damaged_factories":0}]}}}});
    assert_eq!(eval0(&guide, &state)["steps"][0]["status"], "completed");
    state["sample"]["country_overview"]["production"]["lines"][0]["requested_factories"] = json!(9);
    assert_eq!(eval0(&guide, &state)["steps"][0]["status"], "active");
    state["usable"] = json!(false);
    assert_ne!(eval0(&guide, &state)["steps"][0]["status"], "completed");
}

// ---- resources, doctrines and politics

#[test]
fn project_resources_distinguish_insufficient_exact_and_unknown_evidence() {
    let point = "projects.points.specialization_land";
    let guide = json!({"id":"points","tracks":{"operations":[{"key":"project","resource_costs":{point:1},"action":{"kind":"special_project","entity":{"kind":"project","id":"flame"}},"completion":{"op":"fact","key":"projects.completed","test":"contains","value":"flame"}}]}});
    let state = |amount: Value| json!({"schema_version":1,"usable":true,"session_id":"session","facts":{"country":observed(json!("GER")),"projects.completed":observed(json!([])),point:observed(amount)}});
    for (amount, available, status) in [(json!(0), json!(false), "waiting"), (json!(1), json!(true), "ready"), (json!(2), json!(true), "ready"), (json!(null), json!(null), "ready"), (json!(true), json!(null), "ready")] {
        let row = &eval0(&guide, &state(amount.clone()))["steps"][0];
        assert_eq!(row["action_readiness"]["resources_available"], available, "{amount}");
        assert_eq!(row["status"], status, "{amount}");
    }
    let mut missing = guide.clone();
    missing["tracks"]["operations"][0].as_object_mut().unwrap().remove("resource_costs");
    assert!(eval0(&missing, &state(json!(10)))["steps"][0]["action_readiness"]["resources_available"].is_null());
}

#[test]
fn experience_prices_use_their_declared_currency() {
    let guide = json!({"id":"spirit","tracks":{"operations":[{"key":"spirit","action":{"kind":"manual","params":{"category":"politics","idea_id":"loyalty","idea_group":"academy_spirit","cost_currency":"army_experience"}},"resource_costs":{"country.experience.army":20},"completion":{"op":"fact","key":"politics.ideas","test":"contains","value":"loyalty"}}]}});
    let state = |amount: Option<i64>| {
        let mut s = json!({"schema_version":1,"usable":true,"facts":{"politics.ideas":observed(json!([]))}});
        if let Some(a) = amount { s["facts"]["country.experience.army"] = observed(json!(a)); }
        s
    };
    let row = &eval0(&guide, &state(Some(20)))["steps"][0];
    assert_eq!(row["status"], "ready");
    assert_eq!(row["action_readiness"]["resources_available"], true);
    assert!(row["action_readiness"].get("political_power").is_none());
    assert_eq!(eval0(&guide, &state(Some(10)))["steps"][0]["status"], "waiting");
    assert!(eval0(&guide, &state(None))["steps"][0]["action_readiness"]["resources_available"].is_null());
}

fn doctrine_guide() -> Value {
    json!({"schema_version":1,"id":"doctrine","dependency_policy":"explicit","applies_to":{"country":"TST"},"tracks":{"operations":[
        {"key":"doctrine","action":{"kind":"manual","params":{"category":"army","doctrine_id":"grand_assault"}},"completion":{"op":"fact","key":"doctrines.selected","test":"contains","value":"grand_assault"}}]}})
}
fn doctrine_state(facts: Value) -> Value {
    let mut state = json!({"schema_version":1,"usable":true,"facts":{"country":observed(json!("TST"))}});
    for (k, v) in facts.as_object().unwrap() { state["facts"][k] = v.clone(); }
    state
}

#[test]
fn doctrine_identity_is_not_proof_of_selection_research_or_mastery() {
    let guide = doctrine_guide();
    for facts in [json!({}), json!({"research.completed":observed(json!(["grand_assault"]))}), json!({"doctrines.mastered":observed(json!(["grand_assault"]))}),
        json!({"country.experience.army":observed(json!(0))}), json!({"doctrines.selected":observed(json!("grand_assault"))})] {
        let row = &eval0(&guide, &doctrine_state(facts.clone()))["steps"][0];
        assert_ne!(row["status"], "completed", "{facts}");
        assert!(row["completion"]["value"].is_null(), "{facts}");
    }
    assert_eq!(eval0(&guide, &doctrine_state(json!({"doctrines.selected":observed(json!([]))})))["steps"][0]["completion"]["value"], false);
    let selected = doctrine_state(json!({"doctrines.selected":observed(json!(["grand_assault"]))}));
    assert_eq!(eval0(&guide, &selected)["steps"][0]["status"], "completed");
    let mut stale = selected.clone();
    stale["usable"] = json!(false);
    assert_ne!(eval0(&guide, &stale)["steps"][0]["status"], "completed");
}

#[test]
fn doctrine_milestones_admit_production_only_after_qualified_selection() {
    let mut guide = doctrine_guide();
    let condition = guide["tracks"]["operations"][0]["completion"].clone();
    guide["tracks"]["operations"].as_array_mut().unwrap().push(json!({"key":"production","action":{"kind":"manual","params":{"category":"production","production_milestones":[{"step":"doctrine","condition":condition,"token":"[\"doctrine\",\"doctrines.selected\",\"grand_assault\"]"}]}},"completion":{"op":"fact","key":"allocation.done","test":"eq","value":true}}));
    let missing = &eval0(&guide, &doctrine_state(json!({"allocation.done":observed(json!(true))})))["steps"][1];
    assert_eq!(missing["reason"], "milestone_unknown");
    assert_eq!(missing["production_admitted"], false);
    let completed = &eval0(&guide, &doctrine_state(json!({"allocation.done":observed(json!(true)),"doctrines.selected":observed(json!(["grand_assault"]))})))["steps"][1];
    assert_eq!(completed["status"], "completed");
}

#[test]
fn exact_target_and_role_control_commander_completion() {
    let rows = json!([["field_marshal","marshal","army_group","3500000001000000"],["corps_commander","general","army","3500000002000000"],["navy_leader","admiral","fleet","3d00000003000000"]]);
    let items: Vec<Value> = rows.as_array().unwrap().iter().map(|r| json!({"role":r[0],"commander_id":r[1],"target_kind":r[2],"instance_id":r[3],"name":"Native target"})).collect();
    let wire = json!({"usable":true,"sample":{"country":"TST","country_overview":{"country":"TST","politics":{"status":"partial","commander_assignments":{"schema_version":1,"status":"observed","source":"native_reciprocal_commander_assignments","items":items}}}}});
    let node = json!({"op":"fact","key":"politics.commander_assignments","test":"contains","value":"navy_leader:admiral:fleet:3d00000003000000"});
    let guide = json!({"id":"commanders","dependency_policy":"explicit","tracks":{"operations":[{"key":"assign","requirements":{"op":"fact","key":"country","test":"eq","value":"TST"},"completion":node,"action":{"kind":"manual","params":{"commander_id":"admiral","role":"navy_leader","political_binding":{"completion":node}}}}]}});
    let mut state = normalized_state(&wire);
    assert_eq!(eval0(&guide, &state)["steps"][0]["status"], "completed");
    state["facts"]["politics.commander_assignments"]["value"] = json!(["navy_leader:admiral:fleet:3d00000004000000"]);
    assert_ne!(eval0(&guide, &state)["steps"][0]["status"], "completed");
    state["facts"]["politics.commander_assignments"]["value"] = json!([]);
    assert_eq!(eval0(&guide, &state)["steps"][0]["action_readiness"]["value"], true);
    state["facts"].as_object_mut().unwrap().remove("politics.commander_assignments");
    let unknown = eval0(&guide, &state)["steps"][0]["action_readiness"]["unknown"].clone();
    assert!(unknown.as_array().unwrap().iter().any(|u| u == "commander_assignment_unobserved"), "{unknown}");
}

#[test]
fn union_of_start_and_completion_dependencies_keeps_completion_meaning() {
    let mut example = fixture("action-sequences")[1].clone();
    let operations = &mut example["guide"]["tracks"]["operations"];
    operations[1]["depends_on"] = json!(["project"]);
    operations[1].as_object_mut().unwrap().remove("depends_on_started");
    assert_eq!(eval0(&example["guide"], &example["state"])["steps"][1]["status"], "waiting");
}

// ---- foreign interactions, projects and fortifications

fn foreign_wire() -> Value {
    json!({"usable":true,"sample":{"session_id":"session","country":"ROM","country_overview":{"country":"ROM",
        "diplomacy":{"status":"observed","at_war":false,"interactions":{"status":"observed","countries":["ROM","ITA"],"improving_relations":["ITA"],"non_aggression_pacts":[]}},
        "intelligence":{"status":"observed","is_created":true,"in_creation":false,"missions":{"status":"observed","countries":["ROM","ITA"],"items":[{"id":"agent","mission_id":"diplomatic_pressure","target_country":"ITA","state":"active"}]}}}}})
}
fn foreign_guide() -> Value {
    let f = |key: &str, test: &str, value: Value| json!({"op":"fact","key":key,"test":test,"value":value});
    let current = json!({"op":"all","args":[f("diplomacy.countries","contains",json!("ITA")),f("diplomacy.improving_relations","contains",json!("ITA"))]});
    json!({"schema_version":1,"id":"guide","title":"Foreign","dependency_policy":"explicit","applies_to":{"country":"ROM"},"tracks":{"operations":[
        {"key":"relations","action":{"kind":"manual","params":{"operation_id":"improve_relations","target_country":"ITA"}},"started_when":current,"completion":{"op":"any","args":[current,f("guide.campaign.milestones","contains",json!("foreign:recorded"))]}},
        {"key":"later","action":{"kind":"manual","params":{"operation_id":"assign_operative_mission","mission_id":"diplomatic_pressure","target_country":"ITA"}},"when":f("focus.completed","contains",json!("later_focus")),"completion":f("intelligence.active_missions","contains",json!("diplomatic_pressure:ITA"))}]}})
}

#[test]
fn mission_evidence_cannot_complete_a_future_conditional_action() {
    let mut wire = foreign_wire();
    wire["sample"]["completed_focuses"] = json!([]);
    let result = eval0(&foreign_guide(), &wire);
    assert_eq!(result["steps"][0]["status"], "completed");
    assert_eq!(result["steps"][1]["status"], "waiting");
    wire["sample"]["completed_focuses"] = json!(["later_focus"]);
    assert_eq!(eval0(&foreign_guide(), &wire)["steps"][1]["status"], "completed");
}

#[test]
fn session_qualified_foreign_history_cannot_cross_sessions() {
    let mut wire = foreign_wire();
    wire["sample"]["country_overview"]["diplomacy"]["interactions"]["improving_relations"] = json!([]);
    let progress = json!({"schema_version":1,"guide_id":"guide","run_id":"run","session_id":"session","checks":{},"started":{},"campaign_conditions":{"country":"ROM","milestones":{"foreign:recorded":true}}});
    assert_eq!(eval(&foreign_guide(), &wire, &progress)["steps"][0]["status"], "completed");
    wire["sample"]["session_id"] = json!("other");
    assert_ne!(eval(&foreign_guide(), &wire, &progress)["steps"][0]["status"], "completed");
    wire["sample"]["country"] = json!("ITA");
    assert_eq!(eval(&foreign_guide(), &wire, &progress)["steps"][0]["status"], "not_applicable");
}

#[test]
fn projects_start_and_finish_from_distinct_native_states() {
    let guide = json!({"id":"generic","tracks":{"operations":[{"key":"order","action":{"kind":"manual","params":{"category":"projects"}},
        "started_when":{"op":"fact","key":"projects.active","test":"contains","value":"p"},"completion":{"op":"fact","key":"projects.completed","test":"contains","value":"p"}}]}});
    let mut state = json!({"usable":true,"sample":{"country":"TST","country_overview":{"country":"TST","projects":{"status":"observed","items":[{"id":"p","active":true,"completed":false}]}}}});
    assert_eq!(eval0(&guide, &state)["steps"][0]["status"], "active");
    state["sample"]["country_overview"]["projects"]["items"][0] = json!({"id":"p","active":false,"completed":true});
    assert_eq!(eval0(&guide, &state)["steps"][0]["status"], "completed");
    state["sample"]["country_overview"]["projects"]["items"][0].as_object_mut().unwrap().remove("completed");
    assert_ne!(eval0(&guide, &state)["steps"][0]["status"], "completed");
}

#[test]
fn native_fort_levels_must_be_complete_qualified_and_unique() {
    let guide = fixture("fortifications")[0]["guide"].clone();
    let mut wire = json!({"usable":true,"sample":{"country":"HUN","country_overview":{"country":"HUN","fortifications":{"status":"observed","items":[{"id":10,"bunker":1},{"id":11,"bunker":1}],"borders_status":"observed","borders":{"YUG":[10,11]},"controllers":{"10":"HUN","20":"YUG"}}}}});
    assert_eq!(eval0(&guide, &wire)["steps"][0]["status"], "completed");
    let items = &mut wire["sample"]["country_overview"]["fortifications"]["items"];
    items.as_array_mut().unwrap().push(json!({"id":10,"bunker":1}));
    assert_ne!(eval0(&guide, &wire)["steps"][0]["status"], "completed");
    let items = &mut wire["sample"]["country_overview"]["fortifications"]["items"];
    items.as_array_mut().unwrap().pop();
    items[0]["bunker"] = json!(true);
    assert_ne!(eval0(&guide, &wire)["steps"][0]["status"], "completed");
}

#[test]
fn additional_fort_levels_use_the_run_relative_baseline() {
    let case = fixture("fortifications")[0].clone();
    let mut guide = case["guide"].clone();
    let row = &mut guide["tracks"]["operations"][0];
    row["action"]["params"] = json!({"category":"construction","building_id":"bunker","province_id":10,"quantity":2,"quantity_mode":"additional"});
    row["completion"] = json!({"op":"fact","key":"guide.construction.fort.added","test":"gte","value":2});
    row["started_when"] = json!({"op":"fact","key":"construction.queued","test":"contains","value":"province:10:bunker"});
    let mut state = case["state"].clone();
    state["session_id"] = json!("session");
    state["facts"]["country.fortifications.items.10.bunker"]["value"] = json!(5);
    let mut progress = json!({"schema_version":1,"guide_id":guide["id"],"run_id":"r","session_id":"session","checks":{},"baselines":{"fort":{"fact_key":"country.fortifications.items.10.bunker","value":3,"country":"HUN","quantity":2,"source":"fixture"}}});
    let row = &eval(&guide, &state, &progress)["steps"][0];
    assert_eq!(row["status"], "completed");
    assert_eq!(row["fortifications"]["target_level"], 5);
    progress["baselines"]["fort"]["country"] = json!("OTHER");
    assert_ne!(eval(&guide, &state, &progress)["steps"][0]["status"], "completed");
}

#[test]
fn malformed_construction_baselines_are_rejected() {
    let case = fixture("construction-progress")[0].clone();
    for baselines in [json!(null), json!([]), json!({"factory":{"value":true}}), json!({"factory":{"value":-1}})] {
        let mut progress = case["progress"].clone();
        progress["baselines"] = baselines.clone();
        assert!(try_eval(&case["guide"], &case["state"], &progress).is_err(), "{baselines}");
    }
}

// ---- production context (designs, selectors and relative baselines)

fn recipe() -> Value {
    json!({"modules":{"gun":"small_cannon","engine":"engine_1","spare":"empty"},"upgrades":{"armor":"0","engine":"2"}})
}
fn live_design() -> Value {
    json!({"status":"observed","modules":{"engine":"engine_1","gun":"small_cannon"},"upgrades":{"engine":2}})
}
fn production_line(id: &str, design: Option<Value>) -> Value {
    let mut line = json!({"id":id,"equipment":"light_tank_chassis_1","name":"Renamed vehicle","category":"military","requested_factories":7,"assigned_factories":4,"active_factories":3,"damaged_factories":1});
    if let Some(design) = design { line["design"] = design; }
    line
}
fn tank_step() -> Value {
    json!({"key":"tank","action":{"kind":"manual","params":{"category":"production","requested_factories":7,"production_selector":{"equipment_ids":["light_tank_chassis_1"],"variant_names":["Guide name"],"category":"military"},"design_reference":recipe()}},"completion":{"op":"fact","key":"guide.production.tank.requested","test":"eq","value":7}})
}
fn production_projection(kind: &str, args: Value) -> Value {
    let mut args = args;
    args["kind"] = json!(kind);
    request(0, "production_context", args).unwrap()
}
/// Returns `[state, details]` exactly as the backend uses them during evaluation.
fn production_details(lines: Value, step: Value) -> Value {
    let state = normalized_state(&json!({"schema_version":1,"usable":true,"facts":{"country":observed(json!("TST")),"production.lines":observed(lines),"country.production.effective_factories.military":observed(json!(20))}}));
    production_projection("production", json!({"guide":{"tracks":{"production":[step]}},"state":state,"baselines":{}}))
}

#[test]
fn recipe_selectors_ignore_display_names_but_keep_equipment_and_category() {
    assert_eq!(production_projection("selector", json!({"step":tank_step()})), json!({"category":"military","equipment_ids":["light_tank_chassis_1"],"variant_names":["Guide name"],"design":{"modules":{"gun":"small_cannon","engine":"engine_1"},"upgrades":{"engine":2}}}));
    let mut wrong = production_line("wrong", Some(live_design()));
    wrong["design"]["modules"]["gun"] = json!("machine_gun");
    wrong["name"] = json!("Guide name");
    let details = production_details(json!([production_line("one", Some(live_design())), wrong]), tank_step());
    assert_eq!(details[1]["tank"]["requested"], 7);
}

#[test]
fn one_unknown_same_equipment_design_makes_recipe_counts_unknown() {
    let result = production_details(json!([production_line("one", Some(live_design())), production_line("unknown", Some(json!({"status":"unavailable"})))]), tank_step());
    assert!(result[0]["facts"]["guide.production.tank.requested"]["value"].is_null());
    assert!(result[0]["facts"]["guide.production.tank.started"]["value"].is_null());
    let detail = &result[1]["tank"];
    assert_eq!(detail["design_check"]["status"], "unknown");
    assert_eq!(detail["lines"], json!([]));
    assert!(detail["requested"].is_null());
    let missing = production_details(json!([production_line("one", None)]), tank_step());
    assert_eq!(missing[1]["tank"]["design_check"]["status"], "unknown");
}

#[test]
fn different_recipes_are_known_zero_and_unrelated_unknown_designs_are_excluded() {
    let mut wrong = production_line("one", Some(live_design()));
    wrong["design"]["modules"]["gun"] = json!("machine_gun");
    let detail = production_details(json!([wrong]), tank_step())[1]["tank"].clone();
    assert_eq!(detail["assigned"], 0);
    assert_eq!(detail["design_check"]["status"], "different");
    let mut unrelated = production_line("other", Some(json!({"status":"unavailable"})));
    unrelated["equipment"] = json!("other");
    let detail = production_details(json!([unrelated]), tank_step())[1]["tank"].clone();
    assert_eq!(detail["assigned"], 0);
    assert_eq!(detail["design_check"], json!({"status":"missing","candidates":[]}));
}

#[test]
fn generic_selectors_keep_variant_name_matching_without_a_recipe() {
    let mut step = tank_step();
    step["action"]["params"].as_object_mut().unwrap().remove("design_reference");
    let detail = production_details(json!([production_line("one", Some(live_design()))]), step.clone())[1]["tank"].clone();
    assert_eq!(detail["requested"], 0);
    assert!(detail.get("design_check").is_none());
    let mut renamed = production_line("one", Some(live_design()));
    renamed["name"] = json!("Guide-name");
    assert_eq!(production_details(json!([renamed]), step.clone())[1]["tank"]["requested"], 7);
    step["action"]["params"]["production_selector"]["variant_names"] = json!([]);
    assert_eq!(production_details(json!([production_line("one", Some(live_design()))]), step)[1]["tank"]["requested"], 7);
}

#[test]
fn relative_baseline_identity_normalizes_recipes_and_sorts_keys() {
    let mut step = tank_step();
    let params = &mut step["action"]["params"];
    params["quantity_mode"] = json!("additional");
    params["requested_factories"] = json!(3);
    params["design_reference"] = json!({"modules":{"2":"two","10":"ten","gun":"small_cannon"},"upgrades":{"z":"0","engine":"2"}});
    step["completion"] = json!({"op":"fact","key":"guide.production.tank.added","test":"gte","value":3});
    let spec = production_projection("relative", json!({"step":step.clone()}));
    assert_eq!(spec["fact_key"], r#"production_requested:{"category":"military","design":{"modules":{"10":"ten","2":"two","gun":"small_cannon"},"upgrades":{"engine":2}},"equipment_ids":["light_tank_chassis_1"],"variant_names":["Guide name"]}"#);
    step["action"]["params"]["design_reference"] = json!({"upgrades":{"engine":2},"modules":{"gun":"small_cannon","10":"ten","2":"two","spare":"empty"}});
    assert_eq!(production_projection("relative", json!({"step":step.clone()}))["fact_key"], spec["fact_key"]);
    step["action"]["params"]["design_reference"]["modules"]["gun"] = json!("other");
    assert_ne!(production_projection("relative", json!({"step":step.clone()}))["fact_key"], spec["fact_key"]);
    step["action"]["params"]["design_reference"] = json!({"modules":{}});
    assert!(production_projection("selector", json!({"step":step.clone()})).is_null());
    assert!(production_projection("relative", json!({"step":step})).is_null());
}

#[test]
fn unknown_designs_cannot_satisfy_remaining_allocation_or_priority() {
    let mut step = tank_step();
    step["action"]["params"]["quantity_mode"] = json!("remaining");
    step["action"]["params"]["production_priority"] = json!("last");
    let result = production_details(json!([production_line("unknown", Some(json!({"status":"unavailable"})))]), step);
    let detail = &result[1]["tank"];
    assert!(detail["target"].is_null());
    assert!(detail["priority_satisfied"].is_null());
    assert!(result[0]["facts"]["guide.production.tank.remaining_allocated"]["value"].is_null());
    assert!(result[0]["facts"]["guide.production.tank.priority_satisfied"]["value"].is_null());
}

// ---- national aircraft model names (designer chassis)

/// Catalog rows as the real catalog exports them (trimmed aliases).
fn fighter_catalog() -> Value {
    json!({"entries":[
        {"name":"Fighter","aliases":["small plane airframe","Fighters"],"kind":"equipment","id":"small_plane_airframe","family_id":"small_plane_airframe","production_ui_category":"interface_category_air","equipment_ids":["fighter_equipment_0","fighter_equipment_1","fighter_equipment_2","fighter_equipment_3","small_plane_airframe_0","small_plane_airframe_1","small_plane_airframe_2","small_plane_airframe_3","small_plane_airframe_4","small_plane_airframe_5"],"is_archetype":true,"country_aliases":{}},
        {"name":"Basic Small Airframe","aliases":["Fighter I","small plane airframe 1"],"kind":"equipment","id":"small_plane_airframe_1","family_id":"small_plane_airframe","production_ui_category":"interface_category_air","equipment_ids":["small_plane_airframe_1"],"is_archetype":false,"country_aliases":{}},
        {"name":"Fighter I","aliases":["Polikarpov I-16","Messerschmitt Bf 109","Fighter I","fighter equipment 1"],"kind":"equipment","id":"fighter_equipment_1","family_id":"small_plane_airframe","production_ui_category":"interface_category_air","equipment_ids":["fighter_equipment_1"],"is_archetype":false,"country_aliases":{"GER":["Messerschmitt Bf 109","Bf 109"],"SOV":["Polikarpov I-16","I-16"]}},
        {"name":"Infantry Equipment I","aliases":["Infantry Equipment"],"kind":"equipment","id":"infantry_equipment_1","family_id":"infantry_equipment","equipment_ids":["infantry_equipment_1"],"is_archetype":false,"country_aliases":{}}]})
}
fn bind_fighter(name: &str, country: &str) -> Value {
    let mut args = fighter_catalog();
    args["step"] = json!({"key":"word-0002.target-2","action":{"kind":"manual","params":{"category":"production","equipment_name":name,"requested_factories":5}}});
    args["country"] = json!(country);
    args["library"] = json!({});
    args["declared"] = json!({});
    request(0, "import_bind_production", args).unwrap()
}
/// Lines exactly as the live observer reported them for a Soviet game (By
/// Blood Alone designer): the I-16 is a design on `small_plane_airframe_1`.
fn soviet_lines(i16_name: &str) -> Value {
    json!([
        {"id":"1","equipment":"support_equipment_1","name":"support_equipment_1","category":"military","requested_factories":5,"assigned_factories":5,"active_factories":5,"damaged_factories":0,"output_per_day":8.5,"design":{"status":"observed","modules":{},"upgrades":{}}},
        {"id":"2","equipment":"small_plane_airframe_1","name":i16_name,"category":"military","requested_factories":5,"assigned_factories":5,"active_factories":5,"damaged_factories":0,"output_per_day":1.15,"design":{"status":"observed","modules":{"fixed_main_weapon_slot":"aircraft_cannon_1_2x","fixed_auxiliary_weapon_slot_1":"aircraft_cannon_1_2x","engine_type_slot":"engine_2_1x"},"upgrades":{}}},
        {"id":"7","equipment":"infantry_equipment_1","name":"infantry_equipment_1","category":"military","requested_factories":150,"assigned_factories":33,"active_factories":33,"damaged_factories":0,"output_per_day":276.9,"design":{"status":"observed","modules":{},"upgrades":{}}}])
}

#[test]
fn national_fighter_name_matches_the_designer_line_of_that_name() {
    let bound = bind_fighter("I-16", "SOV");
    assert_eq!(bound["reason"], "production_live_assignment");
    let step = bound["step"].clone();
    let selector = &step["action"]["params"]["production_selector"];
    assert!(selector["equipment_ids"].as_array().unwrap().contains(&json!("small_plane_airframe_1")));
    assert!(selector["equipment_ids"].as_array().unwrap().contains(&json!("fighter_equipment_1")));
    assert_eq!(selector["variant_names"], json!(["I-16", "Polikarpov I-16", "fighter_equipment_1"]));
    let result = production_details(soviet_lines("I-16"), step.clone());
    let detail = &result[1]["word-0002.target-2"];
    assert_eq!(detail["requested"], 5);
    assert_eq!(detail["lines"][0]["id"], "2");
    assert_eq!(detail["reason"], "");
    assert_eq!(result[0]["facts"]["guide.production.word-0002.target-2.requested"]["value"], 5);
    // Legacy (no designer) line of the same equipment still matches by its id name.
    let legacy = json!([{"id":"9","equipment":"fighter_equipment_1","name":"fighter_equipment_1","category":"military","requested_factories":5,"assigned_factories":5,"active_factories":5,"damaged_factories":0}]);
    assert_eq!(production_details(legacy, step.clone())[1]["word-0002.target-2"]["requested"], 5);
    // Another fighter design on the same chassis is not the I-16.
    let other = production_details(soviet_lines("I-153"), step)[1]["word-0002.target-2"].clone();
    assert_eq!(other["requested"], 0);
    assert_eq!(other["reason"], "design_name_mismatch");
    assert_eq!(other["near_lines"], json!([{"line_id":"2","name":"I-153","equipment":"small_plane_airframe_1","category":"military","why":"design_name_mismatch"}]));
}

#[test]
fn factory_counts_are_a_minimum_unless_the_guide_asks_for_exactly() {
    let step = bind_fighter("I-16", "SOV")["step"].clone();
    assert_eq!(step["completion"]["test"], "gte");
    let status = |step: &Value, requested: u64| {
        let mut lines = soviet_lines("I-16");
        lines[1]["requested_factories"] = json!(requested);
        let state = production_details(lines, step.clone())[0].clone();
        let guide = json!({"id":"min","dependency_policy":"explicit","tracks":{"operations":[step]}});
        eval0(&guide, &state)["steps"][0]["status"].as_str().unwrap().to_owned()
    };
    // A later guide step raises the I-16 line to 15 on purpose: 5 is reached.
    assert_eq!(status(&step, 15), "completed");
    assert_eq!(status(&step, 5), "completed");
    assert_ne!(status(&step, 4), "completed");
    let mut exact = step.clone();
    exact["action"]["params"]["assignment_test"] = json!("eq");
    exact["completion"]["test"] = json!("eq");
    assert_ne!(status(&exact, 15), "completed");
}

#[test]
fn generic_or_foreign_fighter_names_keep_the_exact_equipment() {
    let generic = bind_fighter("Fighter I", "SOV");
    assert_eq!(generic["step"]["action"]["params"]["production_selector"]["variant_names"], json!([]));
    let foreign = bind_fighter("Bf 109", "SOV");
    let ids = &foreign["step"]["action"]["params"]["production_selector"]["equipment_ids"];
    assert!(ids.is_null() || *ids == json!(["fighter_equipment_1"]), "{foreign}");
}

#[test]
fn unmatched_production_targets_state_why() {
    let bound = bind_fighter("I-16", "SOV")["step"].clone();
    let lines = json!([soviet_lines("I-16")[0].clone()]);
    let detail = production_details(lines, bound)[1]["word-0002.target-2"].clone();
    assert_eq!(detail["reason"], "line_missing");
    assert_eq!(detail["near_lines"], json!([]));
    // Equipment present on a naval line: the category is what differs.
    let mut step = tank_step();
    step["action"]["params"].as_object_mut().unwrap().remove("design_reference");
    step["action"]["params"]["production_selector"]["variant_names"] = json!([]);
    let mut naval = production_line("one", None);
    naval["category"] = json!("naval");
    let detail = production_details(json!([naval]), step)[1]["tank"].clone();
    assert_eq!(detail["reason"], "category_mismatch");
}

// ---- naval production lines

fn naval_catalog() -> Value {
    json!({"vocabulary":{"abbreviations":{"production":{},"naval":{"BBe":"empty battleship"}}},"entries":[
        {"name":"Convoy","aliases":["Convoys"],"kind":"equipment","id":"convoy","family_id":"convoy","production_ui_category":"interface_category_naval","equipment_ids":["convoy_1"],"is_archetype":true,"country_aliases":{}},
        {"name":"Heavy Ship","aliases":["Heavy Ships","ship hull heavy"],"kind":"equipment","id":"ship_hull_heavy","family_id":"ship_hull_heavy","production_ui_category":"interface_category_naval","equipment_ids":["ship_hull_heavy_1","ship_hull_heavy_2","battleship_1","battle_cruiser_1"],"is_archetype":true,"country_aliases":{}},
        {"name":"Battleship I","aliases":["Corazzata I"],"kind":"equipment","id":"battleship_1","family_id":"ship_hull_heavy","equipment_ids":["battleship_1"],"is_archetype":false,"country_aliases":{}},
        {"name":"Battlecruiser I","aliases":[],"kind":"equipment","id":"battle_cruiser_1","family_id":"ship_hull_heavy","equipment_ids":["battle_cruiser_1"],"is_archetype":false,"country_aliases":{}},
        {"name":"Modern Battleship","aliases":["Modern Battleships"],"kind":"equipment","id":"modern_battleship","family_id":"ship_hull_heavy","equipment_ids":["modern_battleship"],"is_archetype":false,"country_aliases":{}}]})
}
fn naval_command(text: &str) -> Value {
    let mut args = naval_catalog();
    args["text"] = json!(text);
    args["country"] = json!("SOV");
    args["library"] = json!({});
    args["declared"] = json!({});
    request(0, "import_production_command", args).unwrap()
}

#[test]
fn navy_lines_become_naval_production_targets_with_all_remaining_dockyards() {
    let parsed = naval_command("Navy: build convoys:5 & BBe: ALL");
    let targets = parsed["params"]["production_targets"].as_array().unwrap_or_else(|| panic!("not parsed: {parsed}"));
    assert_eq!(targets.len(), 2, "{parsed}");
    assert_eq!(targets[0]["requested_factories"], 5);
    assert_eq!(targets[0]["production_selector"]["category"], "naval");
    assert_eq!(targets[1]["quantity_mode"], "remaining");
    assert_eq!(targets[1]["production_selector"]["category"], "naval");
    // A design name with "&" and no quantity before it stays one item.
    assert!(naval_command("Navy: Leningrad & Minsk Class").is_null());
}

#[test]
fn naval_all_takes_the_dockyards_left_after_other_naval_lines() {
    let parsed = naval_command("Navy: build convoys:5 & BBe: ALL");
    let step = json!({"key":"navy","action":{"kind":"manual","params":parsed["params"].clone()}});
    let bound = request(0, "import_bind_production", {
        let mut args = naval_catalog();
        args["step"] = step;
        args["country"] = json!("SOV");
        args["library"] = json!({});
        args["declared"] = json!({});
        args
    }).unwrap();
    let step = bound["step"].clone();
    let line = |id: &str, equipment: &str, category: &str, requested: u64| json!({"id":id,"equipment":equipment,"name":equipment,"category":category,
        "requested_factories":requested,"assigned_factories":requested,"active_factories":requested,"damaged_factories":0,"design":{"status":"observed","modules":{},"upgrades":{}}});
    let details = |lines: Value| {
        let state = normalized_state(&json!({"schema_version":1,"usable":true,"facts":{"country":observed(json!("SOV")),
            "production.lines":observed(lines),"country.production.effective_factories.military":observed(json!(40)),
            "country.production.effective_factories.dockyards":observed(json!(15))}}));
        production_projection("production", json!({"guide":{"tracks":{"operations":[step.clone()]}},"state":state,"baselines":{}}))
    };
    // A land line never counts against the dockyards.
    let result = details(json!([line("1","convoy_1","naval",5),line("2","infantry_equipment_1","military",30),line("3","ship_hull_heavy_1","naval",10)]));
    let all = &result[1]["navy.target-2"];
    assert_eq!(all["target"], 10, "{result}");
    assert_eq!(all["reason"], "");
    assert_eq!(result[0]["facts"]["guide.production.navy.target-2.remaining_allocated"]["value"], true);
    // The ALL line must be the last naval line.
    let result = details(json!([line("3","ship_hull_heavy_1","naval",10),line("1","convoy_1","naval",5)]));
    assert_eq!(result[1]["navy.target-2"]["reason"], "remaining_line_not_last");
}

#[test]
fn up_steps_follow_the_guide_order_when_the_guide_is_loaded_late() {
    // Engines UP three times; the nation starts with e1. Loaded late, with e2
    // and e3 already done: the first two UP steps are credited, the third asks e4.
    let up = |key: &str| json!({"key":key,"action":{"kind":"research","entity":{"kind":"technology","id":"e1"}},"relative_research":{"query":"Engines UP","chain":["e1","e2","e3","e4","e5"]}});
    let mut guide = json!({"schema_version":1,"id":"g","title":"G","applies_to":{"country":"POL"},"tracks":{"research":[up("u1"),up("u2"),up("u3")]}});
    let mut state = relative_state();
    state["facts"]["research.completed"] = observed(json!(["e1","e2","e3"]));
    let record = json!({"schema_version":1,"guide_id":"g","run_id":"x","session_id":"s","country":"POL","checks":{}});
    let observe = |guide: &Value, record: &Value| {
        request(0, "guide_observe", json!({"guide":guide,"state":state,"record":record,"runs":{}})).unwrap()["record"].clone()
    };
    // Without start research the old rule stands: the first UP takes the next missing level.
    let old = observe(&guide, &record);
    assert_eq!(old["research_bindings"]["u1"], "e4");
    guide["research_start"] = json!({"researched":["e1"],"unknown":[]});
    let bound = observe(&guide, &record);
    assert_eq!(bound["research_bindings"], json!({"u1":"e2","u2":"e3","u3":"e4"}));
    // A wrong earlier binding is corrected, and the evaluation follows the guide.
    let mut shifted = record.clone();
    shifted["research_bindings"] = json!({"u1":"e4"});
    let fixed = observe(&guide, &shifted);
    assert_eq!(fixed["research_bindings"]["u1"], "e2");
    let result = eval(&guide, &state, &fixed);
    assert_eq!(statuses(&result), ["completed", "completed", "ready"]);
    // A level whose start state is unknown keeps the observation-time rule.
    guide["research_start"] = json!({"researched":["e1"],"unknown":["e2"]});
    let unsure = observe(&guide, &record);
    assert_eq!(unsure["research_bindings"]["u1"], "e4");
}
