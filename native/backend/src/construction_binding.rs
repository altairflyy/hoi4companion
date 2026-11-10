//! Declared construction/project start and completion predicates; no acquisition.
use crate::{geography::Geography, import_catalog::Index};
use serde_json::{Value, json};
use std::collections::BTreeSet;
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
fn integer(v: &Value, min: u64) -> bool {
    v.as_u64().is_some_and(|n| n >= min)
}
fn fact(key: &str, test: &str, value: Value) -> Value {
    json!({"op":"fact","key":key,"test":test,"value":value})
}
fn attach(step: &mut Value, field: &str, node: Value) {
    if step.get(field).is_none() {
        step[field] = node;
    }
}
fn one(
    index: &Index,
    kind: &str,
    identifier: &Value,
    name: &Value,
) -> Result<Option<Value>, String> {
    let query = if identifier.is_null() {
        name.clone()
    } else if identifier.is_string() {
        identifier.clone()
    } else {
        json!(identifier.to_string())
    };
    let Some(query) = query.as_str().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let values = index.entries(
        json!({"op":"matches","query":query,"kinds":[kind],"explicit":!identifier.is_null()}),
    )?;
    Ok((values.len() == 1).then(|| values[0].clone()))
}
pub fn fortification(step: &mut Value, params: &mut Value, index: &Index) -> bool {
    if params["category"] != "construction"
        || !["bunker", "coastal_bunker"].contains(&s(&params["building_id"]))
    {
        return false;
    }
    let mut target = params
        .get("target_level")
        .or_else(|| params.get("target_quantity"))
        .cloned()
        .unwrap_or(Value::Null);
    if target.is_null() && params["quantity_mode"] == "target_total" {
        target = params["quantity"].clone();
    }
    if !target.as_u64().is_some_and(|n| (1..=1000).contains(&n)) {
        return false;
    }
    let limits = index
        .rows()
        .iter()
        .filter(|r| r["kind"] == "building" && r["id"] == params["building_id"])
        .filter_map(|r| r["max_level"].as_i64())
        .collect::<BTreeSet<_>>();
    if limits.len() == 1 && target.as_u64().unwrap() > (*limits.first().unwrap()).max(0) as u64 {
        return false;
    }
    let selector = &params["province_selector"];
    let provinces = params.get("province_ids").cloned().unwrap_or_else(|| {
        if params.get("province_id").is_some() {
            json!([params["province_id"]])
        } else {
            Value::Null
        }
    });
    let border = selector
        .as_object()
        .is_some_and(|m| m.len() == 2 && m.contains_key("kind") && m.contains_key("country"))
        && selector["kind"] == "border"
        && crate::recipient::tag(s(&selector["country"]));
    let explicit = provinces.as_array().is_some_and(|a| {
        !a.is_empty()
            && a.len() <= 512
            && a.iter()
                .all(|n| n.as_u64().is_some_and(|n| (1..=100000).contains(&n)))
            && a.iter()
                .filter_map(Value::as_u64)
                .collect::<BTreeSet<_>>()
                .len()
                == a.len()
    });
    if border == explicit {
        return false;
    }
    params["target_level"] = target;
    let key = format!("guide.fortifications.{}", s(&step["key"]));
    if params.get("province_id").is_some()
        && step["started_when"]
            == fact(
                "construction.queued",
                "contains",
                json!(format!(
                    "province:{}:{}",
                    params["province_id"],
                    s(&params["building_id"])
                )),
            )
    {
        step.as_object_mut().unwrap().remove("started_when");
    }
    attach(
        step,
        "started_when",
        fact(&(key.clone() + ".started"), "eq", json!(true)),
    );
    attach(
        step,
        "completion",
        fact(&(key + ".completed"), "eq", json!(true)),
    );
    true
}
pub fn bind(step: &Value, index: &Index, geography: &Geography) -> Result<Value, String> {
    let mut step = step.clone();
    let mut params = step["action"].get("params").cloned().unwrap_or(json!({}));
    let mut location = Value::Null;
    if !["manual", "special_project"].contains(&s(&step["action"]["kind"]))
        || step["tracking"] == "reminder"
    {
        return Ok(json!({"step":step,"handled":false,"reason":null,"location":null}));
    }
    let agency = params["operation_id"] == "create_intelligence_agency"
        || (["intelligenceagency", "agenziadiintelligence"].contains(
            &hoi4companion_catalog_core::normal_name(s(&params["project_name"])).as_str(),
        ) && (params["target_quantity"].is_null() || params["target_quantity"] == 1)
            && !truth(&params["upgrade_count"]));
    let reason = if agency {
        params["operation_id"] = json!("create_intelligence_agency");
        let created = fact("country.intelligence.is_created", "eq", json!(true));
        attach(
            &mut step,
            "started_when",
            json!({"op":"any","args":[fact("country.intelligence.in_creation","eq",json!(true)),created]}),
        );
        attach(&mut step, "completion", created);
        "intelligence_creation_flags".to_owned()
    } else if params["category"] == "projects"
        && (truth(&params["project_id"]) || truth(&params["project_name"]))
        || step["action"]["kind"] == "special_project"
    {
        let identifier = if truth(&params["project_id"]) {
            &params["project_id"]
        } else {
            &step["action"]["entity"]["id"]
        };
        let project = one(index, "project", identifier, &params["project_name"])?;
        if let Some(project) = project {
            params["project_id"] = project["id"].clone();
            step["action"]["kind"] = json!("special_project");
            step["action"]["entity"] = json!({"kind":"project","id":project["id"]});
            let done = fact("projects.completed", "contains", project["id"].clone());
            attach(
                &mut step,
                "started_when",
                json!({"op":"any","args":[fact("projects.active","contains",project["id"].clone()),done]}),
            );
            attach(&mut step, "completion", done);
            "project_live_state".into()
        } else {
            "project_identity_unresolved".into()
        }
    } else if params["category"] == "construction" {
        let building = one(
            index,
            "building",
            &params["building_id"],
            &params["building_name"],
        )?;
        let provincial = building
            .as_ref()
            .is_some_and(|b| b["location_scope"] == "province");
        location = geography.resolve(&params, provincial);
        let region = location["state_id"].as_u64();
        let mut reason = "building_or_state_not_resolved".to_owned();
        let has_group =
            params.get("province_selector").is_some() || params.get("province_ids").is_some();
        let group = provincial
            && building
                .as_ref()
                .is_some_and(|b| ["bunker", "coastal_bunker"].contains(&s(&b["id"])))
            && has_group;
        let invalid_group = has_group && !group;
        if invalid_group {
            reason = "province_selector_requires_provincial_fort".into();
        } else if group {
            let building = building.as_ref().unwrap();
            params["building_id"] = building["id"].clone();
            let known = params["province_ids"].as_array().is_none_or(|ids| {
                geography.provinces.is_empty()
                    || ids.iter().filter_map(Value::as_u64).all(|n| {
                        geography.provinces.get(&n).is_some_and(|states| {
                            states.len() == 1 && region.is_none_or(|r| states.contains(&r))
                        })
                    })
            });
            reason = if known && fortification(&mut step, &mut params, index) {
                "provincial_fortification_goal"
            } else {
                "fortification_target_unresolved"
            }
            .into();
        } else if let Some(building) = &building
            && region.is_none()
        {
            if ["state_id", "state_name", "province_id"]
                .iter()
                .all(|k| params[*k].is_null())
            {
                params["building_id"] = building["id"].clone();
                attach(
                    &mut step,
                    "started_when",
                    fact(
                        "construction.queued",
                        "contains",
                        json!(format!("building:{}", s(&building["id"]))),
                    ),
                );
                reason = "building_start_without_location".into();
            } else {
                reason = s(&location["reason"]).into();
            }
        }
        if let Some(building) = &building
            && let Some(region) = region
            && !group
            && !invalid_group
        {
            params["building_id"] = building["id"].clone();
            params["state_id"] = json!(region);
            if provincial {
                params["province_id"] = location["province_id"].clone();
            }
            if ["state", "province"].contains(&s(&building["location_scope"])) {
                params["building_scope"] = building["location_scope"].clone();
            }
            let queue = if provincial {
                format!("province:{}:{}", params["province_id"], s(&building["id"]))
            } else {
                format!("state:{region}:{}", s(&building["id"]))
            };
            attach(
                &mut step,
                "started_when",
                fact("construction.queued", "contains", json!(queue)),
            );
            let mut target = params["target_level"].clone();
            if !integer(&target, 1) {
                target = params["target_quantity"].clone();
            }
            if !integer(&target, 1) {
                if params["quantity_mode"] == "target_total" {
                    target = params["quantity"].clone();
                } else if params["quantity_mode"] == "additional"
                    && integer(&params["baseline_level"], 0)
                    && integer(&params["quantity"], 1)
                {
                    target = json!(
                        params["baseline_level"]
                            .as_u64()
                            .unwrap()
                            .checked_add(params["quantity"].as_u64().unwrap())
                            .ok_or("Construction target exceeds integer range")?
                    );
                }
            }
            let field = if provincial {
                (!matches!(s(&building["id"]), "bunker" | "coastal_bunker"))
                    .then(|| format!("buildings.{}",s(&building["id"])))
            } else {
                match s(&building["id"]) {
                    "infrastructure" => Some("infrastructure".to_owned()),
                    "arms_factory" => Some("military_factories".to_owned()),
                    "industrial_complex" => Some("civilian_factories".to_owned()),
                    "dockyard" => Some("dockyards".to_owned()),
                    id if building["location_scope"] == "state" => {
                        params["building_scope"] = json!("state");
                        Some(format!("buildings.{id}"))
                    },
                    _ => None,
                }
            };
            reason = if params["quantity_mode"] == "additional" {
                "additional_baseline_missing"
            } else {
                "target_not_defined"
            }
            .into();
            if let Some(field) = field {
                if integer(&target, 1) {
                    attach(
                        &mut step,
                        "completion",
                        fact(
                            &if provincial {format!("country.fortifications.items.{}.{field}",params["province_id"])} else {format!("country.states.items.{region}.{field}")},
                            "gte",
                            target.clone(),
                        ),
                    );
                    reason = "observed_building_level".into();
                } else if params["quantity_mode"] == "additional" && integer(&params["quantity"], 1)
                {
                    let key = format!("guide.construction.{}.added", s(&step["key"]));
                    attach(
                        &mut step,
                        "completion",
                        fact(&key, "gte", params["quantity"].clone()),
                    );
                    reason = "baseline_from_live".into();
                }
            } else {
                reason = "building_completion_not_observed".into();
            }
            if provincial
                && ["bunker", "coastal_bunker"].contains(&s(&building["id"]))
                && params["quantity_mode"] == "additional"
                && !integer(&target, 1)
                && integer(&params["quantity"], 1)
            {
                let key = format!("guide.construction.{}.added", s(&step["key"]));
                attach(
                    &mut step,
                    "completion",
                    fact(&key, "gte", params["quantity"].clone()),
                );
                reason = "baseline_from_live".into();
            }
            if provincial && fortification(&mut step, &mut params, index) {
                reason = "provincial_fortification_goal".into();
            }
        }
        reason
    } else {
        return Ok(json!({"step":step,"handled":false,"reason":null,"location":null}));
    };
    step["action"]["params"] = params;
    Ok(json!({"step":step,"handled":true,"reason":reason,"location":location}))
}
