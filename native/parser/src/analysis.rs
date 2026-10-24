//! Bounded local process bridge to the upstream .NET CWTools engine.
//! No game process access, shell invocation, or writes to installation files.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    schema_version: u32,
    rules_dir: String,
    parser_library: String,
    files: BTreeMap<String, String>,
}

fn validate(data: &[u8]) -> Result<Request, String> {
    if data.len() > 16_000_000 {
        return Err("CWTools request exceeds 16 MB".into());
    }
    let request: Request = serde_json::from_slice(data).map_err(|e| e.to_string())?;
    if request.schema_version != 1
        || !Path::new(&request.rules_dir).is_absolute()
        || !Path::new(&request.parser_library).is_absolute()
    {
        return Err("Invalid CWTools request".into());
    }
    if request.files.iter().any(|(logical, source)| {
        logical.is_empty()
            || logical.contains('\\')
            || logical
                .split('/')
                .any(|s| s.is_empty() || s == ".." || s.contains(':'))
            || !Path::new(source).is_absolute()
    }) {
        return Err("Invalid CWTools file path".into());
    }
    Ok(request)
}

pub fn run(host: &Path, data: &[u8], timeout: Duration) -> Result<Vec<u8>, String> {
    validate(data)?;
    if !host.is_absolute() || !host.is_file() {
        return Err("CWTools host missing; run scripts/build-cwtools.ps1".into());
    }
    let mut command = Command::new(host);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let mut output = child.stdout.take().ok_or("No CWTools stdout")?;
    // Drain concurrently so large diagnostics cannot deadlock the child.
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        output
            .by_ref()
            .take(64_000_001)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let mut input = child.stdin.take().ok_or("No CWTools stdin")?;
    if let Err(e) = input.write_all(data) {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        return Err(e.to_string());
    }
    drop(input);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let bytes = reader
                    .join()
                    .map_err(|_| "CWTools reader panic")?
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 64_000_000 {
                    return Err("CWTools response exceeds 64 MB".into());
                }
                let value: serde_json::Value = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("Invalid CWTools response: {e}"))?;
                if let Some(error) = value["error"].as_str() {
                    return Err(format!("CWTools: {error}"));
                }
                if !status.success()
                    || value["schema_version"] != 1
                    || value["engine"] != "cwtools"
                    || value["engine_revision"] != "b377453dee803f9258be92cfc49896d09039702d"
                {
                    return Err("CWTools process/protocol version failure".into());
                }
                return Ok(bytes);
            }
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(25)),
            other => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(match other {
                    Err(e) => e.to_string(),
                    _ => "CWTools analysis timed out".into(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_version_traversal_relative_and_unknown_fields() {
        let root = std::env::temp_dir();
        let valid = serde_json::json!({"schema_version":1,"rules_dir":root,"parser_library":root.join("parser.dll"),"files":{"events/a.txt":root.join("a.txt")}});
        assert!(validate(&serde_json::to_vec(&valid).unwrap()).is_ok());
        for change in [
            serde_json::json!({"schema_version":2}),
            serde_json::json!({"files":{"../a.txt":root.join("a.txt")}}),
            serde_json::json!({"rules_dir":"relative"}),
            serde_json::json!({"command":"anything"}),
        ] {
            let mut value = valid.clone();
            for (k, v) in change.as_object().unwrap() {
                value[k] = v.clone();
            }
            assert!(validate(&serde_json::to_vec(&value).unwrap()).is_err());
        }
    }
    #[test]
    fn missing_host_is_explicit() {
        let data = serde_json::to_vec(
            &serde_json::json!({"schema_version":1,"rules_dir":std::env::temp_dir(),"parser_library":std::env::temp_dir().join("parser.dll"),"files":{}}),
        )
        .unwrap();
        assert!(
            run(Path::new("missing"), &data, Duration::from_secs(1))
                .unwrap_err()
                .contains("host missing")
        );
    }
}
