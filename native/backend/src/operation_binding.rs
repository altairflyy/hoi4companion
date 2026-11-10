//! One deterministic operation compilation pass over one static catalog cohort.
use crate::{
    construction_binding, geography::Geography, import_catalog::Index, political_binding,
    production_binding, production_milestones, reference_identity,
};
use serde_json::{Value, json};
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn automatic(node: &Value) -> bool {
    match s(&node["op"]) {
        "fact" => true,
        "all" | "any" => arr(&node["args"]).iter().all(automatic),
        "not" => automatic(&node["arg"]),
        _ => false,
    }
}
fn title(row: &Value) -> &Value {
    row.get("title").unwrap_or(&row["key"])
}
pub fn bind(guide: &Value, index: &Index, library: &Value) -> Result<Value, String> {
    let mut guide = guide.clone();
    let geography = Geography::new(index.rows());
    let country = guide["applies_to"]["country"].clone();
    let mut variants = reference_identity::bindings(&guide, index, library)?;
    let tracks = guide["tracks"].as_object().ok_or("Missing guide tracks")?;
    for row in tracks.values().flat_map(arr) {
        let params = &row["action"]["params"];
        if params["category"] == "production"
            && let Some(name) = params["variant_name"].as_str()
        {
            let probe = production_binding::bind(row, index, &country, library, &Value::Null)?;
            let selector = &probe["step"]["action"]["params"]["production_selector"];
            if selector.as_object().is_some_and(|m| !m.is_empty()) {
                let key = reference_identity::name_key(name);
                if variants.get(&key).is_none() {
                    variants[&key] = json!([]);
                }
                let values = variants[&key]
                    .as_array_mut()
                    .ok_or("Invalid declared variant list")?;
                if !values.contains(selector) {
                    values.push(selector.clone());
                }
            }
        }
    }
    let mut reports = vec![];
    for step in guide["tracks"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .flat_map(|v| v.as_array_mut().into_iter().flatten())
    {
        if !["manual", "special_project"].contains(&s(&step["action"]["kind"])) {
            continue;
        }
        if step["tracking"] == "reminder" {
            reports.push(json!({"key":step["key"],"title":title(step),"monitoring":"reminder","automatic_start":false,"automatic_completion":false,"reason":"method_reminder"}));
            continue;
        }
        let foreign = political_binding::foreign(step, index)?;
        if foreign["bound"] == true {
            *step = foreign["step"].clone();
            reports.push(json!({"key":step["key"],"title":title(step),"monitoring":if automatic(&step["completion"]){"automatic"}else{"manual"},"automatic_start":automatic(&step["started_when"]),"automatic_completion":automatic(&step["completion"]),"reason":"foreign_interaction_live_state"}));
            continue;
        }
        let foundation = construction_binding::bind(step, index, &geography)?;
        let mut location = Value::Null;
        let mut audit = json!([]);
        let reason = if foundation["handled"] == true {
            *step = foundation["step"].clone();
            location = foundation["location"].clone();
            foundation["reason"].clone()
        } else if step["action"]["params"]["category"] == "production" {
            let result = production_binding::bind(step, index, &country, library, &variants)?;
            *step = result["step"].clone();
            result["reason"].clone()
        } else if ["doctrine_id", "doctrine_name"].iter().any(|k| {
            step["action"]["params"][*k]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        }) {
            let result = political_binding::doctrine(step, index, &country)?;
            *step = result["step"].clone();
            result["reason"].clone()
        } else {
            let result = political_binding::bind(step, index, &country, true)?;
            *step = result["step"].clone();
            audit = result["audit"].clone();
            if result["reason"].is_null() {
                json!("no_supported_live_binding")
            } else {
                result["reason"].clone()
            }
        };
        let started = automatic(&step["started_when"]);
        let mut completed = automatic(&step["completion"]);
        if reason == "doctrine_selection_unobserved" {
            completed = false;
        }
        let reason = if started && !completed && step.get("completion").is_some() {
            json!("declared_manual_completion")
        } else {
            reason
        };
        let mut report = json!({"key":step["key"],"title":title(step),"monitoring":if completed{"automatic"}else if started{"start_only"}else{"manual"},"automatic_start":started,"automatic_completion":completed,"reason":reason});
        if !arr(&audit).is_empty() {
            report["bindings"] = audit;
        }
        if location.get("state_id").is_some() {
            report["location"] = location;
        }
        reports.push(report);
    }
    production_milestones::annotate(&mut guide)?;
    Ok(json!({"guide":guide,"reports":reports}))
}
