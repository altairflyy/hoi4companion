//! Compile declared political identities to qualified memberships, never availability.
use crate::import_catalog::Index;
use hoi4companion_catalog_core::normal_name;
use serde_json::{Value, json};
use std::collections::HashMap;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.),
    }
}
fn fact(key: &str, value: &Value) -> Value {
    json!({"op":"fact","key":key,"test":"contains","value":value})
}
fn attach(step: &mut Value, params: &mut Value, field: &str, node: Value) {
    if step.get(field).is_none() {
        step[field] = node.clone();
        if params.get("political_binding").is_none() {
            params["political_binding"] = json!({});
        }
        params["political_binding"][field] = node;
    }
}
struct Audit {
    record: bool,
    items: Vec<Value>,
}
fn matches(
    index: &Index,
    kinds: &[&str],
    identifier: &Value,
    name: &Value,
    country: &Value,
    filters: Value,
    audit: &mut Audit,
) -> Result<Vec<Value>, String> {
    let query = if identifier.is_null() {
        name
    } else {
        identifier
    };
    let Some(query) = query.as_str().filter(|s| !s.is_empty()) else {
        return Ok(vec![]);
    };
    if audit.record {
        audit.items.push(index.query(json!({"op":"audit","query":query,"kinds":kinds,"country":country,"explicit":!identifier.is_null(),"filters":filters}))?);
    }
    index.entries(json!({"op":"matches","query":query,"kinds":kinds,"country":country,"explicit":!identifier.is_null(),"filters":filters}))
}
fn reason(found: &[Value]) -> &'static str {
    if found.is_empty() {
        "politics_identity_unresolved"
    } else {
        "politics_identity_ambiguous"
    }
}
fn fallback<'a>(left: &'a Value, right: &'a Value) -> &'a Value {
    if truth(left) { left } else { right }
}
pub fn bind(step: &Value, index: &Index, country: &Value, record: bool) -> Result<Value, String> {
    let mut step = step.clone();
    let had_params = step["action"].get("params").is_some();
    let mut params = step["action"].get("params").cloned().unwrap_or(json!({}));
    let mut audit = Audit {
        record,
        items: vec![],
    };
    if params["category"] == "projects" {
        return Ok(json!({"step":step,"reason":null,"audit":audit.items}));
    }
    let previous = params
        .as_object_mut()
        .ok_or("Invalid political parameters")?
        .remove("political_binding")
        .unwrap_or(json!({}));
    if previous.is_object() {
        for field in ["completion", "started_when"] {
            if previous.get(field).is_some() && step.get(field) == previous.get(field) {
                step.as_object_mut()
                    .ok_or("Invalid guide step")?
                    .remove(field);
            }
        }
    }
    let result = (|| -> Result<Option<&'static str>, String> {
        if !params["law_id"].is_null() || !params["law_name"].is_null() {
            let filters = if truth(&params["law_group"]) {
                json!({"group":params["law_group"]})
            } else {
                json!({})
            };
            let found = matches(
                index,
                &["law"],
                &params["law_id"],
                &params["law_name"],
                country,
                filters,
                &mut audit,
            )?;
            if found.len() != 1 {
                return Ok(Some(reason(&found)));
            }
            let item = &found[0];
            params["law_id"] = item["id"].clone();
            params["law_group"] = item["group"].clone();
            attach(
                &mut step,
                &mut params,
                "completion",
                fact("politics.ideas", &item["id"]),
            );
            return Ok(Some("politics_live_law"));
        }
        if !params["commander_id"].is_null()
            || ["corps_commander", "navy_leader"].contains(&s(&params["role"]))
        {
            let slot = &params["role"];
            let found = matches(
                index,
                &["commander"],
                fallback(&params["commander_id"], &params["person_id"]),
                fallback(&params["person_name"], &params["trait_name"]),
                country,
                if truth(slot) {
                    json!({"slot":slot})
                } else {
                    json!({})
                },
                &mut audit,
            )?;
            if found.len() != 1 {
                return Ok(Some(reason(&found)));
            }
            params["commander_id"] = found[0]["id"].clone();
            params["role"] = found[0]["slot"].clone();
            return Ok(Some("commander_assignment_unobserved"));
        }
        if [
            "advisor_id",
            "person_id",
            "person_name",
            "trait_name",
            "role",
        ]
        .iter()
        .any(|k| !params[*k].is_null())
        {
            let identifier = &params["advisor_id"];
            let query = fallback(
                &params["person_id"],
                fallback(
                    &params["person_name"],
                    fallback(&params["trait_name"], &params["role"]),
                ),
            );
            let slot = &params["role"];
            let mut slots = HashMap::new();
            for row in index.entries(json!({"op":"rows","kinds":["advisor"],"country":country}))? {
                slots.insert(normal_name(s(&row["slot"])), row["slot"].clone());
            }
            let groups = [
                "industrial_concern",
                "theorist",
                "political_advisor",
                "army_chief",
                "navy_chief",
                "air_chief",
                "high_command",
            ];
            for row in index.entries(json!({"op":"rows","kinds":["idea"],"country":country}))? {
                if groups.contains(&s(&row["group"])) {
                    slots.insert(normal_name(s(&row["group"])), row["group"].clone());
                }
            }
            if !slot.is_null()
                && (slot.as_str().is_none() || !slots.contains_key(&normal_name(s(slot))))
            {
                return Ok(Some("politics_identity_unresolved"));
            }
            let filters = if slot.is_null() {
                json!({})
            } else {
                json!({"slot":slots[&normal_name(s(slot))]})
            };
            let found = matches(
                index,
                &["advisor"],
                identifier,
                query,
                country,
                filters,
                &mut audit,
            )?;
            let concerns = if identifier.is_null() && params["person_id"].is_null() {
                matches(
                    index,
                    &["idea"],
                    &Value::Null,
                    query,
                    country,
                    json!({}),
                    &mut audit,
                )?
            } else {
                vec![]
            };
            let concerns = concerns
                .into_iter()
                .filter(|row| {
                    (slot.is_null() || normal_name(s(&row["group"])) == normal_name(s(slot)))
                        && groups.contains(&s(&row["group"]))
                })
                .collect::<Vec<_>>();
            if found.is_empty() && concerns.len() == 1 {
                let item = &concerns[0];
                params["idea_id"] = item["id"].clone();
                attach(
                    &mut step,
                    &mut params,
                    "completion",
                    fact("politics.ideas", &item["id"]),
                );
                return Ok(Some("politics_live_idea"));
            }
            if found.len() != 1 {
                return Ok(Some(reason(&found)));
            }
            let item = &found[0];
            params["advisor_id"] = item["id"].clone();
            params["person_id"] = item["character_id"].clone();
            params["role"] = item["slot"].clone();
            attach(
                &mut step,
                &mut params,
                "completion",
                fact("politics.advisors", &item["id"]),
            );
            return Ok(Some("politics_live_advisor"));
        }
        if ["event_id", "event_name", "event_option", "event_option_id"]
            .iter()
            .any(|k| !params[*k].is_null())
        {
            let events = matches(
                index,
                &["event"],
                &params["event_id"],
                &params["event_name"],
                country,
                json!({}),
                &mut audit,
            )?;
            if (!params["event_id"].is_null() || !params["event_name"].is_null())
                && events.len() != 1
            {
                return Ok(Some(if events.is_empty() {
                    "event_identity_unresolved"
                } else {
                    "event_identity_ambiguous"
                }));
            }
            let options = matches(
                index,
                &["event_option"],
                &params["event_option_id"],
                &params["event_option"],
                country,
                if events.is_empty() {
                    json!({})
                } else {
                    json!({"event_id":events[0]["id"]})
                },
                &mut audit,
            )?;
            if options.len() == 1 {
                let option = &options[0];
                params["event_id"] = option["event_id"].clone();
                params["event_option_id"] = option["id"].clone();
                params["event_option_name"] = option
                    .get("localized_name")
                    .unwrap_or(&option["name"])
                    .clone();
                attach(
                    &mut step,
                    &mut params,
                    "started_when",
                    fact(
                        "events.visible_options",
                        &json!(format!("{}:{}", s(&option["event_id"]), s(&option["id"]))),
                    ),
                );
                return Ok(Some("event_visible_option"));
            }
            return Ok(Some(if options.is_empty() {
                "event_identity_unresolved"
            } else {
                "event_identity_ambiguous"
            }));
        }
        for (kind, id_key, name_key, result) in [
            (
                "decision",
                "decision_id",
                "decision_name",
                "decision_identity_resolved",
            ),
            ("idea", "idea_id", "idea_name", "politics_live_idea"),
        ] {
            if !params[id_key].is_null() || !params[name_key].is_null() {
                let found = matches(
                    index,
                    &[kind],
                    &params[id_key],
                    &params[name_key],
                    country,
                    json!({}),
                    &mut audit,
                )?;
                if found.len() != 1 {
                    return Ok(Some(reason(&found)));
                }
                params[id_key] = found[0]["id"].clone();
                if kind == "idea" {
                    attach(
                        &mut step,
                        &mut params,
                        "completion",
                        fact("politics.ideas", &found[0]["id"]),
                    );
                } else if found[0]["targeted"] == false && params.get("target_country").is_none() && params.get("target_state").is_none() {
                    attach(&mut step, &mut params, "started_when", fact("decisions.active", &found[0]["id"]));
                    attach(&mut step, &mut params, "completion", fact("decisions.executed", &found[0]["id"]));
                }
                return Ok(Some(result));
            }
        }
        Ok(None)
    })()?;
    if had_params || params.as_object().is_some_and(|m| !m.is_empty()) {
        step["action"]["params"] = params;
    }
    Ok(json!({"step":step,"reason":result,"audit":audit.items}))
}
pub fn doctrine(step: &Value, index: &Index, country: &Value) -> Result<Value, String> {
    let mut step = step.clone();
    let p = &step["action"]["params"];
    let explicit = truth(&p["doctrine_id"]);
    let query = fallback(&p["doctrine_id"], &p["doctrine_name"]);
    let found = matches(
        index,
        &["doctrine"],
        if explicit { query } else { &Value::Null },
        query,
        country,
        json!({}),
        &mut Audit {
            record: false,
            items: vec![],
        },
    )?;
    if found.len() != 1 {
        return Ok(json!({"step":step,"reason":"doctrine_identity_unresolved"}));
    }
    let row = &found[0];
    let p = &mut step["action"]["params"];
    for (to, from) in [
        ("doctrine_id", "id"),
        ("doctrine_name", "name"),
        ("doctrine_domain", "domain"),
        ("doctrine_track", "track"),
    ] {
        p[to] = row[from].clone();
    }
    if step.get("completion").is_none() {
        step["completion"] = fact("doctrines.selected", &row["id"]);
    }
    Ok(json!({"step":step,"reason":"doctrine_selection_unobserved"}))
}
pub(crate) fn foreign_tasks() -> &'static Value {
    use std::sync::OnceLock;
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../src/hoi4companion/foreign-tasks-v1.json"
        ))
        .expect("checked-in foreign task vocabulary")
    })
}
pub fn foreign(step: &Value, index: &Index) -> Result<Value, String> {
    let data = foreign_tasks();
    let tasks = data["tasks"]
        .as_object()
        .ok_or("Invalid foreign task vocabulary")?;
    let mut step = step.clone();
    let mut params = step["action"].get("params").cloned().unwrap_or(json!({}));
    let mut key = params["operation_id"].as_str().map(str::to_owned);
    if key.as_deref() != Some("assign_operative_mission")
        && !key.as_ref().is_some_and(|k| tasks.contains_key(k))
    {
        let raw = if params["category"] == "projects" {
            &params["operation_name"]
        } else if params["category"] == "politics" {
            &params["task"]
        } else {
            &Value::Null
        };
        key = raw.as_str().and_then(|raw| {
            tasks
                .iter()
                .find(|(key, values)| {
                    std::iter::once(key.as_str())
                        .chain(
                            values
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_str),
                        )
                        .any(|name| normal_name(name) == normal_name(raw))
                })
                .map(|(key, _)| key.clone())
        });
    }
    let Some(key) = key else {
        return Ok(json!({"step":step,"bound":false}));
    };
    let Some(query) = params["target_country"].as_str() else {
        return Ok(json!({"step":step,"bound":false}));
    };
    let target = index.query(json!({"op":"resolve_country","query":query}))?;
    if !target.is_string() {
        return Ok(json!({"step":step,"bound":false}));
    }
    params["target_country"] = target;
    params["operation_id"] = json!(key);
    if ![
        "improve_relations",
        "stop_improving_relations",
        "non_aggression_pact",
        "assign_operative_mission",
    ]
    .contains(&key.as_str()) && !data["relation_actions"][&key].is_object()
    {
        params["operation_id"] = json!("assign_operative_mission");
        params["mission_id"] = json!(key);
    }
    let Some(node) = crate::guide_prepare::foreign_predicate(&params) else {
        return Ok(json!({"step":step,"bound":false}));
    };
    if [
        "network_strength",
        "target_strength",
        "target_quantity",
        "upgrade_count",
    ]
    .iter()
    .any(|k| params.get(*k).is_some())
    {
        return Ok(json!({"step":step,"bound":false}));
    }
    let token = crate::guide_prepare::foreign_token(&step["key"], &node);
    step["action"]["params"] = params;
    if step.get("started_when").is_none() {
        step["started_when"] = node.clone();
    }
    if step.get("completion").is_none() {
        step["completion"] =
            json!({"op":"any","args":[node,fact("guide.campaign.milestones",&json!(token))]});
    }
    Ok(json!({"step":step,"bound":true}))
}
