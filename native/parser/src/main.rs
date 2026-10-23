use std::io::{self, BufRead, Write};

fn main() {
    // JSONL protocol also makes the native parser independently usable without
    // Python. No arbitrary commands, filesystem paths or network are accepted.
    let stdin = io::stdin();
    let mut out = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let result = line.map_err(|e| e.to_string()).and_then(|line| {
            let value: serde_json::Value =
                serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if value["schema_version"] != 1 {
                return Err("Unsupported request schema".into());
            }
            let encoding = match value["encoding"].as_str().unwrap_or("auto") {
                "auto" => 0,
                "utf-8" => 1,
                "windows-1252" => 2,
                _ => return Err("Unsupported encoding".into()),
            };
            let mode = match value["format"].as_str().unwrap_or("ast") {
                "ast" => 0,
                "jomini-json" => match value["duplicate_keys"]
                    .as_str()
                    .unwrap_or("key_value_pairs")
                {
                    "key_value_pairs" => 1,
                    "group" => 2,
                    "preserve" => 3,
                    _ => return Err("Unsupported duplicate key mode".into()),
                },
                _ => return Err("Unsupported output format".into()),
            };
            if value.get("source").is_some() && value.get("source_bytes").is_some() {
                return Err("Provide source or source_bytes, not both".into());
            }
            let source = if let Some(text) = value["source"].as_str() {
                if encoding == 2 {
                    return Err("A decoded JSON string must use UTF-8".into());
                }
                text.as_bytes().to_vec()
            } else {
                value["source_bytes"]
                    .as_array()
                    .ok_or("Source must be a string or source_bytes array")?
                    .iter()
                    .map(|byte| {
                        byte.as_u64()
                            .and_then(|n| u8::try_from(n).ok())
                            .ok_or("Invalid byte")
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            Ok(hoi4companion_parser::process(&source, encoding, mode))
        });
        let bytes = result.unwrap_or_else(|e| {
            serde_json::to_vec(&serde_json::json!({"schema_version":1,"error":e})).unwrap()
        });
        if out
            .write_all(&bytes)
            .and_then(|_| out.write_all(b"\n"))
            .and_then(|_| out.flush())
            .is_err()
        {
            break;
        }
    }
}
