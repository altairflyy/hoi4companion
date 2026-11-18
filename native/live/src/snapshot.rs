//! Explicit optional snapshots only. Live acquisition never calls this module.
use crate::script_document::{all, entries, first, scalar};
use serde_json::{Value, json};
use std::{io::Read, path::Path};
type Result<T> = std::result::Result<T, String>;
pub fn read(path: &Path, tag: &str) -> Result<Value> {
    if !path.is_file() {
        return Err("File stato non trovato".into());
    }
    let mut data = vec![];
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(80_000_001)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > 80_000_000 {
        return Err("File oltre 80 MB: importazione bloccata per evitare rallentamenti".into());
    }
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if extension == "json" {
        let value: Value =
            serde_json::from_slice(data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&data))
                .map_err(|e| e.to_string())?;
        if !value.is_object() {
            return Err("Snapshot JSON non valido".into());
        }
        let empty = json!([]);
        let completed = value.get("completed_focuses").unwrap_or(&empty);
        if !completed
            .as_array()
            .is_some_and(|a| a.iter().all(Value::is_string))
        {
            return Err("completed_focuses deve essere una lista di ID".into());
        }
        if value
            .get("current_focus")
            .is_some_and(|v| !v.is_null() && !v.is_string())
        {
            return Err("current_focus non valido".into());
        }
        if value
            .get("focus_progress")
            .is_some_and(|v| !v.is_null() && !v.is_number())
        {
            return Err("focus_progress deve essere un numero".into());
        }
        let country = match value.get("tag") {
            None => tag.to_owned(),
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) => "None".into(),
            Some(Value::Bool(b)) => if *b { "True" } else { "False" }.into(),
            Some(v) => v.to_string(),
        };
        return Ok(
            json!({"tag":country,"completed":crate::operational_catalog::unique(completed.as_array().unwrap().iter().cloned()),"current":value["current_focus"],"progress":value["focus_progress"],"source":"snapshot_json","note":"Snapshot fornito manualmente, non sincronizzato con HOI4."}),
        );
    }
    if extension != "hoi4" {
        return Err("Sono accettati soltanto .json oppure salvataggi .hoi4 testuali".into());
    }
    if data.starts_with(b"HOI4bin") || data.starts_with(b"PK\x03\x04") {
        return Err("Salvataggio binario/compresso non supportato: non viene interpretato".into());
    }
    let content = data
        .strip_prefix(b"HOI4txt")
        .ok_or("Formato di salvataggio non riconosciuto come HOI4 testuale")?;
    let content = std::str::from_utf8(content.strip_prefix(b"\xef\xbb\xbf").unwrap_or(content))
        .map_err(|e| format!("Impossibile leggere il salvataggio: {e}"))?;
    let root = serde_json::to_value(
        hoi4companion_parser::parse(content.as_bytes())
            .map_err(|e| format!("Impossibile leggere il salvataggio: {e}"))?,
    )
    .map_err(|e| e.to_string())?;
    let countries = first(&root, "countries")
        .filter(|v| v.get("entries").is_some())
        .ok_or("Blocco countries assente nel salvataggio")?;
    let country = first(countries, tag)
        .filter(|v| v.get("entries").is_some())
        .ok_or_else(|| format!("Nazione {tag} non presente nel salvataggio"))?;
    let mut completed = vec![];
    for key in ["completed_focuses", "completed_focus"] {
        for value in all(country, key) {
            if value.get("entries").is_some() {
                for (name, item) in entries(value) {
                    if name.is_empty() && item.is_string() {
                        completed.push(item.clone());
                    } else if !name.is_empty() && [Some("yes"), Some("1")].contains(&item.as_str())
                    {
                        completed.push(json!(name));
                    }
                }
            } else if value.is_string() {
                completed.push(value.clone());
            }
        }
    }
    let focus = first(country, "focus").unwrap_or(&Value::Null);
    let mut current = scalar(country, "national_focus");
    if current.is_empty() {
        current = scalar(country, "current_focus");
    }
    if current.is_empty() {
        current = scalar(focus, "current");
    }
    let current =
        (!current.is_empty() && !["0", "none", "---"].contains(&current)).then_some(current);
    let progress = ["progress", "amount"]
        .iter()
        .find_map(|key| {
            let v = scalar(focus, key);
            (!v.is_empty()).then(|| crate::source_eligibility::number(v))
        })
        .flatten();
    if completed.is_empty() && current.is_none() {
        return Err("Campi focus non riconosciuti: nessuno stato dedotto dal salvataggio".into());
    }
    Ok(
        json!({"tag":tag,"completed":crate::operational_catalog::unique(completed),"current":current,"progress":progress,"source":"save_testuale","note":"Lettura sperimentale: verificare lo stato in gioco. Il progresso non è normalizzato."}),
    )
}
