//! Conservative campaign observations. Bounds cover observations only; native
//! membership and process addresses never establish a repeat by themselves.
use serde_json::{Map, Value, json};
type Result<T> = std::result::Result<T, String>;
fn text(v: &Value) -> &str { v.as_str().unwrap_or("") }
fn tick(v: &Value) -> bool { v.as_u64().is_some_and(|n| n <= 9_007_199_254_740_991) }
fn id(v: &Value) -> bool { v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 160 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))) }
pub fn token(id_value: &Value, target: &Value) -> Option<String> {
    if !id(id_value) { return None; }
    if target.is_null() { return Some(text(id_value).into()); }
    match text(&target["kind"]) {
        "country" if target["id"].as_str().is_some_and(|s| s.len()==3 && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())) => Some(format!("{}:country:{}",text(id_value),text(&target["id"]))),
        "state" if target["id"].as_u64().is_some_and(|n| (1..=4095).contains(&n)) => Some(format!("{}:state:{}",text(id_value),target["id"])),
        _ => None,
    }
}
pub fn identity(token_value: &str) -> Option<(Value, Value)> {
    let p=token_value.split(':').collect::<Vec<_>>();
    let target=match p.as_slice() { [_]=>Value::Null, [_,"country",tag]=>json!({"kind":"country","id":tag}), [_,"state",state]=>json!({"kind":"state","id":state.parse::<u64>().ok()?}), _=>return None };
    let ident=json!(p[0]);
    (token(&ident,&target).as_deref()==Some(token_value)).then_some((ident,target))
}
pub fn validate(value: &Value) -> Result<()> {
    let valid=value.as_object().is_some_and(|m| m.len()==8)
        && value["schema_version"]==1 && super::progress::campaign(&value["campaign_id"])
        && value["country"].as_str().is_some_and(|s| s.len()==3 && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()))
        && !text(&value["session_id"]).is_empty() && tick(&value["first_tick"]) && tick(&value["last_tick"])
        && value["first_tick"].as_u64()<=value["last_tick"].as_u64()
        && value["coverage"].as_object().is_some_and(|m|m.len()==3) && value["coverage"]["scope"]=="qualified_samples_only"
        && value["coverage"]["pre_observation_history"]=="unknown" && value["coverage"]["gaps_observed"].is_boolean()
        && value["entries"].as_object().is_some_and(|entries| entries.len()<=16384 && entries.iter().all(|(key,row)| {
            row.as_object().is_some_and(|m| m.len()==11) && token(&row["id"],&row["target"]).as_deref()==Some(key)
                && ["executions_lower_bound","completions_lower_bound","first_tick","last_tick","phase_tick"].iter().all(|k|tick(&row[*k]))
                && row["first_tick"].as_u64()>=value["first_tick"].as_u64() && row["first_tick"].as_u64()<=row["last_tick"].as_u64()
                && row["last_tick"].as_u64()<=value["last_tick"].as_u64() && row["phase_tick"].as_u64()<=value["last_tick"].as_u64()
                && row["completions_lower_bound"].as_u64()<=row["executions_lower_bound"].as_u64()
                && ["available","active","executed","completed","unknown"].contains(&text(&row["phase"]))
                && !text(&row["phase_session"]).is_empty() && row["armed"].is_boolean() && row["completion_pending"].is_boolean()
        }));
    if valid {Ok(())}else{Err("Storico decisioni campagna non valido".into())}
}
pub fn empty(campaign: &Value, country: &Value, session: &Value, tick: u64) -> Value {
    json!({"schema_version":1,"campaign_id":campaign,"country":country,"session_id":session,"first_tick":tick,"last_tick":tick,"entries":{},"coverage":{"scope":"qualified_samples_only","pre_observation_history":"unknown","gaps_observed":false}})
}
pub fn interrupt(history:&mut Value) {
    history["coverage"]["gaps_observed"]=json!(true);
    for row in history["entries"].as_object_mut().unwrap().values_mut() {
        row["phase"]=json!("unknown");row["armed"]=json!(false);row["completion_pending"]=json!(false);
    }
}
fn fact<'a>(state: &'a Value, key: &str, source: &str) -> Option<&'a Vec<Value>> {
    let f=&state["facts"][key];
    (f["status"]=="calculated" && f["source"]==source).then(||f["value"].as_array()).flatten()
}
pub fn observe(history: &mut Value, state: &Value, native_tick: u64) -> Result<()> {
    validate(history)?;
    if state["usable"]!=true || state["session_id"]!=history["session_id"] || native_tick<history["last_tick"].as_u64().unwrap() { return Ok(()); }
    let before=history.clone();
    let mut phases=Map::new();
    for (prefix,source) in [("decisions","native_decision_membership_adapter"),("decisions.targeted","native_targeted_decision_adapter")] {
        let fields=if prefix=="decisions" {&["executed","active","completed"][..]}else{&["available","executed","active","completed"][..]};
        if !fields.iter().all(|phase|fact(state,&format!("{prefix}.{phase}"),source).is_some_and(|rows|rows.iter().all(|v|identity(text(v)).is_some_and(|(_,target)|(prefix=="decisions.targeted")==!target.is_null())))) {
            history["coverage"]["gaps_observed"]=json!(true);continue;
        }
        for phase in ["available","executed","active","completed"] {
            if prefix=="decisions" && phase=="available" {continue;}
            let Some(rows)=fact(state,&format!("{prefix}.{phase}"),source) else {history["coverage"]["gaps_observed"]=json!(true);continue;};
            for value in rows {
                if let Some((_,target))=identity(text(value)) && (prefix=="decisions.targeted")==!target.is_null() {
                    phases.insert(text(value).into(),json!(phase));
                }
            }
        }
    }
    let session=history["session_id"].clone();
    let entries=history["entries"].as_object_mut().unwrap();
    for row in entries.values_mut() {
        if !phases.contains_key(&token(&row["id"],&row["target"]).unwrap()) {
            if row["phase"]!="unknown" {row["last_tick"]=json!(native_tick);row["phase_tick"]=json!(native_tick);}
            row["phase"]=json!("unknown"); row["armed"]=json!(false); row["completion_pending"]=json!(false);
        }
    }
    for (key,phase) in phases {
        if !entries.contains_key(&key) && entries.len()>=16384 { return Err("Limite storico decisioni superato".into()); }
        let (ident,target)=identity(&key).unwrap();
        let row=entries.entry(key).or_insert_with(||json!({"id":ident,"target":target,"executions_lower_bound":0,"completions_lower_bound":0,"first_tick":native_tick,"last_tick":native_tick,"phase":"unknown","phase_tick":native_tick,"phase_session":session,"armed":false,"completion_pending":false}));
        let old=row.clone();
        let positive=phase!="available";
        let repeat=positive && row["armed"]==true && row["phase_session"]==session;
        if positive && (row["executions_lower_bound"]==0 || repeat) {
            row["executions_lower_bound"]=json!(row["executions_lower_bound"].as_u64().unwrap()+1);
            row["completion_pending"]=json!(true);
        }
        if phase=="completed" && (row["completions_lower_bound"]==0 || row["completion_pending"]==true) {
            row["completions_lower_bound"]=json!(row["completions_lower_bound"].as_u64().unwrap()+1);
            row["completion_pending"]=json!(false);
        }
        if phase=="available" {
            if row["phase"]!="available" || row["phase_session"]!=session {row["phase_tick"]=json!(native_tick);}
            row["armed"]=json!(true); row["completion_pending"]=json!(false);
        } else { row["armed"]=json!(false); if row["phase"]!=phase || row["phase_session"]!=session {row["phase_tick"]=json!(native_tick);} }
        row["phase"]=phase; row["phase_session"]=session.clone();
        if *row!=old {row["last_tick"]=json!(native_tick);}
    }
    if *history!=before {history["last_tick"]=json!(native_tick);}
    validate(history)
}
/// Separate historical facts: missing counters stay unknown, lists contain only
/// positive observations and carry an explicit lower-bound reason.
pub fn facts(history: &Value) -> Result<Map<String,Value>> {
    validate(history)?;
    let mut result=Map::new();
    let known=|v|json!({"status":"calculated","value":v,"source":"campaign_decision_observations","reason":"Observed lower bound; pre-observation history is unknown"});
    for (prefix,targeted) in [("decisions.history",false),("decisions.targeted.history",true)] {
        for (field,counter) in [("executed","executions_lower_bound"),("completed","completions_lower_bound")] {
            result.insert(format!("{prefix}.{field}"),known(json!(history["entries"].as_object().unwrap().iter().filter(|(_,r)| !r["target"].is_null()==targeted && r[counter].as_u64().unwrap()>0).map(|(k,_)|k).collect::<Vec<_>>())));
        }
    }
    for (key,row) in history["entries"].as_object().unwrap() {
        for (field,counter) in [("executions_lower_bound","executions_lower_bound"),("completions_lower_bound","completions_lower_bound")] {
            if row[counter].as_u64().unwrap()>0 {result.insert(format!("decisions.history.{field}:{key}"),known(row[counter].clone()));}
        }
    }
    Ok(result)
}
