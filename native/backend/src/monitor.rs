//! Last-good sample ownership and stale/disconnected state. A failed sample
//! never refreshes freshness or merges fields from two sessions.
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
type Result<T> = std::result::Result<T, String>;
#[derive(Default)]
pub struct Monitor {
    last: Option<Value>,
    last_time: Option<f64>,
}
impl Monitor {
    pub fn accept(&mut self, value: Value, now: f64, observed_at: &str) -> Result<Value> {
        clock(now)?;
        self.last = Some(value.clone());
        self.last_time = Some(now);
        Ok(
            json!({"status":"observed","source":"process_memory","usable":true,"validation":"experimental","sample":value,"age_seconds":0,"observed_at":observed_at,"error":""}),
        )
    }
    pub fn failed(&self, error: &str, now: f64) -> Result<Value> {
        clock(now)?;
        let age = self
            .last_time
            .map(|time| ((now - time) * 10.).round_ties_even() / 10.);
        Ok(
            json!({"status":if self.last.is_some(){"stale"}else{"disconnected"},"source":"process_memory","usable":false,"validation":"experimental","sample":self.last,"age_seconds":age,"error":error}),
        )
    }
}
fn clock(now: f64) -> Result<()> {
    if !now.is_finite() || now < 0. {
        Err("Invalid monotonic clock".into())
    } else {
        Ok(())
    }
}
static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<Mutex<Monitor>>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Arc<Mutex<Monitor>>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    if op == "monitor_open" {
        let mut entries = registry()
            .lock()
            .map_err(|_| "Monitor registry unavailable")?;
        if entries.len() >= 256 {
            return Err("Monitor registry limit exceeded".into());
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        entries.insert(id, Arc::new(Mutex::new(Monitor::default())));
        return Ok(json!(id));
    }
    let id = args["handle"].as_u64().ok_or("Missing monitor handle")?;
    if op == "monitor_release" {
        return Ok(json!(
            registry()
                .lock()
                .map_err(|_| "Monitor registry unavailable")?
                .remove(&id)
                .is_some()
        ));
    }
    let monitor = registry()
        .lock()
        .map_err(|_| "Monitor registry unavailable")?
        .get(&id)
        .cloned()
        .ok_or("Unknown monitor handle")?;
    let mut monitor = monitor.lock().map_err(|_| "Monitor unavailable")?;
    let now = args["now"].as_f64().ok_or("Missing monitor clock")?;
    match op {
        "monitor_accept" => monitor.accept(
            args["value"].clone(),
            now,
            args["observed_at"]
                .as_str()
                .ok_or("Missing observation time")?,
        ),
        "monitor_error" => {
            monitor.failed(args["error"].as_str().ok_or("Missing reader error")?, now)
        }
        _ => Err("Unknown monitor operation".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_never_merges_sessions_or_refreshes_last_good_time() {
        let mut monitor = Monitor::default();
        assert_eq!(
            monitor.failed("closed", 0.).unwrap()["status"],
            "disconnected"
        );
        let mut output = monitor
            .accept(
                json!({"session_id":"one","completed_focuses":["a"]}),
                1.,
                "timestamp",
            )
            .unwrap();
        output["sample"]["completed_focuses"] = json!([]);
        let stale = monitor.failed("identity mismatch", 2.25).unwrap();
        assert_eq!(stale["age_seconds"], 1.2);
        assert_eq!(stale["sample"]["completed_focuses"], json!(["a"]));
        assert_eq!(stale["usable"], false);
        monitor
            .accept(
                json!({"session_id":"two","completed_focuses":[]}),
                3.,
                "later",
            )
            .unwrap();
        assert_eq!(
            monitor.failed("closed", 4.).unwrap()["sample"]["session_id"],
            "two"
        );
    }
}
