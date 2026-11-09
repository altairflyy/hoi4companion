//! Compile authored event answers without prediction or strategy inference.
use crate::{import_catalog::Index, political_binding};
use serde_json::{Map, Value, json};
fn a(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn words(v: &str) -> String {
    let normalized = hoi4companion_catalog_core::casefold(&hoi4companion_catalog_core::nfkd(v));
    normalized
        .split(|c| !hoi4companion_catalog_core::unicode_alnum(c) && c != '_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn labels<'a>(values: impl Iterator<Item = &'a Value>) -> Vec<Value> {
    let mut result = vec![];
    for v in values.filter(|v| v.as_str().is_some_and(|s| !s.is_empty())) {
        if !result.contains(v) {
            result.push(v.clone())
        }
    }
    result
}
pub fn plan(guide: &Value, index: &Index) -> Result<Value, String> {
    let references = guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(a)
        .filter(|row| {
            ["event_id", "event_name", "event_option_id", "event_option"]
                .iter()
                .any(|k| row["action"]["params"].get(k).is_some())
        })
        .collect::<Vec<_>>();
    if references.is_empty() {
        return Ok(
            json!({"schema_version":1,"guide_id":guide["id"],"bindings":{},"unresolved":[]}),
        );
    }
    let country = guide["applies_to"]
        .get("country")
        .filter(|v| crate::state::truth(v))
        .unwrap_or(&guide["country"]);
    let events = index.entries(json!({"op":"rows","kinds":["event"],"country":country}))?;
    let options = index.entries(json!({"op":"rows","kinds":["event_option"],"country":country}))?;
    let mut bindings = Map::new();
    let mut unresolved = vec![];
    for source in references {
        let original = &source["action"]["params"];
        if !["event_id", "event_name", "event_option_id", "event_option"]
            .iter()
            .any(|k| original.get(k).is_some())
        {
            continue;
        }
        let bound = political_binding::bind(source, index, country, false)?;
        let mut params = bound["step"]["action"]["params"].clone();
        let mut reason = bound["reason"].clone();
        let mut resolution = "catalog_identity";
        if reason != "event_visible_option" && params["event_option_id"].is_null() {
            let mut quote = original["event_option"]
                .as_str()
                .map(words)
                .unwrap_or_default();
            let scope = &params["event_id"];
            if !original["event_name"].is_null() && scope.is_null() {
                quote.clear()
            }
            if quote.split_whitespace().count() >= 4 {
                let needle = format!(" {quote} ");
                let mut candidates = vec![];
                for option in &options {
                    if !scope.is_null() && option["event_id"] != *scope {
                        continue;
                    }
                    let names = [&option["name"], &option["localized_name"]]
                        .into_iter()
                        .chain(a(&option["aliases"]));
                    if names
                        .filter_map(Value::as_str)
                        .any(|n| format!(" {} ", words(n)).contains(&needle))
                        && !candidates.iter().any(|v: &&Value| {
                            v["event_id"] == option["event_id"] && v["id"] == option["id"]
                        })
                    {
                        candidates.push(option)
                    }
                }
                if candidates.len() == 1
                    && events.iter().any(|e| e["id"] == candidates[0]["event_id"])
                {
                    params["event_id"] = candidates[0]["event_id"].clone();
                    params["event_option_id"] = candidates[0]["id"].clone();
                    reason = json!("event_visible_option");
                    resolution = "unique_label_fragment";
                }
            }
        }
        let choices = options
            .iter()
            .filter(|e| e["id"] == params["event_option_id"] && e["event_id"] == params["event_id"])
            .collect::<Vec<_>>();
        let definitions = events
            .iter()
            .filter(|e| e["id"] == params["event_id"])
            .collect::<Vec<_>>();
        if reason != "event_visible_option" || choices.len() != 1 || definitions.len() != 1 {
            unresolved.push(json!({"key":source["key"],"title":source.get("title").filter(|v|crate::state::truth(v)).unwrap_or(&source["key"]),"reason":if crate::state::truth(&reason){reason}else{json!("event_reference_missing")}}));
            continue;
        }
        let option = choices[0];
        let event = definitions[0];
        let names = labels(
            [&option["localized_name"], &option["name"]]
                .into_iter()
                .chain(a(&option["aliases"])),
        );
        let titles = labels([&event["localized_name"], &event["name"]].into_iter());
        bindings.insert(s(&source["key"]).into(),json!({"key":source["key"],"panel":"event","identity":format!("{}:{}",s(&params["event_id"]),s(&params["event_option_id"])),"event_id":params["event_id"],"option_id":params["event_option_id"],"labels":names,"event_titles":titles,"event_title":titles.first().unwrap_or(&params["event_id"]),"resolution":resolution,"title":names.first().unwrap_or(&params["event_option_id"])}));
    }
    Ok(
        json!({"schema_version":1,"guide_id":guide["id"],"bindings":bindings,"unresolved":unresolved}),
    )
}
