use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn standalone_protocol_recovers_after_invalid_input_without_python() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hoi4-parser-cli"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{\"schema_version\":2,\"source\":\"x=yes\"}\n{\"schema_version\":1,\"source\":\"x=001 x=002\"}\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let lines = String::from_utf8(output.stdout).unwrap();
    let values: Vec<serde_json::Value> = lines
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(values.len(), 2);
    assert!(values[0].get("error").is_some());
    assert_eq!(
        values[1]["node"]["entries"],
        serde_json::json!([["x", "001"], ["x", "002"]])
    );
}

#[test]
fn standalone_exports_windows1252_and_recovers_after_invalid_bytes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hoi4-parser-cli"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let requests = [
        serde_json::json!({"schema_version":1,"source_bytes":[256]}),
        serde_json::json!({"schema_version":1,"source_bytes":b"name=\"Citt\xe0\" a=1 a=2".to_vec(),"encoding":"windows-1252","format":"jomini-json"}),
    ];
    let mut input = child.stdin.take().unwrap();
    for request in requests {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let lines = String::from_utf8(output.stdout).unwrap();
    let values: Vec<serde_json::Value> = lines
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(values[0].get("error").is_some());
    assert_eq!(values[1]["source_encoding"], "windows-1252");
    let exported: serde_json::Value =
        serde_json::from_str(values[1]["json"].as_str().unwrap()).unwrap();
    assert_eq!(
        exported["val"],
        serde_json::json!([["name", "Città"], ["a", "1"], ["a", "2"]])
    );
}
