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
/// The compiled definition and the owned input `guide_prepare::prepare` makes:
/// the input is deserialized from the prepared payload without copying it.
fn prepared(guide: &Value, state: &Value, progress: &Value) -> Result<(Arc<CompiledGuide>, Input)> {
    let Value::Array(mut prepared) = crate::guide_prepare::prepare(guide, state, progress)? else {
        return Err("Invalid prepared guide".into());
    };
    let (Some(payload), Some(definition), true) = (prepared.pop(), prepared.pop(), prepared.is_empty()) else {
        return Err("Invalid prepared guide".into());
    };
    let compiled = compiled(&definition)?;
    let input: Input = serde_json::from_value(payload).map_err(|e| e.to_string())?;
    Ok((compiled, input))
}
pub fn evaluate(guide: &Value, state: &Value, progress: &Value) -> Result<Value> {
    EVALUATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (compiled, input) = prepared(guide, state, progress)?;
    compiled.evaluate(&input)
}
pub fn choices(guide: &Value, state: &Value, progress: &Value) -> Result<Value> {
    let (compiled, input) = prepared(guide, state, progress)?;
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
#[cfg(test)]
mod set_aside_tests {
    use super::*;
    fn step<'a>(result: &'a Value, key: &str) -> &'a Value {
        result["steps"].as_array().unwrap().iter().find(|s| s["key"] == key).unwrap()
    }
    #[test]
    fn an_accepted_run_hands_its_set_aside_steps_to_guide_core() {
        let guide = json!({"id":"aside","applies_to":{},"tracks":{"operations":[
            {"key":"a","action":{"kind":"manual"}},{"key":"b","action":{"kind":"manual"}}]}});
        let state = json!({"schema_version":1,"usable":false,"facts":{}});
        let run = |aside: Value| json!({"schema_version":1,"guide_id":"aside","run_id":"r","checks":{},"started":{},"set_aside":aside});
        let held = evaluate(&guide, &state, &run(json!({}))).unwrap();
        assert_eq!(held["tracks"]["operations"]["next"], "a");
        let aside = evaluate(&guide, &state, &run(json!({"a":true}))).unwrap();
        assert_eq!(step(&aside, "a")["set_aside"], true);
        assert_eq!(step(&aside, "b")["blocked_by"], json!([]));
        assert_eq!(aside["tracks"]["operations"]["next"], "b");
        // A run of another guide hands nothing over.
        let foreign = json!({"schema_version":1,"guide_id":"other","run_id":"r","checks":{},"set_aside":{"a":true}});
        assert!(step(&evaluate(&guide, &state, &foreign).unwrap(), "a").get("set_aside").is_none());
    }
    #[test]
    fn admission_reports_the_mode_and_progress_status_of_a_full_evaluation() {
        let fact=|v:&str|json!({"status":"observed","source":"fixture","value":v});
        let state=|country:Option<&str>,tree:Option<&str>,usable:bool| {
            let mut facts=json!({"campaign.id":fact("hoi4:12345678-1234-1234-1234-123456789abc:00000001")});
            if let Some(c)=country {facts["country"]=fact(c);}
            if let Some(t)=tree {facts["focus.tree"]=fact(t);}
            json!({"schema_version":1,"usable":usable,"session_id":"s1","facts":facts})
        };
        let states=[state(Some("HUN"),Some("hungary"),true),state(Some("GER"),Some("hungary"),true),state(Some("HUN"),Some("other"),true),
            state(Some("HUN"),None,true),state(None,Some("hungary"),true),state(Some("HUN"),Some("hungary"),false),json!({"usable":false})];
        let run=|extra:Value| {let mut r=json!({"schema_version":1,"guide_id":"g","run_id":"r","checks":{}});for (k,v) in extra.as_object().unwrap() {r[k]=v.clone();}r};
        let progresses=[Value::Null,run(json!({})),run(json!({"guide_id":"other"})),run(json!({"session_id":"s1"})),run(json!({"session_id":"s2"})),
            run(json!({"campaign_id":"hoi4:12345678-1234-1234-1234-123456789abc:00000001","country":"HUN"})),run(json!({"campaign_id":"hoi4:12345678-1234-1234-1234-123456789abc:00000002","country":"HUN"})),run(json!({"campaign_required":true}))];
        for scope in [json!({}),json!({"country":"HUN"}),json!({"country":"HUN","tree_id":"hungary"})] {
            let guide=json!({"id":"g","applies_to":scope,"tracks":{"focus":[{"key":"a","kind":"focus","action":{"kind":"manual"}}]}});
            for state in &states {
                for progress in &progresses {
                    let evaluation=evaluate(&guide,state,progress).unwrap();
                    let (mode,status)=crate::guide_prepare::admission(&guide,state,progress).unwrap();
                    assert_eq!((json!(mode),json!(status)),(evaluation["mode"].clone(),evaluation["progress_status"].clone()),
                        "scope {scope} state {state} progress {progress}");
                }
            }
        }
        let guide=json!({"id":"g","applies_to":{},"tracks":{"focus":[]}});
        let invalid=json!({"schema_version":1,"guide_id":"g","run_id":"r","checks":{"a":"yes"}});
        assert!(evaluate(&guide,&states[0],&invalid).is_err()&&crate::guide_prepare::admission(&guide,&states[0],&invalid).is_err());
    }
}
