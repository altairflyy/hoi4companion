use hoi4companion_parser::process;

#[test]
fn strict_default_and_unknown_sources_stay_strict() {
    for mode in [0, 4] {
        let response: serde_json::Value = serde_json::from_slice(&process(b"unreviewed={x=1", 1, mode)).unwrap();
        assert!(response.get("error").is_some());
    }
}

#[test]
fn reviewed_fixtures_recover_without_modifying_scalar_values() {
    let source=include_bytes!("fixtures/recovery-empty.txt");
    let strict:serde_json::Value=serde_json::from_slice(&process(source,0,0)).unwrap();
    assert!(strict.get("error").is_some());
    let recovered:serde_json::Value=serde_json::from_slice(&process(source,0,4)).unwrap();
    assert!(recovered.get("error").is_none(),"{recovered}");
    assert_eq!(recovered["source_recovery"]["status"],"recovered_static_definition");
    assert_eq!(recovered["node"]["entries"][0][0],"objectTypes");
    let changed=[source.as_slice(),b" # changed"].concat();
    let result:serde_json::Value=serde_json::from_slice(&process(&changed,0,4)).unwrap();
    assert!(result.get("error").is_some());
}

// Opt-in local verification against installed content; no game sources are
// copied into fixtures or changed on disk. The normal suite remains portable.
#[test]
fn installed_reviewed_source_retains_content_and_strict_semantics() {
    let Some(path) = std::env::var_os("HOI4_REVIEWED_SOURCE") else { return; };
    let source = std::fs::read(&path).unwrap();
    let strict: serde_json::Value = serde_json::from_slice(&process(&source,0,0)).unwrap();
    assert!(strict.get("error").is_some());
    let recovered: serde_json::Value = serde_json::from_slice(&process(&source,0,4)).unwrap();
    assert!(recovered.get("error").is_none(),"{recovered}");
    assert_eq!(recovered["source_recovery"]["status"],"recovered_static_definition");
    let edits = recovered["source_recovery"]["edits"].as_array().unwrap();
    assert_eq!(edits.len(),1);
    assert_eq!(edits[0]["kind"],"close_reviewed_eof_block");
    let corrected = [source.as_slice(),b"\n}"].concat();
    let expected: serde_json::Value = serde_json::from_slice(&process(&corrected,0,0)).unwrap();
    assert_eq!(recovered["node"], expected["node"]);
    let changed = [source.as_slice(),b" # unreviewed change"].concat();
    let rejected: serde_json::Value = serde_json::from_slice(&process(&changed,0,4)).unwrap();
    assert!(rejected.get("error").is_some());
    assert_eq!(std::fs::read(path).unwrap(),source);
}
