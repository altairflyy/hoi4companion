//! Run-scoped evidence projections. No acquisition, persistence or step scheduling.
use crate::state::{known, unknown};
use serde_json::{Value, json};
use std::collections::BTreeSet;
type Result<T> = std::result::Result<T, String>;
const MAX: i64 = 9007199254740991;
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn integer(v: &Value, min: i64, max: i64) -> bool {
    v.as_i64().is_some_and(|n| (min..=max).contains(&n))
}
fn nonempty(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty())
}
fn tag(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 3
            && s.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    })
}
fn steps(guide: &Value) -> Vec<&Value> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|v| v.as_array().into_iter().flatten())
        .collect()
}
fn fact_value<'a>(state: &'a Value, key: &str, observed: bool) -> Option<&'a Value> {
    let fact = &state["facts"][key];
    if state["usable"] == true
        && nonempty(&fact["source"])
        && if observed {
            fact["status"] == "observed"
        } else {
            ["observed", "calculated"].contains(&text(&fact["status"]))
        }
    {
        fact.get("value")
    } else {
        None
    }
}
pub fn validate_observations(value: &Value) -> Result<()> {
    let Some(map) = value.as_object() else {
        return Err("Osservazioni campagna non valide".into());
    };
    if map.is_empty() {
        return Ok(());
    }
    if map
        .keys()
        .any(|k| !["country", "war_seen", "milestones"].contains(&k.as_str()))
        || !tag(&value["country"])
        || map.get("war_seen").is_some_and(|v| *v != true)
        || !map.contains_key("war_seen")
            && value["milestones"].as_object().is_none_or(|m| m.is_empty())
        || map.get("milestones").is_some_and(|v| {
            v.as_object().is_none_or(|m| {
                m.is_empty()
                    || m.len() > 4096
                    || m.iter()
                        .any(|(k, v)| k.is_empty() || k.chars().count() > 2048 || *v != true)
            })
        })
    {
        return Err("Osservazioni campagna non valide".into());
    }
    Ok(())
}
pub fn campaign_context(state: &Value, observations: &Value) -> Result<Value> {
    let mut state = state.clone();
    state["facts"]["guide.campaign.after_war"] =
        unknown("Guerra o avanzamento campagna non qualificati");
    if observations.is_null() {
        return Ok(state);
    }
    validate_observations(observations)?;
    let country = fact_value(&state, "country", false).cloned();
    if country.is_some()
        && country.as_ref() == observations.get("country")
        && observations["milestones"]
            .as_object()
            .is_some_and(|m| !m.is_empty())
    {
        state["facts"]["guide.campaign.milestones"] = known(
            json!(
                observations["milestones"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .collect::<BTreeSet<_>>()
            ),
            "calculated",
            "campaign_milestone_adapter",
        );
    }
    if let (Some(country), Some(war)) = (
        country,
        fact_value(&state, "country.diplomacy.at_war", true).cloned(),
    ) && tag(&country)
        && war.is_boolean()
    {
        let seen = observations["country"] == country && observations["war_seen"] == true;
        state["facts"]["guide.campaign.after_war"] = known(
            json!(war == false && seen),
            "calculated",
            "campaign_condition_adapter",
        );
    }
    Ok(state)
}
pub fn validate_baselines(value: &Value) -> Result<()> {
    let Some(rows) = value.as_object() else {
        return Err("Conteggi iniziali della partita non validi".into());
    };
    if rows.len() > 4096 {
        return Err("Conteggi iniziali della partita non validi".into());
    }
    for (key, row) in rows {
        if key.is_empty()
            || row.as_object().is_none_or(|m| {
                m.keys().map(String::as_str).collect::<BTreeSet<_>>()
                    != BTreeSet::from(["fact_key", "value", "country", "quantity", "source"])
            })
            || ["fact_key", "country", "source"]
                .iter()
                .any(|k| !nonempty(&row[k]))
            || !integer(&row["value"], 0, MAX)
            || !integer(&row["quantity"], 1, MAX)
        {
            return Err("Conteggio iniziale della partita non valido".into());
        }
    }
    Ok(())
}
// Python's metadata dictionaries accept numerically equal JSON scalars. This
// helper is only for recognizing authored binding declarations, not fact tests.
pub(crate) fn metadata_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| metadata_equal(v, w)))
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| metadata_equal(a, b))
        }
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}
pub fn construction_spec(step: &Value) -> Option<Value> {
    let p = &step["action"]["params"];
    let building = text(&p["building_id"]);
    let provincial =
        (["bunker", "coastal_bunker"].contains(&building) || p["building_scope"] == "province") && integer(&p["province_id"], 1, MAX);
    let field = if provincial {
        if ["bunker", "coastal_bunker"].contains(&building) { building.to_owned() }
        else if !building.is_empty() && building.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {format!("buildings.{building}")}
        else { return None; }
    } else {
        match building {
            "infrastructure" => "infrastructure".into(),
            "arms_factory" => "military_factories".into(),
            "industrial_complex" => "civilian_factories".into(),
            "dockyard" => "dockyards".into(),
            id if p["building_scope"] == "state" && !id.is_empty()
                && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') => format!("buildings.{id}"),
            _ => return None,
        }
    };
    if !["manual", "special_project"].contains(&text(&step["action"]["kind"]))
        || p["category"] != "construction"
        || p["quantity_mode"] != "additional"
        || !provincial && !integer(&p["state_id"], 1, MAX)
        || !integer(&p["quantity"], 1, MAX)
    {
        return None;
    }
    let key = format!("guide.construction.{}.added", text(&step["key"]));
    if !metadata_equal(
        &step["completion"],
        &json!({"op":"fact","key":key,"test":"gte","value":p["quantity"]}),
    ) {
        return None;
    }
    let mut spec = json!({"quantity":p["quantity"],"derived_key":key});
    if provincial {
        let id = p["province_id"].as_i64()?;
        spec["fact_key"] = json!(format!("country.fortifications.items.{id}.{field}"));
        spec["queue_key"] = json!(format!("province:{id}:{building}"));
        spec["province_id"] = p["province_id"].clone();
        spec["building_id"] = p["building_id"].clone();
    } else {
        let id = p["state_id"].as_i64()?;
        spec["fact_key"] = json!(format!("country.states.items.{id}.{field}"));
        spec["queue_key"] = json!(format!("state:{id}:{building}"));
    }
    Some(spec)
}
pub fn construction_context(guide: &Value, state: &Value, baselines: &Value) -> Result<Value> {
    let mut state = state.clone();
    let mut details = json!({});
    let country = if state["usable"] == true {
        state["facts"]["country"]["value"].clone()
    } else {
        Value::Null
    };
    for step in steps(guide) {
        let Some(spec) = construction_spec(step) else {
            continue;
        };
        let key = text(&step["key"]);
        let baseline = &baselines[key];
        let current = fact_value(&state, text(&spec["fact_key"]), true)
            .filter(|v| integer(v, 0, MAX))
            .cloned();
        let valid = baseline.is_object()
            && baseline["country"] == country
            && baseline["fact_key"] == spec["fact_key"]
            && baseline["quantity"] == spec["quantity"]
            && baseline["value"]
                .as_i64()
                .zip(spec["quantity"].as_i64())
                .is_some_and(|(a, b)| a >= 0 && a.checked_add(b).is_some_and(|n| n <= MAX));
        let initial = if valid {
            baseline["value"].as_i64()
        } else {
            None
        };
        let target = initial.zip(spec["quantity"].as_i64()).map(|(a, b)| a + b);
        state["facts"][text(&spec["derived_key"])] =
            match initial.zip(current.as_ref().and_then(Value::as_i64)) {
                Some((a, b)) => known(json!(b - a), "calculated", &format!("run_baseline:{key}")),
                None => unknown("Conteggio iniziale non verificabile"),
            };
        details[key] = if spec.get("province_id").is_some() {
            json!({"target_level":target,"province_ids":[spec["province_id"]],"completed_provinces":current.as_ref().and_then(Value::as_i64).zip(target).map(|(c,t)|i32::from(c>=t)),"total_provinces":1,"building_id":spec["building_id"]})
        } else {
            json!({"baseline":initial,"current":current,"target":target})
        };
    }
    let fortified = fortification_context(guide, &state);
    state = fortified[0].clone();
    if let Some(rows) = fortified[1].as_object() {
        for (key, value) in rows {
            details[key] = value.clone()
        }
    }
    Ok(json!([state, details]))
}
pub fn fortification_context(guide: &Value, state: &Value) -> Value {
    let mut state = state.clone();
    let mut details = json!({});
    for step in steps(guide) {
        let p = &step["action"]["params"];
        let key = text(&step["key"]);
        let prefix = format!("guide.fortifications.{key}");
        let building = text(&p["building_id"]);
        if !["bunker", "coastal_bunker"].contains(&building)
            || !metadata_equal(
                &step["completion"],
                &json!({"op":"fact","key":format!("{prefix}.completed"),"test":"eq","value":true}),
            )
        {
            continue;
        }
        let selector = &p["province_selector"];
        let provinces = if selector["kind"] == "border" {
            fact_value(
                &state,
                &format!("fortifications.borders.{}", text(&selector["country"])),
                false,
            )
            .cloned()
            .unwrap_or(Value::Null)
        } else {
            p.get("province_ids").cloned().unwrap_or_else(|| {
                p.get("province_id")
                    .map(|id| json!([id]))
                    .unwrap_or(Value::Null)
            })
        };
        let target = p["target_level"].clone();
        let valid = integer(&target, 1, 1000)
            && provinces.as_array().is_some_and(|a| {
                !a.is_empty()
                    && a.len() <= 512
                    && a.iter().all(|v| integer(v, 1, 100000))
                    && a.iter()
                        .map(Value::to_string)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == a.len()
            });
        let values = if valid {
            provinces
                .as_array()
                .unwrap()
                .iter()
                .map(|n| {
                    fact_value(
                        &state,
                        &format!(
                            "country.fortifications.items.{}.{building}",
                            n.as_i64().unwrap()
                        ),
                        true,
                    )
                    .cloned()
                    .unwrap_or(Value::Null)
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let observed = valid && values.iter().all(|v| integer(v, 0, 1000));
        let done = if observed {
            Some(values.iter().all(|n| n.as_i64() >= target.as_i64()))
        } else {
            None
        };
        let queued = fact_value(&state, "construction.queued", false);
        let queue_known =
            queued.is_some_and(|v| v.as_array().is_some_and(|a| a.iter().all(Value::is_string)));
        let started = if valid && queue_known {
            Some(
                done == Some(true)
                    || provinces.as_array().unwrap().iter().any(|n| {
                        queued.unwrap().as_array().unwrap().contains(&json!(format!(
                            "province:{}:{building}",
                            n.as_i64().unwrap()
                        )))
                    }),
            )
        } else if done == Some(true) {
            Some(true)
        } else {
            None
        };
        for (suffix, value) in [("started", started), ("completed", done)] {
            state["facts"][format!("{prefix}.{suffix}")] = value
                .map(|v| known(json!(v), "calculated", &format!("fortification_goal:{key}")))
                .unwrap_or_else(|| unknown("Provincial fortification evidence unavailable"));
        }
        details[key] = json!({"target_level":if integer(&target,1,1000){target.clone()}else{Value::Null},"province_ids":if valid{provinces.clone()}else{json!([])},"completed_provinces":if observed{Some(values.iter().filter(|n|n.as_i64()>=target.as_i64()).count())}else{None},"total_provinces":if valid{provinces.as_array().map(Vec::len)}else{None},"building_id":building});
    }
    json!([state, details])
}
pub fn execute(kind: &str, args: &Value) -> Result<Value> {
    match kind {
        "campaign" => campaign_context(&args["state"], &args["observations"]),
        "validate_campaign" => {
            validate_observations(&args["observations"])?;
            Ok(args["observations"].clone())
        }
        "validate_baselines" => {
            validate_baselines(&args["baselines"])?;
            Ok(args["baselines"].clone())
        }
        "construction_spec" => Ok(construction_spec(&args["step"]).unwrap_or(Value::Null)),
        "construction" => construction_context(&args["guide"], &args["state"], &args["baselines"]),
        "fortification" => Ok(fortification_context(&args["guide"], &args["state"])),
        _ => Err("Unknown evidence projection".into()),
    }
}
