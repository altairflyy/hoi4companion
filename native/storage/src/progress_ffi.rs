//! Owned run stores. A pending mutation must be accepted after persistence or
//! rolled back; no other caller can read an uncommitted run through this ABI.
use super::{progress, reply};
use serde_json::json;
use std::collections::HashMap;
use std::ffi::c_char;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<Mutex<progress::Store>>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Arc<Mutex<progress::Store>>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_progress_open() -> *mut c_char {
    reply((|| {
        let mut registry = registry()
            .lock()
            .map_err(|_| "Registro avanzamento non disponibile")?;
        if registry.len() >= 512 {
            return Err("Limite archivi avanzamento raggiunto".into());
        }
        let handle = NEXT.fetch_add(1, Ordering::Relaxed);
        registry.insert(handle, Arc::new(Mutex::new(progress::Store::default())));
        Ok(json!({"handle":handle}))
    })())
}
/// # Safety
/// data points to size readable bytes until this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_progress_request(
    handle: u64,
    data: *const u8,
    size: usize,
) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            if data.is_null() || size == 0 || size > 96 * 1024 * 1024 {
                return Err("Dimensione richiesta avanzamento non valida".into());
            }
            let request = serde_json::from_slice::<progress::Request>(unsafe {
                std::slice::from_raw_parts(data, size)
            })
            .map_err(|e| e.to_string())?;
            let store = registry()
                .lock()
                .map_err(|_| "Registro avanzamento non disponibile")?
                .get(&handle)
                .cloned()
                .ok_or("Archivio avanzamento non disponibile")?;
            
            store
                .lock()
                .map_err(|_| "Archivio avanzamento non disponibile")?
                .request(request)
        })
        .unwrap_or_else(|_| Err("Richiesta avanzamento rifiutata".into())),
    )
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_progress_release(handle: u64) -> bool {
    registry()
        .lock()
        .is_ok_and(|mut r| r.remove(&handle).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn request(store: &mut progress::Store, op: &str, args: Value, ids: Vec<Value>) -> Value {
        store
            .request(progress::Request {
                schema_version: 1,
                op: op.into(),
                args,
                ids,
            })
            .unwrap()
    }
    #[test]
    fn pending_writes_are_isolated_and_rollback_restores_state() {
        let mut store = progress::Store::default();
        let before = request(&mut store, "snapshot", json!({}), vec![])["value"].clone();
        assert_eq!(
            request(&mut store, "ensure", json!({"guide_id":"g"}), vec![])["needs_id"],
            true
        );
        assert_eq!(
            request(&mut store, "snapshot", json!({}), vec![])["value"],
            before
        );
        assert_eq!(
            request(
                &mut store,
                "ensure",
                json!({"guide_id":"g"}),
                vec![json!("id")]
            )["changed"],
            true
        );
        assert!(
            store
                .request(progress::Request {
                    schema_version: 1,
                    op: "snapshot".into(),
                    args: json!({}),
                    ids: vec![]
                })
                .is_err()
        );
        request(&mut store, "rollback", json!({}), vec![]);
        assert_eq!(
            request(&mut store, "snapshot", json!({}), vec![])["value"],
            before
        );
    }
    #[test]
    fn reads_answer_the_committed_state_and_change_nothing() {
        let mut store = progress::Store::default();
        request(&mut store, "ensure", json!({"guide_id":"g"}), vec![json!("id")]);
        request(&mut store, "accept", json!({}), vec![]);
        let snapshot = request(&mut store, "snapshot", json!({}), vec![])["value"].clone();
        for op in ["get", "metadata", "records", "history", "snapshot", "decision_history"] {
            let reply = request(&mut store, op, json!({"guide_id":"g"}), vec![]);
            assert_eq!(reply["changed"], false, "{op}");
            assert!(reply.get("snapshot").is_none(), "{op}");
        }
        assert_eq!(request(&mut store, "get", json!({"guide_id":"g"}), vec![])["value"]["run_id"], "id");
        assert_eq!(request(&mut store, "metadata", json!({}), vec![])["value"]["g"]["run_id"], "id");
        assert!(store.request(progress::Request { schema_version: 1, op: "get".into(), args: json!({"guide_id":"other"}), ids: vec![] }).is_err());
        assert_eq!(request(&mut store, "snapshot", json!({}), vec![])["value"], snapshot);
        // A write after reads still commits and rolls back as one transaction.
        let changed = request(&mut store, "update", json!({"guide_id":"g","checks":{"a":true}}), vec![]);
        assert_eq!(changed["changed"], true);
        request(&mut store, "rollback", json!({}), vec![]);
        assert_eq!(request(&mut store, "snapshot", json!({}), vec![])["value"], snapshot);
    }
}
