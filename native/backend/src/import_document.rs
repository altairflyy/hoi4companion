//! Source integrity and portable reference remapping, independent of a model.
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
type Result<T> = std::result::Result<T, String>;
pub fn validate_sources(sources: &Value) -> Result<Value> {
    let sources = sources
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 8)
        .ok_or("Servono da 1 a 8 fonti acquisite")?;
    let mut result = Map::new();
    for source in sources {
        let text = source["text"]
            .as_str()
            .filter(|s| !s.is_empty() && s.chars().count() <= 180000)
            .ok_or("Testo fonte mancante o troppo lungo")?;
        let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
        let id = format!("src-{}", &digest[..16]);
        if source["content_sha256"] != digest || source["id"] != id || result.contains_key(&id) {
            return Err(
                "Identità o checksum fonte incoerente/duplicato: riacquisisci la fonte modificata"
                    .into(),
            );
        }
        if ![Some("youtube"), Some("web"), Some("manual"), Some("file")]
            .contains(&source["type"].as_str())
        {
            return Err("Tipo fonte non valido".into());
        }
        if source
            .get("warnings")
            .is_some_and(|v| !v.as_array().is_some_and(|a| a.iter().all(Value::is_string)))
        {
            return Err("Avvisi fonte non validi".into());
        }
        if source.get("segments").is_some_and(|v| {
            !v.as_array().is_some_and(|a| {
                a.len() <= 20000
                    && a.iter().all(|s| {
                        s.is_object()
                            && s["text"].is_string()
                            && s["start"]
                                .as_f64()
                                .is_some_and(|n| n.is_finite() && n >= 0.)
                    })
            })
        }) {
            return Err("Segmenti fonte non validi".into());
        }
        result.insert(id, source.clone());
    }
    Ok(Value::Object(result))
}
pub fn inventory(guide: &Value) -> Result<Option<&Vec<Value>>> {
    let Some(value) = guide.get("source_inventory").filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let Some(items) = value["items"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 4096)
    else {
        return Err("Inventario fonti v1: richiesto un elenco di 1–4096 istruzioni".into());
    };
    if value["schema_version"].as_u64() != Some(1) || value.as_object().is_none_or(|m| m.len() != 2)
    {
        return Err("Inventario fonti v1: richiesto un elenco di 1–4096 istruzioni".into());
    }
    let mut seen = HashSet::new();
    for item in items {
        if item.as_object().is_none_or(|m| {
            m.keys().any(|k| {
                ![
                    "id",
                    "source_id",
                    "quote",
                    "timestamp_seconds",
                    "step_keys",
                    "excluded_reason",
                    "basis",
                ]
                .contains(&k.as_str())
            })
        }) {
            return Err("Inventario fonti: campi non validi".into());
        }
        if item.get("basis").is_some_and(|v| {
            ![
                Some("explicit"),
                Some("visual"),
                Some("inferred"),
                Some("game_derived"),
                Some("unverified"),
            ]
            .contains(&v.as_str())
        }) {
            return Err("Inventario fonti: basis non valida".into());
        }
        for (field, limit) in [("id", 160), ("source_id", 160), ("quote", 4096)] {
            if !item[field]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && s.chars().count() <= limit)
            {
                return Err(format!("Inventario fonti: campo non valido {field}"));
            }
        }
        let id = item["id"].as_str().unwrap();
        if !seen.insert(id) {
            return Err(format!("Inventario fonti: ID duplicato {id}"));
        }
        if item.get("step_keys").is_some_and(|v| {
            !v.as_array().is_some_and(|a| {
                a.len() <= 4096
                    && a.iter().all(|v| {
                        v.as_str()
                            .is_some_and(|s| !s.is_empty() && s.chars().count() <= 160)
                    })
                    && a.iter().map(Value::to_string).collect::<HashSet<_>>().len() == a.len()
            })
        }) {
            return Err("Inventario fonti: step_keys non validi".into());
        }
        if item.get("excluded_reason").is_some_and(|v| {
            !v.as_str()
                .is_some_and(|s| !s.trim().is_empty() && s.chars().count() <= 4096)
        }) {
            return Err("Inventario fonti: motivazione di esclusione vuota o non valida".into());
        }
        if item
            .get("timestamp_seconds")
            .is_some_and(|v| !v.as_f64().is_some_and(|n| n.is_finite() && n >= 0.))
        {
            return Err("Inventario fonti: timestamp non valido".into());
        }
    }
    Ok(Some(items))
}
fn remap(value: &mut Value, mapping: &HashMap<String, String>) -> Result<()> {
    if !value.is_object() {
        return Err("Riferimento fonte non valido".into());
    }
    let field = if value.get("source_id").is_some() {
        "source_id"
    } else {
        "id"
    };
    let id = value[field]
        .as_str()
        .and_then(|s| mapping.get(s))
        .ok_or("Il file cita una fonte non presente negli allegati")?;
    value[field] = json!(id);
    Ok(())
}
pub fn document(input: &Value) -> Result<Value> {
    let input = if input["format"] == "hoi4-guide-authoring" {
        crate::authoring::compile(input)?
    } else {
        input.clone()
    };
    if !input.is_object()
        || input["import_schema_version"].as_u64() != Some(1)
        || input
            .get("format")
            .is_some_and(|v| v != "hoi4-guide-import")
    {
        return Err("File di importazione v1 non valido".into());
    }
    let supplied = input["sources"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 8)
        .ok_or("Il file deve contenere da 1 a 8 fonti con testo ed evidenze")?;
    let mut mapping = HashMap::new();
    let mut sources = vec![];
    for raw in supplied {
        let Some(id) = raw["id"].as_str().filter(|s| !s.is_empty()) else {
            return Err("Fonte del file assente, duplicata o non valida".into());
        };
        if !raw.is_object() || mapping.contains_key(id) || !raw["text"].is_string() {
            return Err("Fonte del file assente, duplicata o non valida".into());
        }
        let mut value = raw.clone();
        if value.get("content_sha256").is_none() {
            let digest = format!(
                "{:x}",
                Sha256::digest(raw["text"].as_str().unwrap().as_bytes())
            );
            value["id"] = json!(format!("src-{}", &digest[..16]));
            value["content_sha256"] = json!(digest);
            value["acquisition"] = json!("external_ai");
            if value.get("warnings").is_none() {
                value["warnings"] = json!([]);
            }
            value["warnings"].as_array_mut().ok_or("Avvisi fonte non validi")?.push(json!("Fonte fornita dall’IA esterna: coerenza interna verificabile, autenticità e copertura da revisionare."));
        }
        validate_sources(&json!([value]))?;
        mapping.insert(id.to_owned(), value["id"].as_str().unwrap().to_owned());
        sources.push(value);
    }
    validate_sources(&json!(sources))?;
    let mut guide = input["guide"].clone();
    if !guide.is_null() {
        if !guide.is_object() || !guide["sources"].is_array() || !guide["tracks"].is_object() {
            return Err("Definizione della guida mancante o non valida".into());
        }
        inventory(&guide)?;
        for value in guide["sources"].as_array_mut().unwrap() {
            remap(value, &mapping)?;
        }
        if let Some(items) = guide
            .get_mut("source_inventory")
            .and_then(|v| v.get_mut("items"))
            .and_then(Value::as_array_mut)
        {
            for item in items {
                remap(item, &mapping)?;
            }
        }
        for lane in guide["tracks"].as_object_mut().unwrap().values_mut() {
            let steps = lane.as_array_mut().ok_or("Traccia del file non valida")?;
            for step in steps {
                if !step.is_object() || step.get("evidence").is_some_and(|v| !v.is_array()) {
                    return Err("Step o evidenze del file non validi".into());
                }
                if let Some(evidence) = step.get_mut("evidence").and_then(Value::as_array_mut) {
                    for value in evidence {
                        remap(value, &mapping)?;
                    }
                }
            }
        }
    }
    Ok(json!({"import_schema_version":1,"sources":sources,"guide":guide}))
}
