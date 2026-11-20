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
    // One instruction may require several different buildings or locations.
    // Bind every declared target independently and require all completions;
    // never drop an unresolved target and accidentally complete the rest.
    if params["category"] == "construction" && params["construction_targets"].is_array() {
        let targets = params["construction_targets"].as_array().unwrap();
        let mut started = Vec::new();
        let mut completed = Vec::new();
        let mut valid = !targets.is_empty() && targets.len() <= 64;
        for (i, target) in targets.iter().enumerate() {
            if !target.is_object() || target.get("construction_targets").is_some()
                || target["quantity_mode"] == "additional" {
                valid = false;
                continue;
            }
            let mut child = json!({"key":format!("{}.target-{}",s(&step["key"]),i+1),"action":{"kind":"manual","params":target}});
            child["action"]["params"]["category"] = json!("construction");
            let result = bind(&child, index, geography)?;
            let mut queued = result["step"]["started_when"].clone();
            // One queue line the reader cannot locate (another owner, a
            // national building) leaves the whole queue unknown; a line it did
            // locate is still certainly queued.
            if queued["op"] == "fact" && queued["key"] == "construction.queued" && queued["test"] == "contains" {
                queued = json!({"op":"any","args":[fact("construction.queued_known","contains",queued["value"].clone()),queued]});
            }
            let built = result["step"]["completion"].clone();
            if params["completion_mode"] == "queued_or_built" && !queued.is_null() {
                // A target without an observable built level (a "maximum" whose
                // effective cap the catalog cannot qualify) is satisfied while
                // queued or by the player's confirmation of this step. Queued
                // alone would revert once the building finishes, and the step
                // could never complete again.
                let done = if built.is_null() {
                    json!({"op":"check","key":s(&step["key"])})
                } else {
                    built
                };
                completed.push(json!({"op":"any","args":[done,queued.clone()]}));
                started.push(queued);
                continue;
            }
            if result["handled"] != true || result["step"]["completion"].is_null() {
                valid = false;
                completed.push(fact(&format!("guide.construction.{}.target-{}.unqualified",s(&step["key"]),i+1), "eq", json!(true)));
            } else {
                completed.push(result["step"]["completion"].clone());
            }
            if !result["step"]["started_when"].is_null() {
                started.push(result["step"]["started_when"].clone());
            }
        }
        if !started.is_empty() {
            attach(&mut step, "started_when", json!({"op":"any","args":started}));
        }
        if !targets.is_empty() && completed.len() == targets.len() {
            attach(&mut step, "completion", json!({"op":"all","args":completed}));
        }
        return Ok(json!({"step":step,"handled":true,"reason":if valid {"compound_construction_targets"}else{"compound_construction_unresolved"},"location":null}));
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
        let scoped = building.as_ref().is_some_and(|b| {
            matches!(s(&b["location_scope"]), "state" | "province")
                || b["location_scope"].is_null()
                    && ["infrastructure", "industrial_complex", "arms_factory", "dockyard"]
                        .contains(&s(&b["id"]))
        });
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
        } else if building.is_some() && !scoped && region.is_some() {
            reason = "building_scope_unknown".into();
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
            && scoped
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
            // Facilities with a catalog-qualified single provincial level have
            // an unambiguous maximum of one. Shared-slot and other variable
            // maxima cannot be inferred from static definitions alone.
            if target.is_null()
                && params["quantity"].is_null()
                && params["quantity_mode"].is_null()
                && provincial
                && building["max_level"] == 1
                && building["specializations"].as_array().is_some_and(|a| !a.is_empty())
            {
                target = json!(1);
                params["target_level"] = target.clone();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_scope_never_creates_a_state_specific_automatic_condition() {
        let entries = json!([
            {"kind":"building","id":"mod_structure","name":"Mod Structure"},
            {"kind":"state","id":"42","name":"Test State"}
        ]);
        let index = Index::new(&json!({"entries":entries})).unwrap();
        let geography = Geography::new(index.rows());
        let step = json!({"key":"build","action":{"kind":"manual","params":{
            "category":"construction","building_id":"mod_structure","state_id":42,
            "quantity_mode":"target_total","quantity":1
        }}});
        let result = bind(&step, &index, &geography).unwrap();
        assert_eq!(result["reason"], "building_scope_unknown");
        assert!(result["step"].get("started_when").is_none());
        assert!(result["step"].get("completion").is_none());
    }

    #[test]
    fn single_level_facility_defaults_to_one_only_when_location_is_qualified() {
        let entries = json!([
            {"kind":"building","id":"land_facility","name":"Land Facility","location_scope":"province","max_level":1,"specializations":["specialization_land"]},
            {"kind":"state","id":"42","name":"Test State","geography":{"province_ids":[10]}}
        ]);
        let index = Index::new(&json!({"entries":entries})).unwrap();
        let geography = Geography::new(index.rows());
        let mut step = json!({"key":"facility","action":{"kind":"manual","params":{
            "category":"construction","building_id":"land_facility","state_id":42,"province_id":10
        }}});
        let result = bind(&step, &index, &geography).unwrap();
        assert_eq!(result["step"]["completion"], fact("country.fortifications.items.10.buildings.land_facility", "gte", json!(1)));
        step["action"]["params"].as_object_mut().unwrap().remove("province_id");
        let result = bind(&step, &index, &geography).unwrap();
        assert!(result["step"].get("completion").is_none());
    }

    #[test]
    fn compound_requires_each_target_and_does_not_discard_an_unknown() {
        let entries = json!([
            {"kind":"building","id":"infrastructure","name":"Infrastructure","location_scope":"state"},
            {"kind":"building","id":"land_facility","name":"Land Facility","location_scope":"province","max_level":1,"specializations":["specialization_land"]},
            {"kind":"state","id":"42","name":"Test State","geography":{"province_ids":[10]}}
        ]);
        let index = Index::new(&json!({"entries":entries})).unwrap();
        let geography = Geography::new(index.rows());
        let mut step = json!({"key":"compound","action":{"kind":"manual","params":{"category":"construction","construction_targets":[
            {"building_id":"infrastructure","state_id":42,"target_level":5},
            {"building_id":"land_facility","state_id":42,"province_id":10}
        ]}}});
        let result = bind(&step, &index, &geography).unwrap();
        assert_eq!(result["step"]["completion"]["op"], "all");
        assert_eq!(result["step"]["completion"]["args"].as_array().unwrap().len(), 2);
        step["action"]["params"]["construction_targets"][1].as_object_mut().unwrap().remove("province_id");
        let result = bind(&step, &index, &geography).unwrap();
        assert_eq!(result["reason"], "compound_construction_unresolved");
        assert!(result["step"]["completion"]["args"][1]["key"].as_str().unwrap().ends_with("unqualified"));
    }

    #[test]
    fn queued_or_built_maximum_target_accepts_the_steps_manual_confirmation() {
        let entries = json!([
            {"kind":"building","id":"infrastructure","name":"Infrastructure","location_scope":"state"},
            {"kind":"building","id":"arms_factory","name":"Military Factory","location_scope":"state"},
            {"kind":"state","id":"42","name":"Test State","geography":{"province_ids":[10]}}
        ]);
        let index = Index::new(&json!({"entries":entries})).unwrap();
        let geography = Geography::new(index.rows());
        let step = json!({"key":"build","action":{"kind":"manual","params":{"category":"construction","completion_mode":"queued_or_built","construction_targets":[
            {"building_id":"infrastructure","state_id":42,"target_level":5,"quantity_mode":"maximum"},
            {"building_id":"arms_factory","state_id":42,"quantity_mode":"maximum"}
        ]}}});
        let result = bind(&step, &index, &geography).unwrap();
        let args = result["step"]["completion"]["args"].as_array().unwrap();
        // The observable level stays game-verified only.
        assert_eq!(args[0]["args"][0], fact("country.states.items.42.infrastructure", "gte", json!(5)));
        let queued = |v: &str| json!({"op":"any","args":[
            fact("construction.queued_known", "contains", json!(v)),
            fact("construction.queued", "contains", json!(v))]});
        assert_eq!(args[0]["args"][1], queued("state:42:infrastructure"));
        // The unqualified maximum: queued now, or confirmed by the player.
        assert_eq!(args[1]["args"][0], json!({"op":"check","key":"build"}));
        assert_eq!(args[1]["args"][1], queued("state:42:arms_factory"));
    }
}
