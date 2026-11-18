//! Source section phase hints, never a reconstruction of past production orders.
use crate::import_catalog::regex;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
struct Section<'a> {
    start: usize,
    body: &'a str,
    columns: Map<String, Value>,
    members: Vec<Value>,
    previous: Vec<Value>,
}
pub fn annotate(guide: &Value) -> Result<Value, String> {
    let mut guide = guide.clone();
    let tracks = guide["tracks"].as_object().ok_or("Missing guide tracks")?;
    let focuses = tracks
        .values()
        .flat_map(arr)
        .filter(|r| r["action"]["kind"] == "select_focus")
        .cloned()
        .collect::<Vec<_>>();
    let sources = arr(&guide["sources"])
        .iter()
        .filter(|r| r["type"] == "file" && r["text"].is_string())
        .map(|r| (s(&r["id"]).to_owned(), r.clone()))
        .collect::<Map<_, _>>();
    let mut source_sections = std::collections::HashMap::new();
    for (id, source) in &sources {
        let text = s(&source["text"]);
        let starts = text
            .match_indices("[Table row]")
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        let mut previous = vec![];
        let mut sections = vec![];
        for (i, start) in starts.iter().enumerate() {
            let end = starts.get(i + 1).copied().unwrap_or(text.len());
            let body = &text[start + "[Table row]".len()..end];
            let tags = regex(r"\[Column (\d+)\]\s*")
                .captures_iter(body)
                .collect::<Vec<_>>();
            let mut columns = Map::new();
            for (j, hit) in tags.iter().enumerate() {
                let end = tags
                    .get(j + 1)
                    .map(|m| m.get(0).unwrap().start())
                    .unwrap_or(body.len());
                columns.insert(hit[1].into(), json!(&body[hit.get(0).unwrap().end()..end]));
            }
            if !["1", "2", "3"].iter().all(|k| columns.contains_key(*k)) {
                continue;
            }
            let members = focuses
                .iter()
                .filter(|f| {
                    arr(&f["evidence"]).iter().any(|e| {
                        e["source_id"] == *id
                            && !s(&e["quote"]).trim().is_empty()
                            && s(&columns["3"]).contains(s(&e["quote"]).trim())
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            sections.push(Section {
                start: *start,
                body,
                columns,
                members: members.clone(),
                previous: previous.clone(),
            });
            if !members.is_empty() {
                previous = members;
            }
        }
        source_sections.insert(id.clone(), sections);
    }
    let mut changed = vec![];
    for row in guide["tracks"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .flat_map(|v| v.as_array_mut().into_iter().flatten())
    {
        let params = &row["action"]["params"];
        if params["category"] != "production" || params.get("production_revision").is_some() {
            continue;
        }
        for evidence in arr(&row["evidence"]).to_vec() {
            let Some(source) = sources.get(s(&evidence["source_id"])) else {
                continue;
            };
            let quote = s(&evidence["quote"]);
            if quote.is_empty() {
                continue;
            }
            let sections = &source_sections[s(&evidence["source_id"])];
            let candidates = sections
                .iter()
                .filter(|v| s(&v.columns["1"]).contains(quote))
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                continue;
            }
            let section = candidates[0];
            let fallback = vec![params.clone()];
            let targets = params
                .get("production_targets")
                .map(arr)
                .unwrap_or(&fallback);
            let categories = targets
                .iter()
                .map(|t| t["production_category"].to_string())
                .collect::<HashSet<_>>();
            if categories.len() != 1 {
                continue;
            }
            let category = &targets[0]["production_category"];
            if !["military", "naval"].contains(&s(category)) {
                continue;
            }
            let instruction = arr(&params["instructions"])
                .iter()
                .map(s)
                .collect::<Vec<_>>()
                .join("\n");
            let offset = quote.find(&instruction).unwrap_or(0);
            let column = s(&section.columns["1"]);
            let order = s(&source["text"])[..section.start].chars().count()
                + section.body[..section.body.find(column).unwrap_or(0)]
                    .chars()
                    .count()
                + column[..column.find(quote).unwrap_or(0)].chars().count()
                + quote[..offset].chars().count();
            let revision =
                json!({"group":format!("{}:{}",s(&source["id"]),s(category)),"order":order});
            let mut boundary = None;
            if let Some(last) = section.previous.last() {
                let mut facts = vec![
                    json!({"op":"fact","key":"focus.completed","test":"contains","value":last["action"]["entity"]["id"]}),
                ];
                for focus in &section.members {
                    for key in ["focus.active", "focus.completed"] {
                        facts.push(json!({"op":"fact","key":key,"test":"contains","value":focus["action"]["entity"]["id"]}));
                    }
                }
                while facts.len() > 32 {
                    facts = facts
                        .chunks(32)
                        .map(|c| json!({"op":"any","args":c}))
                        .collect();
                }
                boundary = Some(json!({"op":"any","args":facts}));
            }
            row["action"]["params"]["production_revision"] = revision;
            if let Some(boundary) = boundary {
                row["action"]["params"]["production_section_when"] = boundary;
            }
            changed.push(json!({"key":row["key"],"actions":0,"step_keys":[row["key"]],"unverifiable_timing":false}));
            break;
        }
    }
    Ok(json!({"guide":guide,"changes":changed}))
}
