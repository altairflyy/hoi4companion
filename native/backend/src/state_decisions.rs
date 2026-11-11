//! Versioned target-qualified memberships. No counter is an execution count.
use super::*;
pub(crate) const FIELDS: [&str; 4] = ["available", "active", "executed", "completed"];
pub(crate) fn token(id_value: &Value, target: &Value) -> Option<String> {
    if !id(id_value) { return None; }
    let tail = match text(&target["kind"]) {
        "country" if tag(&target["id"]) => format!("country:{}", text(&target["id"])),
        "state" if integer_range(&target["id"], 1, 4095) => format!("state:{}", target["id"]),
        _ => return None,
    };
    Some(format!("{}:{tail}", text(id_value)))
}
pub(crate) fn valid_fact(key: &str, value: &Value) -> bool {
    FIELDS.iter().any(|f| key == format!("decisions.targeted.{f}")) && value.as_array().is_some_and(|a| a.len() <= 8192 && unique(a) && a.iter().all(|v| {
        let parts = text(v).split(':').collect::<Vec<_>>();
        parts.len() == 3 && token(&json!(parts[0]), &json!({"kind":parts[1],"id":if parts[1]=="state" {parts[2].parse::<u64>().ok().map(Value::from).unwrap_or(Value::Null)}else{json!(parts[2])}})).as_deref() == v.as_str()
    }))
}
pub(crate) fn facts(history: &Value) -> Map<String, Value> {
    let mut out = FIELDS.into_iter().map(|f| (format!("decisions.targeted.{f}"), unknown("Targeted decision records not qualified"))).collect::<Map<_,_>>();
    let target = &history["targeted"];
    let Some(rows) = target["records"].as_array() else { return out; };
    if history["status"] != "observed" || history["source"] != "native_decisions_taken_and_timed_state" || target["schema_version"] != 1 || target["status"] != "observed" || target["source"] != "native_targeted_decision_records" || target["scope"] != "current_native_records" || rows.len()>8192 { return out; }
    let mut seen = BTreeSet::new();
    if !rows.iter().all(|r| token(&r["id"], &r["target"]).is_some() && id(&r["instance_id"]) && seen.insert(text(&r["instance_id"])) && ["targeted","targeted_timed"].contains(&text(&r["record_kind"])) && r["ignored"].is_boolean() && ["available","active","completed","cooldown","failed","aborted"].contains(&text(&r["phase"])) && (r["phase"]!="active" || r["record_kind"]=="targeted_timed") && (r["phase"]!="available" || r["record_kind"]=="targeted")) { return out; }
    for field in FIELDS {
        let values=rows.iter().filter(|r| r["ignored"]==false && match field { "executed"=>r["phase"]!="available", _=>r["phase"]==field }).filter_map(|r| token(&r["id"],&r["target"])).collect::<BTreeSet<_>>();
        out.insert(format!("decisions.targeted.{field}"),known(json!(values),"calculated","native_targeted_decision_adapter"));
    }
    out
}
