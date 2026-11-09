//! Definition discovery and optimistic local identity corrections. This module
//! never owns observations, run state or source files from the installed game.
use crate::guide_schema;
use serde_json::{Value, json};
use std::{collections::HashSet, io::Read, path::Path};
type Result<T> = std::result::Result<T, String>;
pub fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
pub fn json_file(path: &Path, limit: u64) -> Result<Value> {
    let mut data = vec![];
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > limit {
        return Err("Guide data exceeds size limit".into());
    }
    serde_json::from_slice(&data).map_err(|e| e.to_string())
}
pub fn associations(id: &str, directory: &Path) -> Result<Value> {
    if !identity(id) {
        return Err("ID guida non valido".into());
    }
    let path = directory.join(format!("{id}.json"));
    if !path.exists() {
        return Ok(json!({"schema_version":1,"guide_id":id,"changes":[]}));
    }
    let value = json_file(&path, 2_000_000)?;
    if value["schema_version"] != 1
        || value["guide_id"] != id
        || !value["changes"].as_array().is_some_and(|a| a.len() <= 4096)
    {
        return Err("Correzioni associazioni non valide".into());
    }
    let mut keys = HashSet::new();
    for item in value["changes"].as_array().unwrap() {
        let Some(key) = item["key"].as_str().filter(|s| !s.is_empty()) else {
            return Err("Correzione associazione duplicata o non valida".into());
        };
        if !keys.insert(key) {
            return Err("Correzione associazione duplicata o non valida".into());
        }
        for field in ["original", "entity"] {
            guide_schema::entity(&item[field])?;
            if item[field].as_object().is_none_or(|m| {
                m.keys()
                    .any(|k| !["kind", "id", "scope"].contains(&k.as_str()))
            }) {
                return Err("Campi associazione non validi".into());
            }
        }
        if item["original"]["kind"] != item["entity"]["kind"] {
            return Err("Tipo associazione cambiato".into());
        }
    }
    Ok(value)
}
pub fn apply(guide: &Value, changes: &Value) -> Result<Value> {
    let mut result = guide_schema::normalize(guide)?;
    let mut warnings = vec![];
    for item in changes["changes"].as_array().into_iter().flatten() {
        let row = result["tracks"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .flat_map(|v| v.as_array_mut().into_iter().flatten())
            .find(|r| r["key"] == item["key"]);
        let Some(row) = row.filter(|r| r["action"]["entity"] == item["original"]) else {
            warnings.push(json!(format!(
                "{}: definizione originale cambiata; correzione precedente non applicata.",
                item["key"].as_str().unwrap_or("")
            )));
            continue;
        };
        row["action"]["entity"] = item["entity"].clone();
        row["target"] = item["entity"]["id"].clone();
        if let Some(resolution) = row.get_mut("focus_resolution") {
            resolution["entity"] = item["entity"].clone();
            resolution["locked"] = json!(true);
            resolution["classification"] = json!("direct");
        }
        for field in [
            "research_upgrade",
            "relative_research",
            "relative_research_unresolved",
            "research_choice",
        ] {
            row.as_object_mut().unwrap().remove(field);
        }
    }
    if !warnings.is_empty() {
        result["association_warnings"] = json!(warnings);
    }
    guide_schema::normalize(&result)
}
pub fn load(directories: &Value, association_directory: Option<&Path>) -> Result<Value> {
    let mut result = vec![];
    let mut ids = HashSet::new();
    for directory in directories.as_array().ok_or("Missing guide directories")? {
        let directory = Path::new(directory.as_str().ok_or("Invalid guide directory")?);
        if !directory.exists() {
            continue;
        }
        let mut files = std::fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>>>()?;
        files.retain(|p| p.is_file() && p.extension().is_some_and(|s| s == "json"));
        files.sort();
        for path in files {
            if result.len() >= 4096 {
                return Err("Guide registry limit exceeded".into());
            }
            let mut guide = guide_schema::normalize(&json_file(&path, 32 * 1024 * 1024)?)?;
            let id = guide["id"].as_str().unwrap_or("");
            if !identity(id) || !ids.insert(id.to_owned()) {
                return Err("Identità guida assente, duplicata o non valida".into());
            }
            if let Some(directory) = association_directory {
                guide = apply(&guide, &associations(id, directory)?)?;
            }
            result.push(guide);
        }
    }
    Ok(json!({"guides":result}))
}

/// Prepare the complete, validated correction before publishing a local override.
/// The HTTP owner serializes this transaction; source and progress are untouched.
pub fn change(
    guide: &Value,
    key: &Value,
    expected: &Value,
    entity: &Value,
    index: &crate::import_catalog::Index,
    directory: &Path,
) -> Result<Value> {
    let key = key
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Passaggio non valido")?;
    let source = guide_schema::normalize(guide)?;
    let id = source["id"].as_str().ok_or("ID guida non valido")?;
    let mut data = associations(id, directory)?;
    let mut current = apply(&source, &data)?;
    let locate = |g: &Value| {
        g["tracks"]
            .as_object()
            .into_iter()
            .flat_map(|m| m.values())
            .flat_map(|v| v.as_array().into_iter().flatten())
            .find(|r| r["key"] == key)
            .cloned()
    };
    let row = locate(&current)
        .filter(|r| {
            ["select_focus", "research", "special_project"]
                .contains(&r["action"]["kind"].as_str().unwrap_or(""))
        })
        .ok_or("Passaggio senza associazione modificabile")?;
    let original = locate(&source).ok_or("Passaggio non valido")?["action"]["entity"].clone();
    let old = &row["action"]["entity"];
    if expected != old {
        return Err("Associazione cambiata: ricarica prima di salvare".into());
    }
    let replacement = if entity.is_null() { &original } else { entity };
    guide_schema::entity(replacement)?;
    if replacement.as_object().is_none_or(|m| {
        m.keys()
            .any(|k| !["kind", "id", "scope"].contains(&k.as_str()))
    }) || replacement["kind"] != old["kind"]
    {
        return Err("Tipo associazione incompatibile".into());
    }
    if !entity.is_null() {
        let scope = source["applies_to"]
            .get("tree_id")
            .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
            .unwrap_or(&old["scope"]);
        if old["kind"] == "focus"
            && scope.as_str().is_some_and(|s| !s.is_empty())
            && replacement.get("scope") != Some(scope)
        {
            return Err("Albero focus incompatibile con la guida".into());
        }
        let country = &source["applies_to"]["country"];
        let matches = index
            .rows()
            .iter()
            .filter(|r| {
                r["kind"] == replacement["kind"]
                    && r["id"] == replacement["id"]
                    && (r["kind"] != "focus" || r.get("scope") == replacement.get("scope"))
                    && (country.is_null()
                        || country == ""
                        || r["countries"]
                            .as_array()
                            .is_none_or(|a| a.is_empty() || a.contains(country)))
            })
            .count();
        if matches != 1 {
            return Err("ID non presente o ambiguo nel catalogo compatibile".into());
        }
    }
    // Reuse the same identity patch logic used at boot and for restored overrides.
    let patch = json!({"changes":[{"key":key,"original":old,"entity":replacement}]});
    current = apply(&current, &patch)?;
    if old["kind"] == "focus" && !entity.is_null() {
        let before = crate::focus_routes::resolve(&apply(&source, &data)?, index)?;
        let after = crate::focus_routes::resolve(&current, index)?;
        let previous = before["errors"]
            .as_array()
            .ok_or("Invalid focus validation result")?;
        let added: Vec<_> = after["errors"]
            .as_array()
            .ok_or("Invalid focus validation result")?
            .iter()
            .filter(|e| !previous.contains(e))
            .filter_map(Value::as_str)
            .collect();
        if !added.is_empty() {
            return Err(added.join("; "));
        }
    }
    let changes = data["changes"].as_array_mut().unwrap();
    changes.retain(|r| r["key"] != key);
    if *replacement != original {
        changes.push(json!({"key":key,"original":original,"entity":replacement}));
    }
    if changes.len() > 4096 {
        return Err("Correzioni associazioni non valide".into());
    }
    let result = apply(&source, &data)?;
    let payload = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
    if payload.len() > 2_000_000 {
        return Err("Correzioni associazioni troppo grandi".into());
    }
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    crate::reference_library::atomic_write(&directory.join(format!("{id}.json")), &payload)?;
    Ok(result)
}
