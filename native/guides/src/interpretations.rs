//! The player's own reading of a guide line the companion follows only by hand
//! (UX R13, stage A): a display category and, optionally, the game entity the
//! line is about. Presentation only. Interpretations live in their own file,
//! `<guide id>.interpretations.json` next to the binding corrections, and are
//! never part of a guide definition, an archive or an evaluation: they are
//! attached to a projection's steps after the evaluation and change neither
//! status, completion, progress nor what the HUD may highlight. A step keeps
//! its verification (manual); only "Not interpreted" becomes "Confirm yourself".
//!
//! Each item records the line text it was written for. When the guide text of
//! that key changes, the item is stale and is not shown.
use crate::guide_repository::{identity, json_file};
use serde_json::{Value, json};
use std::{collections::HashSet, path::Path};
type Result<T> = std::result::Result<T, String>;

/// The display categories a player can choose (`display_category::category`).
pub const CATEGORIES: &[&str] = &[
    "construction",
    "production",
    "army",
    "diplomacy",
    "trade",
    "politics",
    "projects",
    "checkpoints",
];
const LIMIT: usize = 4096;
const TARGET_KINDS: &[&str] = &["country", "focus", "technology", "state", "equipment", "project", "building"];

fn path(id: &str, directory: &Path) -> std::path::PathBuf {
    directory.join(format!("{id}.interpretations.json"))
}
fn steps(guide: &Value) -> impl Iterator<Item = &Value> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|v| v.as_array().into_iter().flatten())
}
/// The line a player reads for this step: its source title, else its first instruction.
pub fn text(step: &Value) -> String {
    let clean = |v: &Value| v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    clean(&step["title"])
        .or_else(|| step["action"]["params"]["instructions"].as_array().into_iter().flatten().find_map(clean))
        .unwrap_or_default()
}
/// Steps a player may interpret: lines confirmed by hand without a game-entity
/// binding (bound steps are corrected through the associations instead).
/// Focus, research and observed steps are automatic; reminders stay reminders.
pub fn editable(step: &Value) -> bool {
    !step["action"]["entity"].is_object()
        && ["manual", "uninterpreted"].contains(&crate::import_reports::verification(step))
        && !text(step).is_empty()
}
fn valid_target(target: &Value) -> bool {
    target.as_object().is_some_and(|m| {
        m.keys().all(|k| ["kind", "id", "scope", "name"].contains(&k.as_str()))
            && TARGET_KINDS.contains(&target["kind"].as_str().unwrap_or(""))
            && target["id"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 512)
            && target["name"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 512)
            && target.get("scope").is_none_or(|s| s.as_str().is_some_and(|s| !s.is_empty()))
    })
}
fn valid_item(item: &Value) -> bool {
    item.as_object().is_some_and(|m| m.keys().all(|k| ["key", "text", "category", "target"].contains(&k.as_str())))
        && item["key"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 512)
        && item["text"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 20_000)
        && CATEGORIES.contains(&item["category"].as_str().unwrap_or(""))
        && (item["target"].is_null() || valid_target(&item["target"]))
}
pub fn load(id: &str, directory: &Path) -> Result<Value> {
    if !identity(id) {
        return Err("ID guida non valido".into());
    }
    let file = path(id, directory);
    if !file.exists() {
        return Ok(json!({"schema_version":1,"guide_id":id,"items":[]}));
    }
    let value = json_file(&file, 4_000_000)?;
    let items = value["items"].as_array().filter(|a| a.len() <= LIMIT);
    let mut keys = HashSet::new();
    if value["schema_version"] != 1
        || value["guide_id"] != id
        || items.is_none_or(|a| a.iter().any(|i| !valid_item(i) || !keys.insert(i["key"].as_str().unwrap_or(""))))
    {
        return Err("Interpretazioni della guida non valide".into());
    }
    Ok(value)
}
/// Attach the current interpretations to the steps of a projected guide. An
/// item applies only to an editable step whose line is still the one it was
/// written for. Returns the keys that no longer match.
pub fn attach(guide: &mut Value, data: &Value) -> Vec<String> {
    let mut stale = vec![];
    for item in data["items"].as_array().into_iter().flatten() {
        let step = guide["tracks"]
            .as_object_mut()
            .into_iter()
            .flat_map(|m| m.values_mut())
            .flat_map(|v| v.as_array_mut().into_iter().flatten())
            .find(|s| s["key"] == item["key"]);
        match step {
            Some(step) if editable(step) && text(step) == item["text"].as_str().unwrap_or("") => {
                step["interpretation"] = json!({"category":item["category"],"target":item["target"]});
            }
            _ => stale.push(item["key"].as_str().unwrap_or("").to_owned()),
        }
    }
    stale
}
/// Save (or, with a null `interpretation`, remove) the player's reading of one
/// step of `guide`, the definition the player currently sees. `expected` is the
/// line text the player read; a changed line is refused. The target must
/// already be resolved by the caller against the game catalog.
pub fn change(guide: &Value, key: &Value, expected: &Value, interpretation: &Value, directory: &Path) -> Result<Value> {
    let id = guide["id"].as_str().ok_or("ID guida non valido")?;
    let key = key.as_str().filter(|s| !s.is_empty()).ok_or("Passaggio non valido")?;
    let step = steps(guide).find(|s| s["key"] == key).ok_or("Passaggio non valido")?;
    if !editable(step) {
        return Err("Passaggio senza riga da interpretare".into());
    }
    let line = text(step);
    if expected.as_str() != Some(line.as_str()) {
        return Err("Testo del passaggio cambiato: ricarica prima di salvare".into());
    }
    let mut data = load(id, directory)?;
    let items = data["items"].as_array_mut().unwrap();
    items.retain(|i| i["key"] != key);
    if !interpretation.is_null() {
        let fields = interpretation.as_object().ok_or("Interpretazione non valida")?;
        if fields.keys().any(|k| !["category", "target"].contains(&k.as_str())) {
            return Err("Interpretazione non valida".into());
        }
        let target = interpretation.get("target").cloned().unwrap_or(Value::Null);
        let item = json!({"key":key,"text":line,"category":interpretation["category"],"target":target});
        if !valid_item(&item) {
            return Err("Interpretazione non valida".into());
        }
        items.push(item);
    }
    if items.len() > LIMIT {
        return Err("Troppe interpretazioni per questa guida".into());
    }
    let payload = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    crate::reference_library::atomic_write(&path(id, directory), &payload)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn guide() -> Value {
        let source = |n: u64, line: &str| json!({"source_action":{"source_id":"s","key":"k","ordinal":n},"category":"checkpoints","instructions":[line]});
        json!({"id":"g","tracks":{"operations":[
            {"key":"raw","title":"Def against YUG","action":{"kind":"manual","params":source(1,"Def against YUG")}},
            {"key":"law","title":"Partial mobilisation","action":{"kind":"manual","params":{"category":"politics"}}},
            {"key":"note","title":"Save PP","tracking":"reminder","action":{"kind":"manual","params":source(2,"Save PP")}},
            {"key":"focus","title":"Focus","action":{"kind":"select_focus","entity":{"kind":"focus","id":"ROM_x"}}},
            {"key":"project","title":"Rocket","action":{"kind":"special_project","entity":{"kind":"project","id":"sp_rocket"}}}
        ]}})
    }
    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("hoi4-interpretations-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }
    #[test]
    fn only_hand_confirmed_lines_without_a_binding_are_editable() {
        let g = guide();
        let editable_keys: Vec<_> = steps(&g).filter(|s| editable(s)).map(|s| s["key"].as_str().unwrap()).collect();
        assert_eq!(editable_keys, ["raw", "law"]);
    }
    #[test]
    fn a_saved_interpretation_attaches_to_its_line_and_never_to_a_changed_one() {
        let d = dir("attach");
        let g = guide();
        let target = json!({"kind":"country","id":"YUG","name":"Yugoslavia"});
        change(&g, &json!("raw"), &json!("Def against YUG"), &json!({"category":"army","target":target}), &d).unwrap();
        let data = load("g", &d).unwrap();
        let mut projected = g.clone();
        assert!(attach(&mut projected, &data).is_empty());
        let raw = steps(&projected).find(|s| s["key"] == "raw").unwrap();
        assert_eq!(raw["interpretation"], json!({"category":"army","target":target}));
        assert_eq!(crate::import_reports::verification(raw), "manual", "an interpreted line is confirmed by hand, still never automatic");
        assert_eq!(raw["completion"], Value::Null, "no completion criterion is added");
        let mut edited = g.clone();
        edited["tracks"]["operations"][0]["title"] = json!("Defend against YUG and HUN");
        assert_eq!(attach(&mut edited, &data), ["raw"]);
        assert!(steps(&edited).all(|s| s.get("interpretation").is_none()));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn change_refuses_bound_reminder_unknown_or_stale_steps_and_removes_on_null() {
        let d = dir("change");
        let g = guide();
        let ok = json!({"category":"politics"});
        for (key, text) in [("focus", "Focus"), ("project", "Rocket"), ("note", "Save PP"), ("missing", "x")] {
            assert!(change(&g, &json!(key), &json!(text), &ok, &d).is_err(), "{key}");
        }
        assert!(change(&g, &json!("raw"), &json!("old text"), &ok, &d).is_err());
        for bad in [json!({"category":"weather"}), json!({"category":"army","target":{"kind":"province","id":"1","name":"x"}}),
            json!({"category":"army","completion":true}), json!({"category":"army","target":{"kind":"country","id":"YUG"}})] {
            assert!(change(&g, &json!("raw"), &json!("Def against YUG"), &bad, &d).is_err(), "{bad}");
        }
        change(&g, &json!("raw"), &json!("Def against YUG"), &ok, &d).unwrap();
        change(&g, &json!("raw"), &json!("Def against YUG"), &json!({"category":"army"}), &d).unwrap();
        assert_eq!(load("g", &d).unwrap()["items"].as_array().unwrap().len(), 1, "one item per step");
        change(&g, &json!("raw"), &json!("Def against YUG"), &Value::Null, &d).unwrap();
        assert_eq!(load("g", &d).unwrap()["items"], json!([]));
        let _ = std::fs::remove_dir_all(&d);
    }
}
