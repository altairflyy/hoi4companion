//! Shared event state, coalescing and liveness. The caller supplies a monotonic
//! time from its host; none of the diagnostic timestamps prove reader freshness.
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};
type Result<T> = std::result::Result<T, String>;

pub(crate) fn calendar(sample: &Value) -> Option<u64> {
    if sample["country_identity_validation"] != "primary_selector_and_country_map"
        || sample["session_id"].as_str().is_none_or(str::is_empty)
        || !sample["executable_sha256"].as_str().is_some_and(|s| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    {
        return None;
    }
    let tick = sample["native_tick"]
        .as_u64()
        .filter(|t| *t <= 2_147_483_647)?;
    let days = tick / 24;
    let year = (days / 365)
        .checked_sub(5000)
        .filter(|y| (1..=9999).contains(y))?;
    let mut day = days % 365;
    for (month, length) in [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        .into_iter()
        .enumerate()
    {
        if day < length {
            return Some(year * 10000 + (month as u64 + 1) * 100 + day + 1);
        }
        day -= length;
    }
    None
}
fn semantic(value: &Value) -> Value {
    match value {
        Value::Object(m) => {
            let sorted: BTreeMap<_, _> = m
                .iter()
                .filter(|(k, _)| {
                    ![
                        "observed_at",
                        "age_seconds",
                        "native_tick",
                        "metrics",
                        "checked_at",
                    ]
                    .contains(&k.as_str())
                })
                .map(|(k, v)| (k.clone(), semantic(v)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(semantic).collect()),
        v => v.clone(),
    }
}
pub fn signature(value: &Value) -> Result<String> {
    if !value.is_object() {
        return Err("State stream requires an object".into());
    }
    let mut result = semantic(value);
    if value["usable"] == true
        && let Some(day) = calendar(&value["sample"])
    {
        result["_calendar_day"] = json!(day)
    }
    let sorted: BTreeMap<_, _> = result
        .as_object()
        .expect("object signature")
        .iter()
        .collect();
    serde_json::to_string(&sorted).map_err(|e| e.to_string())
}
#[derive(Default)]
struct Inner {
    event: Option<Value>,
    signature: Option<String>,
    revision: u64,
    last_check: Option<f64>,
    clients: HashMap<u64, Option<u64>>,
    next: u64,
    closed: bool,
}
pub struct Hub {
    inner: Mutex<Inner>,
    changed: Condvar,
    threshold: f64,
    maximum: usize,
}
impl Hub {
    pub fn new(interval: f64, maximum: usize) -> Result<Self> {
        if !interval.is_finite() || interval <= 0. || maximum == 0 || maximum > 1024 {
            return Err("Positive interval and subscriber limit required".into());
        }
        Ok(Self {
            inner: Mutex::new(Inner::default()),
            changed: Condvar::new(),
            threshold: 5f64.max(interval * 8.),
            maximum,
        })
    }
    fn expire(&self, inner: &mut Inner, now: f64) -> Result<()> {
        if inner.event.as_ref().is_none_or(|v| v["usable"] != true)
            || inner
                .last_check
                .is_none_or(|last| now - last < self.threshold)
        {
            return Ok(());
        }
        let mut value = inner.event.as_ref().expect("existing event").clone();
        value["usable"] = json!(false);
        value["status"] = json!("stale");
        value["error"] = json!("Il lettore non conferma più uno stato fresco");
        inner.revision = inner
            .revision
            .checked_add(1)
            .ok_or("State revision exceeded")?;
        inner.event = Some(value);
        inner.signature = None;
        self.changed.notify_all();
        Ok(())
    }
    pub fn subscribe(&self, now: f64) -> Result<Value> {
        let mut inner = self.inner.lock().map_err(|_| "State stream unavailable")?;
        if inner.closed {
            return Err("State stream closed".into());
        }
        if inner.clients.len() >= self.maximum {
            return Err("Too many state stream subscribers".into());
        }
        if inner.clients.is_empty() {
            inner.event = None;
            inner.signature = None;
            inner.last_check = None
        }
        self.expire(&mut inner, now)?;
        inner.next = inner
            .next
            .checked_add(1)
            .ok_or("Subscription identity exceeded")?;
        let id = inner.next;
        inner.clients.insert(id, None);
        Ok(json!(id))
    }
    pub fn unsubscribe(&self, id: u64) -> Result<Value> {
        let mut inner = self.inner.lock().map_err(|_| "State stream unavailable")?;
        inner.clients.remove(&id);
        self.changed.notify_all();
        Ok(Value::Null)
    }
    pub fn publish(&self, value: Value, now: f64) -> Result<Value> {
        let signature = signature(&value)?;
        let mut inner = self.inner.lock().map_err(|_| "State stream unavailable")?;
        if inner.closed || inner.clients.is_empty() {
            return Ok(json!(false));
        }
        self.expire(&mut inner, now)?;
        inner.last_check = Some(now);
        if inner.signature.as_ref() != Some(&signature) {
            inner.revision = inner
                .revision
                .checked_add(1)
                .ok_or("State revision exceeded")?;
            inner.event = Some(value);
            inner.signature = Some(signature);
            self.changed.notify_all();
            return Ok(json!(true));
        }
        Ok(json!(false))
    }
    pub fn heartbeat(&self, now: f64) -> Result<Value> {
        let mut inner = self.inner.lock().map_err(|_| "State stream unavailable")?;
        self.expire(&mut inner, now)?;
        let age = inner.last_check.map(|last| now - last);
        let fresh = age.is_some_and(|a| a >= 0. && a < self.threshold);
        let value = inner.event.as_ref().unwrap_or(&Value::Null);
        Ok(
            json!({"revision":inner.revision,"usable":fresh && value["usable"]==true,"status":if fresh {value.get("status").cloned().unwrap_or(json!("connecting"))}else{json!("stale")},"age_seconds":age.map(|a|(a*1000.).round()/1000.)}),
        )
    }
    pub fn wait(&self, id: u64, seconds: f64, now: f64) -> Result<Value> {
        if !seconds.is_finite() || !(0. ..=300.).contains(&seconds) {
            return Err("Invalid state stream timeout".into());
        }
        let began = Instant::now();
        let timeout = Duration::from_secs_f64(seconds);
        let mut inner = self.inner.lock().map_err(|_| "State stream unavailable")?;
        loop {
            let elapsed = began.elapsed();
            self.expire(&mut inner, now + elapsed.as_secs_f64())?;
            let Some(previous) = inner.clients.get(&id) else {
                return Ok(Value::Null);
            };
            if inner.closed {
                return Ok(Value::Null);
            }
            if let Some(value) = &inner.event
                && previous.is_none_or(|revision| inner.revision > revision)
            {
                let result = json!({"revision":inner.revision,"value":value});
                let revision = inner.revision;
                inner.clients.insert(id, Some(revision));
                return Ok(result);
            }
            if elapsed >= timeout {
                return Ok(Value::Null);
            }
            let mut remaining = timeout - elapsed;
            if inner.event.as_ref().is_some_and(|v| v["usable"] == true)
                && let Some(last) = inner.last_check
            {
                let until = self.threshold - (now + elapsed.as_secs_f64() - last);
                if until > 0. {
                    remaining = remaining.min(Duration::from_secs_f64(until))
                }
            }
            inner = self
                .changed
                .wait_timeout(inner, remaining)
                .map_err(|_| "State stream unavailable")?
                .0;
        }
    }
    pub fn peek(&self, now: f64) -> Result<Value> {
        let mut inner = self.inner.lock().map_err(|_| "State stream unavailable")?;
        self.expire(&mut inner, now)?;
        Ok(inner
            .event
            .as_ref()
            .map(|v| json!({"revision":inner.revision,"value":v}))
            .unwrap_or(Value::Null))
    }
    pub fn close(&self) -> Result<Value> {
        let mut inner = self.inner.lock().map_err(|_| "State stream unavailable")?;
        inner.closed = true;
        self.changed.notify_all();
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn liveness_does_not_restore_unknown_or_stale_evidence() {
        let hub = Hub::new(0.25, 2).unwrap();
        let id = hub.subscribe(10.).unwrap().as_u64().unwrap();
        hub.publish(json!({"usable":true}), 10.).unwrap();
        assert_eq!(hub.wait(id, 0., 10.).unwrap()["value"]["usable"], true);
        assert_eq!(hub.heartbeat(16.).unwrap()["usable"], false);
        let expired = hub.wait(id, 0., 16.).unwrap();
        assert_eq!(expired["value"]["usable"], false);
        hub.publish(json!({"usable":true}), 17.).unwrap();
        assert_eq!(hub.wait(id, 0., 17.).unwrap()["value"]["usable"], true);
    }
}
