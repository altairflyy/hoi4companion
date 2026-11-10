//! Pure v1 state contracts and provenance-preserving legacy projection.
//! No process, filesystem, DOM or guide-specific dependencies.
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
#[path = "state_legacy.rs"]
mod legacy;
#[path = "state_memberships.rs"]
mod memberships;
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn list(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn nonempty(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty())
}
pub(crate) fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
fn integer(v: &Value) -> bool {
    v.as_i64().is_some() || v.as_u64().is_some()
}
fn number(v: &Value) -> bool {
    v.as_f64()
        .is_some_and(|n| n.is_finite() && (n.fract() != 0. || n.abs() <= 9007199254740991.))
}
fn integer_range(v: &Value, min: i64, max: i64) -> bool {
    integer(v) && v.as_i64().is_some_and(|n| (min..=max).contains(&n))
}
fn id(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        !s.is_empty()
            && s.len() <= 160
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    })
}
fn tag(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 3
            && s.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    })
}
fn unique(values: &[Value]) -> bool {
    values
        .iter()
        .map(Value::to_string)
        .collect::<BTreeSet<_>>()
        .len()
        == values.len()
}
fn strings(v: &Value, max: usize, length: usize, distinct: bool) -> bool {
    v.as_array().is_some_and(|a| {
        a.len() <= max
            && a.iter().all(|v| {
                v.as_str()
                    .is_some_and(|s| !s.is_empty() && s.chars().count() <= length)
            })
            && (!distinct || unique(a))
    })
}
pub fn unknown(reason: &str) -> Value {
    let mut v = json!({"status":"unknown","value":null,"source":""});
    if !reason.is_empty() {
        v["reason"] = json!(reason)
    }
    v
}
pub(crate) fn known(value: Value, status: &str, source: &str) -> Value {
    json!({"status":status,"value":value,"source":source})
}
pub(crate) fn valid_lines(value: &Value) -> bool {
    let Some(lines) = value.as_array() else {
        return false;
    };
    if lines.len() > 4096 {
        return false;
    }
    let mut ids = BTreeSet::new();
    for row in lines {
        if !row.is_object()
            || ["id", "equipment", "name"]
                .iter()
                .any(|key| !nonempty(&row[key]))
            || !ids.insert(text(&row["id"]))
            || !["military", "naval"].contains(&text(&row["category"]))
        {
            return false;
        }
        if [
            "requested_factories",
            "assigned_factories",
            "active_factories",
            "damaged_factories",
        ]
        .iter()
        .any(|k| {
            !number(&row[k])
                || row[k]
                    .as_f64()
                    .is_none_or(|n| n.fract() != 0. || !(0. ..=9007199254740991.).contains(&n))
        }) {
            return false;
        }
        let assigned = row["assigned_factories"].as_f64().unwrap();
        if assigned
            != row["active_factories"].as_f64().unwrap()
                + row["damaged_factories"].as_f64().unwrap()
            || assigned > row["requested_factories"].as_f64().unwrap()
        {
            return false;
        }
        if row
            .get("design")
            .is_some_and(|v| !crate::production_context::valid_live_design(v))
        {
            return false;
        }
    }
    true
}
pub fn valid_fact(key: &str, value: &Value) -> bool {
    if key.starts_with("diplomacy.") || key.starts_with("intelligence.") {
        return memberships::valid(key, value);
    }
    match key {
        "country.metrics.war_support" | "country.metrics.stability" => {
            number(value) && value.as_f64().is_some_and(|n| (0. ..=1.).contains(&n))
        }
        "country.diplomacy.at_war"
        | "country.intelligence.is_created"
        | "country.intelligence.in_creation" => value.is_boolean(),
        "country.experience.army" | "country.experience.navy" | "country.experience.air" => {
            number(value)
                && value
                    .as_f64()
                    .is_some_and(|n| (0. ..=10_000_000.).contains(&n))
        }
        "country.production.effective_factories.military"
        | "country.production.effective_factories.dockyards" => integer_range(value, 0, 10_000_000),
        "doctrines.selected"
        | "politics.ideas"
        | "politics.advisors"
        | "politics.characters" | "politics.unit_leaders" | "politics.scientists"
        | "events.visible"
        | "events.visible_options"
        | "decisions.listed"
        | "decisions.executed" | "decisions.active" | "decisions.completed" => strings(value, 4096, 512, true),
        "production.lines" => valid_lines(value),
        "country" | "focus.tree" => nonempty(value),
        "focus.completed"
        | "focus.active"
        | "focus.continuous"
        | "focus.ids"
        | "research.completed"
        | "research.active"
        | "research.ids"
        | "construction.queued"
        | "projects.active"
        | "projects.completed" => value.as_array().is_some_and(|a| a.iter().all(nonempty)),
        "research.slots" => value.as_array().is_some_and(|a| {
            let indices = a.iter().filter_map(|v| v.get("slot")).collect::<Vec<_>>();
            a.len() <= 64
                && a.iter().all(|v| {
                    v.is_object()
                        && v.get("technology_id")
                            .is_some_and(|v| v.is_null() || nonempty(v))
                })
                && indices.iter().all(|v| integer_range(v, 0, 64))
                && indices
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == indices.len()
        }),
        _ => true,
    }
}
pub fn normalize(value: &Value) -> Value {
    if value.get("schema_version").is_some() {
        if !number(&value["schema_version"])
            || value["schema_version"].as_f64() != Some(1.)
            || value["usable"] != true
            || !value["facts"].is_object()
        {
            return json!({"schema_version":1,"usable":false,"session_id":null,"facts":{}});
        }
        let mut facts = Map::new();
        for (key, item) in value["facts"].as_object().unwrap() {
            let status = text(&item["status"]);
            let source = &item["source"];
            let data = &item["value"];
            let valid = item.is_object()
                && ["observed", "calculated", "unknown"].contains(&status)
                && if status == "unknown" {
                    data.is_null() && source == ""
                } else {
                    !data.is_null() && nonempty(source) && valid_fact(key, data)
                };
            if valid {
                let mut f = known(data.clone(), status, text(source));
                if let Some(reason) = item.get("reason").filter(|v| truth(v)) {
                    f["reason"] = reason.clone()
                }
                facts.insert(key.clone(), f);
            } else {
                facts.insert(key.clone(), unknown("Dato contratto non valido"));
            }
        }
        json!({"schema_version":1,"usable":true,"session_id":value["session_id"],"facts":facts})
    } else {
        legacy::normalize(value)
    }
}
