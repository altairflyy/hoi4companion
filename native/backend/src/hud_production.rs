//! Production hints describe only runtime-admitted author targets. File recipes
//! remain navigation hints; they never prove a live production model match.
use crate::{production_context, state};
use serde_json::{Value, json};
use std::collections::BTreeSet;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn a(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn number(v: &Value) -> bool {
    v.is_number()
}
fn unique(values: impl Iterator<Item = Value>) -> Vec<Value> {
    let mut result = vec![];
    for value in values.filter(|v| v.as_str().is_some_and(|s| !s.is_empty())) {
        if !result.contains(&value) {
            result.push(value)
        }
    }
    result
}
fn add_control(category: &Value) -> Value {
    json!(match s(category) {
        "interface_category_land" => Some("infantry_button"),
        "interface_category_armor" => Some("tank_button"),
        "interface_category_air" => Some("aircraft_button"),
        "interface_category_capital_ships"
        | "interface_category_screen_ships"
        | "interface_category_other_ships" => Some("naval_button"),
        _ => None,
    })
}
pub fn proposals(
    row: &Value,
    item: &Value,
    catalog: &[Value],
    country: &Value,
    live: &Value,
) -> Vec<Value> {
    let params = &row["action"]["params"];
    let children = params["production_targets"].as_array();
    let layout = &item["production"]["layout"];
    let tuples = if let Some(children) = children {
        children
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    p,
                    layout.get(i).unwrap_or(&Value::Null),
                    format!(".target-{}", i + 1),
                )
            })
            .collect::<Vec<_>>()
    } else {
        vec![(params, &item["production"], String::new())]
    };
    let equipment = catalog
        .iter()
        .filter(|e| e["kind"] == "equipment")
        .collect::<Vec<_>>();
    let mut result = vec![];
    for (p, detail, suffix) in tuples {
        if state::truth(&detail["superseded_by"]) {
            continue;
        }
        let Some(selector) = production_context::selector_for(&json!({"action":{"params":p}}))
        else {
            continue;
        };
        let mut target = detail["target"].clone();
        let basis = &detail["basis"];
        let count = detail.get(s(basis)).unwrap_or(&Value::Null);
        if target.is_null()
            && p["quantity_mode"] != "additional"
            && p["quantity_mode"] != "remaining"
        {
            target = p
                .get("requested_factories")
                .unwrap_or(&p["assigned_factories"])
                .clone();
        }
        let numeric = number(&target) && number(count);
        let comparison = if p["quantity_mode"] == "additional" {
            "gte"
        } else {
            p["assignment_test"].as_str().unwrap_or("eq")
        };
        let mut satisfied = if numeric {
            json!(if comparison == "eq" {
                count.as_f64() == target.as_f64()
            } else {
                count.as_f64() >= target.as_f64()
            })
        } else {
            Value::Null
        };
        if *basis == "remaining" {
            satisfied = if number(&target) && number(&detail["requested"]) {
                json!(
                    detail["requested"].as_f64().unwrap() > 0.
                        && detail["requested"].as_f64() >= target.as_f64()
                        && !state::truth(&detail["reason"])
                )
            } else {
                Value::Null
            };
        }
        if state::truth(&p["production_priority"]) {
            satisfied = if detail["priority_satisfied"] == false || satisfied == false {
                json!(false)
            } else if detail["priority_satisfied"] != true {
                Value::Null
            } else {
                satisfied
            };
        }
        let reference = &p["design_reference"];
        let mut names = if !a(&selector["variant_names"]).is_empty() {
            vec![
                reference
                    .get("name")
                    .filter(|v| state::truth(v))
                    .unwrap_or(&p["variant_name"])
                    .clone(),
            ]
        } else {
            vec![]
        };
        let id = p
            .get("equipment_id")
            .filter(|v| state::truth(v))
            .unwrap_or(&p["equipment_name"]);
        let mut catalog_rows = equipment
            .iter()
            .copied()
            .filter(|e| e["id"] == *id)
            .collect::<Vec<_>>();
        if catalog_rows.is_empty() {
            let selected = a(&selector["equipment_ids"])
                .iter()
                .map(s)
                .collect::<BTreeSet<_>>();
            let roots = equipment
                .iter()
                .copied()
                .filter(|e| {
                    state::truth(&e["is_archetype"])
                        && a(&e["equipment_ids"])
                            .iter()
                            .map(s)
                            .collect::<BTreeSet<_>>()
                            == selected
                })
                .collect::<Vec<_>>();
            if roots.len() == 1 {
                catalog_rows = roots
            }
        }
        if names.is_empty() {
            names = catalog_rows
                .iter()
                .flat_map(|e| [e["localized_name"].clone(), e["name"].clone()])
                .filter(state::truth)
                .collect();
        }
        names = unique(names.into_iter());
        if names.is_empty() {
            names = unique(std::iter::once(
                p.get("variant_name")
                    .filter(|v| state::truth(v))
                    .or_else(|| p.get("equipment_name").filter(|v| state::truth(v)))
                    .unwrap_or(&p["equipment_id"])
                    .clone(),
            ));
        }
        if names.is_empty() {
            continue;
        }
        let controls = a(&selector["equipment_ids"])
            .iter()
            .map(|id| {
                equipment
                    .iter()
                    .find(|e| e["id"] == *id)
                    .map(|e| add_control(&e["production_ui_category"]))
                    .unwrap_or(Value::Null)
            })
            .collect::<Vec<_>>();
        let control = controls
            .first()
            .filter(|first| !first.is_null() && controls.iter().all(|c| c == *first))
            .cloned()
            .unwrap_or(Value::Null);
        let known = detail["requested"].is_i64() && detail["assigned"].is_i64();
        let missing = if known {
            json!(!state::truth(&detail["lines"]))
        } else {
            Value::Null
        };
        let mut proposal = json!({"key":format!("{}{suffix}",s(&row["key"])),"guide_step":row["key"],"panel":"production","identity":p.get("variant_name").filter(|v|state::truth(v)).or_else(||p.get("equipment_id").filter(|v|state::truth(v))).unwrap_or(&p["equipment_name"]),"title":names[0],"labels":names,"status":item["status"],"country":country,"quantity":target,"quantity_mode":p.get("quantity_mode").unwrap_or(&json!("total")),"production_basis":basis,"requested":detail["requested"],"assigned":detail["assigned"],"goal_satisfied":satisfied,"production_missing":missing,"production_add_control":control,"production_priority":p.get("production_priority").filter(|v|state::truth(v)).cloned().unwrap_or_else(||if p["quantity_mode"]=="remaining"{json!("last")}else{Value::Null}),"production_selector":selector,"production_line_ids":a(&detail["lines"]).iter().filter_map(|l|l["id"].as_str()).collect::<Vec<_>>()});
        let check = &detail["design_check"];
        if state::truth(check) {
            proposal["production_design_check"] = check.clone();
            if ["different", "unknown"].contains(&s(&check["status"]))
                && a(&check["candidates"]).len() == 1
            {
                let candidate = &check["candidates"][0];
                proposal["production_design_unverified"] = json!(true);
                proposal["goal_satisfied"] = Value::Null;
                proposal["production_candidate_line_ids"] = json!([candidate["line_id"]]);
                proposal["production_candidate_name"] = candidate["name"].clone();
            }
        }
        if selector.get("design").is_none()
            && !a(&selector["variant_names"]).is_empty()
            && missing == true
            && !live.is_null()
        {
            let normalized = state::normalize(live);
            let observed = &normalized["facts"]["production.lines"];
            if normalized["usable"] == true
                && normalized["facts"]["country"]["value"] == *country
                && ["observed", "calculated"].contains(&s(&observed["status"]))
                && state::truth(&observed["source"])
            {
                let candidates = a(&observed["value"])
                    .iter()
                    .filter(|line| {
                        a(&selector["equipment_ids"]).contains(&line["equipment"])
                            && (selector["category"].is_null()
                                || selector["category"] == line["category"])
                    })
                    .collect::<Vec<_>>();
                if candidates.len() == 1 {
                    let candidate = candidates[0];
                    proposal["production_design_unverified"] = json!(true);
                    proposal["goal_satisfied"] = Value::Null;
                    proposal["production_candidate_line_ids"] = json!([candidate["id"]]);
                    proposal["production_candidate_name"] = candidate["name"].clone();
                }
            }
        }
        result.push(proposal);
    }
    result
}
