//! Player-confirmed answers for ambiguous guide event steps. Rows are written in
//! the same transaction as the progress save; the table is created lazily by
//! the first save that writes one, so opening an existing database adds no schema.
use rusqlite::{Connection, Transaction, params};
use serde_json::{Value, json};

const MAX_FIELD: usize = 512;
const TABLE: &str = "CREATE TABLE IF NOT EXISTS confirmed_event_answers (guide_id TEXT NOT NULL, step_key TEXT NOT NULL, quote TEXT NOT NULL, event_id TEXT NOT NULL, option_id TEXT NOT NULL, confirmed_at TEXT NOT NULL, PRIMARY KEY(guide_id, step_key, quote))";

fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .filter(|v| !v.is_empty() && v.chars().count() <= MAX_FIELD)
        .ok_or_else(|| format!("Invalid confirmed answer field {key}"))
}

fn identifier<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    let value = text(row, key)?;
    if value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-')) {
        Ok(value)
    } else {
        Err(format!("Invalid confirmed answer field {key}"))
    }
}

/// Checks one `put` or `delete` row before any connection is opened.
pub fn validate(row: &Value) -> Result<(), String> {
    match row["op"].as_str() {
        Some("put") => {
            identifier(row, "guide_id")?;
            identifier(row, "step_key")?;
            text(row, "quote")?;
            identifier(row, "event_id")?;
            identifier(row, "option_id")?;
            text(row, "confirmed_at")?;
            Ok(())
        }
        Some("delete") => {
            identifier(row, "guide_id")?;
            identifier(row, "step_key")?;
            text(row, "quote")?;
            Ok(())
        }
        _ => Err("Invalid confirmed answer operation".into()),
    }
}

/// Applies already validated rows inside the progress transaction.
pub(crate) fn apply(tx: &Transaction, rows: &[Value]) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    tx.execute_batch(TABLE).map_err(|e| e.to_string())?;
    for row in rows {
        let s = |key: &str| row[key].as_str().unwrap_or_default();
        if row["op"] == "put" {
            tx.execute(
                "INSERT OR REPLACE INTO confirmed_event_answers VALUES(?1,?2,?3,?4,?5,?6)",
                params![s("guide_id"), s("step_key"), s("quote"), s("event_id"), s("option_id"), s("confirmed_at")],
            )
        } else {
            tx.execute(
                "DELETE FROM confirmed_event_answers WHERE guide_id=?1 AND step_key=?2 AND quote=?3",
                params![s("guide_id"), s("step_key"), s("quote")],
            )
        }
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Every stored row; empty when the table was never created.
pub(crate) fn load(db: &Connection) -> Result<Vec<Value>, String> {
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='confirmed_event_answers')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !exists {
        return Ok(Vec::new());
    }
    let mut query = db
        .prepare("SELECT guide_id,step_key,quote,event_id,option_id,confirmed_at FROM confirmed_event_answers ORDER BY guide_id,step_key,quote")
        .map_err(|e| e.to_string())?;
    query
        .query_map([], |row| {
            Ok(json!({
                "guide_id": row.get::<_, String>(0)?,
                "step_key": row.get::<_, String>(1)?,
                "quote": row.get::<_, String>(2)?,
                "event_id": row.get::<_, String>(3)?,
                "option_id": row.get::<_, String>(4)?,
                "confirmed_at": row.get::<_, String>(5)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use crate::request;
    use serde_json::{Value, json};

    struct Db(std::path::PathBuf);
    impl Db {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(
                format!("hoi4-answers-{name}-{}-{:?}", std::process::id(), std::time::SystemTime::now())
                    .replace([':', ' ', '{', '}', ','], "_"),
            );
            std::fs::create_dir_all(&dir).unwrap();
            let db = Self(dir.join("progress.sqlite3"));
            db.call(json!({"op":"init"})).unwrap();
            db
        }
        fn call(&self, mut v: Value) -> Result<Value, String> {
            v["schema_version"] = json!(1);
            v["path"] = json!(self.0.to_string_lossy());
            request(v)
        }
        fn save(&self, expected: i64, answers: Value) -> Result<Value, String> {
            let payload = json!({"schema_version":1,"runs":{},"active":{},"campaign_active":{}});
            self.call(json!({"op":"save","expected_revision":expected,"payload":payload.to_string(),"answers":answers}))
        }
        fn answers(&self) -> Vec<Value> {
            self.call(json!({"op":"load"})).unwrap()["answers"].as_array().unwrap().clone()
        }
    }
    impl Drop for Db {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }
    fn put(quote: &str, option: &str) -> Value {
        json!({"op":"put","guide_id":"g1","step_key":"s.1","quote":quote,"event_id":"ev.1","option_id":option,"confirmed_at":"2026-11-19T10:00:00Z"})
    }

    #[test]
    fn put_then_load_returns_the_row() {
        let db = Db::new("put");
        db.save(0, json!([put("Choose wisely", "ev.1.a")])).unwrap();
        assert_eq!(
            db.answers(),
            vec![json!({"guide_id":"g1","step_key":"s.1","quote":"Choose wisely","event_id":"ev.1","option_id":"ev.1.a","confirmed_at":"2026-11-19T10:00:00Z"})]
        );
    }

    #[test]
    fn put_on_same_key_and_quote_replaces() {
        let db = Db::new("replace");
        db.save(0, json!([put("q", "ev.1.a")])).unwrap();
        db.save(1, json!([put("q", "ev.1.b")])).unwrap();
        let rows = db.answers();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["option_id"], "ev.1.b");
    }

    #[test]
    fn delete_removes_only_that_quote() {
        let db = Db::new("delete");
        db.save(0, json!([put("one", "ev.1.a"), put("two", "ev.1.b")])).unwrap();
        db.save(1, json!([{"op":"delete","guide_id":"g1","step_key":"s.1","quote":"one"}])).unwrap();
        let rows = db.answers();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["quote"], "two");
    }

    #[test]
    fn rows_survive_reopening_the_database() {
        let db = Db::new("reopen");
        db.save(0, json!([put("q", "ev.1.a")])).unwrap();
        db.call(json!({"op":"init"})).unwrap();
        assert_eq!(db.answers().len(), 1);
    }

    #[test]
    fn invalid_row_rejects_the_whole_save() {
        let db = Db::new("invalid");
        db.save(0, json!([])).unwrap();
        let bad = [
            put("", "ev.1.a"),
            put("q", "bad id"),
            put(&"x".repeat(513), "ev.1.a"),
            json!({"op":"put","guide_id":"g1"}),
            json!({"op":"update"}),
        ];
        for row in bad {
            assert!(db.save(1, json!([put("ok", "ev.1.a"), row])).is_err());
        }
        let state = db.call(json!({"op":"load"})).unwrap();
        assert_eq!(state["revision"], 1);
        assert!(state["answers"].as_array().unwrap().is_empty());
    }

    #[test]
    fn load_without_table_returns_empty_answers() {
        let db = Db::new("notable");
        db.save(0, Value::Null).unwrap();
        let state = db.call(json!({"op":"load"})).unwrap();
        assert_eq!(state["answers"], json!([]));
        let tables: i64 = rusqlite::Connection::open(&db.0)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='confirmed_event_answers'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tables, 0);
    }
}
