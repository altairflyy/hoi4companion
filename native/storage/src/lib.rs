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
pub mod confirmed_answers;
mod progress_ffi;
const MAX_TEXT: usize = 16 * 1024 * 1024;
/// Progress row versions. v1 = original format; v2 = same payload root plus
/// optional typed per-run `definition_ref` values backed by `guide_definitions`.
/// Binaries that only know v1 reject v2 rows (fail closed on downgrade).
pub const PROGRESS_VERSION: i64 = 2;
const DEFINITIONS_TABLE: &str = "CREATE TABLE IF NOT EXISTS guide_definitions (id TEXT PRIMARY KEY NOT NULL, archive_format INTEGER NOT NULL, payload TEXT NOT NULL)";
/// Archive identity: `gd1_` + lowercase SHA-256 hex of the canonical payload.
/// The digest itself is verified by the backend (this crate has no digest
/// dependency); storage enforces format, insert-only immutability and
/// referential integrity inside the progress transaction.
pub fn definition_id(v: &str) -> bool {
    v.len() == 68
        && v.starts_with("gd1_")
        && v[4..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
/// Archive IDs referenced by a progress payload (pins and legacy observations).
pub fn referenced_definitions(payload: &Value) -> Vec<String> {
    let mut ids: Vec<String> = payload["runs"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|row| {
            // Definitions a run was moved away from stay referenced, so its
            // previous progress keeps its meaning.
            let previous = row["previous_definitions"].as_array().into_iter().flatten().flat_map(|p| {
                [&p["definition_ref"]["id"], &p["definition_ref"]["observed_definition_id"]]
            });
            [&row["definition_ref"]["id"], &row["definition_ref"]["observed_definition_id"]]
                .into_iter()
                .chain(previous)
                .collect::<Vec<_>>()
        })
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}
fn supported(version: i64) -> bool {
    version == 1 || version == PROGRESS_VERSION
}
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
    /// Row version to write (old callers: 1). Never lower than the stored one.
    #[serde(default)]
    version: Option<i64>,
    /// New archive rows published atomically with this progress save.
    #[serde(default)]
    definitions: Option<serde_json::Map<String, Value>>,
    /// Confirmed event answer rows (`put`/`delete`) published atomically with this save.
    #[serde(default)]
    answers: Option<Vec<Value>>,
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
        if !supported(request.version.unwrap_or(1)) {
            return Err("Unsupported progress storage version".into());
        }
        for row in request.answers.iter().flatten() {
            confirmed_answers::validate(row)?;
        }
        for (id, payload) in request.definitions.iter().flatten() {
            if !definition_id(id)
                || payload
                    .as_str()
                    .is_none_or(|p| p.is_empty() || p.chars().count() > MAX_TEXT)
            {
                return Err("Invalid guide definition archive".into());
            }
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
            // The archive table is created lazily by the first save that needs it,
            // so opening an existing database adds no new schema.
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
                if !supported(version) || revision < 1 || payload.chars().count() > MAX_TEXT {
                    return Err("Unsupported progress storage version, revision or size".into());
                }
                let parsed = serde_json::from_str::<Value>(&payload).map_err(|e| e.to_string())?;
                // Referenced archives only. A missing row is reported by omission
                // so the backend fails closed for that run instead of inventing one.
                let mut definitions = serde_json::Map::new();
                let ids = referenced_definitions(&parsed);
                if !ids.is_empty() {
                    if version == 1 {
                        return Err("Version 1 progress cannot reference guide definitions".into());
                    }
                    let mut query = db
                        .prepare("SELECT payload FROM guide_definitions WHERE id=?1 AND archive_format=1")
                        .map_err(|e| e.to_string())?;
                    for id in ids {
                        if let Some(text) = query
                            .query_row([&id], |row| row.get::<_, String>(0))
                            .optional()
                            .map_err(|e| e.to_string())?
                        {
                            definitions.insert(id, json!(text));
                        }
                    }
                }
                let answers = confirmed_answers::load(&db)?;
                Ok(json!({"revision":revision,"version":version,"payload":payload,"definitions":definitions,"answers":answers}))
            } else {
                Ok(Value::Null)
            }
        }
        Operation::Save => {
            let stored: Option<(i64, i64)> = db
                .query_row("SELECT version,revision FROM progress WHERE id=1", [], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .optional()
                .map_err(|e| e.to_string())?;
            // Version-unaware callers keep the stored version (never a downgrade).
            let target = request
                .version
                .unwrap_or_else(|| stored.map_or(1, |(version, _)| version));
            // Keep a restorable copy before the first v2 write over existing v1
            // data. VACUUM INTO cannot run inside the write transaction; the
            // stored version and revision are re-checked under it below.
            if let Some((1, revision)) = stored
                && target > 1
            {
                let backup = std::path::PathBuf::from(format!(
                    "{}.v1-backup-r{revision}.sqlite3",
                    request.path
                ));
                if !backup.exists() {
                    let partial = backup.with_extension("partial");
                    let _ = std::fs::remove_file(&partial);
                    db.execute("VACUUM INTO ?1", params![partial.to_string_lossy()])
                        .map_err(|e| format!("Progress backup failed: {e}"))?;
                    std::fs::rename(&partial, &backup)
                        .map_err(|e| format!("Progress backup failed: {e}"))?;
                }
            }
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
            if actual.is_some_and(|(version, revision)| !supported(version) || revision < 1) {
                return Err("Unsupported progress storage version or revision".into());
            }
            if actual.is_some_and(|(version, _)| version > target) {
                return Err("Progress storage downgrade refused".into());
            }
            let expected = request.expected_revision.expect("validated revision");
            if actual.map_or(0, |(_, revision)| revision) != expected {
                return Err("Progress revision conflict".into());
            }
            let revision = expected.checked_add(1).ok_or("Storage revision overflow")?;
            let snapshot: Value =
                serde_json::from_str(request.payload.as_deref().expect("validated payload"))
                    .map_err(|e| e.to_string())?;
            let referenced = referenced_definitions(&snapshot);
            if !referenced.is_empty() || request.definitions.as_ref().is_some_and(|d| !d.is_empty())
            {
                if target < 2 {
                    return Err("Version 1 progress cannot reference guide definitions".into());
                }
                tx.execute_batch(DEFINITIONS_TABLE).map_err(|e| e.to_string())?;
            }
            // Insert-only: an existing ID must already hold byte-identical content.
            for (id, payload) in request.definitions.iter().flatten() {
                let payload = payload.as_str().expect("validated archive");
                let existing: Option<String> = tx
                    .query_row("SELECT payload FROM guide_definitions WHERE id=?1", [id], |row| {
                        row.get(0)
                    })
                    .optional()
                    .map_err(|e| e.to_string())?;
                match existing {
                    Some(text) if text == payload => {}
                    Some(_) => return Err("Guide definition archive conflict".into()),
                    None => {
                        tx.execute(
                            "INSERT INTO guide_definitions VALUES(?1,1,?2)",
                            params![id, payload],
                        )
                        .map_err(|e| e.to_string())?;
                    }
                }
            }
            for id in &referenced {
                let present: Option<i64> = tx
                    .query_row(
                        "SELECT 1 FROM guide_definitions WHERE id=?1 AND archive_format=1",
                        [id],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;
                if present.is_none() {
                    return Err("Progress references an unarchived guide definition".into());
                }
            }
            confirmed_answers::apply(&tx, request.answers.as_deref().unwrap_or_default())?;
            tx.execute("INSERT INTO progress VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET version=excluded.version,revision=excluded.revision,payload=excluded.payload",params![target,revision,request.payload]).map_err(|e|e.to_string())?;
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
            version: None,
            definitions: None,
            answers: None,
        };
        assert!(execute(make(2, "{}")).is_err());
        assert!(execute(make(1, "NaN")).is_err());
    }

    struct Db(std::path::PathBuf);
    impl Db {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "hoi4-storage-{name}-{}-{:?}",
                std::process::id(),
                std::time::SystemTime::now()
            ).replace([':', ' ', '{', '}', ','], "_"));
            std::fs::create_dir_all(&dir).unwrap();
            let db = Self(dir.join("progress.sqlite3"));
            db.call(json!({"op":"init"})).unwrap();
            db
        }
        fn path(&self) -> String {
            self.0.to_string_lossy().into_owned()
        }
        fn call(&self, mut v: Value) -> Result<Value, String> {
            v["schema_version"] = json!(1);
            v["path"] = json!(self.path());
            request(v)
        }
        fn save(&self, expected: i64, version: Option<i64>, payload: &Value, defs: Value) -> Result<Value, String> {
            self.call(json!({"op":"save","expected_revision":expected,"version":version,"payload":payload.to_string(),"definitions":defs}))
        }
        fn backups(&self) -> usize {
            std::fs::read_dir(self.0.parent().unwrap())
                .unwrap()
                .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains(".v1-backup-r"))
                .count()
        }
    }
    impl Drop for Db {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }
    fn id(c: char) -> String {
        format!("gd1_{}", c.to_string().repeat(64))
    }
    fn payload(pin: Option<&str>) -> Value {
        let mut row = json!({"schema_version":1,"guide_id":"g","run_id":"r","session_id":null,"checks":{},"started":{},"run_revision":0});
        if let Some(pin) = pin {
            row["definition_ref"] = json!({"status":"pinned","id":pin,"archive_format":1});
        }
        json!({"schema_version":1,"runs":{"r":row},"active":{"g":"r"},"campaign_active":{}})
    }

    #[test]
    fn v1_data_is_backed_up_once_then_upgraded_atomically_with_its_archive() {
        let db = Db::new("upgrade");
        db.save(0, None, &payload(None), Value::Null).unwrap();
        let v1 = db.call(json!({"op":"load"})).unwrap();
        assert_eq!(v1["version"], 1);
        assert_eq!(db.backups(), 0, "loading never writes");
        let a = id('a');
        db.save(1, Some(2), &payload(Some(&a)), json!({a.clone():"{\"x\":1}"})).unwrap();
        assert_eq!(db.backups(), 1);
        let backup = std::fs::read_dir(db.0.parent().unwrap()).unwrap()
            .map(|e| e.unwrap().path()).find(|p| p.to_string_lossy().contains("v1-backup-r1")).unwrap();
        let old = request(json!({"schema_version":1,"op":"load","path":backup.to_string_lossy()})).unwrap();
        assert_eq!(old["version"], 1);
        assert_eq!(old["revision"], 1);
        db.save(2, Some(2), &payload(Some(&a)), json!({a.clone():"{\"x\":1}"})).unwrap();
        assert_eq!(db.backups(), 1, "backup only before the first v2 write");
        let v2 = db.call(json!({"op":"load"})).unwrap();
        assert_eq!(v2["version"], 2);
        assert_eq!(v2["revision"], 3);
        assert_eq!(v2["definitions"][&a], "{\"x\":1}");
    }

    #[test]
    fn missing_conflicting_or_downgraded_archives_commit_nothing() {
        let db = Db::new("integrity");
        let (a, b) = (id('a'), id('b'));
        db.save(0, Some(2), &payload(Some(&a)), json!({a.clone():"one"})).unwrap();
        // Unarchived reference.
        assert!(db.save(1, Some(2), &payload(Some(&b)), json!({})).unwrap_err().contains("unarchived"));
        // Same ID, different bytes.
        assert!(db.save(1, Some(2), &payload(Some(&a)), json!({a.clone():"two"})).unwrap_err().contains("conflict"));
        // Pins cannot be written as v1 and a v2 row is never downgraded.
        assert!(db.save(1, Some(1), &payload(None), json!({})).unwrap_err().contains("downgrade"));
        // Version-unaware writers keep v2 and its integrity checks.
        assert!(db.save(1, None, &payload(Some(&b)), json!({})).is_err());
        // Stale writer.
        assert!(db.save(0, Some(2), &payload(Some(&a)), json!({})).unwrap_err().contains("conflict"));
        assert!(db.save(1, Some(2), &payload(None), json!({"bad":"x"})).is_err());
        let state = db.call(json!({"op":"load"})).unwrap();
        assert_eq!(state["revision"], 1);
        assert_eq!(state["definitions"][&a], "one");
        assert!(state["definitions"].get(&b).is_none());
    }

    #[test]
    fn unknown_versions_fail_closed_and_missing_rows_are_omitted() {
        let db = Db::new("versions");
        let a = id('a');
        db.save(0, Some(2), &payload(Some(&a)), json!({a.clone():"one"})).unwrap();
        let conn = Connection::open(&db.0).unwrap();
        conn.execute("DELETE FROM guide_definitions", []).unwrap();
        let state = db.call(json!({"op":"load"})).unwrap();
        assert!(state["definitions"].as_object().unwrap().is_empty());
        conn.execute("UPDATE progress SET version=3", []).unwrap();
        drop(conn);
        assert!(db.call(json!({"op":"load"})).is_err());
        assert!(db.save(1, Some(2), &payload(None), json!({})).is_err());
        assert!(db.save(1, Some(3), &payload(None), json!({})).is_err());
        assert!(!definition_id("gd1_ABC") && definition_id(&a));
    }
}
