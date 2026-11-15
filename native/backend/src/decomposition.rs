//! Compiler contracts 2-3: decomposition of opaque multiline reminder containers
//! (2) plus grounded entity resolution of trigger references (3).
//!
//! Contract 1 (`operation_language::promote`) already splits a multiline
//! reminder line by line, but only when at least one line is recognized by an
//! existing command family; otherwise the whole block stays one uncheckable
//! reminder. Contract 2 applies the same line structure to those remaining
//! containers WITHOUT inventing meaning:
//! - a line ending in `:` is a header, never a step. A timing header (the
//!   promote timing grammar) becomes the `when` of the following lines,
//!   compiled with promote's condition compiler or kept as promote's unknown
//!   placeholder fact (never satisfied by itself); any other header is a lead-in
//!   that prefixes the titles of its items ("Justify ... on: Japan");
//! - an inline `When X: Y` scopes Y (and following lines) exactly like promote;
//! - explanatory/advice text stays a non-completable reminder;
//! - every other line becomes a manually tracked gate with its original text
//!   (`ambiguous` when visibly truncated or questioning). No completion
//!   condition is invented; the user confirms it.
//! Quantities and intra-line sequences stay inside the preserved text; line
//! order is kept; parent `when`/`depends_on` are inherited, so no child is
//! more unconditional than its container. Children use the new `--line-N` key
//! namespace and never reuse a container key, so no saved check can attach to
//! them. Applied only to NEW archived definitions (see `definition_archive`).
use crate::entity_resolution::{self, Candidate};
use crate::import_catalog::regex;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
type Result<T> = std::result::Result<T, String>;

pub const CONTRACT: u64 = 3;
/// Same strings as `operation_language::promote` (pinned by a test).
const LINE_BREAK: &str = r"\r\n|[\n\r\x0B\x0C\x1C-\x1E\u0085\u2028\u2029]";
const BULLET: &str = r"^\s*[-•]\s*";
const INLINE_TIMING: &str = r"(?i)^((?:on|after|before|shortly before|when|once|at)\b[^:]+):\s*(.*)$";
const ELLIPSIS_TIMING: &str = r"(?i)^(?:on|after|before|shortly before|when|once)\s+[^:\n]+(?:…|\.{3})$";
/// Generic informational markers (not author vocabulary): a whole-line
/// parenthetical aside, permission ("you can") or an annotation label.
/// Prohibitions ("do not", "ignore", "never") are trackable constraints, not
/// advice: downgrading an instruction to an uncheckable reminder is the unsafe
/// direction, so only clearly informational lines are advice.
const ADVICE: &str = r"(?i)^(?:\(.*\)|(?:you can|you may|you could|note|nb|n\.b\.|tip|ps|p\.s\.)\b.*)$";
const AMBIGUOUS: &str = r"(?:…|\.{3}|\?)\s*\*?$";

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn rows_mut(guide: &mut Value) -> Result<Vec<&mut Vec<Value>>> {
    Ok(guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
        .filter_map(Value::as_array_mut)
        .collect())
}
fn is_container(row: &Value) -> bool {
    crate::occurrence_manifest::is_container(row)
        && !row["action"]["params"].get("production_residual_for").is_some_and(crate::state::truth)
}
fn dependencies(guide: &Value) -> HashSet<String> {
    guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|v| v.as_array().into_iter().flatten())
        .flat_map(|r| {
            ["depends_on", "depends_on_started"]
                .iter()
                .flat_map(move |f| r[*f].as_array().into_iter().flatten())
        })
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// Classification of one source line.
#[derive(Debug, PartialEq)]
pub enum Line {
    Blank,
    TimingHeader(String),
    LeadHeader(String),
    Step { timing: Option<String>, text: String, bullet: bool, class: &'static str },
}
pub fn classify(raw: &str) -> Line {
    let bullet = regex(BULLET).is_match(raw);
    let line = regex(BULLET).replace(raw, "").trim().to_owned();
    if line.is_empty() {
        return Line::Blank;
    }
    if let Some(head) = line.strip_suffix(':').map(str::trim) {
        let head = head.to_owned();
        return if regex(INLINE_TIMING).is_match(&format!("{head}:")) {
            Line::TimingHeader(head)
        } else {
            Line::LeadHeader(head)
        };
    }
    if regex(ELLIPSIS_TIMING).is_match(&line) {
        return Line::TimingHeader(line);
    }
    let (timing, body) = match regex(INLINE_TIMING).captures(&line) {
        Some(hit) if !hit[2].trim().is_empty() => (Some(hit[1].trim().to_owned()), hit[2].trim().to_owned()),
        _ => (None, line.clone()),
    };
    let class = if regex(ADVICE).is_match(&body) {
        "advice"
    } else if regex(AMBIGUOUS).is_match(&body) {
        "ambiguous"
    } else {
        "manual"
    };
    Line::Step { timing, text: body, bullet, class }
}

/// `condition` compiles a timing text with the guide's catalog rules when
/// available; `extra` adds grounded catalog candidates (same focus tree).
/// Contract 3: references in trigger position are resolved against real game
/// entities (`entity_resolution`) when the timing rules cannot compile them.
pub fn decompose(guide: &Value, condition: &dyn Fn(&str) -> Result<Option<Value>>) -> Result<Value> {
    decompose_with(guide, condition, &[])
}
pub fn decompose_with(
    guide: &Value,
    condition: &dyn Fn(&str) -> Result<Option<Value>>,
    extra: &[Candidate],
) -> Result<Value> {
    let mut guide = guide.clone();
    let mut report = vec![];
    if guide["dependency_policy"] != "explicit" {
        return Ok(json!({"guide":guide,"report":report}));
    }
    let candidates = entity_resolution::merge(entity_resolution::guide_candidates(&guide), extra.to_vec());
    let id = text(&guide["id"]).to_owned();
    let referenced = dependencies(&guide);
    let mut keys: HashSet<String> = guide["tracks"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|v| v.as_array().into_iter().flatten())
        .map(|r| text(&r["key"]).to_owned())
        .collect();
    let mut triggers = Triggers { condition, candidates: &candidates, cache: HashMap::new() };
    for lane in rows_mut(&mut guide)? {
        let original = std::mem::take(lane);
        for row in original {
            let key = text(&row["key"]).to_owned();
            if !is_container(&row) || referenced.contains(&key) {
                lane.push(row);
                continue;
            }
            let children = split(&id, &row, &mut triggers)?;
            let child_keys: Vec<String> = children.iter().map(|c| text(&c["key"]).to_owned()).collect();
            let actionable = children.iter().filter(|c| c.get("tracking").is_none()).count();
            if children.is_empty()
                || children.len() > 512
                || child_keys.iter().any(|k| keys.contains(k))
            {
                report.push(json!({"key":key,"status":"kept","reason":"no_safe_split"}));
                lane.push(row);
                continue;
            }
            let classes = children.iter().fold(Map::new(), |mut m, c| {
                let class = text(&c["action"]["params"]["decomposition"]["status"]).to_owned();
                *m.entry(class).or_insert(json!(0)) = json!(m.get(&class).and_then(Value::as_u64).unwrap_or(0) + 1);
                m
            });
            report.push(json!({"key":key,"status":"decomposed","children":child_keys,"actionable":actionable,"classes":classes}));
            keys.extend(child_keys);
            lane.extend(children);
        }
    }
    Ok(json!({"guide":guide,"report":report}))
}

/// Trigger interpretation, cached per text: catalog timing rules first (the
/// promote compiler), then grounded entity resolution + intent.
struct Triggers<'a> {
    condition: &'a dyn Fn(&str) -> Result<Option<Value>>,
    candidates: &'a [Candidate],
    cache: HashMap<String, Value>,
}
impl Triggers<'_> {
    fn get(&mut self, text: &str) -> Result<Value> {
        if let Some(v) = self.cache.get(text) {
            return Ok(v.clone());
        }
        let value = if let Some(compiled) = (self.condition)(text)? {
            json!({"text":text,"condition":compiled,"method":"timing_rules","verification":"observable"})
        } else {
            let resolution = entity_resolution::resolve(text, self.candidates);
            let (compiled, relation, verification) = entity_resolution::trigger_condition(&resolution);
            json!({"text":text,"condition":compiled,"method":"entity_resolution","relation":relation,
                "verification":verification,"resolution":resolution})
        };
        self.cache.insert(text.to_owned(), value.clone());
        Ok(value)
    }
    /// A lead-in or scope prefix is a trigger only when it names grounded game
    /// entities: resolved (condition compiled) or ambiguous (kept unknown with
    /// its candidates listed, never silently resolved or dropped).
    fn names_entity(&mut self, text: &str) -> bool {
        let r = entity_resolution::resolve(text, self.candidates);
        matches!(r["status"].as_str(), Some("resolved" | "ambiguous")) && entity_resolution::fully_understood(&r)
    }
}

/// `A: B:` — a header holding both a scope and an action ("On free Mil: Inf
/// Equip to 25 &:"). Returns (scope, action) when the action part is non-empty.
pub fn mixed_header(line: &str) -> Option<(String, String)> {
    let inner = line.strip_suffix(':')?.trim_end();
    let (scope, action) = inner.split_once(": ")?;
    (!scope.trim().is_empty() && !action.trim().is_empty()).then(|| (scope.trim().to_owned(), action.trim().to_owned()))
}

fn split(guide_id: &str, row: &Value, triggers: &mut Triggers<'_>) -> Result<Vec<Value>> {
    let key = text(&row["key"]);
    let body = text(&row["action"]["params"]["instructions"][0]);
    let (mut timing, mut lead, mut last_line): (Option<String>, Option<String>, Option<String>) = (None, None, None);
    // A lead header whose first item is a bullet covers only that bullet list.
    let mut lead_bullets: Option<bool> = None;
    let mut headers: Vec<Value> = vec![];
    let mut children = vec![];
    for (n, raw) in regex(LINE_BREAK).split(body).enumerate() {
        let bare = regex(BULLET).replace(raw, "").trim().to_owned();
        // Mixed header: the scope part is a header, the action part a step.
        let pieces: Vec<String> = match mixed_header(&bare) {
            Some((scope, action)) => {
                let prefix = if regex(BULLET).is_match(raw) { "- " } else { "" };
                vec![format!("{prefix}{scope}:"), format!("{prefix}{action}")]
            }
            None => vec![raw.to_owned()],
        };
        for piece in pieces {
            let (step_timing, mut line, bullet, class) = match classify(&piece) {
                Line::Blank => continue,
                Line::TimingHeader(head) => {
                    headers.push(json!({"line":n + 1,"quote":raw.trim(),"kind":"condition"}));
                    timing = Some(head);
                    lead = None;
                    continue;
                }
                Line::LeadHeader(head) => {
                    // Wording-independent: a header naming a grounded game
                    // entity is a trigger, not a mere lead-in.
                    if triggers.names_entity(&head) {
                        headers.push(json!({"line":n + 1,"quote":raw.trim(),"kind":"condition"}));
                        timing = Some(head);
                        lead = None;
                    } else {
                        headers.push(json!({"line":n + 1,"quote":raw.trim(),"kind":"lead"}));
                        lead = Some(head);
                        lead_bullets = None;
                    }
                    continue;
                }
                Line::Step { timing, text, bullet, class } => (timing, text, bullet, class),
            };
            let mut step_timing = step_timing;
            // Inline `scope: action` whose scope names a grounded entity.
            if step_timing.is_none()
                && let Some(hit) = regex(r"^([^:]{3,60}):\s+(\S.*)$").captures(&line)
                && triggers.names_entity(&hit[1])
            {
                step_timing = Some(hit[1].trim().to_owned());
                line = hit[2].trim().to_owned();
            }
            if lead.is_some() {
                match lead_bullets {
                    None => lead_bullets = Some(bullet),
                    Some(true) if !bullet => {
                        lead = None;
                        lead_bullets = None;
                    }
                    _ => {}
                }
            }
            if let Some(inline) = step_timing {
                // promote keeps an inline timing for the following lines too.
                timing = Some(inline);
            }
            let context = if bullet { last_line.clone() } else {
                last_line = Some(line.clone());
                None
            };
            let mut item = row.clone();
            let map = item.as_object_mut().ok_or("Invalid guide step")?;
            // Presentation of the container belongs to the container only.
            for field in ["name", "verification", "entity", "operational_glossary", "catalog_match", "title", "note"] {
                map.remove(field);
            }
            let title = match &lead {
                Some(head) => format!("{head}: {line}"),
                None => line.clone(),
            };
            item["key"] = json!(format!("{key}--line-{}", n + 1));
            item["title"] = json!(title);
            item["action"] = json!({"kind":"manual","params":row["action"]["params"].clone()});
            item["action"]["params"]["instructions"] = json!([line]);
            if class == "advice" {
                item["tracking"] = json!("reminder");
            } else {
                item.as_object_mut().unwrap().remove("tracking");
            }
            let mut condition_status = if row.get("when").is_some() { "inherited" } else { "none" };
            let mut trigger = Value::Null;
            let mut notes: Vec<String> = [text(&row["note"])].iter().filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
            if let Some(timing) = &timing {
                trigger = triggers.get(timing)?;
                // Same rule as promote: a real parent condition wins; otherwise
                // the compiled trigger, or promote's unknown placeholder.
                let placeholders = crate::operation_language::generated(guide_id, key, timing);
                if item.get("when").is_none() || placeholders.contains(&item["when"]) {
                    if trigger["condition"].is_object() {
                        item["when"] = trigger["condition"].clone();
                        condition_status = "compiled";
                    } else {
                        item["when"] = placeholders[0].clone();
                        condition_status = "unknown";
                    }
                }
                if !notes.contains(timing) {
                    notes.push(timing.clone());
                }
            }
            if let Some(context) = &context {
                notes.push(context.clone());
            }
            if !notes.is_empty() {
                item["note"] = json!(notes.join("\n"));
            }
            item["action"]["params"]["decomposition"] = json!({"contract":CONTRACT,"parent_key":key,
                "line":n + 1,"status":class,"timing":timing,"lead":lead,"context":context,
                "condition_status":condition_status,"trigger":trigger,"headers":headers});
            children.push(item);
        }
    }
    Ok(children)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn container(block: &str) -> Value {
        json!({"key":"word-0007","kind":"gate","tracking":"reminder","title":"Block","name":"Block",
            "evidence":[{"source_id":"s","basis":"explicit","quote":block}],
            "action":{"kind":"manual","params":{"instructions":[block],"source_action":{"key":"word-0007","ordinal":7}}}})
    }
    fn guide(rows: Value) -> Value {
        json!({"id":"g","dependency_policy":"explicit","tracks":{"operations":rows}})
    }
    fn unknown(_: &str) -> Result<Option<Value>> {
        Ok(None)
    }
    fn run(rows: Value) -> (Vec<Value>, Value) {
        let out = decompose(&guide(rows), &unknown).unwrap();
        (out["guide"]["tracks"]["operations"].as_array().unwrap().clone(), out["report"].clone())
    }

    #[test]
    fn promote_grammar_is_shared_verbatim() {
        let source = include_str!("operation_language.rs");
        for pattern in [LINE_BREAK, BULLET, INLINE_TIMING, ELLIPSIS_TIMING] {
            assert!(source.contains(&format!("r\"{pattern}\"")), "{pattern}");
        }
    }

    #[test]
    fn timing_headers_scope_following_lines_without_inventing_completion() {
        let (rows, report) = run(json!([container("When at war:\nRafforza la difesa ⚑\nRafforza la difesa ⚑")]));
        assert_eq!(rows.len(), 2);
        assert_eq!(report[0]["status"], "decomposed");
        for (row, line) in rows.iter().zip([2, 3]) {
            assert_eq!(row["key"], format!("word-0007--line-{line}"));
            assert_eq!(row["title"], "Rafforza la difesa ⚑");
            assert!(row.get("tracking").is_none(), "manually tracked");
            assert!(row.get("completion").is_none(), "no invented completion");
            assert_eq!(row["when"]["key"].as_str().unwrap().starts_with("guide.conditions."), true);
            assert_eq!(row["note"], "When at war");
            assert_eq!(row["action"]["params"]["decomposition"]["condition_status"], "unknown");
            assert!(row.get("name").is_none());
        }
        assert_ne!(rows[0]["key"], rows[1]["key"]);
        // A compiled condition is used exactly as promote would.
        let compiled = |_: &str| Ok(Some(json!({"op":"fact","key":"country.at_war","test":"eq","value":true})));
        let out = decompose(&guide(json!([container("When at war:\nA\nB")])), &compiled).unwrap();
        assert_eq!(out["guide"]["tracks"]["operations"][1]["when"]["key"], "country.at_war");
    }

    #[test]
    fn leads_bullets_advice_and_ambiguity_are_distinguished() {
        let (rows, _) = run(json!([container(
            "Justify but do not declare war on:\n- Japan\n- USSR\nTrain divisions\n- Over time: GBP, tact\n(If reloading: defeat ROM first)\n- Ignore Czechoslovakia\nPrioritize Eco…"
        )]));
        let titles: Vec<_> = rows.iter().map(|r| r["title"].as_str().unwrap()).collect();
        assert_eq!(titles, ["Justify but do not declare war on: Japan", "Justify but do not declare war on: USSR",
            "Train divisions", "Over time: GBP, tact", "(If reloading: defeat ROM first)", "Ignore Czechoslovakia",
            "Prioritize Eco…"]);
        let status: Vec<_> = rows.iter().map(|r| r["action"]["params"]["decomposition"]["status"].as_str().unwrap()).collect();
        // "Ignore ..." is a trackable constraint, never downgraded to advice.
        assert_eq!(status, ["manual", "manual", "manual", "manual", "advice", "manual", "ambiguous"]);
        assert_eq!(rows[4]["tracking"], "reminder", "advice never becomes completable");
        assert_eq!(rows[3]["action"]["params"]["decomposition"]["context"], "Train divisions");
        assert_eq!(rows[3]["action"]["params"]["instructions"][0], "Over time: GBP, tact");
    }

    #[test]
    fn parent_conditions_dependencies_and_order_are_inherited() {
        let mut parent = container("Do A\nDo B (2x)");
        parent["when"] = json!({"op":"fact","key":"country.at_war","test":"eq","value":true});
        parent["depends_on"] = json!(["word-0001"]);
        let before = json!({"key":"word-0001","kind":"gate","action":{"kind":"manual"}});
        let after = json!({"key":"word-0009","kind":"gate","action":{"kind":"manual"}});
        let (rows, _) = run(json!([before, parent, after]));
        let keys: Vec<_> = rows.iter().map(|r| r["key"].as_str().unwrap()).collect();
        assert_eq!(keys, ["word-0001", "word-0007--line-1", "word-0007--line-2", "word-0009"]);
        for row in &rows[1..3] {
            assert_eq!(row["when"]["key"], "country.at_war");
            assert_eq!(row["depends_on"], json!(["word-0001"]));
            assert_eq!(row["action"]["params"]["decomposition"]["condition_status"], "inherited");
        }
        assert_eq!(rows[2]["title"], "Do B (2x)", "quantity kept in the source text");
    }

    #[test]
    fn unsafe_or_out_of_scope_containers_are_kept() {
        let mut referenced = container("Do A\nDo B");
        referenced["key"] = json!("word-0002");
        let dependent = json!({"key":"word-0003","kind":"gate","depends_on":["word-0002"],"action":{"kind":"manual"}});
        let (rows, _) = run(json!([referenced, dependent]));
        assert_eq!(rows[0]["key"], "word-0002");
        let mut implicit = guide(json!([container("Do A\nDo B")]));
        implicit["dependency_policy"] = json!("sequential");
        assert_eq!(decompose(&implicit, &unknown).unwrap()["guide"], implicit);
        let (single, _) = run(json!([container("Only one line")]));
        assert_eq!(single[0]["key"], "word-0007");
    }

    fn focus(key: &str, title: &str, id: &str) -> Value {
        json!({"key":key,"kind":"focus","title":title,"action":{"kind":"select_focus","entity":{"kind":"focus","id":id}}})
    }

    #[test]
    fn grounded_trigger_references_compile_to_observable_focus_conditions() {
        let (rows, _) = run(json!([focus("word-0003", "Seek Peace With The Allies", "ITA_seek_peace"),
            container("On Seek…:\nExpand Lateran…(70PP)\nAfter Seek…:\nJustify ETH")]));
        let (a, b) = (&rows[1], &rows[2]);
        assert_eq!(a["when"], json!({"op":"any","args":[{"op":"fact","key":"focus.active","test":"contains","value":"ITA_seek_peace"},
            {"op":"fact","key":"focus.completed","test":"contains","value":"ITA_seek_peace"}]}));
        assert_eq!(a["action"]["params"]["decomposition"]["condition_status"], "compiled");
        assert_eq!(a["action"]["params"]["decomposition"]["trigger"]["resolution"]["match"], "truncated");
        assert_eq!(b["when"], json!({"op":"fact","key":"focus.completed","test":"contains","value":"ITA_seek_peace"}));
        assert!(a.get("completion").is_none(), "the action itself stays manually confirmed");
    }

    #[test]
    fn mixed_condition_action_headers_and_entity_leads() {
        let (rows, _) = run(json!([focus("word-0002", "Organize the Military", "USA_organize_military"),
            container("On free Mil: Inf Equip to 25 &:\n- Art to 10\nOrganize the Military:\n- Train ALL\nProduction:\n- C2 to 50")]));
        let titles: Vec<_> = rows[1..].iter().map(|r| r["title"].as_str().unwrap()).collect();
        // The action part of the mixed header is its own step under the scope.
        assert_eq!(titles, ["Inf Equip to 25 &", "Art to 10", "Train ALL", "Production: C2 to 50"]);
        assert_eq!(rows[1]["action"]["params"]["decomposition"]["timing"], "On free Mil");
        assert_eq!(rows[1]["action"]["params"]["decomposition"]["condition_status"], "unknown");
        assert_eq!(rows[1]["action"]["params"]["decomposition"]["trigger"]["verification"], "reference_not_resolved");
        // A header naming a grounded focus is a trigger without any "On"/"After".
        assert_eq!(rows[3]["action"]["params"]["decomposition"]["condition_status"], "compiled");
        assert_eq!(rows[3]["when"]["args"][1]["value"], "USA_organize_military");
        // A lead naming nothing stays a lead-in title prefix, and the earlier
        // trigger keeps scoping (promote semantics).
        assert_eq!(rows[4]["action"]["params"]["decomposition"]["lead"], "Production");
    }

    #[test]
    fn inline_scope_prefixes_and_ambiguity_stay_explicit() {
        let (rows, _) = run(json!([focus("word-0002", "Seek Peace", "ITA_seek_peace"), focus("word-0004", "Seek Glory", "ITA_seek_glory"),
            focus("word-0005", "Rhineland", "GER_rhineland"),
            container("Rhineland: move troops (2 divisions)\nSeek…: do X\nMIL: train")]));
        assert_eq!(rows[3]["title"], "move troops (2 divisions)", "quantity preserved");
        assert_eq!(rows[3]["when"]["args"][1]["value"], "GER_rhineland");
        // "Seek…" names two focuses: header kept, condition unknown, candidates listed.
        let ambiguous = &rows[4]["action"]["params"]["decomposition"]["trigger"];
        assert_eq!(ambiguous["resolution"]["status"], "ambiguous");
        assert_eq!(rows[4]["action"]["params"]["decomposition"]["condition_status"], "unknown");
        // A label-like prefix naming nothing is not a scope.
        assert_eq!(rows[5]["title"], "MIL: train");
    }
}
