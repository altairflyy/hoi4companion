//! One bounded immutable graph cache shared by API and HUD projections.
//! Requests retain an Arc after releasing the cache lock; live inputs never
//! become part of a cached graph or survive into a subsequent evaluation.
use hoi4companion_guide_core::{CompiledGuide, Guide, Input};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock},
};
type Result<T> = std::result::Result<T, String>;
static EVALUATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
type Cache = VecDeque<(String, Arc<CompiledGuide>)>;
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}
pub fn clear() -> Result<Value> {
    cache()
        .lock()
        .map_err(|_| "Guide cache unavailable")?
        .clear();
    Ok(Value::Null)
}
fn compiled(definition: &Value) -> Result<Arc<CompiledGuide>> {
    let key = definition.to_string();
    let mut cache = cache().lock().map_err(|_| "Guide cache unavailable")?;
    if let Some(index) = cache.iter().position(|(k, _)| *k == key) {
        let value = cache.remove(index).unwrap();
        let guide = value.1.clone();
        cache.push_back(value);
        return Ok(guide);
    }
    let guide: Guide = serde_json::from_value(definition.clone()).map_err(|e| e.to_string())?;
    let compiled = Arc::new(CompiledGuide::compile(guide)?);
    if cache.len() >= 16 {
        cache.pop_front();
    }
    cache.push_back((key, compiled.clone()));
    Ok(compiled)
}
pub fn evaluate(guide: &Value, state: &Value, progress: &Value) -> Result<Value> {
    EVALUATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let prepared = crate::guide_prepare::prepare(guide, state, progress)?;
    let compiled = compiled(&prepared[0])?;
    let input: Input = serde_json::from_value(prepared[1].clone()).map_err(|e| e.to_string())?;
    compiled.evaluate(&input)
}
pub(crate) fn choices(guide: &Value, state: &Value, progress: &Value) -> Result<Value> {
    let prepared = crate::guide_prepare::prepare(guide, state, progress)?;
    let compiled = compiled(&prepared[0])?;
    let input: Input = serde_json::from_value(prepared[1].clone()).map_err(|e| e.to_string())?;
    Ok(Value::Object(
        compiled.research_choices(&input).into_iter().collect(),
    ))
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    match op {
        "guide_info" => Ok(hoi4companion_guide_core::engine_info()),
        "guide_clear" => clear(),
        "guide_evaluate" => evaluate(&args["guide"], &args["state"], &args["progress"]),
        "guide_cache_info" => Ok(
            json!({"compiled_definitions":cache().lock().map_err(|_|"Guide cache unavailable")?.len(),"limit":16,"stores_live_state":false,"evaluations":EVALUATIONS.load(std::sync::atomic::Ordering::Relaxed)}),
        ),
        _ => Err("Unknown guide runtime operation".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn research_alternative_order_is_independent_of_object_insertion_order() {
        let guide = json!({"id":"choice-order","applies_to":{},"tracks":{"research":[{"key":"r","kind":"technology","action":{"kind":"research","entity":{"kind":"technology","id":"c1"}},"research_choice":{"group":"industry","query":"Concentrated/dispersed","branches":{"z":["d1","d2"],"a":["c1","c2"]}}}]}});
        let state = json!({"schema_version":1,"usable":true,"facts":{"research.completed":{"status":"observed","value":[],"source":"fixture"},"research.active":{"status":"observed","value":[],"source":"fixture"},"research.slots":{"status":"observed","value":[{"technology_id":null}],"source":"fixture"}}});
        let result = evaluate(
            &guide,
            &state,
            &json!({"schema_version":1,"guide_id":"choice-order","run_id":"r","checks":{}}),
        )
        .unwrap();
        assert_eq!(result["steps"][0]["research_options"], json!(["c1", "d1"]));
    }
    #[test]
    fn cached_graph_never_retains_completion_or_check_values() {
        let guide = json!({"id":"cache-test","applies_to":{},"dependency_policy":"explicit","tracks":{"operations":[{"key":"law","kind":"gate","action":{"kind":"manual","params":{"category":"politics"}},"completion":{"op":"fact","key":"law_done","test":"eq","value":true}}]}});
        let state = |done| json!({"schema_version":1,"usable":true,"session_id":"fixture","facts":{"law_done":{"status":"observed","value":done,"source":"fixture"}}});
        let done = evaluate(&guide, &state(true), &Value::Null).unwrap();
        let pending = evaluate(&guide, &state(false), &Value::Null).unwrap();
        assert_eq!(done["steps"][0]["status"], "completed");
        assert_ne!(pending["steps"][0]["status"], "completed");
        clear().unwrap();
        assert_eq!(
            evaluate(&guide, &state(false), &Value::Null).unwrap(),
            pending
        );
    }
}
