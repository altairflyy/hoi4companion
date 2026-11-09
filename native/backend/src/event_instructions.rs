//! Explicit answer identity only. A button match never proves an event outcome.
use crate::{
    import_catalog::{Index, regex},
    political_binding, word_sources,
};
use serde_json::{Value, json};
fn unquote(s: &str) -> &str {
    s.trim().trim_matches(['"', '“', '”', '‘', '’']).trim()
}
pub fn command(text: &Value, index: &Index, country: &Value) -> Result<Option<Value>, String> {
    let Some(text) = text
        .as_str()
        .filter(|s| !s.is_empty() && s.chars().count() <= 1024 && !s.contains('\n'))
    else {
        return Ok(None);
    };
    let mut value = text.trim().to_owned();
    let mut policies = json!({});
    if let Some(hit) = regex(r"(?i)\s*\((\d{1,6})\s*PP\)\s*$").captures(&value) {
        policies["political_power_cost"] = json!(hit[1].parse::<u64>().map_err(|e| e.to_string())?);
        value = value[..hit.get(0).unwrap().start()].trim().into();
    }
    let mut event_name = None;
    let mut explicit = false;
    let mut query = value.clone();
    if let Some(canonical) = word_sources::explicit_action(&value).filter(|v| v["type"] == "event")
    {
        event_name = canonical["event"].as_str().map(str::to_owned);
        query = canonical["option"].as_str().unwrap_or("").into();
        explicit = true;
    } else if let Some(hit) = regex(r"(?i)^(?:on\s+)?event\s*:\s*(.+)$").captures(&value) {
        query = hit[1].into();
        explicit = true;
    } else if let Some(hit) = regex(
        r"(?i)^(?:on\s+)?(?:the\s+)?(.+?)\s+event\s*:\s*(?:(?:choose|select|pick)\s*:?\s*)?(.+)$",
    )
    .captures(&value)
    {
        event_name = Some(hit[1].into());
        query = hit[2].into();
        explicit = true;
    } else if let Some(hit) =
        regex(r"(?i)^(.+?)\s*\(\s*event(?:\s*x\s*\d+)?\s*\)$").captures(&value)
    {
        query = hit[1].into();
        explicit = true;
    }
    query = unquote(&query).into();
    let ellipsis = regex(r"(?:…|\.{3})\s*$");
    let abbreviated = ellipsis.is_match(&query);
    query = ellipsis.replace(&query, "").trim().into();
    if (!explicit && !abbreviated) || query.is_empty() || query.contains('/') || query.contains(';')
    {
        return Ok(None);
    }
    let words = regex(r"\w+")
        .find_iter(&query)
        .map(|m| m.as_str())
        .collect::<Vec<_>>();
    if abbreviated
        && (words.len() < 2 || words.iter().map(|s| s.chars().count()).sum::<usize>() < 7)
    {
        return Ok(None);
    }
    let mut filters = json!({});
    if let Some(name) = event_name {
        let parents = index.matches(unquote(&name), &["event"], country, &Value::Null, false)?;
        if parents.len() != 1 {
            return Ok(None);
        }
        filters["event_id"] = parents[0]["id"].clone();
    }
    let options=index.entries(json!({"op":"matches","query":query,"kinds":["event_option"],"country":country,"filters":filters}))?;
    if options.len() != 1 {
        return Ok(None);
    }
    let option = &options[0];
    let parents=index.entries(json!({"op":"matches","query":option["event_id"],"kinds":["event"],"country":country,"explicit":true}))?;
    if parents.len() != 1 {
        return Ok(None);
    }
    let parent = &parents[0];
    if !explicit
        && words.len() < 4
        && (country.as_str().is_none_or(|s| s.is_empty())
            || !parent["identity_countries"]
                .as_array()
                .is_some_and(|a| a.contains(country)))
    {
        return Ok(None);
    }
    if !explicit {
        for kind in ["focus", "technology", "advisor", "decision", "idea"] {
            if !index
                .matches(&query, &[kind], country, &Value::Null, false)?
                .is_empty()
            {
                return Ok(None);
            }
        }
    }
    let params =
        json!({"category":"politics","event_id":parent["id"],"event_option_id":option["id"]});
    let probe = json!({"action":{"kind":"manual","params":params}});
    let bound = political_binding::bind(&probe, index, country, false)?;
    if bound["reason"] != "event_visible_option" {
        return Ok(None);
    }
    Ok(Some(
        json!({"params":bound["step"]["action"]["params"],"policies":policies}),
    ))
}
