//! Shared bounded source-predicate validation for metadata and runtime.
use serde_json::Value;
fn text(v:&Value)->&str {v.as_str().unwrap_or("")}
fn keys(v:&Value,allowed:&[&str],required:&[&str])->bool {v.as_object().is_some_and(|m|m.keys().all(|k|allowed.contains(&k.as_str()))&&required.iter().all(|k|m.contains_key(*k)))}
pub fn validate(node: &Value, depth: usize, budget: &mut usize) -> Result<(),String> {
    *budget += 1;
    if depth > 32 || *budget > 4096 || !node.is_object() {
        return Err("Invalid focus criterion".into());
    }
    match text(&node["op"]) {
        "all" | "any" => {
            if !keys(node, &["op", "args"], &["op", "args"]) || !node["args"].is_array() {
                return Err("Invalid focus criteria group".into());
            }
            for child in node["args"].as_array().unwrap() {
                validate(child, depth + 1, budget)?
            }
        }
        "not" => {
            if !keys(node, &["op", "arg"], &["op", "arg"]) {
                return Err("Invalid focus criteria negation".into());
            }
            validate(&node["arg"], depth + 1, budget)?
        }
        "if" => {
            if !keys(
                node,
                &["op", "condition", "then"],
                &["op", "condition", "then"],
            ) {
                return Err("Invalid focus criteria implication".into());
            }
            validate(&node["condition"], depth + 1, budget)?;
            validate(&node["then"], depth + 1, budget)?
        }
        "const" => {
            if !keys(node, &["op", "value"], &["op", "value"]) || !node["value"].is_boolean() {
                return Err("Invalid focus constant".into());
            }
        }
        "unknown" => {
            if !keys(node, &["op", "trigger", "value"], &[])
                || !node["trigger"].is_string()
                || node.get("value").is_some_and(|v| !v.is_string())
            {
                return Err("Invalid unknown focus predicate".into());
            }
        }
        "compare" => {
            let test = text(&node["test"]);
            let value = &node["value"];
            if !keys(
                node,
                &["op", "fact", "test", "value", "trigger"],
                &["op", "fact", "test", "value", "trigger"],
            ) || !node["fact"].is_string()
                || ![">", "<", ">=", "<=", "=", "!=", "contains"].contains(&test)
                || !(value.is_string() || value.is_boolean() || value.is_number())
                || [">", "<", ">=", "<="].contains(&test) && !value.is_number()
                || test == "contains" && !value.is_string()
                || !node["trigger"].is_string()
            {
                return Err("Invalid focus comparison".into());
            }
        }
        _ => return Err("Unsupported focus criteria operator".into()),
    }
    Ok(())
}
