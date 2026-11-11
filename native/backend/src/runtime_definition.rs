//! Static runtime adaptation shared by old imports and new native boot. It
//! repairs supported legacy declarations in memory, never source guide files.
use crate::{
    guide_schema, import_catalog::Index, operation_binding, operation_language, research_import,
    timing_compiler,
};
use serde_json::{Map, Value, json};
fn rows(guide: &Value) -> impl Iterator<Item = &Value> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|v| v.as_array().into_iter().flatten())
}
fn a(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
pub fn legacy(guide: &Value, index: &Index) -> Result<Value, String> {
    let mut guide = guide.clone();
    let by_id = index
        .entries(json!({"op":"rows","kinds":["technology"]}))?
        .into_iter()
        .map(|v| (s(&v["id"]).to_owned(), v))
        .collect::<Map<_, _>>();
    let pattern = crate::import_catalog::regex(r"(?i)^\s*(.+?)\s+UP\s*(?:\([^)]*\))?\s*$");
    let mut warnings = vec![];
    for step in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
        .flat_map(|v| v.as_array_mut().into_iter().flatten())
    {
        if crate::state::truth(&step["relative_research"])
            || !step["research_upgrade"].is_object()
            || step["action"]["kind"] != "research"
        {
            continue;
        }
        let query = step["research_upgrade"]["query"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        let Some(found) = pattern.captures(&query) else {
            continue;
        };
        let anchor = &step["research_upgrade"]["from_id"];
        let chain =
            research_import::relative_chain(&found[1], &by_id, index.vocabulary()).or_else(|| {
                anchor
                    .as_str()
                    .filter(|key| by_id.contains_key(*key))
                    .and_then(|key| research_import::chains(&[key.into()], &by_id, None))
            });
        if let Some(chain) = chain.filter(|v| !v.is_empty()) {
            step["relative_research"] = json!({"query":query,"chain":chain});
            step.as_object_mut()
                .unwrap()
                .remove("relative_research_unresolved");
        } else {
            step["relative_research_unresolved"] = json!(true);
            warnings.push(json!(format!("{}: ramo UP non identificato nel catalogo corrente; la vecchia associazione non viene usata.",s(&step["key"]))));
        }
    }
    if !warnings.is_empty() {
        guide
            .as_object_mut()
            .unwrap()
            .entry("association_warnings")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or("Invalid association warnings")?
            .extend(warnings);
    }
    Ok(guide)
}
pub fn prepare(
    guide: &Value,
    index: &Index,
    continuous: &Value,
    library: &Value,
) -> Result<Value, String> {
    let mut guide = if rows(guide).any(|r| {
        crate::state::truth(&r["research_upgrade"]) && !crate::state::truth(&r["relative_research"])
    }) {
        legacy(guide, index)?
    } else {
        guide.clone()
    };
    for step in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
        .flat_map(|v| v.as_array_mut().into_iter().flatten())
    {
        if step["action"]["kind"] == "select_focus"
            && continuous.get(s(&step["action"]["entity"]["id"])).is_some()
        {
            if step["action"].get("params").is_none() {
                step["action"]["params"] = json!({});
            }
            step["action"]["params"]["focus_mode"] = json!("continuous");
        }
    }
    let timing = rows(&guide).any(|step| crate::state::truth(&step["action"]["params"]["timing"]));
    let mut known = false;
    for step in rows(&guide) {
        if timing_compiler::compile(
            &step["action"]["params"]["timing"],
            timing.then_some(index),
            &guide["applies_to"]["country"],
            &guide["applies_to"]["tree_id"],
            s(&step["action"]["params"]["cost_currency"]),
            0,
        )?
        .is_some()
        {
            known = true;
            break;
        }
    }
    let word = a(&guide["sources"]).iter().any(|v| v["type"] == "file")
        && (a(&guide["sources"])
            .iter()
            .any(|v| s(&v["text"]).contains("[Table row]"))
            || rows(&guide).any(|r| r["tracking"] == "reminder"));
    let production = rows(&guide).any(|r| r["action"]["params"]["category"] == "production");
    let refresh_bindings = rows(&guide).any(|r| {
        let p=&r["action"]["params"];
        ["decision_id","advisor_id","law_id","person_id","idea_id"].iter().any(|k|p[*k].is_string()) || p["category"] == "construction"
            || crate::political_binding::foreign_tasks()["relation_actions"][s(&p["operation_id"])].is_object()
    });
    if guide["dependency_policy"] == "explicit" && (word || known || production || refresh_bindings) {
        let promoted = operation_language::promote(&guide, index, library)?;
        if !a(&promoted["changes"]).is_empty() || refresh_bindings {
            guide = guide_schema::normalize(
                &operation_binding::bind(&promoted["guide"], index, library)?["guide"],
            )?;
        }
    }
    Ok(guide)
}
