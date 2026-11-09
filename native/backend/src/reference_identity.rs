//! Literal design references and recipes. Images never constitute completion.
use crate::import_catalog::{Index, regex};
use hoi4companion_catalog_core::{casefold, nfkd, unicode_alnum};
use serde_json::{Value, json};
use std::collections::BTreeSet;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn name_key(value: &str) -> String {
    casefold(&nfkd(value))
        .chars()
        .filter(|c| unicode_alnum(*c) || *c == '+')
        .collect()
}
fn base_name(value: &str) -> &str {
    value.split('(').next().unwrap_or("").trim()
}
fn py_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(v) => if *v { "True" } else { "False" }.into(),
        _ => value.to_string(),
    }
}
pub fn design_matches(query: &str, designs: &[Value], upgrades: &Value) -> Vec<Value> {
    let needle = name_key(query);
    let mut rows = designs
        .iter()
        .filter(|r| name_key(s(&r["name"])) == needle)
        .cloned()
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows = designs
            .iter()
            .filter(|r| name_key(base_name(s(&r["name"]))) == needle)
            .cloned()
            .collect();
    }
    if rows.is_empty() {
        let pattern = regex(r"^(.+?)\s*(\([^()]+\))$");
        if let Some(qualified) = pattern.captures(query.trim())
            && !regex(r"\d$").is_match(&name_key(&qualified[1]))
        {
            for row in designs {
                if let Some(candidate) = pattern.captures(s(&row["name"]).trim())
                    && name_key(&candidate[2]) == name_key(&qualified[2])
                    && regex(r"\d+$").replace(&name_key(&candidate[1]), "")
                        == name_key(&qualified[1])
                {
                    rows.push(row.clone());
                }
            }
        }
    }
    if let Some(upgrades) = upgrades.as_object().filter(|m| !m.is_empty()) {
        rows.retain(|row| {
            upgrades
                .iter()
                .all(|(k, v)| row["upgrades"][k].as_str() == Some(py_string(v).as_str()))
        });
    }
    rows
}
fn compatible(index: &Index, rows: Vec<Value>) -> Result<Vec<Value>, String> {
    rows.into_iter()
        .map(|row| {
            let query = s(&row["equipment_id"]);
            let found = index.entries(
                json!({"op":"matches","query":query,"kinds":["equipment"],"explicit":true}),
            )?;
            Ok((!found.is_empty()).then_some(row))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|rows| rows.into_iter().flatten().collect())
}
fn recipe(row: &Value) -> Value {
    json!({"id":row["id"],"name":row["name"],"modules":row["modules"],"upgrades":row["upgrades"]})
}
fn selection(row: &Value, query: &str, reference: Value) -> Value {
    json!({"equipment_ids":[row["equipment_id"]],"variant_names":[query,row["name"]],"category":if row["category"]=="naval"{"naval"}else{"military"},"reference":reference})
}
pub fn bindings(guide: &Value, index: &Index, library: &Value) -> Result<Value, String> {
    let designs = arr(&library["designs"]);
    let images = arr(&library["images"]);
    let mut result = json!({});
    if designs.is_empty() {
        return Ok(result);
    }
    let text = guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(arr)
        .flat_map(|row| arr(&row["action"]["params"]["instructions"]))
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    for line in text.lines() {
        let re = regex(r"(?i)\bdesign(?:\s+and\s+produce)?\s+(.+)");
        let Some(hit) = re.captures(line) else {
            continue;
        };
        for part in hit[1].split(',') {
            let pattern = regex(r"^\s*([^():]+?)\s*(?:\(\s*(\d+)\s*/\s*(\d+)\s*\))?\s*$");
            let Some(m) = pattern.captures(part) else {
                continue;
            };
            let name = m[1].trim();
            let upgrades=m.get(2).map(|v|json!({"tank_nsb_engine_upgrade":v.as_str(),"tank_nsb_armor_upgrade":&m[3]})).unwrap_or(Value::Null);
            let mut rows = design_matches(name, designs, &upgrades);
            if rows.is_empty() {
                for (short, label) in index.vocabulary()["abbreviations"]["designs"]
                    .as_object()
                    .into_iter()
                    .flatten()
                {
                    let label = s(label);
                    let long = label.strip_suffix(" tank").unwrap_or(label);
                    if name.to_lowercase().starts_with(&long.to_lowercase()) {
                        let Some(suffix) = name.get(long.len()..) else {
                            continue;
                        };
                        rows.extend(design_matches(
                            &format!("{short}{suffix}"),
                            designs,
                            &upgrades,
                        ));
                    }
                }
            }
            let compatible = compatible(index, rows)?;
            if compatible.len() != 1 {
                continue;
            }
            let row = &compatible[0];
            let mut reference = recipe(row);
            let mut keys = BTreeSet::from([
                name_key(base_name(s(&row["name"]))),
                name_key(s(&row["name"])),
            ]);
            if let Some(qualifier) =
                regex(r"^(.+?)\s*\(\d+/\d+/([A-Za-z]+)\)$").captures(s(&row["name"]))
            {
                keys.insert(name_key(&format!("{}{}", &qualifier[1], &qualifier[2])));
            }
            let candidates = images
                .iter()
                .filter(|image| keys.contains(&name_key(s(&image["name"]))))
                .collect::<Vec<_>>();
            if candidates.len() == 1 {
                reference["image_id"] = candidates[0]["id"].clone();
            }
            let item = selection(row, name, reference);
            let key = name_key(name);
            if result.get(&key).is_none() {
                result[&key] = json!([]);
            }
            let values = result[&key].as_array_mut().unwrap();
            if !values.contains(&item) {
                values.push(item);
            }
        }
    }
    Ok(result)
}
pub fn selector(
    query: &str,
    index: &Index,
    library: &Value,
    declared: &Value,
) -> Result<Value, String> {
    let key = name_key(query);
    if declared[&key].as_array().is_some_and(|a| !a.is_empty()) {
        return Ok(declared[&key].clone());
    }
    Ok(json!(
        compatible(
            index,
            design_matches(query, arr(&library["designs"]), &Value::Null)
        )?
        .iter()
        .map(|row| selection(row, query, recipe(row)))
        .collect::<Vec<_>>()
    ))
}
pub fn attach_images(guide: &Value, library: &Value) -> Result<Value, String> {
    let images = arr(&library["images"]);
    let mut guide = guide.clone();
    let mut changed = vec![];
    fn word(c: char) -> bool {
        unicode_alnum(c) || c == '_' || c == '+'
    }
    for row in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
        .flat_map(|v| v.as_array_mut().into_iter().flatten())
    {
        let text = arr(&row["action"]["params"]["instructions"])
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        let found = images
            .iter()
            .filter(|image| {
                let name = s(&image["name"]);
                !name.is_empty()
                    && text.match_indices(name).any(|(i, _)| {
                        text[..i].chars().next_back().is_none_or(|c| !word(c))
                            && text[i + name.len()..]
                                .chars()
                                .next()
                                .is_none_or(|c| !word(c))
                    })
            })
            .map(|image| json!({"id":image["id"],"name":image["name"]}))
            .take(8)
            .collect::<Vec<_>>();
        if !found.is_empty() && row["action"]["params"]["reference_images"] != json!(found) {
            row["action"]["params"]["reference_images"] = json!(found);
            changed.push(json!({"key":row["key"],"actions":0,"step_keys":[row["key"]],"unverifiable_timing":false}));
        }
    }
    Ok(json!({"guide":guide,"changes":changed}))
}
pub fn execute(op: &str, args: &Value) -> Result<Value, String> {
    match op {
        "reference_matches" => Ok(json!(design_matches(
            s(&args["query"]),
            arr(&args["designs"]),
            &args["upgrades"]
        ))),
        "reference_bindings" => bindings(&args["guide"], &Index::new(args)?, &args["library"]),
        "reference_selector" => selector(
            s(&args["query"]),
            &Index::new(args)?,
            &args["library"],
            &args["declared"],
        ),
        "reference_attach_images" => attach_images(&args["guide"], &args["library"]),
        _ => Err("Unknown reference identity operation".into()),
    }
}
