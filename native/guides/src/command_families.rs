//! Literal foreign, fortification and doctrine commands; no strategy synthesis.
use crate::{
    import_catalog::{Index, regex},
    informal_targets, political_binding,
};
use hoi4companion_catalog_core::normal_name;
use serde_json::{Value, json};
use std::collections::HashMap;
type Result<T> = std::result::Result<T, String>;
fn aliases() -> &'static HashMap<String, String> {
    static ALIASES: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();
    ALIASES.get_or_init(|| {
        let mut result = HashMap::new();
        for (key, names) in political_binding::foreign_tasks()["tasks"]
            .as_object()
            .into_iter()
            .flatten()
        {
            for name in std::iter::once(key.as_str()).chain(
                names
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            ) {
                result.insert(normal_name(name), key.clone());
            }
        }
        result
    })
}
pub fn foreign(text: &str, index: &Index) -> Result<Option<Value>> {
    if text.chars().count() > 4096 {
        return Ok(None);
    }
    let aliases = aliases();
    let mut previous = None;
    let mut result = vec![];
    let mut prior_group = vec![];
    for clause in text.split(',') {
        let clause = clause.trim();
        let cost = regex(r"(?i)\s*\((\d+)\s*PP\)\s*$").find(clause);
        let group_cost = cost.map(|m| m.as_str().trim().to_owned());
        let clause = cost.map(|m| clause[..m.start()].trim()).unwrap_or(clause);
        let mut body = None;
        let mut target = None;
        for hit in regex(r"(?i)\s+(?:(?:with|in|against|on|verso|con)\s+)?").find_iter(clause) {
            let query = &clause[hit.end()..];
            let candidate = if aliases.contains_key(&normal_name(query)) {
                Value::Null
            } else {
                index.query(json!({"op":"resolve_country","query":query}))?
            };
            if let Some(candidate) = candidate.as_str().filter(|s| !s.is_empty()) {
                body = Some(clause[..hit.start()].trim());
                target = Some(candidate.to_owned());
                break;
            }
        }
        let body = body.unwrap_or(clause);
        let Some(target) = target.or_else(|| previous.clone()) else {
            return Ok(None);
        };
        let mut group = vec![];
        for label in regex(r"\s*[+&]\s*").split(body) {
            let Some(key) = aliases.get(&normal_name(label)) else {
                return Ok(None);
            };
            let mission = ![
                "improve_relations",
                "stop_improving_relations",
                "non_aggression_pact",
            ]
            .contains(&key.as_str()) && !political_binding::foreign_tasks()["relation_actions"][key].is_object();
            let mut params = json!({"category":if mission{"projects"}else{"politics"},"operation_id":if mission{"assign_operative_mission"}else{key},"target_country":target});
            if mission {
                params["mission_id"] = json!(key);
            }
            if let Some(cost) = &group_cost {
                params["source_group_cost"] = json!(cost);
            }
            group.push(result.len());
            result.push(json!({"text":format!("{} {target}",label.trim()),"params":params,"after_keys":prior_group,"after_previous":!prior_group.is_empty()}));
        }
        previous = Some(target);
        prior_group = group;
    }
    Ok((!result.is_empty()).then(|| json!(result)))
}
pub fn foreign_prefix(text: &str, index: &Index) -> Result<Value> {
    if let Some(full) = foreign(text, index)? {
        return Ok(json!([full, ""]));
    }
    for (cut, _) in text.match_indices(',').rev() {
        if let Some(parsed) = foreign(text[..cut].trim(), index)? {
            return Ok(json!([parsed, text[cut + 1..].trim()]));
        }
    }
    Ok(json!([null, text]))
}
pub fn fortification(text: &str, index: &Index) -> Option<Value> {
    let text = regex(r"(?i)^\s*BUILD\s*:\s*")
        .replace(text, "")
        .trim()
        .to_owned();
    let (first, remainder) = text.split_once(',').unwrap_or((&text, ""));
    let hit = regex(
        r"(?i)^(?:build\s+)?(\d+|max(?:imum)?)\s+(?:(land|coastal)\s+)?forts?\s+(?:in|on)\s+(.+)$",
    )
    .captures(first.trim())?;
    let building = if hit
        .get(2)
        .is_some_and(|m| m.as_str().eq_ignore_ascii_case("coastal"))
    {
        "coastal_bunker"
    } else {
        "bunker"
    };
    let entries = index
        .rows()
        .iter()
        .filter(|r| r["kind"] == "building" && r["id"] == building)
        .collect::<Vec<_>>();
    if entries.len() != 1 {
        return None;
    }
    let level = if hit[1].to_lowercase().starts_with("max") {
        entries[0]["max_level"].as_u64()?
    } else {
        hit[1].parse::<u64>().ok()?
    };
    if !(1..=1000).contains(&level) {
        return None;
    }
    let mut params = json!({"category":"construction","building_id":building,"quantity_mode":"target_total","quantity":level,"target_level":level});
    if let Some(border) = regex(
        r"(?i)^each\s+(?:tile|province)\s+(?:at|on|along)\s+([A-Z0-9]{3})\s+(?:border|frontier)$",
    )
    .captures(&hit[3])
    {
        params["province_selector"] = json!({"kind":"border","country":border[1].to_uppercase()});
    } else if regex(r"(?i)^(?:province\s+)?\d+$").is_match(&hit[3]) {
        params["province_id"] = json!(regex(r"\d+").find(&hit[3])?.as_str().parse::<u64>().ok()?);
    } else {
        params["state_name"] = json!(hit[3].trim());
    }
    Some(json!({"params":params,"remainder":remainder.trim()}))
}
pub fn doctrine(text: &str, index: &Index, country: &Value) -> Result<Option<Value>> {
    let mut value = regex(r"(?i)^ASAP(?:\s*:\s*|\s+)")
        .replace(text.trim(), "")
        .to_string();
    let mut prefix = Value::Null;
    if let Some(adjacent) = regex(r"(?i)^(.+?)\s+ASAP:\s*(.+)$").captures(&value) {
        let Some(mut command) = informal_targets::named(&adjacent[1], index, country)?
            .filter(|c| c["timing"].as_str().is_none_or(|s| s.is_empty()))
        else {
            return Ok(None);
        };
        command["text"] = json!(&adjacent[1]);
        prefix = command;
        value = adjacent[2].into();
    }
    let Some(hit) =
        regex(r"(?i)^(.+?)\s+(Land|Air|Naval|Navy|Sea)\s+Doc(?:trine)?\b").captures(&value)
    else {
        return Ok(None);
    };
    let mut tail = value[hit.get(0).unwrap().end()..].trim().to_owned();
    let mut tactic = false;
    let mut notes = vec![];
    let mut cost = None;
    let mut unit = String::new();
    if let Some(m) = regex(r"(?i)^[+&]\s*Tactic\b").find(&tail) {
        tactic = true;
        tail = tail[m.end()..].trim().into();
        notes.push("Preferred Tactic (not specified)".into());
    }
    tail = regex(r"(?i)^ASAP\b\s*").replace(&tail, "").to_string();
    if let Some(m) =
        regex(r"(?i)^\(\s*(\d{1,6})\s*(AE|NE|AFE)(?:\s*,\s*discounted)?\s*\)").captures(&tail)
    {
        cost = m[1].parse::<u64>().ok();
        unit = m[2].to_uppercase();
        tail = tail[m.get(0).unwrap().end()..].trim().into();
    }
    tail = regex(r"(?i)^ASAP\b\s*").replace(&tail, "").to_string();
    if let Some(m) = regex(r"(?i)^\(([^()]+\s+BUILD)\)").find(&tail) {
        notes.push(m.as_str().to_owned());
        tail = tail[m.end()..].trim().into();
    }
    if !tail.is_empty() {
        let Some(remainder) = regex(r"^[,;+&]\s*(.+)$").captures(&tail) else {
            return Ok(None);
        };
        if regex(r"(?i)\b(?:if|when|unless|until|before|after|instead|or)\b")
            .is_match(&remainder[1])
        {
            return Ok(None);
        }
        notes.push(remainder[1].to_owned());
    }
    let domain = match hit[2].to_lowercase().as_str() {
        "land" => "land",
        "air" => "air",
        _ => "naval",
    };
    let domain_key = format!("{domain}_doctrines");
    let query = index.vocabulary()["abbreviations"][&domain_key]
        .as_object()
        .into_iter()
        .flatten()
        .find(|(alias, _)| normal_name(alias) == normal_name(&hit[1]))
        .and_then(|(_, v)| v.as_str())
        .unwrap_or(hit[1].trim());
    let matches=index.entries(json!({"op":"matches","query":query,"kinds":["doctrine"],"country":country,"filters":{"domain":domain}}))?;
    if matches.len() != 1 {
        return Ok(None);
    }
    let row = &matches[0];
    let currency = row["currency"]
        .as_str()
        .ok_or("Missing doctrine currency")?;
    if (unit == "NE" && currency != "navy_experience")
        || (unit == "AFE" && currency != "air_experience")
    {
        return Ok(None);
    }
    let params = json!({"category":"army","doctrine_id":row["id"],"doctrine_name":row["name"],"doctrine_domain":domain,"doctrine_track":row["track"],"vocabulary_binding":{"input":hit[1].trim(),"expanded":query,"domain":domain_key}});
    let mut policies = json!({});
    if let Some(cost) = cost.filter(|_| !tactic) {
        let key = match currency {
            "army_experience" => "country.experience.army",
            "navy_experience" => "country.experience.navy",
            "air_experience" => "country.experience.air",
            _ => return Err("Invalid doctrine currency".into()),
        };
        policies["resource_costs"] = json!({key:cost});
    }
    Ok(Some(
        json!({"params":params,"policies":policies,"remainder":if notes.is_empty(){Value::Null}else{json!(notes.join("\n"))},"prefix":prefix,"text":&value[..hit.get(0).unwrap().end()]}),
    ))
}
