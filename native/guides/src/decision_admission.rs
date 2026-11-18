//! Import-only decision safety. Never repairs installed definitions or evaluates progress.
use crate::{import_catalog::Index, recipient};
use hoi4companion_catalog_core::normal_name;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}

/// Independent current/original identities: a guide country is NOT an original_tag fact.
pub fn ownership(
    node: &Value,
    current: Option<&str>,
    original: Option<&str>,
    depth: usize,
) -> Option<bool> {
    if depth > 32 {
        return None;
    }
    match s(&node["op"]) {
        "tag" | "original_tag" => {
            let country = if node["op"] == "tag" {
                current
            } else {
                original
            }?;
            let value = node["value"].as_str().filter(|v| recipient::tag(v))?;
            recipient::tag(country).then_some(country == value)
        }
        "true" => Some(true),
        "false" => Some(false),
        "not" => ownership(&node["arg"], current, original, depth + 1).map(|v| !v),
        "all" | "any" => {
            let args = node["args"].as_array()?;
            let decisive = node["op"] == "any";
            let mut unknown = false;
            for arg in args {
                match ownership(arg, current, original, depth + 1) {
                    Some(value) if value == decisive => return Some(decisive),
                    None => unknown = true,
                    _ => (),
                }
            }
            if unknown { None } else { Some(!decisive) }
        }
        _ => None,
    }
}

/// Capture ID presence BEFORE promotion/binding. Presence alone proves no historical intent.
pub fn existing_ids(guide: &Value) -> HashSet<String> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(arr)
        .filter(|row| !s(&row["action"]["params"]["decision_id"]).is_empty())
        .filter_map(|row| row["key"].as_str().map(str::to_owned))
        .collect()
}

fn exact(query: &str, entry: &Value, country: &str, index: &Index) -> bool {
    if query.is_empty() {
        return false;
    }
    if query == s(&entry["id"]) {
        return true;
    }
    let q = normal_name(query);
    if q.is_empty() {
        return false;
    }
    if std::iter::once(&entry["name"])
        .chain(std::iter::once(&entry["localized_name"]))
        .chain(arr(&entry["aliases"]).iter())
        .chain(arr(&entry["country_aliases"][country]).iter())
        .filter_map(Value::as_str)
        .any(|label| normal_name(label) == q)
    {
        return true;
    }
    index.vocabulary()["entity_names"]["decision"]
        .as_object()
        .is_some_and(|aliases| {
            aliases.iter().any(|(alias, target)| {
                normal_name(alias) == q
                    && (target == &entry["id"]
                        || normal_name(s(target)) == normal_name(s(&entry["name"])))
            })
        })
}

pub fn validate(
    guide: &mut Value,
    existing: &HashSet<String>,
    index: &Index,
) -> Result<Value, String> {
    let country = guide["applies_to"]["country"].clone();
    let mut catalog: HashMap<&str, Vec<&Value>> = HashMap::new();
    for entry in index.rows().iter().filter(|e| e["kind"] == "decision") {
        catalog.entry(s(&entry["id"])).or_default().push(entry);
    }
    let mut resolutions = vec![];
    let mut errors = vec![];
    let mut warnings = vec![];
    let mut legacy = false;
    for row in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
        .flat_map(|v| v.as_array_mut().into_iter().flatten())
    {
        let params = &row["action"]["params"];
        let Some(id) = params["decision_id"]
            .as_str()
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
        else {
            continue;
        };
        let key = s(&row["key"]).to_owned();
        let provenance = if let Some(record) = params.get("decision_resolution") {
            record.clone()
        } else if existing.contains(&key) {
            json!({"schema_version":1,"id":id,"origin":"legacy_unknown","query":null})
        } else {
            let literal = if arr(&params["instructions"]).len() == 1 {
                s(&params["instructions"][0])
            } else {
                s(&params["decision_name"])
            };
            let query =
                crate::informal_targets::named_query(literal).unwrap_or_else(|| literal.to_owned());
            json!({"schema_version":1,"id":id,"origin":if query==id {"explicit_id"}else{"inferred"},"query":query})
        };
        let origin = s(&provenance["origin"]);
        let query = s(&provenance["query"]);
        let valid_provenance = provenance["schema_version"] == 1
            && provenance["id"] == id
            && ["explicit_id", "inferred", "user_reviewed", "legacy_unknown"].contains(&origin)
            && (origin == "legacy_unknown" || !query.trim().is_empty());
        let entries = catalog.get(id.as_str()).cloned().unwrap_or_default();
        let condition = recipient::combine(
            "any",
            entries
                .iter()
                .map(|e| e["country_condition"].clone())
                .collect(),
        );
        // No qualified original-country observation exists at this static import boundary.
        // An execution condition/timing may describe a later national context. Do not
        // use today's declared guide tag as proof about that future actor.
        let conditional = row.get("when").is_some() || params["timing"].is_string();
        let actor = if conditional {
            Value::Null
        } else {
            country.clone()
        };
        let owned = if entries.is_empty() {
            None
        } else {
            ownership(&condition, actor.as_str(), None, 0)
        };
        let mut resolution = json!({"key":key,"decision_id":id,"provenance":provenance,
            "ownership":match owned {Some(false)=>"incompatible",Some(true)=>"compatible",None=>"unknown"},
            "status":"unchanged","reason":"no_proven_conflict","event_option_candidates":[]});
        if !valid_provenance {
            resolution["status"] = json!("rejected");
            resolution["reason"] = json!("invalid_decision_provenance");
            errors.push(json!(format!(
                "{key}: invalid_decision_provenance; resolve the decision association explicitly."
            )));
        } else if owned == Some(false) {
            resolution["status"] = json!("rejected");
            resolution["reason"] = json!("decision_country_incompatible");
            errors.push(json!(format!("{key}: decision_country_incompatible ({id}); the original ID is retained for review.")));
        } else if origin == "inferred"
            && !entries.is_empty()
            && !entries.iter().any(|e| exact(query, e, s(&country), index))
        {
            // Separate kinds: the resolver's first nonempty tier can hide answer rivals in a combined query.
            let rivals = index.matches(query, &["event_option"], &actor, &Value::Null, false)?;
            if !rivals.is_empty() {
                resolution["status"] = json!("ambiguous");
                resolution["reason"] = json!("ambiguous_decision_event_option");
                resolution["event_option_candidates"] = json!(
                    rivals
                        .iter()
                        .map(|e| json!({"id":e["id"],"event_id":e["event_id"],"name":e["name"]}))
                        .collect::<Vec<_>>()
                );
                errors.push(json!(format!("{key}: ambiguous_decision_event_option ({query}); a weak decision match also names event answers. No replacement was selected.")));
            }
        }
        if origin == "legacy_unknown" {
            legacy = true;
        }
        row["action"]["params"]["decision_resolution"] = provenance;
        resolutions.push(resolution);
    }
    if legacy {
        warnings.push(json!("Some decision IDs have legacy_unknown provenance. Their presence does not prove an explicit choice or verified original country; existing IDs are retained."));
    }
    Ok(json!({"resolutions":resolutions,"errors":errors,"warnings":warnings}))
}
