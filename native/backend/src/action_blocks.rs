//! Stable source action ordinals. Presentation metadata, never implicit gates.
use crate::import_catalog::regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub fn annotate(guide: &Value) -> Result<Value, String> {
    let mut guide = guide.clone();
    let mut groups: BTreeMap<(u64, String), Vec<(String, usize)>> = BTreeMap::new();
    let tracks = guide["tracks"].as_object().ok_or("Missing guide tracks")?;
    for (track, lane) in tracks {
        for (i, row) in lane.as_array().into_iter().flatten().enumerate() {
            if ["select_focus", "research", "free"]
                .contains(&row["action"]["kind"].as_str().unwrap_or(""))
            {
                continue;
            }
            if let Some(hit) = regex(r"^(word-(\d+))(?:(?:--operation-|--note-)\d+)*$")
                .captures(row["key"].as_str().unwrap_or(""))
            {
                let ordinal = hit[2]
                    .parse::<u64>()
                    .map_err(|_| "Invalid Word action ordinal")?;
                groups
                    .entry((ordinal, hit[1].into()))
                    .or_default()
                    .push((track.clone(), i));
            }
        }
    }
    let sources = guide["sources"].as_array().cloned().unwrap_or_default();
    let mut changed = vec![];
    for source in sources.iter().filter(|s| s["type"] == "file") {
        let text = source["text"].as_str().unwrap_or("");
        let starts = regex(r"\[Column 1\]\n").find_iter(text).collect::<Vec<_>>();
        let stop = regex(r"\[Column [23]\]");
        let columns = starts
            .iter()
            .map(|hit| {
                let tail = &text[hit.end()..];
                &tail[..stop.find(tail).map(|m| m.start()).unwrap_or(tail.len())]
            })
            .collect::<Vec<_>>();
        if columns.is_empty() {
            continue;
        }
        let mut ordinal = 0;
        for ((_, key), rows) in &groups {
            let quotes = rows
                .iter()
                .flat_map(|(track, i)| {
                    guide["tracks"][track][*i]["evidence"]
                        .as_array()
                        .into_iter()
                        .flatten()
                })
                .filter(|e| e["source_id"] == source["id"])
                .filter_map(|e| e["quote"].as_str().filter(|s| !s.is_empty()))
                .collect::<BTreeSet<_>>();
            if quotes.len() != 1 {
                continue;
            }
            let quote = *quotes.first().unwrap();
            if !columns.iter().any(|column| {
                column.match_indices(quote).any(|(i, _)| {
                    (i == 0 || column.as_bytes()[i - 1] == b'\n')
                        && (i + quote.len() == column.len()
                            || column.as_bytes()[i + quote.len()] == b'\n')
                })
            }) {
                continue;
            }
            ordinal += 1;
            let position = json!({"source_id":source["id"],"key":key,"ordinal":ordinal});
            for (track, i) in rows {
                let row = &mut guide["tracks"][track][*i];
                if row["action"].get("params").is_none() {
                    row["action"]["params"] = json!({});
                }
                if row["action"]["params"].get("source_action").is_some() {
                    continue;
                }
                row["action"]["params"]["source_action"] = position.clone();
                changed.push(json!({"key":row["key"],"actions":0,"step_keys":[row["key"]],"unverifiable_timing":false}));
            }
        }
    }
    Ok(json!({"guide":guide,"changes":changed}))
}
