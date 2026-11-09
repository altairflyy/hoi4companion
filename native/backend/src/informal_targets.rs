//! Complete literal commands resolved through the immutable country catalog.
use crate::{
    import_catalog::{Index, regex},
    political_binding, production_binding,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
type Result<T> = std::result::Result<T, String>;
pub fn sequence(text: &str, index: &Index, country: &Value) -> Result<Option<Value>> {
    let separator = regex(r"(?i)\s*(,?\s+then\s+|\s+&\s+)\s*");
    let mut parts = vec![];
    let mut separators = vec![];
    let mut start = 0;
    for hit in separator.captures_iter(text) {
        let m = hit.get(0).unwrap();
        parts.push(text[start..m.start()].trim());
        separators.push(hit[1].to_lowercase().contains("then"));
        start = m.end();
    }
    if separators.is_empty() || separators.len() > 15 {
        return Ok(None);
    }
    parts.push(text[start..].trim());
    let mut result = vec![];
    for (i, command) in parts.iter().enumerate() {
        let mut item = None;
        if let Some(law) =
            crate::word_sources::explicit_action(command).filter(|v| v["type"] == "law")
        {
            let probe = json!({"action":{"kind":"manual","params":{"category":"politics","law_name":law["target"]}}});
            let bound = political_binding::bind(&probe, index, country, false)?;
            if bound["reason"] == "politics_live_law" {
                let mut policies = json!({});
                if let Some(cost) = law.get("political_power_cost") {
                    policies["political_power_cost"] = cost.clone();
                }
                item = Some(
                    json!({"params":bound["step"]["action"]["params"],"policies":policies,"milestone":"completion"}),
                );
            }
        }
        if item.is_none()
            && let Some(hit) = regex(r"(?i)^(start|complete)\s+(?:special\s+)?project\s*:\s*(.+)$")
                .captures(command)
        {
            let matches = index.matches(&hit[2], &["project"], country, &Value::Null, false)?;
            if matches.len() == 1 {
                item = Some(
                    json!({"params":{"category":"projects","project_id":matches[0]["id"],"project_name":matches[0]["name"]},"policies":{},"milestone":if hit[1].eq_ignore_ascii_case("start"){"start"}else{"completion"}}),
                );
            }
        }
        if item.is_none()
            && let Some(named) = named(command, index, country)?
                .filter(|v| v["timing"].as_str().is_none_or(|s| s.is_empty()))
        {
            item = Some(
                json!({"params":named["params"],"policies":named["policies"],"milestone":"completion"}),
            );
        }
        let Some(mut item) = item else {
            return Ok(None);
        };
        item["text"] = json!(command);
        item["after_previous"] = json!(i > 0 && separators[i - 1]);
        result.push(item);
    }
    Ok(Some(json!(result)))
}
fn spirit(group: &str) -> Option<(&str, &str)> {
    match group {
        "academy_spirit" | "army_spirit" | "division_command_spirit" => {
            Some(("army_experience", "country.experience.army"))
        }
        "naval_academy_spirit" | "navy_spirit" | "naval_command_spirit" => {
            Some(("navy_experience", "country.experience.navy"))
        }
        "air_force_spirit" | "air_force_command_spirit" | "air_force_academy_spirit" => {
            Some(("air_experience", "country.experience.air"))
        }
        _ => None,
    }
}
pub fn named(text: &str, index: &Index, country: &Value) -> Result<Option<Value>> {
    if country.as_str().is_none_or(|s| s.is_empty()) {
        return Ok(None);
    }
    let text = regex(r"(?i)^ASAP(?:\s*:\s*|\s+)")
        .replace(text, "")
        .trim()
        .to_owned();
    let Some(hit)=regex(r"(?i)^([^:\n&]+?)(?:\s*\(\s*(\d{1,6})\s*(PP|AE|NE|AFE)\s*\))?(?:\s*\(\s*((?:before|after|when|if|on)\s+[^()]+)\s*\))?\s*$").captures(&text)else{return Ok(None)};
    let mut found = vec![];
    for kind in ["advisor", "idea", "decision"] {
        found.extend(index.matches(hit[1].trim(), &[kind], country, &Value::Null, false)?);
    }
    let roles = [
        "industrial_concern",
        "theorist",
        "political_advisor",
        "army_chief",
        "navy_chief",
        "air_chief",
        "high_command",
    ];
    found.retain(|e| {
        e["kind"] != "idea"
            || e["group"]
                .as_str()
                .is_some_and(|s| roles.contains(&s) || spirit(s).is_some())
    });
    if found.len() != 1 {
        return Ok(None);
    }
    let entry = &found[0];
    let kind = entry["kind"].as_str().unwrap_or("");
    if kind == "decision"
        && regex(r"(?i)\[(?:FROM|TARGET)(?:\.|\])").is_match(entry["name"].as_str().unwrap_or(""))
    {
        return Ok(None);
    }
    let currency = hit
        .get(3)
        .map(|m| m.as_str().to_uppercase())
        .unwrap_or_default();
    let count = hit.get(2).and_then(|m| m.as_str().parse::<u64>().ok());
    let mut params = json!({"category":if kind=="advisor"{"characters"}else{"politics"}});
    let mut policies = json!({});
    match kind {
        "advisor" => {
            params["advisor_id"] = entry["id"].clone();
            params["person_name"] = entry["name"].clone();
            params["role"] = entry["slot"].clone();
        }
        "idea" => {
            params["idea_id"] = entry["id"].clone();
            params["idea_group"] = entry["group"].clone();
        }
        _ => {
            params["decision_id"] = entry["id"].clone();
            params["decision_name"] = entry["name"].clone();
            params["decision_group"] = entry["group"].clone();
            policies["completion"] = json!({"op":"fact","key":"decisions.executed","test":"contains","value":entry["id"]});
        }
    }
    if currency == "PP" {
        policies["political_power_cost"] = json!(count);
    } else if !currency.is_empty() {
        let Some((native, key)) = entry["group"]
            .as_str()
            .and_then(spirit)
            .filter(|_| kind == "idea")
        else {
            return Ok(None);
        };
        let selected = match currency.as_str() {
            "AE" => native,
            "NE" => "navy_experience",
            _ => "air_experience",
        };
        if selected != native {
            return Ok(None);
        }
        params["cost_currency"] = json!(selected);
        policies["resource_costs"] = json!({key:count});
    }
    let bound = political_binding::bind(
        &json!({"action":{"kind":"manual","params":params}}),
        index,
        country,
        false,
    )?;
    if ![
        "politics_live_advisor",
        "politics_live_idea",
        "decision_identity_resolved",
    ]
    .contains(&bound["reason"].as_str().unwrap_or(""))
    {
        return Ok(None);
    }
    Ok(Some(
        json!({"params":bound["step"]["action"]["params"],"policies":policies,"timing":hit.get(4).map(|m|m.as_str())}),
    ))
}
pub fn production(
    text: &str,
    index: &Index,
    country: &Value,
    library: &Value,
    declared: &Value,
) -> Result<Option<Value>> {
    let pattern = regex(r"(?i)^\s*(MIL|NAV|Production)\s*:\s*(.+)$");
    let fallback;
    let hit = if let Some(hit) = pattern.captures(text) {
        hit
    } else {
        let body = regex(r"(?i)^design\s*(?:&|and)\s*produce\s+")
            .replace(text.trim(), "")
            .to_string();
        if !body.contains(':') {
            return Ok(None);
        }
        let body = regex(r"(?i)\s*\(\s*\d+\s*(?:AE|NE|AFE|PP)\s*\)(\s*:)").replace_all(&body, "$1");
        fallback = format!("MIL: {body}");
        pattern
            .captures(&fallback)
            .ok_or("Invalid production prefix")?
    };
    let prefix = hit[1].to_owned();
    let body = &hit[2];
    let category = match prefix.to_lowercase().as_str() {
        "mil" => Some("military"),
        "nav" => Some("naval"),
        _ => None,
    };
    if regex(r"\s&\s").is_match(body) {
        return Ok(None);
    }
    let aliases = index.vocabulary()["abbreviations"]["production"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| !["MIL", "NAV"].contains(&k.as_str()))
        .map(|(k, v)| (k.to_lowercase(), v.as_str().unwrap_or("").to_owned()))
        .collect::<HashMap<_, _>>();
    let mut parts = vec![];
    let mut original = vec![];
    let mut notes = vec![];
    let mut priorities = HashMap::new();
    for part in body.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let mut part = part.to_owned();
        if let Some(note) = regex(r"(?i)^(.*?)\s*(\((?:add\s+)?MIOs?\))$").captures(&part) {
            notes.push(note[2].to_owned());
            part = note[1].trim().into();
        }
        original.push(part.clone());
        if let Some(priority) =
            regex(r"(?i)^(.*?)\s*\((both\s+)?(top|highest|lowest)\s+(?:prio|priority)\)$")
                .captures(&part)
        {
            let mode = if priority[3].eq_ignore_ascii_case("lowest") {
                "last"
            } else {
                "first"
            };
            priorities.insert(parts.len(), mode);
            if priority.get(2).is_some() && !parts.is_empty() {
                priorities.insert(parts.len() - 1, mode);
            }
            part = priority[1].trim().into();
        }
        parts.push(part);
    }
    if parts.iter().any(|p| regex(r"^\d+$").is_match(p)) {
        return Ok(None);
    }
    let item = regex(
        r"(?i)^([^:\n]+?)\s*(?::\s*|\s+to\s+|\s+)(\d{1,6}(?:\s*(?:->|→)\s*\d{1,6})?|ALL|rest)\s*$",
    );
    let all_parts = parts
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            item.captures(p)
                .filter(|m| ["all", "rest"].contains(&m[2].to_lowercase().as_str()))
                .map(|_| i)
        })
        .collect::<Vec<_>>();
    let mut shared = HashMap::new();
    let mut pending = vec![];
    for (i, part) in parts.iter().enumerate() {
        if let Some(entry) = item.captures(part) {
            if entry[2].chars().all(|c| c.is_ascii_digit()) {
                for previous in pending.drain(..) {
                    shared.insert(previous, entry[2].to_owned());
                }
            }
            pending.clear();
        } else {
            pending.push(i);
        }
    }
    let mut parsed = vec![];
    for (i, part) in parts.iter().enumerate() {
        let part = shared
            .get(&i)
            .map(|q| format!("{part}:{q}"))
            .unwrap_or_else(|| part.clone());
        let Some(entry) = item.captures(&part) else {
            parsed.push(None);
            continue;
        };
        let query = entry[1].trim();
        let name = aliases
            .get(&query.to_lowercase())
            .map(String::as_str)
            .unwrap_or(query);
        let remaining = ["all", "rest"].contains(&entry[2].to_lowercase().as_str());
        if remaining
            && (category == Some("naval")
                || all_parts != vec![parts.len() - 1]
                || i != parts.len() - 1)
        {
            parsed.push(None);
            continue;
        }
        let mut params = json!({"equipment_name":name,"category":"production"});
        if remaining {
            params["quantity_mode"] = json!("remaining");
            params["production_category"] = json!("military");
        } else {
            let bounds = regex(r"\s*(?:->|→)\s*")
                .split(&entry[2])
                .map(|s| s.parse::<u64>())
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            if bounds.len() == 2 && bounds[0] > bounds[1] {
                parsed.push(None);
                continue;
            }
            params["requested_factories"] = json!(bounds.last().unwrap());
        }
        if let Some(category) = category {
            params["production_category"] = json!(category);
        }
        if let Some(priority) = priorities.get(&i) {
            params["production_priority"] = json!(priority);
            params["production_category"] = json!("military");
        }
        let bound = production_binding::bind(
            &json!({"key":"probe","action":{"kind":"manual","params":params}}),
            index,
            country,
            library,
            declared,
        )?;
        parsed.push(
            if [
                "production_live_assignment",
                "production_remaining_assignment",
            ]
            .contains(&bound["reason"].as_str().unwrap_or(""))
            {
                Some(bound["step"]["action"]["params"].clone())
            } else {
                None
            },
        );
    }
    for priority in ["first", "last"] {
        let group = priorities
            .iter()
            .filter_map(|(i, mode)| (*mode == priority).then_some(*i))
            .collect::<Vec<_>>();
        if group.iter().any(|i| parsed[*i].is_none()) {
            for i in group {
                parsed[i] = None;
            }
        }
    }
    let ids = parsed
        .iter()
        .map(|p| {
            p.as_ref()
                .and_then(|p| p["production_selector"]["equipment_ids"].as_array())
                .map(|a| a.iter().map(Value::to_string).collect::<HashSet<_>>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let mut targets = vec![];
    let mut remainder = vec![];
    for (i, params) in parsed.into_iter().enumerate() {
        match params {
            Some(params)
                if !ids
                    .iter()
                    .enumerate()
                    .any(|(j, set)| i != j && !ids[i].is_disjoint(set)) =>
            {
                targets.push(params)
            }
            _ => remainder.push(original[i].clone()),
        }
    }
    remainder.extend(notes);
    if targets.is_empty() || targets.len() > 64 {
        return Ok(None);
    }
    Ok(Some(
        json!({"params":{"category":"production","production_targets":targets},"remainder":if remainder.is_empty(){Value::Null}else{json!(format!("{prefix}: {}",remainder.join(", ")))}}),
    ))
}
