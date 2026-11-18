//! Source-order adoption anchors, distinct from time and current allocations.
use crate::{guide_prepare, import_catalog::regex};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.),
    }
}
fn eligible(step: &Value) -> bool {
    let node = &step["completion"];
    step["tracking"] != "reminder"
        && node["op"] == "fact"
        && [
            "politics.ideas",
            "politics.advisors",
            "research.completed",
            "focus.completed",
            "projects.completed",
            "doctrines.selected",
        ]
        .contains(&s(&node["key"]))
        && node["test"] == "contains"
        && node["value"].as_str().is_some_and(|s| !s.is_empty())
        && !["when", "skip_when", "depends_on", "depends_on_started"]
            .iter()
            .any(|k| truth(&step[*k]))
}
fn signature(value: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(guide_prepare::canonical(value).to_string().as_bytes())
    )
}
fn columns(text: &str) -> Vec<(usize, &str)> {
    let starts = text
        .match_indices("[Table row]")
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let mut result = vec![];
    for (i, start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(text.len());
        let body = &text[start + "[Table row]".len()..end];
        let pattern = regex(r"\[Column (\d+)\]\s*");
        let tags = pattern.captures_iter(body).collect::<Vec<_>>();
        let mut values = Map::new();
        for (j, hit) in tags.iter().enumerate() {
            let end = tags.get(j + 1).unwrap_or(hit).get(0).unwrap().start();
            let end = if j + 1 == tags.len() { body.len() } else { end };
            values.insert(hit[1].into(), json!(&body[hit.get(0).unwrap().end()..end]));
        }
        if ["1", "2", "3"].iter().all(|k| values.contains_key(*k)) {
            let tag = tags
                .iter()
                .enumerate()
                .rev()
                .find(|(_, h)| &h[1] == "1")
                .unwrap();
            let end = tags
                .get(tag.0 + 1)
                .map(|h| h.get(0).unwrap().start())
                .unwrap_or(body.len());
            result.push((*start, &body[tag.1.get(0).unwrap().end()..end]));
        }
    }
    result
}
pub fn annotate(guide: &mut Value) -> Result<Vec<Value>, String> {
    if guide["dependency_policy"] != "explicit" {
        return Ok(vec![]);
    }
    let tracks = guide["tracks"].as_object().ok_or("Missing guide tracks")?;
    let mut rows = tracks
        .iter()
        .flat_map(|(track, lane)| {
            arr(lane)
                .iter()
                .enumerate()
                .map(move |(i, r)| (track.clone(), i, r.clone()))
        })
        .collect::<Vec<_>>();
    let sources = arr(&guide["sources"]).to_vec();
    let mut changed = vec![];
    for source in sources.iter().filter(|s| s["type"] == "file") {
        let columns = columns(s(&source["text"]));
        let mut located = vec![];
        for (i, (_, _, row)) in rows.iter().enumerate() {
            let instructions = arr(&row["action"]["params"]["instructions"]);
            if instructions.len() != 1 {
                continue;
            }
            let instruction = s(&instructions[0]).trim();
            if instruction.is_empty() {
                continue;
            }
            let mut positions = BTreeSet::new();
            for evidence in arr(&row["evidence"]) {
                let quote = s(&evidence["quote"]);
                if evidence["source_id"] != source["id"]
                    || quote.is_empty()
                    || quote.matches(instruction).count() != 1
                {
                    continue;
                }
                for (start, column) in &columns {
                    if column.matches(quote).count() == 1 {
                        positions.insert(
                            start + column.find(quote).unwrap() + quote.find(instruction).unwrap(),
                        );
                    }
                }
            }
            if positions.len() == 1 {
                located.push((*positions.first().unwrap(), i));
            }
        }
        located.sort_by_key(|(pos, _)| *pos);
        let mut anchors = Map::new();
        for (position, index) in &located {
            let row = &rows[*index].2;
            let p = &row["action"]["params"];
            if eligible(row) {
                let bucket = ["law_group", "role", "idea_group"]
                    .iter()
                    .map(|key| &p[*key])
                    .find(|v| truth(v))
                    .cloned()
                    .unwrap_or_else(|| {
                        if truth(&p["doctrine_domain"]) && truth(&p["doctrine_track"]) {
                            json!(format!(
                                "{}:{}",
                                s(&p["doctrine_domain"]),
                                s(&p["doctrine_track"])
                            ))
                        } else {
                            row["action"]["kind"].clone()
                        }
                    });
                anchors.insert(
                    json!([row["completion"]["key"], bucket]).to_string(),
                    json!(index),
                );
                continue;
            }
            if p["category"] != "production"
                || row["tracking"] == "reminder"
                || anchors.is_empty()
                || row.get("depends_on").is_some()
                || row.get("depends_on_started").is_some()
            {
                continue;
            }
            let previous = &p["production_milestones"];
            if !previous.is_null() && p["production_milestones_generated"] != signature(previous) {
                continue;
            }
            if located.iter().filter(|(p, _)| p == position).count() != 1 {
                continue;
            }
            let milestones = anchors
                .values()
                .map(|value| {
                    let anchor = &rows[value.as_u64().unwrap() as usize].2;
                    let condition = anchor["completion"].clone();
                    let token =
                        json!([anchor["key"], condition["key"], condition["value"]]).to_string();
                    json!({"step":anchor["key"],"token":token,"condition":condition})
                })
                .collect::<Vec<_>>();
            if milestones.len() > 64 || *previous == json!(milestones) {
                continue;
            }
            let row = &mut rows[*index].2;
            let milestones = json!(milestones);
            row["action"]["params"]["production_milestones_generated"] =
                json!(signature(&milestones));
            row["action"]["params"]["production_milestones"] = milestones;
            changed.push(json!({"key":row["key"],"actions":0,"step_keys":[row["key"]],"unverifiable_timing":false}));
        }
    }
    for (track, index, row) in rows {
        guide["tracks"][&track][index] = row;
    }
    Ok(changed)
}
