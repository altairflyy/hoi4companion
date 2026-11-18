//! Conservative static recipient constraints. Unknown scopes never prove absence.
use crate::script_document::{blocks, entries, scalar};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub fn tag(s: &str) -> bool {
    s.len() == 3
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}
pub fn combine(op: &str, args: Vec<Value>) -> Value {
    let (identity, decisive) = if op == "all" {
        ("true", "false")
    } else {
        ("false", "true")
    };
    let mut kept = vec![];
    for arg in args {
        let children = if arg["op"] == op {
            arg["args"].as_array().cloned().unwrap_or_default()
        } else {
            vec![arg]
        };
        for child in children {
            if child["op"] == decisive {
                return json!({"op":decisive});
            }
            if child["op"] != identity && !kept.contains(&child) {
                kept.push(child);
            }
        }
    }
    match kept.len() {
        0 => json!({"op":identity}),
        1 => kept.remove(0),
        _ => json!({"op":op,"args":kept}),
    }
}
pub fn condition(node: &Value, op: &str, political: bool, depth: usize) -> Value {
    if node.get("entries").is_none() || depth > 32 {
        return json!({"op":"unknown"});
    }
    combine(op,entries(node).map(|(key,value)|{
        if (["tag","tag !="].contains(&key) || political && ["original_tag","original_tag !="].contains(&key)) && value.as_str().is_some_and(tag) {
            let item=json!({"op":if key.starts_with("original_tag") {"original_tag"}else{"tag"},"value":value});
            if key.ends_with(" !=") {json!({"op":"not","arg":item})} else {item}
        } else if !political && key=="always" && ["yes","no"].contains(&value.as_str().unwrap_or("")) {
            json!({"op":if value=="yes" {"true"}else{"false"}})
        } else if ["AND","OR"].contains(&key.to_uppercase().as_str()) && value.get("entries").is_some() {
            condition(value,if key.eq_ignore_ascii_case("AND") {"all"} else {"any"},political,depth+1)
        } else if key.eq_ignore_ascii_case("NOT") && value["entries"].as_array().is_some_and(|a|a.len()==1) {
            let inner=condition(value,"all",political,depth+1);
            match inner["op"].as_str().unwrap_or("") {"true"=>json!({"op":"false"}),"false"=>json!({"op":"true"}),"unknown"=>inner,_=>json!({"op":"not","arg":inner})}
        } else {json!({"op":"unknown"})}
    }).collect())
}
pub fn political(nodes: &[&Value]) -> Value {
    let args: Vec<_> = nodes
        .iter()
        .flat_map(|node| {
            ["allowed", "visible", "available"]
                .into_iter()
                .flat_map(move |key| blocks(node, key))
        })
        .map(|block| condition(block, "all", true, 0))
        .collect();
    if args.is_empty() {
        json!({"op":"unknown"})
    } else {
        combine("all", args)
    }
}
pub fn event(kind: &str, node: &Value) -> Value {
    if !["country_event", "news_event"].contains(&kind) {
        return json!({"op":"unknown"});
    }
    let args: Vec<_> = blocks(node, "trigger")
        .map(|b| condition(b, "all", false, 0))
        .collect();
    let result = if args.is_empty() {
        json!({"op":"unknown"})
    } else {
        combine("all", args)
    };
    if scalar(node, "major") == "yes" || kind == "news_event" {
        combine(
            "any",
            vec![
                result,
                combine(
                    "all",
                    blocks(node, "show_major")
                        .map(|b| condition(b, "all", false, 0))
                        .collect(),
                ),
            ],
        )
    } else {
        result
    }
}
pub fn identity(node: &Value) -> Vec<String> {
    fn visit(node: &Value, tags: &mut BTreeSet<String>, depth: usize) {
        if depth > 128 {
            return;
        }
        for (key, value) in entries(node) {
            if ["tag", "original_tag"].contains(&key) && value.as_str().is_some_and(tag) {
                tags.insert(value.as_str().unwrap().into());
            } else if key.eq_ignore_ascii_case("AND") {
                visit(value, tags, depth + 1);
            }
        }
    }
    let mut tags = BTreeSet::new();
    for block in blocks(node, "trigger") {
        visit(block, &mut tags, 0);
    }
    if tags.len() == 1 {
        tags.into_iter().collect()
    } else {
        vec![]
    }
}
pub fn origins(node: &Value) -> Option<BTreeSet<String>> {
    if node["op"] == "original_tag" {
        return Some(BTreeSet::from([node["value"].as_str()?.into()]));
    }
    let values: Vec<_> = node["args"]
        .as_array()
        .into_iter()
        .flatten()
        .map(origins)
        .collect();
    if node["op"] == "all" {
        let mut known = values.into_iter().flatten();
        let mut result = known.next()?;
        for v in known {
            result.retain(|x| v.contains(x));
        }
        Some(result)
    } else if node["op"] == "any" && !values.is_empty() && values.iter().all(Option::is_some) {
        Some(values.into_iter().flatten().flatten().collect())
    } else {
        None
    }
}
/// Countries an event option's own `trigger` blocks certainly require to be
/// played by a human (`is_ai = no`) or by the AI (`is_ai = yes`):
/// `{"human":[..],"ai":[..]}`, "ROOT" standing for the country receiving the
/// event. Only tests reached from the trigger root through AND blocks and
/// country-tag or ROOT scopes are read, so each one is a necessary condition;
/// OR, NOT, `if`, other scopes and any other value add nothing. Null when
/// nothing is certain.
pub fn players(triggers: &[&Value]) -> Value {
    fn visit(node: &Value, scope: &str, human: &mut BTreeSet<String>, ai: &mut BTreeSet<String>, depth: usize) {
        if depth > 32 {
            return;
        }
        for (key, value) in entries(node) {
            if key == "is_ai" {
                match value.as_str() {
                    Some("no") => human.insert(scope.into()),
                    Some("yes") => ai.insert(scope.into()),
                    _ => false,
                };
            } else if value.get("entries").is_none() {
                continue;
            } else if key.eq_ignore_ascii_case("AND") {
                visit(value, scope, human, ai, depth + 1);
            } else if key.eq_ignore_ascii_case("ROOT") {
                visit(value, "ROOT", human, ai, depth + 1);
            } else if tag(key) && key.as_bytes()[0].is_ascii_uppercase() && !key.eq_ignore_ascii_case("NOT") {
                visit(value, key, human, ai, depth + 1);
            }
        }
    }
    let (mut human, mut ai) = (BTreeSet::new(), BTreeSet::new());
    for trigger in triggers {
        visit(trigger, "ROOT", &mut human, &mut ai, 0);
    }
    if human.is_empty() && ai.is_empty() {
        Value::Null
    } else {
        json!({"human":human,"ai":ai})
    }
}

#[cfg(test)]
mod tests {
    use super::players;
    use serde_json::{Value, json};

    /// A parsed script block from `[key, value]` pairs.
    fn block(pairs: Value) -> Value {
        json!({"entries": pairs})
    }

    #[test]
    fn players_reads_is_ai_only_through_and_and_country_scopes() {
        // poland.2102.b: another nation must be human-played.
        let danzig = block(json!([["DNZ", block(json!([["is_ai", "no"]]))]]));
        assert_eq!(players(&[&danzig]), json!({"human":["DNZ"],"ai":[]}));
        // The receiving country itself, directly and through ROOT and AND.
        let own = block(json!([["is_ai", "yes"], ["AND", block(json!([["ROOT", block(json!([["is_ai", "no"]]))]]))]]));
        assert_eq!(players(&[&own]), json!({"human":["ROOT"],"ai":["ROOT"]}));
        // OR, NOT, other scopes and state ids are not certain requirements.
        let unsure = block(json!([
            ["OR", block(json!([["DNZ", block(json!([["is_ai", "no"]]))], ["has_war", "yes"]]))],
            ["NOT", block(json!([["is_ai", "yes"]]))],
            ["FROM", block(json!([["is_ai", "no"]]))],
            ["807", block(json!([["is_ai", "no"]]))],
            ["is_ai", "maybe"],
        ]));
        assert_eq!(players(&[&unsure]), Value::Null);
        assert_eq!(players(&[]), Value::Null);
    }
}
