//! Bounded declarative timing compiler. Never reads the game or infers outcomes.
use crate::{
    import_catalog::{Index, regex},
    research_import,
};
use serde_json::{Value, json};
use std::collections::HashSet;
fn fact(key: &str, test: &str, value: Value) -> Value {
    json!({"op":"fact","key":key,"test":test,"value":value})
}
fn selected(target: &Value) -> Value {
    json!({"op":"any","args":[fact("focus.active","contains",target.clone()),fact("focus.completed","contains",target.clone())]})
}
fn after_war(text: &str) -> bool {
    [
        "after war",
        "after the war",
        "once war is over",
        "once the war is over",
        "when war is over",
        "when the war is over",
        "once the war ends",
        "when the war ends",
        "dopo la guerra",
        "dopo guerra",
        "quando la guerra finisce",
        "una volta finita la guerra",
    ]
    .contains(&text.to_lowercase().trim_matches([' ', '.', ':']))
}
fn experience(currency: &str) -> &str {
    match currency {
        "navy_experience" => "country.experience.navy",
        "air_experience" => "country.experience.air",
        _ => "country.experience.army",
    }
}
fn atom(
    text: &str,
    index: Option<&Index>,
    country: &Value,
    scope: &Value,
    currency: &str,
) -> Result<Option<Value>, String> {
    if let Some(hit)=regex(r"(?i)^(?:(?:on|after)\s+([A-Z0-9]{3})\s+down\s*/\s*)?no later than\s+([A-Za-z]+)\s+(\d{4}|\d{2})$").captures(text){
 let months=["january","february","march","april","may","june","july","august","september","october","november","december"];let month=months.iter().position(|m|hit[2].eq_ignore_ascii_case(m)||hit[2].eq_ignore_ascii_case(&m[..3]));let Some(month)=month else{return Ok(None)};let year=hit[3].parse::<u32>().unwrap_or(0)+if hit[3].len()==2{1900}else{0};if !(1..=9999).contains(&year){return Ok(None)}let days=[31,28,31,30,31,30,31,31,30,31,30,31];let date=fact("campaign.date","gte",json!(year*10000+(month as u32+1)*100+days[month]));return Ok(Some(if let Some(tag)=hit.get(1){json!({"op":"any","args":[fact(&format!("campaign.defeated.{}",tag.as_str().to_uppercase()),"eq",json!(true)),date]})}else{date}));}
    if after_war(text) {
        return Ok(Some(fact("guide.campaign.after_war", "eq", json!(true))));
    }
    if let Some(hit)=regex(r"(?i)^(?:at|when|once)\s+(\d{1,7}(?:\.\d{1,4})?)\s*(PP|political\s+power|AE|army\s+experience|NE|naval\s+experience|AFE|air\s+experience)$").captures(text){let unit=hit[2].split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();let key=match unit.as_str(){"pp"|"political power"=>"country.politics.political_power","ae"=>experience(currency),"ne"|"naval experience"=>experience("navy_experience"),"afe"|"air experience"=>experience("air_experience"),_=>experience("army_experience")};let amount=hit[1].parse::<f64>().map_err(|e|e.to_string())?;return Ok(Some(fact(key,"gte",if amount.fract()==0.{json!(amount as i64)}else{json!(amount)})));}
    let Some(index) = index else { return Ok(None) };
    let Some(hit) = regex(r"(?i)^(on|after|when|once|before)\s+(.+)$").captures(text) else {
        return Ok(None);
    };
    let prefix = hit[1].to_lowercase();
    let mut query = hit[2].to_owned();
    let mut explicit = None;
    let mut mode = if ["after", "once"].contains(&prefix.as_str()) {
        "completed"
    } else {
        "selected"
    };
    if let Some(hit) = regex(r"(?i)^(?:picking|selecting|starting)\s+").find(&query) {
        query = query[hit.end()..].into();
        explicit = Some("focus");
        mode = if prefix == "before" {
            "before"
        } else {
            "selected"
        };
    } else if prefix == "before" {
        return Ok(None);
    }
    if let Some(hit) = regex(r"(?i)\s+(?:is\s+)?(?:done|completed|finished)$").find(&query) {
        query = query[..hit.start()].trim().into();
        mode = "completed";
    }
    if let Some(hit) = regex(r"(?i)\s+(focus|research|tech|technology)$").captures(&query) {
        explicit = Some(if hit[1].eq_ignore_ascii_case("focus") {
            "focus"
        } else {
            "technology"
        });
        query = query[..hit.get(0).unwrap().start()].trim().into();
    }
    if let Some(hit) = regex(r"(?i)\s+(?:pick|selected|started)$").find(&query) {
        explicit = Some("focus");
        query = query[..hit.start()].trim().into();
        mode = "selected";
    }
    query = query.trim_matches([' ', '"', '“', '”', '\'']).into();
    if query.is_empty()
        || regex(r"(?i)[,;():]|\b(?:if|until|before|after|around|usually)\b").is_match(&query)
    {
        return Ok(None);
    }
    let kinds = explicit
        .map(|s| vec![s])
        .unwrap_or_else(|| vec!["focus", "technology"]);
    let mut candidates = vec![];
    for kind in &kinds {
        candidates.extend(index.matches(&query, &[kind], country, scope, false)?);
    }
    if candidates.is_empty() && kinds.contains(&"technology") {
        let expanded = research_import::expanded(&query, index.vocabulary());
        if expanded != query {
            candidates = index.matches(&expanded, &["technology"], country, scope, false)?;
        }
    }
    if candidates.len() != 1 {
        return Ok(None);
    }
    let entry = &candidates[0];
    Ok(Some(if entry["kind"] == "technology" {
        fact("research.completed", "contains", entry["id"].clone())
    } else if mode == "completed" {
        fact("focus.completed", "contains", entry["id"].clone())
    } else {
        let selected = selected(&entry["id"]);
        if mode == "before" {
            json!({"op":"not","arg":selected})
        } else {
            selected
        }
    }))
}
pub fn compile(
    text: &Value,
    index: Option<&Index>,
    country: &Value,
    scope: &Value,
    currency: &str,
    depth: usize,
) -> Result<Option<Value>, String> {
    let Some(text) = text
        .as_str()
        .filter(|s| s.chars().count() <= 1000 && depth <= 6)
    else {
        return Ok(None);
    };
    let text = text
        .trim()
        .trim_end_matches(':')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return Ok(None);
    }
    if let Some(atom) = atom(&text, index, country, scope, currency)? {
        return Ok(Some(atom));
    }
    let mut quote = false;
    let mut tokens = vec![];
    let mut start = 0;
    for hit in regex(r#"["“”]|\s+(and|or)\s+|;\s*"#).captures_iter(&text) {
        let m = hit.get(0).unwrap();
        if ["\"", "“", "”"].contains(&m.as_str()) {
            quote = !quote;
            continue;
        }
        if quote {
            continue;
        }
        tokens.push((
            text[start..m.start()].trim(),
            if hit
                .get(1)
                .is_some_and(|m| m.as_str().eq_ignore_ascii_case("or"))
            {
                "any"
            } else {
                "all"
            },
        ));
        start = m.end();
    }
    if tokens.is_empty()
        || tokens.len() > 15
        || tokens
            .iter()
            .map(|(_, op)| *op)
            .collect::<HashSet<_>>()
            .len()
            != 1
    {
        return Ok(None);
    }
    let op = tokens[0].1;
    let mut conditions = vec![];
    for part in tokens
        .iter()
        .map(|(p, _)| *p)
        .chain(std::iter::once(text[start..].trim()))
    {
        let Some(value) = compile(&json!(part), index, country, scope, currency, depth + 1)? else {
            return Ok(None);
        };
        conditions.push(value);
    }
    Ok(Some(json!({"op":op,"args":conditions})))
}
pub fn execute(args: &Value) -> Result<Value, String> {
    let index = if args.get("entries").is_some() || args.get("identity_handle").is_some() {
        Some(Index::new(args)?)
    } else {
        None
    };
    Ok(compile(
        &args["text"],
        index.as_ref(),
        &args["country"],
        &args["scope"],
        args["currency"].as_str().unwrap_or(""),
        args["depth"].as_u64().unwrap_or(0) as usize,
    )?
    .unwrap_or(Value::Null))
}
