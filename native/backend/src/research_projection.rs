//! A campaign binding selects presentation identity; missing data selects no ID.
use serde_json::{Value, json};
pub fn project(guide: &Value, bindings: &Value) -> Result<Value, String> {
    let mut guide = guide.clone();
    let bindings = bindings.as_object();
    for row in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
        .flat_map(|v| v.as_array_mut().into_iter().flatten())
    {
        if row["research_choice"].is_object() {
            crate::guide_schema::choice(row)?;
            let selected = bindings
                .and_then(|m| m.get(row["key"].as_str().unwrap_or("")))
                .and_then(Value::as_str);
            let valid = selected.is_some_and(|id| {
                row["research_choice"]["branches"]
                    .as_object()
                    .into_iter()
                    .flat_map(|m| m.values())
                    .flat_map(|v| v.as_array().into_iter().flatten())
                    .any(|v| v == id)
            });
            let target = if valid {
                row.as_object_mut().unwrap().remove("catalog_match");
                selected.unwrap().to_owned()
            } else {
                row["catalog_match"] = json!(false);
                format!("unbound-choice:{}", row["key"].as_str().unwrap_or(""))
            };
            row["action"]["entity"]["id"] = json!(target);
            row["target"] = json!(target);
        }
        if row["relative_research"].is_null() && row["relative_research_unresolved"] != true {
            continue;
        }
        crate::guide_schema::relative(row)?;
        let selected = bindings
            .and_then(|m| m.get(row["key"].as_str().unwrap_or("")))
            .and_then(Value::as_str);
        let valid = selected.is_some_and(|id| {
            row["relative_research"]["chain"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v == id))
        });
        let target = if valid {
            if row["action"]["entity"]["id"]
                .as_str()
                .is_some_and(|s| s.starts_with("unbound-up:"))
            {
                row.as_object_mut().unwrap().remove("catalog_match");
            }
            selected.unwrap().to_owned()
        } else {
            row["catalog_match"] = json!(false);
            format!("unbound-up:{}", row["key"].as_str().unwrap_or(""))
        };
        row["action"]["entity"]["id"] = json!(target);
        row["target"] = json!(target);
    }
    Ok(guide)
}
