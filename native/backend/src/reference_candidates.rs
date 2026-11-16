//! Every real game entity a free-text reference could name, for the playing
//! nation: decisions, events and event answers, focuses, laws, ideas,
//! technologies. Uses the catalog's own name/alias/word-prefix matcher with
//! the guide's country and focus tree; entries the catalog proves unavailable
//! to the country are dropped, unprovable ones are kept and marked. Nothing is
//! chosen here: callers bind only a single compatible candidate and otherwise
//! keep the ambiguity visible.
use crate::import_catalog::Index;
use serde_json::{Value, json};
type Result<T> = std::result::Result<T, String>;

pub const KINDS: &[&str] = &["decision", "event", "event_option", "focus", "law", "idea", "technology"];

/// Reference text without truncation marks, quotes or a trailing colon.
pub fn query(text: &str) -> String {
    text.trim()
        .trim_end_matches(':')
        .trim_end_matches('…')
        .trim_end_matches("...")
        .trim_matches(['"', '“', '”', '\''])
        .trim()
        .to_owned()
}

fn words(text: &str) -> Vec<String> {
    crate::entity_resolution::words(text)
}
/// Does the event belong to the playing nation? Either the catalog tags the
/// event for it (`identity_countries`), or its namespace names the nation
/// (tag, name or alias words of the catalog country row: "germany_mefo_bills").
fn event_affinity(event_id: &str, index: &Index, country: &str) -> Result<bool> {
    let parents = index.entries(json!({"op":"matches","query":event_id,"kinds":["event"],"country":country,"explicit":true}))?;
    if parents.iter().any(|p| p["identity_countries"].as_array().is_some_and(|a| a.iter().any(|c| c == country))) {
        return Ok(true);
    }
    let namespace = words(event_id.split('.').next().unwrap_or(""));
    let mut names = vec![country.to_lowercase()];
    for row in index.rows().iter().filter(|r| r["kind"] == "country" && r["id"] == country) {
        for name in std::iter::once(&row["name"]).chain(row["aliases"].as_array().into_iter().flatten()).filter_map(Value::as_str) {
            names.extend(words(name).into_iter().filter(|w| w.len() >= 4));
        }
    }
    Ok(namespace.iter().any(|w| names.contains(w)))
}
fn bound(params: &Value) -> bool {
    ["decision_id", "event_option_id", "advisor_id", "law_id", "idea_id", "operation_id", "production_targets", "building_id"]
        .iter()
        .any(|k| !params[*k].is_null())
}
/// Ground truncated author references ("Mobilize The…", "Our King Will be…")
/// that earlier passes left unbound: attach the nation-compatible catalog
/// candidates for display, and bind only a single event answer whose event
/// belongs to the playing nation, through the existing political binding
/// (live event observation, HUD option highlight). Everything else stays a
/// manually tracked instruction with its candidates visible.
pub fn ground_references(guide: &mut Value, index: &Index) -> Result<()> {
    let country = guide["applies_to"]["country"].clone();
    let scope = guide["applies_to"]["tree_id"].clone();
    let Some(tag) = country.as_str().map(str::to_owned) else { return Ok(()) };
    let tracks = guide["tracks"].as_object_mut().ok_or("Missing guide tracks")?;
    for row in tracks.values_mut().filter_map(Value::as_array_mut).flatten() {
        let params = &row["action"]["params"];
        let Some(text) = params["instructions"].as_array().filter(|a| a.len() == 1).and_then(|a| a[0].as_str()) else { continue };
        let text = text.trim().to_owned();
        if row["action"]["kind"] != "manual" || bound(params) || text.contains('\n') || text.chars().count() > 80
            || !(text.ends_with('…') || text.ends_with("..."))
        {
            continue;
        }
        let found = candidates(&text, index, &country, &scope)?;
        let compatible: Vec<Value> = found.as_array().into_iter().flatten().filter(|c| c["compatible"] != false).cloned().collect();
        if compatible.is_empty() {
            if !found.as_array().is_some_and(|a| a.is_empty()) {
                row["action"]["params"]["reference_candidates"] = json!({"status":"no_compatible_candidate","candidates":found.as_array().map(|a| a.iter().take(6).cloned().collect::<Vec<_>>())});
            }
            continue;
        }
        if compatible.len() == 1 && compatible[0]["kind"] == "event_option" {
            let option = &compatible[0];
            let event_id = option["event_id"].as_str().unwrap_or("");
            if !event_id.is_empty() && event_affinity(event_id, index, &tag)? {
                let probe = json!({"action":{"kind":"manual","params":{"category":"politics","event_id":event_id,"event_option_id":option["id"]}}});
                let result = crate::political_binding::bind(&probe, index, &country, false)?;
                if result["reason"] == "event_visible_option" {
                    let mut params = result["step"]["action"]["params"].clone();
                    for keep in ["instructions", "source_action"] {
                        if !row["action"]["params"][keep].is_null() {
                            params[keep] = row["action"]["params"][keep].clone();
                        }
                    }
                    params["reference_resolution"] = json!({"basis":"single_nation_event_answer","event_id":event_id,"option_id":option["id"],"option_text":option["name"]});
                    row["action"]["params"] = params;
                    if let Some(map) = row.as_object_mut() {
                        map.remove("tracking");
                    }
                    for field in ["completion", "started_when"] {
                        if !result["step"][field].is_null() {
                            row[field] = result["step"][field].clone();
                        }
                    }
                    continue;
                }
            }
        }
        row["action"]["params"]["reference_candidates"] = json!({"status":if compatible.len() == 1 {"single_unconfirmed"} else {"ambiguous"},
            "candidates":compatible.into_iter().take(6).collect::<Vec<_>>()});
    }
    Ok(())
}

pub fn candidates(text: &str, index: &Index, country: &Value, scope: &Value) -> Result<Value> {
    let q = query(text);
    if q.chars().filter(|c| c.is_alphanumeric()).count() < 4 {
        return Ok(json!([]));
    }
    let actor = country.as_str();
    let mut out = vec![];
    let mut seen = std::collections::HashSet::new();
    for row in index.matches(&q, KINDS, country, scope, false)? {
        let (kind, id) = (row["kind"].as_str().unwrap_or(""), row["id"].as_str().unwrap_or(""));
        if id.is_empty() || !seen.insert((kind.to_owned(), id.to_owned())) {
            continue;
        }
        let ownership = if row["country_condition"].is_null() {
            None
        } else {
            crate::decision_admission::ownership(&row["country_condition"], actor, None, 0)
        };
        if ownership == Some(false) {
            continue;
        }
        // A country-tagged ID of another nation is never a confident match.
        // Events carry DLC codes ("BBA_", "DOD_"), not tags: judged by affinity.
        let prefix = id.split(['_', '.']).next().unwrap_or("");
        let foreign = ["decision", "focus", "law", "idea"].contains(&kind)
            && prefix.len() == 3
            && prefix.chars().all(|c| c.is_ascii_uppercase())
            && actor.is_some_and(|a| a != prefix);
        out.push(json!({"kind":kind,"id":id,"name":row["name"],"event_id":row["event_id"],
            "compatible":match ownership { Some(true) => json!(true), _ => if foreign { json!(false) } else { Value::Null } },
            "foreign_tag":foreign}));
    }
    Ok(json!(out))
}
