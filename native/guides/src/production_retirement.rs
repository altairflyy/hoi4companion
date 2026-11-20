//! Explicit product replacements written by the author ("Change I-16 to F1ca").
//!
//! A later line of the same track that names an earlier production target as
//! the thing to change marks that target `retired_by` the line's step. The
//! Rust evaluation retires it (superseded, never completed) only once that
//! line is itself completed. Names must match exactly after normalization;
//! nothing is guessed and no new production target is created.
use crate::import_catalog::regex;
use serde_json::{Value, json};

fn normal(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}
/// The product the line says to change away from, normalized.
fn replaced(step: &Value) -> Option<String> {
    let mut lines: Vec<&str> = step["action"]["params"]["instructions"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    lines.extend(step["title"].as_str());
    let pattern = regex(r"(?i)^\s*(?:change|switch|replace|convert)\s+(.+?)\s+(?:to|with|into|for)\s+\S");
    lines.iter().find_map(|line| pattern.captures(line).map(|c| normal(&c[1]))).filter(|name| !name.is_empty())
}
fn names(target: &Value) -> Vec<String> {
    let mut names: Vec<String> = ["display_name", "equipment_name"].iter().filter_map(|k| target[*k].as_str()).map(normal).collect();
    names.extend(target["production_selector"]["variant_names"].as_array().into_iter().flatten().filter_map(Value::as_str).map(normal));
    names
}
pub fn annotate(guide: &mut Value) {
    let Some(tracks) = guide["tracks"].as_object_mut() else { return };
    for rows in tracks.values_mut().filter_map(Value::as_array_mut) {
        for later in 0..rows.len() {
            let Some(name) = replaced(&rows[later]) else { continue };
            let by = rows[later]["key"].clone();
            for row in rows[..later].iter_mut() {
                let Some(params) = row.get_mut("action").and_then(|a| a.get_mut("params")) else { continue };
                if params["category"] != "production" {
                    continue;
                }
                // `get_mut`: indexing a missing key would insert a null.
                let targets: Vec<&mut Value> = if params.get("production_targets").is_some_and(Value::is_array) {
                    params.get_mut("production_targets").and_then(Value::as_array_mut).unwrap().iter_mut().collect()
                } else {
                    vec![params]
                };
                for target in targets {
                    if target.get("retired_by").is_none() && names(target).contains(&name) {
                        target["retired_by"] = json!(by);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_later_change_line_retires_only_the_named_earlier_target() {
        let target = |name: &str| json!({"category":"production","display_name":name,"equipment_name":name,"production_selector":{"variant_names":[name]},"requested_factories":5});
        let mut guide = json!({"tracks":{"operations":[
            {"key":"prod","action":{"kind":"manual","params":{"category":"production","production_targets":[target("Sup Equip"),target("I-16")]}}},
            {"key":"change","title":"Change I-16 to F1ca (12AE), production to 15","action":{"kind":"manual","params":{"category":"checkpoints","instructions":["Change I-16 to F1ca (12AE), production to 15"]}}},
            {"key":"later","action":{"kind":"manual","params":{"category":"production","production_targets":[target("I-16")]}}}
        ]}});
        annotate(&mut guide);
        let rows = &guide["tracks"]["operations"];
        assert!(rows[0]["action"]["params"]["production_targets"][0].get("retired_by").is_none());
        assert_eq!(rows[0]["action"]["params"]["production_targets"][1]["retired_by"], "change");
        // A target written after the change line is a new order, not retired.
        assert!(rows[2]["action"]["params"]["production_targets"][0].get("retired_by").is_none());
    }
    #[test]
    fn unmatched_or_unrelated_lines_retire_nothing() {
        let mut guide = json!({"tracks":{"operations":[
            {"key":"prod","action":{"kind":"manual","params":{"category":"production","display_name":"I-16","equipment_name":"I-16"}}},
            {"key":"a","title":"Change I-153 to F1ca","action":{"kind":"manual","params":{"category":"checkpoints"}}},
            {"key":"b","title":"Build I-16 to the max","action":{"kind":"manual","params":{"category":"checkpoints"}}}
        ]}});
        let before = guide.clone();
        annotate(&mut guide);
        assert_eq!(guide, before);
    }
}
