//! Guide Schema v1 compatibility at the data boundary. Unknown descriptive
//! metadata is preserved; executable conditions and cross-references are bounded.
use crate::{domain_context::metadata_equal, state::truth};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
#[path = "guide_schema_extensions.rs"]
mod extensions;
type Result<T> = std::result::Result<T, String>;
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
fn nonempty(value: &Value, limit: usize) -> bool {
    value
        .as_str()
        .is_some_and(|s| !s.is_empty() && s.chars().count() <= limit)
}
fn integer(value: &Value, min: i64, max: i64) -> bool {
    value.as_i64().is_some_and(|n| (min..=max).contains(&n))
}
fn number(value: &Value) -> bool {
    value
        .as_f64()
        .is_some_and(|n| n.is_finite() && (n.fract() != 0. || n.abs() <= 9007199254740991.))
}
fn keys(value: &Value, allowed: &[&str], required: &[&str]) -> bool {
    value.as_object().is_some_and(|m| {
        m.keys().all(|k| allowed.contains(&k.as_str()))
            && required.iter().all(|k| m.contains_key(*k))
    })
}
fn strings(value: &Value, min: usize, max: usize, limit: usize, unique: bool) -> bool {
    value.as_array().is_some_and(|a| {
        (min..=max).contains(&a.len())
            && a.iter().all(|v| nonempty(v, limit))
            && (!unique || a.iter().map(Value::to_string).collect::<HashSet<_>>().len() == a.len())
    })
}
pub fn condition(value: &Value, depth: usize) -> Result<()> {
    if depth > 16 || !value.is_object() {
        return Err("Condizione non valida o troppo profonda".into());
    }
    match text(&value["op"]) {
        "all" | "any" => {
            let args = value["args"]
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 64)
                .ok_or("Operandi condizione non validi")?;
            for child in args {
                condition(child, depth + 1)?
            }
        }
        "not" => condition(&value["arg"], depth + 1)?,
        "fact" | "check" => {
            if !nonempty(&value["key"], 512) {
                return Err("Chiave condizione non valida".into());
            }
            if value["op"] == "fact" {
                if !["eq", "contains", "gte"].contains(&text(&value["test"]))
                    || value.get("value").is_none()
                {
                    return Err("Confronto condizione non valido".into());
                }
                let v = &value["value"];
                if !(v.is_string() || v.is_boolean() || number(v)) {
                    return Err("Valore condizione non scalare o non finito".into());
                }
            }
        }
        _ => return Err("Operatore condizione non supportato".into()),
    }
    Ok(())
}
pub fn entity(value: &Value) -> Result<()> {
    if !value.is_object()
        || ![
            "country",
            "focus",
            "technology",
            "state",
            "equipment",
            "project",
            "building",
        ]
        .contains(&text(&value["kind"]))
    {
        return Err("Tipo entità non valido".into());
    }
    if !nonempty(&value["id"], 512) {
        return Err("Identità entità non valida".into());
    }
    if value.get("scope").is_some_and(|v| !nonempty(v, usize::MAX)) {
        return Err("Ambito entità non valido".into());
    }
    Ok(())
}
fn action(value: &Value) -> Result<()> {
    if !value.is_object() {
        return Err("Azione non supportata".into());
    }
    let expected = match text(&value["kind"]) {
        "select_focus" => Some("focus"),
        "research" => Some("technology"),
        "special_project" => Some("project"),
        "manual" | "free" => None,
        _ => return Err("Azione non supportata".into()),
    };
    if let Some(kind) = expected {
        entity(&value["entity"])?;
        if value["entity"]["kind"] != kind {
            return Err("Entità incompatibile con azione".into());
        }
    } else if value.get("entity").is_some() {
        return Err("Azione descrittiva senza entità richiesta".into());
    }
    Ok(())
}
fn default(value: &mut Value, key: &str, item: Value) {
    if value.get(key).is_none() {
        value[key] = item
    }
}
fn rows(guide: &Value) -> Vec<&Value> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|v| v.as_array().into_iter().flatten())
        .collect()
}
pub fn normalize(guide: &Value) -> Result<Value> {
    if !guide.is_object() {
        return Err("Guida non valida".into());
    }
    let mut result = guide.clone();
    if result
        .get("dependency_policy")
        .is_some_and(|v| !["legacy", "explicit"].contains(&text(v)))
    {
        return Err("Politica dipendenze guida non valida".into());
    }
    for field in ["schema_version", "schema"] {
        if result
            .get(field)
            .is_some_and(|v| !number(v) || v.as_f64() != Some(1.))
        {
            return Err("Versione contratto guida non supportata".into());
        }
    }
    result["schema_version"] = json!(1);
    let mut scope = json!({});
    for field in ["country", "tree_id"] {
        if truth(&result[field]) {
            scope[field] = result[field].clone()
        }
    }
    default(&mut result, "applies_to", scope);
    if result["applies_to"].as_object().is_none_or(|m| {
        m.iter()
            .any(|(k, v)| !["country", "tree_id"].contains(&k.as_str()) || !nonempty(v, usize::MAX))
    }) {
        return Err("Ambito guida non valido".into());
    }
    for field in ["country", "tree_id"] {
        if let Some(value) = result["applies_to"].get(field).cloned() {
            if truth(&result[field]) && result[field] != value {
                return Err("Ambito guida in conflitto con i campi legacy".into());
            }
            default(&mut result, field, value);
        }
    }
    let scope = result["applies_to"].clone();
    let tracks = result["tracks"]
        .as_object_mut()
        .filter(|m| m.len() <= 32)
        .ok_or("Tracce guida non valide")?;
    let mut all_keys = HashSet::new();
    for (track, steps) in tracks {
        let steps = steps
            .as_array_mut()
            .filter(|a| a.len() <= 4096)
            .ok_or("Traccia guida non valida")?;
        for (i, step) in steps.iter_mut().enumerate() {
            if !step.is_object() {
                return Err("Passaggio guida non valido".into());
            }
            default(step, "key", json!(format!("{track}-{}", i + 1)));
            if !nonempty(&step["key"], usize::MAX)
                || !all_keys.insert(text(&step["key"]).to_owned())
            {
                return Err("Chiave passaggio assente o duplicata".into());
            }
            if step.get("action").is_none() {
                let (kind, entity) = match text(&step["kind"]) {
                    "focus" => ("select_focus", Some("focus")),
                    "technology" => ("research", Some("technology")),
                    "project" => ("special_project", Some("project")),
                    "gate" => ("manual", None),
                    "free" => ("free", None),
                    _ => return Err("Tipo passaggio non supportato".into()),
                };
                let mut value = json!({"kind":kind});
                if let Some(kind) = entity {
                    value["entity"] = json!({"kind":kind,"id":step.get("target").cloned().unwrap_or_else(||json!(""))});
                }
                step["action"] = value;
            }
            action(&step["action"])?;
            if step["action"]["entity"]["kind"] == "focus" && truth(&scope["tree_id"]) {
                let entity = &mut step["action"]["entity"];
                if entity.get("scope").is_some_and(|v| *v != scope["tree_id"]) {
                    return Err("Ambito focus in conflitto con la guida".into());
                }
                default(entity, "scope", scope["tree_id"].clone());
            }
            step["kind"] = json!(match text(&step["action"]["kind"]) {
                "select_focus" => "focus",
                "research" => "technology",
                "special_project" => "project",
                "manual" => "gate",
                _ => "free",
            });
            if truth(&step["action"]["entity"]) {
                step["target"] = step["action"]["entity"]["id"].clone()
            }
            if truth(&step["gate"]) {
                default(step, "when", json!({"op":"check","key":step["gate"]}));
            }
            if truth(&step["available_gate"]) && truth(&step["priority_after"]) {
                default(step, "priority", json!(100));
                default(
                    step,
                    "priority_when",
                    json!({"op":"all","args":[{"op":"fact","key":"focus.completed","test":"contains","value":step["priority_after"]},{"op":"check","key":step["available_gate"]}]}),
                );
            }
        }
    }
    let before = result.clone();
    let before_rows = rows(&before);
    for steps in result["tracks"].as_object_mut().unwrap().values_mut() {
        for step in steps.as_array_mut().unwrap() {
            if truth(&step["optional_before"]) && step.get("skip_when").is_none() {
                let mut args = vec![
                    json!({"op":"fact","key":"focus.completed","test":"contains","value":step["optional_before"]}),
                ];
                args.extend(
                    before_rows
                        .iter()
                        .filter(|s| {
                            s["target"] == step["optional_before"] && truth(&s["priority_when"])
                        })
                        .map(|s| s["priority_when"].clone()),
                );
                step["skip_when"] = json!({"op":"any","args":args});
            }
            for field in ["when", "priority_when", "skip_when"] {
                if let Some(value) = step.get(field) {
                    condition(value, 0)?
                }
            }
            if let Some(value) = step.get("priority") {
                if !number(value)
                    || value
                        .as_f64()
                        .is_none_or(|n| n.fract() != 0. || !(-100000. ..=100000.).contains(&n))
                {
                    return Err("Priorità guida non valida".into());
                }
                step["priority"] = json!(value.as_f64().unwrap() as i64);
            }
        }
    }
    default(&mut result, "milestones", json!([]));
    default(&mut result, "assumptions", json!([]));
    let title = result.get("id").cloned().unwrap_or_else(|| json!("Guida"));
    default(&mut result, "title", title);
    default(&mut result, "subtitle", json!(""));
    extensions::validate(&result)?;
    Ok(result)
}
fn identifiers(value: &Value, label: &str, limit: usize) -> Result<HashSet<String>> {
    let rows = value
        .as_array()
        .filter(|a| a.len() <= limit)
        .ok_or_else(|| format!("{label}: elenco non valido"))?;
    let mut ids = HashSet::new();
    for item in rows {
        if !item.is_object() {
            return Err(format!("{label}: elemento non valido"));
        }
        if !nonempty(&item["id"], 160) || !ids.insert(text(&item["id"]).into()) {
            return Err(format!("{label}: identificatore mancante o duplicato"));
        }
    }
    Ok(ids)
}
fn action_params(value: &Value) -> Result<()> {
    if value.as_object().is_none_or(|m|m.len()>128) {return Err("Parametri azione non validi".into());}
    fn source(v:&Value)->bool {
        keys(v,&["schema_version","available","branch"],&["schema_version","available","branch"])
            && v["schema_version"]==1 && {let mut budget=0;hoi4companion_guide_core::source_criteria::validate(&v["available"],0,&mut budget).is_ok() && hoi4companion_guide_core::source_criteria::validate(&v["branch"],0,&mut budget).is_ok()}
    }
    // Typed script predicates have their own depth/node bounds. Other action
    // parameters retain the existing generic depth limit, including bad trees.
    let mut rest=value.clone();
    if source(&rest["operational_criteria"]) {rest.as_object_mut().unwrap().remove("operational_criteria");}
    for (binding,fields) in [("political_binding",&["source_criteria","explicit_source_criteria"][..]),("project_source_binding",&["operational_criteria"][..])] {
        if let Some(map)=rest.get_mut(binding).and_then(Value::as_object_mut).filter(|m|m.len()<=128) {for field in fields {if map.get(*field).is_some_and(source) {map.remove(*field);}}}
    }
    json_data(&rest,0)
}
fn json_data(value: &Value, depth: usize) -> Result<()> {
    if depth > 8 {
        return Err("Parametri azione troppo profondi".into());
    }
    match value {
        Value::Null | Value::Bool(_) => (),
        Value::String(s) => {
            if s.chars().count() > 4096 {
                return Err("Parametro testuale troppo lungo".into());
            }
        }
        Value::Number(_) => {
            if !number(value) {
                return Err("Parametro numerico non finito".into());
            }
        }
        Value::Array(rows) if rows.len() <= 512 => {
            for row in rows {
                json_data(row, depth + 1)?
            }
        }
        Value::Object(map) if map.len() <= 128 => {
            for (key, value) in map {
                if key.is_empty() || key.chars().count() > 128 {
                    return Err("Nome parametro non valido".into());
                }
                json_data(value, depth + 1)?
            }
        }
        _ => return Err("Parametri azione non validi".into()),
    }
    Ok(())
}
fn valid_url(value: &Value) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    if !(value.starts_with("https://") || value.starts_with("http://"))
        || value.chars().any(|c| c.is_whitespace() || c < ' ')
    {
        return false;
    }
    let Some((_, rest)) = value.split_once("://") else {
        return false;
    };
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let Some((host, end)) = rest.split_once(']') else {
            return false;
        };
        if !end.is_empty() && !end.starts_with(':') {
            return false;
        }
        (host, end.strip_prefix(':'))
    } else {
        authority
            .rsplit_once(':')
            .map(|(h, p)| (h, Some(p)))
            .unwrap_or((authority, None))
    };
    !host.is_empty()
        && port.is_none_or(|p| {
            p.is_empty()
                || p.bytes().all(|b| b.is_ascii_digit())
                    && p.parse::<u32>().is_ok_and(|v| v <= 65535)
        })
}
pub(crate) fn relative(step: &Value) -> Result<()> {
    if step
        .get("relative_research_unresolved")
        .is_some_and(|v| step["action"]["kind"] != "research" || *v != true)
    {
        return Err("Stato ricerca relativa non valido".into());
    }
    let relative = &step["relative_research"];
    if relative.is_null() {
        return Ok(());
    }
    if step["action"]["kind"] != "research"
        || !keys(relative, &["query", "chain"], &["query", "chain"])
        || !nonempty(&relative["query"], 512)
        || !strings(&relative["chain"], 1, 128, 512, true)
    {
        return Err("Ramo ricerca relativa non valido".into());
    }
    Ok(())
}
pub(crate) fn choice(step: &Value) -> Result<()> {
    let choice = &step["research_choice"];
    if choice.is_null() {
        return Ok(());
    }
    if step["action"]["kind"] != "research"
        || !keys(
            choice,
            &["group", "query", "branches", "level", "mode"],
            &["group", "query", "branches"],
        )
        || choice
            .get("mode")
            .is_some_and(|v| !["persistent", "once"].contains(&text(v)))
        || choice["mode"] == "once" && choice.get("level").is_some()
        || !["group", "query"].iter().all(|k| nonempty(&choice[k], 512))
        || choice["branches"]
            .as_object()
            .is_none_or(|m| !(2..=8).contains(&m.len()))
        || step.get("relative_research").is_some()
    {
        return Err("Scelta ramo ricerca non valida".into());
    }
    let mut ids = HashSet::new();
    for (key, chain) in choice["branches"].as_object().unwrap() {
        if key.is_empty() || key.chars().count() > 160 || !strings(chain, 1, 128, 512, false) {
            return Err("Ramo alternativo non valido".into());
        }
        for id in chain.as_array().unwrap() {
            if !ids.insert(text(id)) {
                return Err("Livello o identità alternative non validi".into());
            }
        }
    }
    if let Some(level) = choice.get("level") {
        if !integer(level, 1, 128) {
            return Err("Livello o identità alternative non validi".into());
        }
        if choice["branches"]
            .as_object()
            .unwrap()
            .values()
            .any(|chain| level.as_i64().unwrap() > chain.as_array().unwrap().len() as i64)
        {
            return Err("Livello alternativo non presente".into());
        }
    }
    Ok(())
}
