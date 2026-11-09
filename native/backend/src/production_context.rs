//! Qualified production selectors, complete recipe matching and run-relative
//! assignments. No display name is used when a complete recipe is declared.
use crate::{domain_context::metadata_equal, state::unknown};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
const MAX: i64 = 9007199254740991;
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn integer(v: &Value, min: i64) -> bool {
    v.as_i64().is_some_and(|v| (min..=MAX).contains(&v))
}
fn id(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        !s.is_empty()
            && s.len() <= 160
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    })
}
fn steps(guide: &Value) -> Vec<Value> {
    let mut result = vec![];
    for rows in guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
    {
        for step in rows.as_array().into_iter().flatten() {
            if let Some(targets) = step["action"]["params"]["production_targets"].as_array() {
                for (i, target) in targets.iter().enumerate() {
                    if target.is_object() {
                        result.push(json!({"key":format!("{}.target-{}",text(&step["key"]),i+1),"layout_key":step["key"],"action":{"kind":"manual","params":target}}));
                    }
                }
            } else {
                result.push(step.clone())
            }
        }
    }
    result
}
pub fn configuration(value: &Value) -> Option<Value> {
    let modules = value["modules"].as_object()?;
    let upgrades = value["upgrades"].as_object()?;
    if modules.len() > 256 || upgrades.len() > 256 {
        return None;
    }
    let mut result = json!({"modules":{},"upgrades":{}});
    for (key, module) in modules {
        if !id(&json!(key)) || !id(module) {
            return None;
        }
        if module != "empty" {
            result["modules"][key] = module.clone()
        }
    }
    for (key, level) in upgrades {
        if !id(&json!(key)) {
            return None;
        }
        let number = if let Some(s) = level.as_str() {
            if s.is_empty()
                || s.len() > 5
                || !s.bytes().all(|b| b.is_ascii_digit())
                || s.len() > 1 && s.starts_with('0')
            {
                return None;
            }
            s.parse::<i64>().ok()? as f64
        } else {
            level.as_f64()?
        };
        if !number.is_finite() || !(0. ..=10000.).contains(&number) || number.fract() != 0. {
            return None;
        }
        if number != 0. {
            result["upgrades"][key] = json!(number as i64)
        }
    }
    Some(result)
}
pub fn valid_live_design(value: &Value) -> bool {
    if !value.is_object() {
        return false;
    }
    if value["status"] == "unavailable" {
        return value.get("modules").is_none() && value.get("upgrades").is_none();
    }
    value["status"] == "observed"
        && value["upgrades"]
            .as_object()
            .is_some_and(|m| m.values().all(Value::is_number))
        && configuration(value).is_some()
}
pub(crate) fn compare(expected: &Value, observed: &Value) -> Value {
    let wanted = configuration(expected);
    let actual = configuration(observed);
    if wanted.is_none() || !valid_live_design(observed) || observed["status"] != "observed" {
        return json!({"status":"unknown","differences":[]});
    }
    let wanted = wanted.unwrap();
    let actual = actual.unwrap();
    let mut differences = vec![];
    for kind in ["modules", "upgrades"] {
        let default = if kind == "modules" {
            Value::Null
        } else {
            json!(0)
        };
        let keys = wanted[kind]
            .as_object()
            .unwrap()
            .keys()
            .chain(actual[kind].as_object().unwrap().keys())
            .collect::<BTreeSet<_>>();
        for key in keys {
            let before = wanted[kind].get(key).unwrap_or(&default);
            let after = actual[kind].get(key).unwrap_or(&default);
            if before != after {
                differences.push(json!({"kind":kind,"key":key,"expected":before,"actual":after}));
            }
        }
    }
    json!({"status":if differences.is_empty(){"match"}else{"different"},"differences":differences})
}
pub fn selector_for(step: &Value) -> Option<Value> {
    let p = &step["action"]["params"];
    let value = &p["production_selector"];
    let keys = value
        .as_object()?
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if keys != BTreeSet::from(["equipment_ids", "variant_names", "category"])
        || !value["category"].is_null()
            && !["military", "naval"].contains(&text(&value["category"]))
    {
        return None;
    }
    let mut result = json!({"category":value["category"]});
    for (key, limit) in [("equipment_ids", 512), ("variant_names", 128)] {
        let values = value[key].as_array()?;
        if values.len() > limit
            || key == "equipment_ids" && values.is_empty()
            || values.iter().any(|v| v.as_str().is_none_or(str::is_empty))
        {
            return None;
        }
        result[key] = json!(
            values
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<BTreeSet<_>>()
        );
    }
    if p.get("design_reference").is_some() {
        result["design"] = configuration(&p["design_reference"])?
    }
    Some(result)
}
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), canonical(v)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        v => v.clone(),
    }
}
pub fn selector_identity(selector: &Value) -> String {
    let mut value = selector.clone();
    if selector.get("design").is_some() {
        value.as_object_mut().unwrap().remove("variant_names");
    }
    canonical(&value).to_string()
}
pub fn relative_spec(step: &Value) -> Option<Value> {
    let params = &step["action"]["params"];
    let selector = selector_for(step)?;
    let requested = params.get("requested_factories").is_some();
    let quantity = &params[if requested {
        "requested_factories"
    } else {
        "assigned_factories"
    }];
    if params["category"] != "production"
        || params["quantity_mode"] != "additional"
        || !integer(quantity, 1)
        || !metadata_equal(
            &step["completion"],
            &json!({"op":"fact","key":format!("guide.production.{}.added",text(&step["key"])),"test":"gte","value":quantity}),
        )
    {
        return None;
    }
    Some(
        json!({"fact_key":format!("{}:{}",if requested{"production_requested"}else{"production"},canonical(&selector)),"quantity":quantity}),
    )
}
#[derive(Clone)]
struct Matches {
    indices: Vec<usize>,
    complete: bool,
    check: Option<Value>,
}
fn matching(selector: Option<&Value>, lines: &[Value], known: bool) -> Matches {
    let empty = Matches {
        indices: vec![],
        complete: true,
        check: None,
    };
    let Some(selector) = selector.filter(|_| known) else {
        return empty;
    };
    let candidates = lines
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            selector["equipment_ids"]
                .as_array()
                .unwrap()
                .contains(&r["equipment"])
                && (selector["category"].is_null() || selector["category"] == r["category"])
        })
        .collect::<Vec<_>>();
    let Some(design) = selector.get("design") else {
        let names = selector["variant_names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| hoi4companion_catalog_core::production_name(text(v)))
            .collect::<HashSet<_>>();
        return Matches {
            indices: candidates
                .into_iter()
                .filter(|(_, r)| {
                    names.is_empty()
                        || names.contains(&hoi4companion_catalog_core::production_name(text(
                            &r["name"],
                        )))
                })
                .map(|(i, _)| i)
                .collect(),
            ..empty
        };
    };
    let mut comparisons = vec![];
    let mut complete = true;
    let mut indices = vec![];
    for (index, row) in candidates {
        let mut result = compare(design, &row["design"]);
        if result["status"] == "unknown" {
            complete = false
        }
        if result["status"] == "match" {
            indices.push(index)
        }
        result["line_id"] = row["id"].clone();
        result["name"] = row["name"].clone();
        comparisons.push(result);
    }
    let status = if !complete {
        "unknown"
    } else if !indices.is_empty() {
        "match"
    } else if !comparisons.is_empty() {
        "different"
    } else {
        "missing"
    };
    Matches {
        indices,
        complete,
        check: Some(json!({"status":status,"candidates":comparisons})),
    }
}
fn project(guide: &Value, state: &Value, baselines: &Value) -> (Value, usize) {
    let mut state = state.clone();
    let mut details = json!({});
    let fact = state["facts"]["production.lines"].clone();
    let known_lines = state["usable"] == true
        && ["observed", "calculated"].contains(&text(&fact["status"]))
        && fact["source"].as_str().is_some_and(|s| !s.is_empty())
        && crate::state::valid_lines(&fact["value"]);
    let lines = fact["value"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let country = if state["usable"] == true {
        state["facts"]["country"]["value"].clone()
    } else {
        Value::Null
    };
    let rows = steps(guide);
    let military = if known_lines {
        lines
            .iter()
            .enumerate()
            .filter(|(_, r)| r["category"] == "military")
            .map(|(i, _)| i)
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    let selectors = rows
        .iter()
        .filter(|s| s["action"]["params"]["category"] == "production")
        .map(|s| (text(&s["key"]).to_owned(), selector_for(s)))
        .collect::<HashMap<_, _>>();
    let mut peers = HashMap::<(String, String), Vec<Option<Value>>>::new();
    let mut comparisons = HashMap::<String, Matches>::new();
    for step in &rows {
        let priority = text(&step["action"]["params"]["production_priority"]);
        if !priority.is_empty() {
            let layout = step.get("layout_key").unwrap_or(&step["key"]);
            peers
                .entry((text(layout).into(), priority.into()))
                .or_default()
                .push(selectors.get(text(&step["key"])).cloned().flatten());
        }
    }
    let mut comparison = |selector: Option<&Value>| {
        let Some(selector) = selector.filter(|_| known_lines) else {
            return Matches {
                indices: vec![],
                complete: true,
                check: None,
            };
        };
        let key = selector_identity(selector);
        comparisons
            .entry(key)
            .or_insert_with(|| matching(Some(selector), lines, known_lines))
            .clone()
    };
    let mut priority_allowed =
        HashMap::<(String, String), (HashSet<usize>, HashSet<String>)>::new();
    for step in &rows {
        let params = &step["action"]["params"];
        if params["category"] != "production" || params.get("production_selector").is_none() {
            continue;
        }
        let step_key = text(&step["key"]);
        let key = format!("guide.production.{step_key}");
        let selector = selectors.get(step_key).and_then(Option::as_ref);
        let matched = comparison(selector);
        let mut metrics = json!({});
        for (label, field) in [
            ("assigned", "assigned_factories"),
            ("requested", "requested_factories"),
            ("active", "active_factories"),
            ("damaged", "damaged_factories"),
        ] {
            let value = if known_lines && selector.is_some() && matched.complete {
                Some(
                    matched
                        .indices
                        .iter()
                        .map(|i| lines[*i][field].as_f64().unwrap_or(0.) as u128)
                        .sum::<u128>(),
                )
            } else {
                None
            };
            metrics[label] = value
                .filter(|n| *n <= MAX as u128)
                .map(|v| json!(v as i64))
                .unwrap_or(Value::Null);
        }
        // One overflowing aggregate invalidates every aggregate, as in the
        // legacy adapter; no prefix or partial total can qualify an order.
        if known_lines
            && selector.is_some()
            && matched.complete
            && [
                "assigned_factories",
                "requested_factories",
                "active_factories",
                "damaged_factories",
            ]
            .iter()
            .any(|field| {
                matched
                    .indices
                    .iter()
                    .map(|i| lines[*i][field].as_f64().unwrap_or(0.) as u128)
                    .sum::<u128>()
                    > MAX as u128
            })
        {
            for value in metrics.as_object_mut().unwrap().values_mut() {
                *value = Value::Null;
            }
        }
        let assigned = metrics["assigned"].as_i64();
        let requested = metrics["requested"].as_i64();
        let requested_basis = params.get("requested_factories").is_some();
        let count = if requested_basis { requested } else { assigned };
        let raw_target = &params[if requested_basis {
            "requested_factories"
        } else {
            "assigned_factories"
        }];
        let mut target = if integer(raw_target, 0) {
            raw_target.as_i64()
        } else {
            None
        };
        let spec = relative_spec(step);
        let baseline = spec.as_ref().map(|_| &baselines[step_key]);
        let valid = baseline.zip(spec.as_ref()).is_some_and(|(base, spec)| {
            base.is_object()
                && base["country"] == country
                && base["fact_key"] == spec["fact_key"]
                && base["quantity"] == spec["quantity"]
                && base["value"]
                    .as_i64()
                    .zip(spec["quantity"].as_i64())
                    .is_some_and(|(a, b)| a >= 0 && a.checked_add(b).is_some_and(|n| n <= MAX))
        });
        let initial = if valid {
            baseline.and_then(|v| v["value"].as_i64())
        } else {
            None
        };
        let relative = params["quantity_mode"] == "additional";
        if relative {
            target = if valid {
                initial
                    .zip(spec.as_ref().and_then(|v| v["quantity"].as_i64()))
                    .map(|(a, b)| a + b)
            } else {
                None
            }
        }
        let remaining = params["quantity_mode"] == "remaining";
        let mut allocated = None;
        let mut reason = "";
        if remaining {
            let total = &state["facts"]["country.production.effective_factories.military"];
            let total = if state["usable"] == true
                && ["observed", "calculated"].contains(&text(&total["status"]))
                && total["source"].as_str().is_some_and(|s| !s.is_empty())
                && integer(&total["value"], 0)
            {
                total["value"].as_i64()
            } else {
                None
            };
            target = None;
            if known_lines
                && matched.complete
                && selector.is_some_and(|v| v["category"] == "military")
                && let Some(total) = total
            {
                let others = military
                    .iter()
                    .filter(|i| !matched.indices.contains(i))
                    .map(|i| lines[*i]["requested_factories"].as_f64().unwrap_or(0.) as u128)
                    .sum::<u128>();
                if others <= MAX as u128 {
                    let desired = (total - others as i64).max(0);
                    target = Some(desired);
                    allocated = requested.map(|requested| {
                        matched.indices.len() == 1
                            && matched.indices.first() == military.last()
                            && requested > 0
                            && requested >= desired
                    });
                }
            }
            reason = if target.is_none() {
                "remaining_capacity_unknown"
            } else if matched.indices.len() > 1 {
                "remaining_line_ambiguous"
            } else if matched.indices.is_empty() {
                "remaining_line_missing"
            } else if matched.indices.first() != military.last() {
                "remaining_line_not_last"
            } else {
                ""
            };
        }
        let started = match (relative, valid, assigned, requested, initial) {
            (true, true, Some(assigned), Some(requested), Some(initial)) => {
                Some(requested > initial || assigned > initial)
            }
            (false, _, Some(assigned), Some(requested), _) => Some(requested > 0 || assigned > 0),
            _ => None,
        };
        let priority = text(&params["production_priority"]);
        let mut priority_value = None;
        if !priority.is_empty()
            && known_lines
            && matched.complete
            && selector.is_some_and(|v| v["category"] == "military")
        {
            let group = (
                text(step.get("layout_key").unwrap_or(&step["key"])).to_owned(),
                priority.to_owned(),
            );
            if !priority_allowed.contains_key(&group) {
                let mut allowed = HashSet::new();
                let mut uncertain = HashSet::new();
                for peer in &peers[&group] {
                    let result = comparison(peer.as_ref());
                    allowed.extend(result.indices);
                    if !result.complete
                        && let Some(check) = result.check
                    {
                        for row in check["candidates"].as_array().into_iter().flatten() {
                            if row["status"] == "unknown" {
                                uncertain.insert(text(&row["line_id"]).to_owned());
                            }
                        }
                    }
                }
                priority_allowed.insert(group.clone(), (allowed, uncertain));
            }
            let (allowed, uncertain) = &priority_allowed[&group];
            priority_value = Some(false);
            if matched.indices.len() == 1
                && let Some(position) = military.iter().position(|i| *i == matched.indices[0])
            {
                let surrounding = if priority == "first" {
                    &military[..position]
                } else {
                    &military[position + 1..]
                };
                let mut unknown = false;
                let mut wrong = false;
                for index in surrounding {
                    if allowed.contains(index) {
                        continue;
                    }
                    if uncertain.contains(text(&lines[*index]["id"])) {
                        unknown = true
                    } else {
                        wrong = true
                    }
                }
                priority_value = if wrong {
                    Some(false)
                } else if unknown {
                    None
                } else {
                    Some(true)
                };
            }
        }
        if !priority.is_empty() {
            state["facts"][format!("{key}.priority_satisfied")] = priority_value
                .map(|v| crate::state::known(json!(v), "calculated", "production_adapter"))
                .unwrap_or_else(|| unknown("Production priority unavailable"));
        }
        for (suffix, value) in [
            ("assigned", assigned.map(|v| json!(v))),
            ("requested", requested.map(|v| json!(v))),
            ("added", count.zip(initial).map(|(c, i)| json!(c - i))),
            ("started", started.map(|v| json!(v))),
            ("remaining_allocated", allocated.map(|v| json!(v))),
        ] {
            state["facts"][format!("{key}.{suffix}")] = value
                .map(|v| crate::state::known(v, "calculated", "production_adapter"))
                .unwrap_or_else(|| unknown("Produzione o conteggio iniziale non verificabile"));
        }
        let mut detail = metrics;
        detail["baseline"] = json!(initial);
        detail["target"] = json!(target);
        detail["lines"] = json!(if assigned.is_some() {
            matched
                .indices
                .iter()
                .map(|i| lines[*i].clone())
                .collect::<Vec<_>>()
        } else {
            vec![]
        });
        detail["basis"] = json!(if remaining {
            "remaining"
        } else if requested_basis {
            "requested"
        } else {
            "assigned"
        });
        detail["reason"] = json!(if remaining {
            reason
        } else if assigned.is_none() {
            "production_unavailable"
        } else if relative && !valid {
            "baseline_missing"
        } else {
            ""
        });
        if !priority.is_empty() {
            detail["priority"] = json!(priority);
            detail["priority_satisfied"] = json!(priority_value)
        }
        if let Some(check) = matched.check {
            detail["design_check"] = check
        }
        details[step_key] = detail;
    }
    for rows in guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
    {
        for step in rows.as_array().into_iter().flatten() {
            if let Some(targets) = step["action"]["params"]["production_targets"].as_array() {
                let key = text(&step["key"]);
                let layout = (0..targets.len())
                    .map(|i| {
                        details
                            .get(format!("{key}.target-{}", i + 1))
                            .cloned()
                            .unwrap_or_else(|| json!({}))
                    })
                    .collect::<Vec<_>>();
                details[key] = json!({"layout":layout});
            }
        }
    }
    (json!([state, details]), comparisons.len())
}
pub fn context(guide: &Value, state: &Value, baselines: &Value) -> Value {
    project(guide, state, baselines).0
}
pub fn execute(kind: &str, args: &Value) -> Result<Value, String> {
    Ok(match kind {
        "production" => context(&args["guide"], &args["state"], &args["baselines"]),
        "production_stats" => {
            let (projection, comparisons) =
                project(&args["guide"], &args["state"], &args["baselines"]);
            json!({"projection":projection,"distinct_product_comparisons":comparisons})
        }
        "selector" => selector_for(&args["step"]).unwrap_or(Value::Null),
        "relative" => relative_spec(&args["step"]).unwrap_or(Value::Null),
        "identity" => json!(selector_identity(&args["selector"])),
        "configuration" => configuration(&args["value"]).unwrap_or(Value::Null),
        "compare" => compare(&args["expected"], &args["observed"]),
        "name" => json!(hoi4companion_catalog_core::production_name(text(
            &args["value"]
        ))),
        _ => return Err("Unknown production projection".into()),
    })
}
