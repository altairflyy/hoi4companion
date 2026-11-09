//! Literal production orders. Requested, assigned and remaining capacity stay distinct.
use crate::{
    import_catalog::{Index, regex},
    reference_identity,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
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
fn fact(key: &str, test: &str, value: Value) -> Value {
    json!({"op":"fact","key":key,"test":test,"value":value})
}
fn attach(step: &mut Value, field: &str, node: Value) {
    if step.get(field).is_none() {
        step[field] = node;
    }
}
fn owned(node: &Value, prefix: &str) -> bool {
    if !node.is_object() {
        return false;
    }
    if node["op"] == "fact" {
        return s(&node["key"]).strip_prefix(prefix).is_some_and(|tail|regex(r"^(?:\d+\.)?(?:assigned|requested|added|remaining_allocated|priority_satisfied|started)$").is_match(tail));
    }
    ["all", "any"].contains(&s(&node["op"]))
        && !arr(&node["args"]).is_empty()
        && arr(&node["args"]).iter().all(|n| owned(n, prefix))
}
fn run(
    step: &mut Value,
    params: &mut Value,
    index: &Index,
    country: &Value,
    library: &Value,
    declared: &Value,
    depth: usize,
) -> Result<&'static str, String> {
    if depth > 8 {
        return Err("Production target nesting exceeds limit".into());
    }
    let key = s(&step["key"]).to_owned();
    if let Some(targets) = params.get("production_targets").cloned() {
        for (field, op) in [("completion", "all"), ("started_when", "any")] {
            if step[field]["op"] == op
                && owned(&step[field], &format!("guide.production.{key}.target-"))
            {
                step.as_object_mut().unwrap().remove(field);
            }
        }
        if !targets.as_array().is_some_and(|a| {
            !a.is_empty()
                && a.len() <= 64
                && a.iter()
                    .all(|p| p.is_object() && p.get("production_targets").is_none())
        }) {
            return Ok("production_targets_invalid");
        }
        if arr(&targets)
            .iter()
            .any(|p| p["quantity_mode"] == "additional")
        {
            return Ok("production_layout_requires_totals");
        }
        let remaining = arr(&targets)
            .iter()
            .enumerate()
            .filter(|(_, p)| p["quantity_mode"] == "remaining")
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if !remaining.is_empty() && remaining != vec![arr(&targets).len() - 1] {
            return Ok("production_targets_invalid");
        }
        let mut children = vec![];
        let mut reasons = vec![];
        for (i, p) in arr(&targets).iter().enumerate() {
            let mut p = p.clone();
            p["category"] = json!("production");
            let mut child =
                json!({"key":format!("{key}.target-{}",i+1),"action":{"kind":"manual"}});
            reasons.push(run(
                &mut child,
                &mut p,
                index,
                country,
                library,
                declared,
                depth + 1,
            )?);
            child["action"]["params"] = p;
            children.push(child);
        }
        params["production_targets"] = json!(
            children
                .iter()
                .map(|c| c["action"]["params"].clone())
                .collect::<Vec<_>>()
        );
        if !remaining.is_empty() {
            let selected=arr(&children.last().unwrap()["action"]["params"]["production_selector"]["equipment_ids"]).iter().map(s).collect::<BTreeSet<_>>();
            if children[..children.len() - 1].iter().any(|child| {
                arr(&child["action"]["params"]["production_selector"]["equipment_ids"])
                    .iter()
                    .any(|id| selected.contains(s(id)))
            }) {
                return Ok("production_targets_invalid");
            }
        }
        let complete = children.iter().all(|c| c.get("completion").is_some());
        if complete {
            attach(
                step,
                "completion",
                json!({"op":"all","args":children.iter().map(|c|c["completion"].clone()).collect::<Vec<_>>()}),
            );
        }
        if children.iter().all(|c| c.get("started_when").is_some()) {
            attach(
                step,
                "started_when",
                json!({"op":"any","args":children.iter().map(|c|c["started_when"].clone()).collect::<Vec<_>>()}),
            );
        }
        return Ok(if complete {
            "production_layout"
        } else {
            reasons
                .into_iter()
                .find(|r| {
                    ![
                        "production_live_assignment",
                        "production_relative_assignment",
                        "production_remaining_assignment",
                    ]
                    .contains(r)
                })
                .unwrap_or("production_target_missing")
        });
    }
    if params.get("production_selector").is_some() {
        let prefix = format!("guide.production.{key}.");
        for field in ["completion", "started_when"] {
            if owned(&step[field], &prefix) {
                step.as_object_mut().unwrap().remove(field);
            }
        }
        params
            .as_object_mut()
            .unwrap()
            .remove("production_selector");
    }
    params
        .as_object_mut()
        .ok_or("Invalid production parameters")?
        .remove("design_reference");
    if !params["mio_name"].is_null() {
        return Ok("production_mio_not_observed");
    }
    let query = params
        .get("equipment_id")
        .filter(|v| !v.is_null())
        .unwrap_or(&params["equipment_name"]);
    let Some(query) = query.as_str().filter(|q| !q.is_empty()).map(str::to_owned) else {
        return Ok("production_equipment_unresolved");
    };
    let explicit = !params["equipment_id"].is_null();
    let mut matches=index.entries(json!({"op":"matches","query":query,"kinds":["equipment"],"country":country,"explicit":explicit}))?;
    let roots = matches
        .iter()
        .filter(|e| truth(&e["is_archetype"]))
        .cloned()
        .collect::<Vec<_>>();
    if roots.len() == 1 && matches.iter().all(|e| e["family_id"] == roots[0]["id"]) {
        matches = roots;
    }
    let choices = if !explicit && matches.is_empty() {
        reference_identity::selector(&query, index, library, declared)?
    } else {
        json!([])
    };
    let choices = arr(&choices);
    if choices.len() > 1 && choices.iter().any(|row| truth(&row["reference"])) {
        return Ok("production_equipment_ambiguous");
    }
    if choices.len() == 1 && truth(&choices[0]["reference"]) {
        matches.clear();
        params["design_reference"] = choices[0]["reference"].clone();
    }
    if matches.is_empty() && choices.len() > 1 {
        return Ok("production_equipment_ambiguous");
    }
    let (mut identifiers, mut variants) = if matches.is_empty() && choices.len() == 1 {
        (
            choices[0]["equipment_ids"].clone(),
            choices[0]["variant_names"].clone(),
        )
    } else if matches.is_empty() {
        return Ok("production_equipment_unresolved");
    } else {
        if matches.len() != 1 {
            return Ok("production_equipment_ambiguous");
        }
        let equipment = &matches[0];
        (
            equipment
                .get("equipment_ids")
                .cloned()
                .unwrap_or(json!([equipment["id"]])),
            json!([]),
        )
    };
    if let Some(variant) = params.get("variant_name").filter(|v| !v.is_null()) {
        let Some(variant) = variant
            .as_str()
            .filter(|v| !v.trim().is_empty() && !v.contains(';'))
        else {
            return Ok("production_variant_ambiguous");
        };
        variants = json!([variant]);
        let named = index.matches(variant, &["equipment"], country, &Value::Null, false)?;
        if named.len() > 1 {
            return Ok("production_variant_ambiguous");
        }
        if let Some(chosen) = named.first() {
            let selected = chosen
                .get("equipment_ids")
                .cloned()
                .unwrap_or(json!([chosen["id"]]));
            let allowed = arr(&identifiers).iter().map(s).collect::<BTreeSet<_>>();
            if !arr(&selected).iter().all(|id| allowed.contains(s(id))) {
                return Ok("production_variant_incompatible");
            }
            identifiers = selected;
            variants = json!([]);
        }
    }
    let category = if truth(&params["production_category"]) {
        params["production_category"].clone()
    } else if choices.len() == 1 {
        choices[0]["category"].clone()
    } else {
        Value::Null
    };
    if !category.is_null() && !["military", "naval"].contains(&s(&category)) {
        return Ok("production_category_invalid");
    }
    if arr(&identifiers).is_empty() {
        return Ok("production_equipment_unresolved");
    }
    let mut ids = arr(&identifiers).to_vec();
    ids.sort_by(|a, b| s(a).cmp(s(b)));
    let mut names = arr(&variants).to_vec();
    names.sort_by(|a, b| s(a).cmp(s(b)));
    params["production_selector"] =
        json!({"equipment_ids":ids,"variant_names":names,"category":category});
    let key = format!("guide.production.{key}");
    let priority = params["production_priority"].clone();
    if !priority.is_null()
        && (!["first", "last"].contains(&s(&priority))
            || category != "military"
            || params["quantity_mode"] == "additional")
    {
        return Ok("production_priority_invalid");
    }
    let complete = |step: &mut Value, condition: Value| {
        let condition = if truth(&priority) {
            json!({"op":"all","args":[condition,fact(&format!("{key}.priority_satisfied"),"eq",json!(true))]})
        } else {
            condition
        };
        attach(step, "completion", condition);
    };
    attach(
        step,
        "started_when",
        fact(&format!("{key}.started"), "eq", json!(true)),
    );
    if params["quantity_mode"] == "remaining" {
        if category != "military"
            || [
                "requested_factories",
                "assigned_factories",
                "assignment_test",
            ]
            .iter()
            .any(|k| params.get(*k).is_some())
        {
            return Ok("production_target_invalid");
        }
        complete(
            step,
            fact(&format!("{key}.remaining_allocated"), "eq", json!(true)),
        );
        return Ok("production_remaining_assignment");
    }
    let requested = params.get("requested_factories").is_some();
    let target = if requested {
        &params["requested_factories"]
    } else {
        &params["assigned_factories"]
    };
    if requested && params.get("assigned_factories").is_some() {
        return Ok("production_target_invalid");
    }
    if target.as_u64().is_none() {
        return Ok("production_target_missing");
    }
    let relative = params["quantity_mode"] == "additional";
    if relative && target == 0 {
        return Ok("production_target_missing");
    }
    let test = if relative {
        json!("gte")
    } else {
        params
            .get("assignment_test")
            .cloned()
            .unwrap_or(json!("eq"))
    };
    if !["eq", "gte"].contains(&s(&test)) {
        return Ok("production_target_invalid");
    }
    complete(
        step,
        fact(
            &format!(
                "{key}.{}",
                if relative {
                    "added"
                } else if requested {
                    "requested"
                } else {
                    "assigned"
                }
            ),
            s(&test),
            target.clone(),
        ),
    );
    Ok(if relative {
        "production_relative_assignment"
    } else {
        "production_live_assignment"
    })
}
pub fn bind(
    step: &Value,
    index: &Index,
    country: &Value,
    library: &Value,
    declared: &Value,
) -> Result<Value, String> {
    let mut step = step.clone();
    let had_params = step["action"].get("params").is_some();
    let mut params = step["action"].get("params").cloned().unwrap_or(json!({}));
    let reason = run(&mut step, &mut params, index, country, library, declared, 0)?;
    if had_params || params.as_object().is_some_and(|m| !m.is_empty()) {
        step["action"]["params"] = params;
    }
    Ok(json!({"step":step,"reason":reason}))
}
