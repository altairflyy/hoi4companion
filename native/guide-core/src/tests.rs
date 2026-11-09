use super::*;

fn state(facts: Value) -> State {
    serde_json::from_value(json!({"usable":true,"facts":facts})).unwrap()
}
fn observed(value: Value) -> Value {
    json!({"status":"observed","source":"fixture","value":value})
}
fn guide(rows: Value) -> Guide {
    serde_json::from_value(
        json!({"id":"test","dependency_policy":"explicit","tracks":{"focus":rows}}),
    )
    .unwrap()
}
fn focus(key: &str, id: &str) -> Value {
    json!({"key":key,"action":{"kind":"select_focus","entity":{"kind":"focus","id":id}}})
}
fn input(facts: Value) -> Input {
    let state = json!({"usable":true,"facts":facts});
    serde_json::from_value(json!({"schema_version":1,"state":state,"original_state":state,"progress_status":"none","run_id":null})).unwrap()
}
#[test]
fn negation_and_boolean_numeric_types_preserve_unknown() {
    let c: Condition = serde_json::from_value(
        json!({"op":"not","arg":{"op":"fact","key":"missing","test":"eq","value":false}}),
    )
    .unwrap();
    assert_eq!(c.evaluate(&state(json!({})), &BTreeMap::new()), None);
    let c: Condition =
        serde_json::from_value(json!({"op":"fact","key":"value","test":"contains","value":1}))
            .unwrap();
    assert_eq!(
        c.evaluate(
            &state(json!({"value":observed(json!([true]))})),
            &BTreeMap::new()
        ),
        Some(false)
    );
    let c: Condition =
        serde_json::from_value(json!({"op":"fact","key":"value","test":"eq","value":1})).unwrap();
    assert_eq!(
        c.evaluate(
            &state(json!({"value":observed(json!(true))})),
            &BTreeMap::new()
        ),
        None
    );
    assert_eq!(
        c.evaluate(
            &state(json!({"value":observed(json!(1.0))})),
            &BTreeMap::new()
        ),
        Some(true)
    );
}
#[test]
fn conditional_groups_have_three_valued_short_circuit_semantics() {
    for (op, known, wanted) in [
        ("all", false, Some(false)),
        ("all", true, None),
        ("any", true, Some(true)),
        ("any", false, None),
    ] {
        let c: Condition = serde_json::from_value(
            json!({"op":op,"args":[{"op":"check","key":"missing"},{"op":"check","key":"known"}]}),
        )
        .unwrap();
        assert_eq!(
            c.evaluate(
                &state(json!({})),
                &BTreeMap::from([("known".into(), known)])
            ),
            wanted
        );
    }
}
#[test]
fn compilation_rejects_missing_duplicate_and_cyclic_dependencies() {
    let mut a = focus("a", "A");
    a["depends_on"] = json!(["absent"]);
    assert!(CompiledGuide::compile(guide(json!([a]))).is_err());
    assert!(CompiledGuide::compile(guide(json!([focus("a", "A"), focus("a", "B")]))).is_err());
    let mut a = focus("a", "A");
    a["depends_on"] = json!(["b"]);
    let mut b = focus("b", "B");
    b["depends_on"] = json!(["a"]);
    assert!(CompiledGuide::compile(guide(json!([a, b]))).is_err());
}
#[test]
fn compiled_definition_never_retains_completion_between_evaluations() {
    let compiled = CompiledGuide::compile(guide(json!([focus("a", "A")]))).unwrap();
    let complete =
        input(json!({"focus.completed":observed(json!(["A"])),"focus.active":observed(json!([]))}));
    let pending =
        input(json!({"focus.completed":observed(json!([])),"focus.active":observed(json!([]))}));
    assert_eq!(
        compiled.evaluate(&complete).unwrap()["steps"][0]["status"],
        "completed"
    );
    assert_eq!(
        compiled.evaluate(&pending).unwrap()["steps"][0]["status"],
        "ready"
    );
}
#[test]
fn completed_selection_does_not_consume_another_slot_for_duplicate_entity() {
    let compiled = CompiledGuide::compile(guide(json!([
        focus("a", "A"),
        focus("again", "A"),
        focus("b", "B")
    ])))
    .unwrap();
    let input =
        input(json!({"focus.completed":observed(json!([])),"focus.active":observed(json!([]))}));
    assert_eq!(
        compiled.evaluate(&input).unwrap()["parallel"]["focus"]["steps"],
        json!(["a"])
    );
}
#[test]
fn empty_or_unqualified_sources_never_establish_completion() {
    let c = CompiledGuide::compile(guide(json!([focus("a", "A")]))).unwrap();
    for status in ["unknown", "stale"] {
        let i =
            input(json!({"focus.completed":{"status":status,"source":"fixture","value":["A"]}}));
        assert_ne!(c.evaluate(&i).unwrap()["steps"][0]["status"], "completed");
    }
    let i = input(json!({"focus.completed":{"status":"observed","source":"","value":["A"]}}));
    assert_ne!(c.evaluate(&i).unwrap()["steps"][0]["status"], "completed");
}
