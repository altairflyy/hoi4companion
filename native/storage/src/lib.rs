//! Existing progress format, transactional writes and optimistic concurrency.
//! A request owns and closes its SQLite connection before returning.
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::ffi::{CString, c_char};
use std::path::Path;
use std::time::Duration;
pub mod progress;
pub mod decision_history;
mod progress_ffi;
const MAX_TEXT: usize = 16 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Init,
    Load,
    Save,
}
#[derive(Deserialize)]
struct Request {
    schema_version: u32,
    op: Operation,
    path: String,
    #[serde(default)]
    expected_revision: Option<i64>,
    #[serde(default)]
    payload: Option<String>,
}
fn execute(request: Request) -> Result<Value, String> {
    if request.schema_version != 1 || request.path.is_empty() || request.path.contains('\0') {
        return Err("Invalid storage request".into());
    }
    let path = Path::new(&request.path);
    if matches!(request.op, Operation::Init)
        && let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Validate data before opening a writable connection.
    if matches!(request.op, Operation::Save) {
        let payload = request
            .payload
            .as_deref()
            .ok_or("Missing progress snapshot")?;
        if payload.chars().count() > MAX_TEXT {
            return Err("Progress snapshot exceeds limit".into());
        }
        serde_json::from_str::<Value>(payload).map_err(|e| e.to_string())?;
        if request.expected_revision.is_none_or(|r| r < 0) {
            return Err("Invalid storage revision".into());
        }
    }
    let mut db = Connection::open(path).map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    db.pragma_update(None, "synchronous", "FULL")
        .map_err(|e| e.to_string())?;
    match request.op {
        Operation::Init => {
            db.execute_batch("CREATE TABLE IF NOT EXISTS progress (id INTEGER PRIMARY KEY CHECK(id=1), version INTEGER NOT NULL, revision INTEGER NOT NULL, payload TEXT NOT NULL)").map_err(|e|e.to_string())?;
            Ok(json!({"initialized":true}))
        }
        Operation::Load => {
            let row: Option<(i64, i64, String)> = db
                .query_row(
                    "SELECT version,revision,payload FROM progress WHERE id=1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if let Some((version, revision, payload)) = row {
                if version != 1 || revision < 1 || payload.chars().count() > MAX_TEXT {
                    return Err("Unsupported progress storage version, revision or size".into());
                }
                serde_json::from_str::<Value>(&payload).map_err(|e| e.to_string())?;
                Ok(json!({"revision":revision,"payload":payload}))
            } else {
                Ok(Value::Null)
            }
        }
        Operation::Save => {
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            let actual: Option<(i64, i64)> = tx
                .query_row(
                    "SELECT version,revision FROM progress WHERE id=1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if actual.is_some_and(|(version, revision)| version != 1 || revision < 1) {
                return Err("Unsupported progress storage version or revision".into());
            }
            let expected = request.expected_revision.expect("validated revision");
            if actual.map_or(0, |(_, revision)| revision) != expected {
                return Err("Progress revision conflict".into());
            }
            let revision = expected.checked_add(1).ok_or("Storage revision overflow")?;
            tx.execute("INSERT INTO progress VALUES(1,1,?1,?2) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,payload=excluded.payload",params![revision,request.payload]).map_err(|e|e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(json!({"revision":revision}))
        }
    }
}
/// Safe Rust entrypoint shared with the application host. It retains all
/// request validation and optimistic transaction guards used by the C ABI.
pub fn request(value:Value)->Result<Value,String>{execute(serde_json::from_value(value).map_err(|e|e.to_string())?)}
fn reply(value: Result<Value, String>) -> *mut c_char {
    let value = match value {
        Ok(result) => json!({"schema_version":1,"result":result}),
        Err(error) => json!({"schema_version":1,"error":error}),
    };
    CString::new(value.to_string())
        .expect("escaped JSON")
        .into_raw()
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_storage_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_storage_info() -> *mut c_char {
    reply(Ok(
        json!({"engine":"rust-storage","schema_version":1,"abi_version":1,"sqlite_version":rusqlite::version(),"campaign_transitions":1,"game_access":false}),
    ))
}
/// # Safety
/// data points to size readable bytes until this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_storage_request(data: *const u8, size: usize) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            if data.is_null() || size == 0 || size > 96 * 1024 * 1024 {
                return Err("Invalid storage request size".into());
            }
            let request: Request =
                serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
                    .map_err(|e| e.to_string())?;
            execute(request)
        })
        .unwrap_or_else(|_| Err("Storage request failed".into())),
    )
}
/// # Safety
/// data is null or one live result allocated by this library, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_storage_free(data: *mut c_char) {
    if !data.is_null() {
        drop(unsafe { CString::from_raw(data) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_versions_and_invalid_data_before_filesystem_access() {
        let make = |schema, payload: &str| Request {
            schema_version: schema,
            op: Operation::Save,
            path: "".into(),
            expected_revision: Some(0),
            payload: Some(payload.into()),
        };
        assert!(execute(make(2, "{}")).is_err());
        assert!(execute(make(1, "NaN")).is_err());
    }
}
