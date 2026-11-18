//! Static source predicates. Unsupported or unqualified triggers stay unknown.
use crate::script_document::{entries, first};
use serde_json::{Value, json};
fn unknown(key: &str) -> Value {
    json!({"op":"unknown","trigger":key})
}
fn compile(
    node: &Value,
    depth: usize,
    budget: &mut usize,
    scripts: &Value,
    dlcs: &Value,
    stack: &mut Vec<String>,
) -> Value {
    if node.get("entries").is_none() {
        return unknown("invalid_condition_block");
    }
    if depth > 20 || *budget >= 768 {
        return unknown("criteria_processing_limit");
    }
    let mut parts = vec![];
    for (key, value) in entries(node) {
        *budget += 1;
        if *budget >= 768 {
            parts.push(unknown("criteria_processing_limit"));
            break;
        }
        let mut name = key;
        let mut operator = "=";
        for op in [">=", "<=", "!=", ">", "<", "="] {
            if let Some(prefix) = key.strip_suffix(op)
                && prefix.ends_with(char::is_whitespace)
            {
                name = prefix.trim_end();
                operator = op;
                break;
            }
        }
        let upper = name.to_uppercase();
        let text = value.as_str();
        let part = if name == "always" && [Some("yes"), Some("no")].contains(&text) {
            json!({"op":"const","value":text==Some("yes")})
        } else if ["AND", "OR", "NOT", "NAND", "NOR"].contains(&upper.as_str())
            && value.get("entries").is_some()
        {
            let mut children = vec![];
            for (k, v) in entries(value) {
                children.push(compile(
                    &json!({"entries":[[k,v]]}),
                    depth + 1,
                    budget,
                    scripts,
                    dlcs,
                    stack,
                ));
                if *budget >= 768 {
                    break;
                }
            }
            if upper == "NOT" && children.len() != 1 {
                unknown("NOT with multiple predicates")
            } else {
                let group = json!({"op":if ["OR","NOR"].contains(&upper.as_str()){"any"}else{"all"},"args":children});
                if ["NOT", "NAND", "NOR"].contains(&upper.as_str()) {
                    json!({"op":"not","arg":if upper=="NOT"{children[0].clone()}else{group}})
                } else {
                    group
                }
            }
        } else if name == "custom_trigger_tooltip" && value.get("entries").is_some() {
            compile(
                &json!({"entries":entries(value).filter(|(k,_)|*k!="tooltip").map(|(k,v)|json!([k,v])).collect::<Vec<_>>()}),
                depth + 1,
                budget,
                scripts,
                dlcs,
                stack,
            )
        } else if upper == "IF" && first(value, "limit").is_some_and(|v| v.get("entries").is_some())
        {
            json!({"op":"if","condition":compile(first(value,"limit").unwrap(),depth+1,budget,scripts,dlcs,stack),"then":compile(&json!({"entries":entries(value).filter(|(k,_)|*k!="limit").map(|(k,v)|json!([k,v])).collect::<Vec<_>>()}),depth+1,budget,scripts,dlcs,stack)})
        } else if scripts.get(name).is_some()
            && [Some("yes"), Some("no")].contains(&text)
            && !stack.iter().any(|s| s == name)
            && stack.len() < 24
        {
            stack.push(name.into());
            let part = compile(&scripts[name], depth + 1, budget, scripts, dlcs, stack);
            stack.pop();
            if text == Some("no") {
                json!({"op":"not","arg":part})
            } else {
                part
            }
        } else if name == "has_dlc"
            && let Some(name) = text
            && let Some(enabled) = crate::source_eligibility::dlc_enabled(dlcs, name)
        {
            json!({"op":"const","value":enabled})
        } else {
            let number = match name {
                "has_war_support" => Some("country.metrics.war_support"),
                "has_stability" => Some("country.metrics.stability"),
                "political_power" => Some("country.politics.political_power"),
                "has_army_experience" => Some("country.experience.army"),
                "has_navy_experience" => Some("country.experience.navy"),
                "has_air_experience" => Some("country.experience.air"),
                _ => None,
            };
            let member = match name {
                "has_completed_focus" => Some("focus.completed"),
                "has_tech" => Some("research.completed"),
                "has_idea" => Some("politics.ideas"),
                _ => None,
            };
            let boolean = match name {
                "has_war" => Some("country.diplomacy.at_war"),
                "is_subject" => Some("country.diplomacy.is_subject"),
                _ => None,
            };
            let word = match name {
                "tag" => Some("country"),
                "original_tag" => Some("country.original_tag"),
                "has_government" => Some("country.politics.government"),
                _ => None,
            };
            if let Some(fact) = number
                && text.is_some()
                && [">", "<", ">=", "<="].contains(&operator)
            {
                match crate::source_eligibility::number(text.unwrap()) {
                    Some(number) => {
                        json!({"op":"compare","fact":fact,"test":operator,"value":number,"trigger":name})
                    }
                    _ => unknown(key),
                }
            } else if let Some(fact) = member
                && text.is_some()
                && operator == "="
            {
                json!({"op":"compare","fact":fact,"test":"contains","value":text,"trigger":name})
            } else if let Some(fact) = boolean
                && [Some("yes"), Some("no")].contains(&text)
                && operator == "="
            {
                json!({"op":"compare","fact":fact,"test":"=","value":text==Some("yes"),"trigger":name})
            } else if let Some(fact) = word
                && text.is_some()
                && ["=", "!="].contains(&operator)
            {
                json!({"op":"compare","fact":fact,"test":operator,"value":text,"trigger":name})
            } else {
                let mut part = unknown(key);
                if text.is_some() {
                    part["value"] = value.clone();
                }
                part
            }
        };
        parts.push(part);
    }
    json!({"op":"all","args":parts})
}
pub fn rules(node: &Value) -> Value {
    let empty = json!({"entries":[]});
    json!({"schema_version":1,"available":compile(first(node,"available").unwrap_or(&empty),0,&mut 0,&Value::Null,&Value::Null,&mut vec![]),"branch":compile(first(node,"allow_branch").unwrap_or(&empty),0,&mut 0,&Value::Null,&Value::Null,&mut vec![])})
}
/// Reuse the focus predicate compiler for operational definitions. Missing
/// source constraints are not a universal eligibility proof.
pub fn operational(nodes: &[&Value]) -> Value {
    let parts=nodes.iter().flat_map(|node| ["allowed","available"].into_iter().filter_map(move |key| first(node,key))).map(|n|compile(n,0,&mut 0,&Value::Null,&Value::Null,&mut vec![])).collect::<Vec<_>>();
    if parts.is_empty() { Value::Null } else { json!({"schema_version":1,"available":{"op":"all","args":parts},"branch":{"op":"const","value":true}}) }
}
pub fn expand(rules: &Value, scripts: &Value, dlcs: &Value) -> Value {
    fn visit(
        node: &Value,
        scripts: &Value,
        dlcs: &Value,
        depth: usize,
        budget: &mut usize,
    ) -> Value {
        *budget += 1;
        if *budget >= 768 || depth > 20 {
            return unknown("criteria_processing_limit");
        }
        if node["op"] == "unknown"
            && (scripts
                .get(node["trigger"].as_str().unwrap_or(""))
                .is_some()
                || node["trigger"] == "has_dlc")
            && node.get("value").is_some()
        {
            return compile(
                &json!({"entries":[[node["trigger"],node["value"]]]}),
                depth,
                budget,
                scripts,
                dlcs,
                &mut vec![],
            );
        }
        let mut result = node.clone();
        if let Some(args) = node["args"].as_array() {
            result["args"] = json!(
                args.iter()
                    .map(|n| visit(n, scripts, dlcs, depth + 1, budget))
                    .collect::<Vec<_>>()
            );
        }
        for field in ["arg", "condition", "then"] {
            if let Some(child) = node.get(field) {
                result[field] = visit(child, scripts, dlcs, depth + 1, budget);
            }
        }
        result
    }
    let mut result = rules.clone();
    let mut budget = 0;
    result["available"] = visit(&rules["available"], scripts, dlcs, 0, &mut budget);
    result["branch"] = visit(&rules["branch"], scripts, dlcs, 0, &mut budget);
    result
}
