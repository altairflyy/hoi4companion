//! Conservative static selectors. Only provable country/DLC predicates count;
//! uncertainty or equal tree priority never selects an arbitrary alternative.
use crate::script_document::{blocks, entries, first, scalar};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
fn and(values: &[Option<bool>]) -> Option<bool> {
    if values.contains(&Some(false)) {
        Some(false)
    } else if values.contains(&None) {
        None
    } else {
        Some(true)
    }
}
fn or(values: &[Option<bool>]) -> Option<bool> {
    if values.contains(&Some(true)) {
        Some(true)
    } else if values.contains(&None) {
        None
    } else {
        Some(false)
    }
}
fn not(value: Option<bool>) -> Option<bool> {
    value.map(|v| !v)
}
fn tag(value: &str) -> bool {
    value.len() == 3
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}
pub fn dlc_enabled(inventory: &Value, name: &str) -> Option<bool> {
    if !inventory.is_object() {
        return None;
    }
    let key = hoi4companion_catalog_core::dlc_name(name);
    let matches: Vec<_> = inventory["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| {
            hoi4companion_catalog_core::dlc_name(item["name"].as_str().unwrap_or("")) == key
        })
        .collect();
    if matches
        .iter()
        .any(|item| [Some("enabled"), Some("integrated")].contains(&item["status"].as_str()))
    {
        Some(true)
    } else if !matches.is_empty()
        && matches
            .iter()
            .all(|item| [Some("disabled"), Some("missing")].contains(&item["status"].as_str()))
        || matches.is_empty() && inventory["complete"] == true
    {
        Some(false)
    } else {
        None
    }
}
fn condition_inner(
    node: &Value,
    country: &str,
    dlcs: &Value,
    scripts: &Value,
    stack: &mut Vec<String>,
) -> Option<bool> {
    let mut values = vec![];
    for (key, value) in entries(node) {
        let answer = if ["AND", "OR", "NOT", "NOR", "NAND"].contains(&key) && value.is_object() {
            if key == "AND" {
                condition_inner(value, country, dlcs, scripts, stack)
            } else {
                let parts: Vec<_> = entries(value)
                    .map(|(key, value)| {
                        condition_inner(
                            &json!({"entries":[[key,value]]}),
                            country,
                            dlcs,
                            scripts,
                            stack,
                        )
                    })
                    .collect();
                match key {
                    "OR" => or(&parts),
                    "NOR" => not(or(&parts)),
                    "NAND" => not(and(&parts)),
                    _ => {
                        if parts.len() == 1 {
                            not(parts[0])
                        } else {
                            None
                        }
                    }
                }
            }
        } else if key == "has_dlc" && value.is_string() {
            dlc_enabled(dlcs, value.as_str().unwrap())
        } else if ["tag", "original_tag"].contains(&key) && value.as_str().is_some_and(tag) {
            Some(value == country)
        } else if key == "always" && [Some("yes"), Some("no")].contains(&value.as_str()) {
            Some(value == "yes")
        } else if scripts.get(key).is_some()
            && [Some("yes"), Some("no")].contains(&value.as_str())
            && !stack.iter().any(|s| s == key)
            && stack.len() < 30
        {
            stack.push(key.into());
            let answer = condition_inner(&scripts[key], country, dlcs, scripts, stack);
            stack.pop();
            if value == "no" { not(answer) } else { answer }
        } else {
            None
        };
        values.push(answer);
    }
    and(&values)
}
pub fn condition(node: &Value, country: &str, dlcs: &Value, scripts: &Value) -> Option<bool> {
    condition_inner(node, country, dlcs, scripts, &mut vec![])
}
fn tags(node: &Value, scripts: &Value, stack: &mut Vec<String>, out: &mut BTreeSet<String>) {
    for (key, value) in entries(node) {
        if ["tag", "original_tag"].contains(&key) && value.as_str().is_some_and(tag) {
            out.insert(value.as_str().unwrap().into());
        } else if value.is_object() {
            tags(value, scripts, stack, out);
        } else if scripts.get(key).is_some() && !stack.iter().any(|s| s == key) && stack.len() < 30
        {
            stack.push(key.into());
            tags(&scripts[key], scripts, stack, out);
            stack.pop();
        }
    }
}
pub fn number(raw: &str) -> Option<f64> {
    raw.trim()
        .replace('_', "")
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
}
pub fn score(node: &Value, country: &str, dlcs: &Value, scripts: &Value) -> Option<f64> {
    let mut value = number(
        first(node, "factor")
            .and_then(Value::as_str)
            .unwrap_or(scalar(node, "base")),
    )?;
    if entries(node).any(|(k, _)| !["factor", "base", "modifier"].contains(&k)) {
        return None;
    }
    for modifier in blocks(node, "modifier") {
        let rules = json!({"entries":entries(modifier).filter(|(k,_)|!["add","factor"].contains(k)).map(|(k,v)|json!([k,v])).collect::<Vec<_>>()});
        match condition(&rules, country, dlcs, scripts) {
            Some(false) => continue,
            None => return None,
            _ => {}
        }
        let add = scalar(modifier, "add");
        let factor = scalar(modifier, "factor");
        if add.is_empty() == factor.is_empty() {
            return None;
        }
        let change = number(if add.is_empty() { factor } else { add })?;
        value = if add.is_empty() {
            value * change
        } else {
            value + change
        };
        if !value.is_finite() {
            return None;
        }
    }
    Some(value)
}
pub fn filter(trees: &Value, dlcs: &Value, scripts: &Value) -> Result<Value, String> {
    let trees = trees.as_object().ok_or("Missing tree selectors")?;
    let mut countries = BTreeSet::from([String::new()]);
    let mut scores = HashMap::new();
    for tree in trees.values() {
        if tree["country_selector"].get("entries").is_some() {
            tags(
                &tree["country_selector"],
                scripts,
                &mut vec![],
                &mut countries,
            );
        }
    }
    for (id, tree) in trees {
        if tree["country_selector"].get("entries").is_some() {
            scores.insert(
                id.clone(),
                countries
                    .iter()
                    .map(|tag| {
                        (
                            tag.clone(),
                            score(&tree["country_selector"], tag, dlcs, scripts),
                        )
                    })
                    .collect::<HashMap<_, _>>(),
            );
        }
    }
    let mut winners = HashMap::new();
    let mut uncertain = HashSet::new();
    for country in &countries {
        let candidates: Vec<_> = scores
            .iter()
            .map(|(id, values)| (id, values.get(country).copied().flatten()))
            .collect();
        if candidates.iter().any(|(_, value)| value.is_none()) {
            uncertain.insert(country);
            continue;
        }
        let best = candidates
            .iter()
            .filter_map(|(_, v)| *v)
            .max_by(f64::total_cmp)
            .unwrap_or(0.);
        let selected: Vec<_> = candidates
            .iter()
            .filter(|(_, v)| *v == Some(best) && best > 0.)
            .map(|(id, _)| (*id).clone())
            .collect();
        if selected.len() == 1 {
            winners.insert(country, selected[0].clone());
        }
    }
    let (mut visible, mut excluded, mut eligible) = (vec![], vec![], Map::new());
    for (id, tree) in trees {
        let selected: Vec<_> = countries
            .iter()
            .filter(|tag| winners.get(tag) == Some(id))
            .collect();
        if !selected.is_empty() {
            visible.push(id);
            eligible.insert(
                id.clone(),
                json!(
                    selected
                        .iter()
                        .filter(|tag| !tag.is_empty())
                        .collect::<Vec<_>>()
                ),
            );
            continue;
        }
        let values = scores.get(id);
        let (reason, category) = if values.is_none_or(|v| {
            v.values().any(Option::is_none)
                || v.iter()
                    .any(|(tag, value)| uncertain.contains(tag) && value.is_some_and(|v| v > 0.))
        }) {
            (
                "Condizioni della partita o attivazione DLC non verificabili dai file locali.",
                "unknown",
            )
        } else if values.is_some_and(|v| v.values().any(|value| value.is_some_and(|v| v > 0.))) {
            (
                "Albero alternativo: priorità inferiore o parità con un altro albero per gli stessi paesi.",
                "alternative",
            )
        } else {
            (
                "Requisiti DLC non soddisfatti, oppure assegnazione solo tramite eventi della partita.",
                "unavailable",
            )
        };
        excluded.push(json!({"id":id,"source":tree["source"],"reason":reason,"category":category}));
    }
    Ok(json!({"visible":visible,"excluded":excluded,"countries":eligible}))
}
pub fn scripts(files: &Value) -> Result<Value, String> {
    let mut scripts = Map::new();
    let mut warnings = vec![];
    for path in files.as_array().ok_or("Missing scripted trigger files")? {
        let path = std::path::Path::new(path.as_str().ok_or("Invalid trigger path")?);
        match crate::script_document::read(path) {
            Ok(node) => {
                for (key, value) in entries(&node) {
                    if value.is_object() {
                        scripts.insert(key.into(), value.clone());
                    }
                }
            }
            Err(error) => warnings.push(format!(
                "Trigger non leggibili ({}): {error}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }
    Ok(json!({"scripts":scripts,"warnings":warnings}))
}
