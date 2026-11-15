//! Read-only diagnostic: replay the existing resolver, do not change admission.
use hoi4companion_catalog_core::{Catalog, normal_name};
use serde_json::{Value, json};
use std::{env, fs};

fn text(value: &Value) -> &str { value.as_str().unwrap_or("") }
fn strength(query: &str, entry: &Value, country: &str, vocabulary: &Value) -> &'static str {
    if query == text(&entry["id"]) { return "exact_id"; }
    let normalized = normal_name(query);
    if normalized.is_empty() { return "weak_or_unclassified"; }
    if normalized == normal_name(text(&entry["name"])) { return "exact_normalized_name"; }
    let labels = entry["aliases"].as_array().into_iter().flatten().filter_map(Value::as_str)
        .chain(entry["localized_name"].as_str())
        .chain(entry["id"].as_str())
        .chain(entry["country_aliases"][country].as_array().into_iter().flatten().filter_map(Value::as_str));
    if labels.into_iter().any(|label| !label.is_empty() && normal_name(label)==normalized) { return "exact_normalized_alias"; }
    if vocabulary["entity_names"][text(&entry["kind"])].as_object().is_some_and(|aliases| aliases.iter().any(|(alias,target)| {
        normal_name(alias)==normalized && (text(target)==text(&entry["id"]) || normal_name(text(target))==normal_name(text(&entry["name"])))
    })) { return "exact_vocabulary_alias"; }
    "weak_or_unclassified"
}
fn run() -> Result<(), String> {
    let args: Vec<_>=env::args().collect();
    if args.len()!=3 { return Err("Usage: binding_probe <input.json> <NEW-output.json>".into()); }
    let input: Value=serde_json::from_slice(&fs::read(&args[1]).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    let build=json!({"schema_version":1,"entries":input["entries"],"country_names":input["country_names"],"adjectives":input["adjectives"],"vocabulary":input["vocabulary"]});
    let catalog=Catalog::compile(serde_json::from_value(build).map_err(|e|e.to_string())?)?;
    let mut results=vec![];
    for probe in input["probes"].as_array().ok_or("Missing probes")? {
        let mut query=probe["request"].clone();query["schema_version"]=json!(1);query["op"]=json!("matches");
        let selected=catalog.query(serde_json::from_value(query.clone()).map_err(|e|e.to_string())?)?;
        let matches=selected.as_array().ok_or("Invalid resolver response")?.iter().map(|i| {
            let entry=&catalog.entries()[i.as_u64().unwrap() as usize];
            json!({"kind":entry["kind"],"id":entry["id"],"name":entry["name"],"slot":entry["slot"],"event_id":entry["event_id"],"scope":entry["scope"],"country_condition":entry["country_condition"],"match_evidence":strength(text(&query["query"]),entry,text(&query["country"]),&input["vocabulary"])})
        }).collect::<Vec<_>>();
        results.push(json!({"key":probe["key"],"source_query":query["query"],"request":query,"matches":matches}));
    }
    let output=serde_json::to_vec(&json!({"schema_version":1,"engine":"existing_rust_catalog_core","purpose":"diagnostic_match_strength_not_production_policy","results":results})).map_err(|e|e.to_string())?;
    use std::io::Write;
    fs::OpenOptions::new().write(true).create_new(true).open(&args[2]).and_then(|mut f|f.write_all(&output)).map_err(|e|e.to_string())
}
fn main(){if let Err(error)=run(){eprintln!("{error}");std::process::exit(1)}}
