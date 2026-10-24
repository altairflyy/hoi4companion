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
