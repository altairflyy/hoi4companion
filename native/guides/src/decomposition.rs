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

pub const CONTRACT: u64 = 4;
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
/// Contract 4: a single-line manual reminder (an unrecognized instruction or a
/// promote residual such as "Production: Sup Equip:") is read with the same
/// line rules, so it becomes manually trackable unless it is advice or a
/// header. Auxiliary vocabulary notes (`--note-N`) stay reminders.
fn is_single_reminder(row: &Value) -> bool {
    let instructions = row["action"]["params"]["instructions"].as_array();
    row["tracking"] == "reminder"
        && row["action"]["kind"] == "manual"
        && instructions.is_some_and(|a| a.len() == 1 && a[0].as_str().is_some_and(|t| !t.trim().is_empty() && !t.contains('\n')))
        && !regex(r"--note-\d+$").is_match(text(&row["key"]))
        && !row["action"]["params"].get("production_residual_for").is_some_and(crate::state::truth)
}
/// "Lead: A, then B" -> [("Lead: A", "Lead: A"), ("Lead: B", "B")]: an explicit
/// sequence with a comma/semicolon before "then". Returns (title, exact source
/// text) pairs; the lead is carried into later titles only. Each part needs
/// >= 3 words, otherwise a verb may be elided ("Justify X, then Y") and the
/// line stays whole.
pub fn sequence_parts(line: &str) -> Vec<(String, String)> {
    let whole = vec![(line.to_owned(), line.to_owned())];
    let (lead, body) = match line.split_once(": ") {
        Some((lead, body)) if lead.len() <= 30 && !lead.chars().any(|c| c.is_ascii_digit()) && !lead.contains(',') => (Some(lead.trim()), body),
        _ => (None, line),
    };
    let parts: Vec<&str> = regex(r"(?i)\s*[,;]\s*then\s+").split(body).map(str::trim).collect();
    if parts.len() < 2 || parts.iter().any(|p| entity_resolution::words(p).len() < 3) {
        return whole;
    }
    parts
        .into_iter()
        .enumerate()
        .map(|(i, p)| match lead {
            Some(l) if i == 0 => (format!("{l}: {p}"), format!("{l}: {p}")),
            Some(l) => (format!("{l}: {p}"), p.to_owned()),
            None => (p.to_owned(), p.to_owned()),
        })
        .collect()
}
/// Contract 4 re-reads production residuals, which can bind targets the main
/// production step of the same Word row already monitors. Drop a step whose
/// every target duplicates an earlier step of the same row (nothing depends on
/// it); its unparsed remainder lives in its own child step and is kept.
pub fn drop_duplicate_production(guide: &mut Value) -> Result<()> {
    let referenced = dependencies(guide);
    let signature = |t: &Value| json!([t["equipment_name"], t["production_selector"], t["requested_factories"], t["quantity_mode"]]).to_string();
    let mut seen: HashMap<String, HashSet<String>> = HashMap::new();
    for lane in rows_mut(guide)? {
        lane.retain(|row| {
            let Some(targets) = row["action"]["params"]["production_targets"].as_array().filter(|t| !t.is_empty()) else { return true };
            let base = regex(r"(?:--(?:operation|note|line|seq)-\d+)+$").replace(text(&row["key"]), "").to_string();
            let sigs: Vec<String> = targets.iter().map(signature).collect();
            let known = seen.entry(base).or_default();
            if sigs.iter().all(|s| known.contains(s)) && !referenced.contains(text(&row["key"])) {
                return false;
            }
            known.extend(sigs);
            true
        });
    }
    Ok(())
}
/// Withdraw decision bindings the catalog proves impossible for the guide's
/// country (e.g. a weak "Mobilize The…" bound to a German decision in a
/// Soviet guide). The step keeps its text as a manually tracked instruction
/// with the reason; no replacement is guessed. Conditional steps (a timing or
/// `when` may describe another future actor) are left untouched.
pub fn withdraw_incompatible_decisions(guide: &mut Value, index: &crate::import_catalog::Index) -> Result<()> {
    let country = guide["applies_to"]["country"].as_str().map(str::to_owned);
    let mut catalog: HashMap<String, Vec<Value>> = HashMap::new();
    for entry in index.rows().iter().filter(|e| e["kind"] == "decision") {
        catalog.entry(text(&entry["id"]).to_owned()).or_default().push(entry["country_condition"].clone());
    }
    for lane in rows_mut(guide)? {
        for row in lane.iter_mut() {
            let Some(id) = row["action"]["params"]["decision_id"].as_str().map(str::to_owned) else { continue };
            let conditional = row.get("when").is_some() || row["action"]["params"]["timing"].is_string();
            if conditional {
                continue;
            }
            let owned = catalog.get(&id).and_then(|conditions| {
                crate::decision_admission::ownership(&crate::recipient::combine("any", conditions.clone()), country.as_deref(), None, 0)
            });
            // Weak foreign binding: the author's text is truncated (the ID came
            // from an incomplete-name guess), the ID carries another country's
            // tag, and the catalog cannot prove the decision is available here.
            // Exact-name foreign decisions (legitimate controls) are untouched.
            let truncated = row["action"]["params"]["instructions"][0].as_str().is_some_and(|t| t.trim_end().ends_with('…') || t.trim_end().ends_with("..."));
            let prefix = id.split('_').next().unwrap_or("");
            let foreign = prefix.len() == 3 && prefix.chars().all(|c| c.is_ascii_uppercase()) && country.as_deref().is_some_and(|c| c != prefix);
            let reason = match owned {
                Some(false) => "decision_country_incompatible",
                None if truncated && foreign => "weak_foreign_decision_binding",
                _ => continue,
            };
            let name = row["action"]["params"]["decision_name"].clone();
            let params = row["action"]["params"].as_object_mut().ok_or("Invalid step parameters")?;
            params.retain(|k, _| !k.starts_with("decision"));
            params.insert("binding_withdrawn".into(), json!({"kind":"decision","id":id,"name":name,"reason":reason}));
            let map = row.as_object_mut().ok_or("Invalid guide step")?;
            for field in ["completion", "started_when", "name", "entity", "verification"] {
                if field == "name" || field == "entity" || field == "verification" || map.get(field).is_some_and(|v| v.to_string().contains(&id)) {
                    map.remove(field);
                }
            }
        }
    }
    Ok(())
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
            let single = !is_container(&row) && is_single_reminder(&row);
            if !(is_container(&row) || single) || referenced.contains(&key) {
                lane.push(row);
                continue;
            }
            let children = split(&id, &row, &mut triggers)?;
            let child_keys: Vec<String> = children.iter().map(|c| text(&c["key"]).to_owned()).collect();
            let actionable = children.iter().filter(|c| c.get("tracking").is_none()).count();
            // A single line that stays one advice/header reminder is unchanged.
            if single && actionable == 0 && children.len() <= 1 {
                lane.push(row);
                continue;
            }
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
    let (mut line_timing, mut bullet_timing): (Option<String>, Option<String>) = (None, None);
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
                    (line_timing, bullet_timing) = (None, None);
                    lead = None;
                    continue;
                }
                Line::LeadHeader(head) => {
                    // Wording-independent: a header naming a grounded game
                    // entity is a trigger, not a mere lead-in.
                    if triggers.names_entity(&head) {
                        headers.push(json!({"line":n + 1,"quote":raw.trim(),"kind":"condition"}));
                        timing = Some(head);
                        (line_timing, bullet_timing) = (None, None);
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
            // A header line ("When at war:") scopes every following line. An
            // inline "At 24% paranoia: X" scopes X and the bullets under it
            // only: the next top-level line starts over from the header, and a
            // bullet's own inline timing covers that bullet alone.
            if bullet {
                bullet_timing = step_timing;
            } else {
                line_timing = step_timing;
                bullet_timing = None;
            }
            let timing = bullet_timing.clone().or_else(|| line_timing.clone()).or_else(|| timing.clone());
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
            // Explicit sequence inside the line ("A, then B"): ordered parts,
            // each depending on the previous one, each keeping the full line.
            let parts = sequence_parts(&line);
            if parts.len() == 1 {
                children.push(item);
                continue;
            }
            let base = text(&item["key"]).to_owned();
            for (m, (part_title, part)) in parts.iter().enumerate() {
                let mut piece = item.clone();
                let piece_key = format!("{base}--seq-{}", m + 1);
                piece["key"] = json!(piece_key);
                piece["title"] = json!(match &lead { Some(head) => format!("{head}: {part_title}"), None => part_title.clone() });
                piece["action"]["params"]["instructions"] = json!([part]);
                // Reminders cannot be dependencies (schema): only actionable parts chain.
                if m > 0 && class != "advice" {
                    let mut deps = piece["depends_on"].as_array().cloned().unwrap_or_default();
                    deps.push(json!(format!("{base}--seq-{m}")));
                    piece["depends_on"] = json!(deps);
                }
                let context = format!("{} ({}/{})", line, m + 1, parts.len());
                piece["note"] = json!(match piece["note"].as_str() { Some(n) if !n.is_empty() => format!("{n}\n{context}"), _ => context });
                piece["action"]["params"]["decomposition"]["sequence"] = json!({"part":m + 1,"of":parts.len(),"line_text":line});
                children.push(piece);
            }
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
    fn an_inline_timing_scopes_its_line_and_bullets_only() {
        let (rows, _) = run(json!([container(
            "At 24% paranoia: Inspection nav (50PP)\n- Quiet intel in GER\nMW doctrine & last 2 spirits ASAP\n- MILs in 100%\nOn claim: Invade NED\n- At 20% Paranoia: Forge Report (25PP)\n- Adjust trade"
        )]));
        let timing: Vec<_> = rows.iter().map(|r| r["action"]["params"]["decomposition"]["timing"].as_str()).collect();
        assert_eq!(timing, [Some("At 24% paranoia"), Some("At 24% paranoia"), None, None, Some("On claim"), Some("At 20% Paranoia"), Some("On claim")]);
        assert!(rows[2].get("when").is_none() && rows[3].get("when").is_none(), "no condition from an earlier line");
        assert_eq!(rows[3]["note"], "MW doctrine & last 2 spirits ASAP", "only the parent line, never another line's condition");
        // A header line still scopes every following line.
        let (rows, _) = run(json!([container("When at war:\nAt 5%: C\nD")]));
        let timing: Vec<_> = rows.iter().map(|r| r["action"]["params"]["decomposition"]["timing"].as_str()).collect();
        assert_eq!(timing, [Some("At 5%"), Some("When at war")]);
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
        // Contract 4: a single instruction line becomes manually trackable...
        let (single, _) = run(json!([container("Only one line")]));
        assert_eq!(single[0]["key"], "word-0007--line-1");
        assert!(single[0].get("tracking").is_none());
        // ...but advice, a bare header and auxiliary notes stay reminders, unchanged.
        for (key, text) in [("word-0007", "(If reloading: skip)"), ("word-0007", "Production:"), ("word-0007--note-1", "Fix trade")] {
            let mut row = container(text);
            row["key"] = json!(key);
            let (kept, _) = run(json!([row.clone()]));
            assert_eq!(kept[0], row, "{text}");
        }
    }

    #[test]
    fn explicit_sequences_split_into_ordered_parts_keeping_the_line() {
        // The Soviet guide's step 1, verbatim.
        let line = "Build: INFRA+Land Facility in Moscow, then INFRA+MILs in: Moscow, Kharkov, Krasnodar, Odessa, Vinnytsia, Minsk";
        let (rows, _) = run(json!([container(line)]));
        let titles: Vec<_> = rows.iter().map(|r| r["title"].as_str().unwrap()).collect();
        assert_eq!(titles, ["Build: INFRA+Land Facility in Moscow", "Build: INFRA+MILs in: Moscow, Kharkov, Krasnodar, Odessa, Vinnytsia, Minsk"]);
        assert_eq!(rows[1]["depends_on"], json!(["word-0007--line-1--seq-1"]), "second part follows the first");
        assert_eq!(rows[1]["action"]["params"]["instructions"][0], "INFRA+MILs in: Moscow, Kharkov, Krasnodar, Odessa, Vinnytsia, Minsk");
        assert!(rows[0]["note"].as_str().unwrap().contains(line), "full original line kept");
        // An elided verb is not split.
        assert_eq!(sequence_parts("Justify Gibraltar, then DEI, cancel Gibraltar").len(), 1);
    }

    #[test]
    fn catalog_contradicted_decisions_are_withdrawn_not_replaced() {
        let index = crate::import_catalog::Index::new(&json!({"entries":[
            {"kind":"decision","id":"GER_mobilize_the_selbschutz","name":"Mobilize the Selbstschutz","country_condition":{"op":"tag","value":"GER"}},
            {"kind":"decision","id":"SOV_ok","name":"Soviet decision","country_condition":{"op":"tag","value":"SOV"}}]})).unwrap();
        let step = |key: &str, id: &str| json!({"key":key,"kind":"gate","name":"Mobilize the Selbstschutz","title":"Mobilize The…",
            "action":{"kind":"manual","params":{"instructions":["Mobilize The…"],"decision_id":id,"decision_name":"x"}},
            "completion":{"op":"fact","key":"decisions.executed","test":"contains","value":id},
            "started_when":{"op":"fact","key":"decisions.active","test":"contains","value":id}});
        let mut g = json!({"id":"g","applies_to":{"country":"SOV"},"tracks":{"operations":[step("word-0008","GER_mobilize_the_selbschutz"),step("word-0009","SOV_ok")]}});
        withdraw_incompatible_decisions(&mut g, &index).unwrap();
        let (bad, ok) = (&g["tracks"]["operations"][0], &g["tracks"]["operations"][1]);
        assert!(bad["action"]["params"].get("decision_id").is_none());
        assert_eq!(bad["action"]["params"]["binding_withdrawn"]["reason"], "decision_country_incompatible");
        assert!(bad.get("completion").is_none() && bad.get("started_when").is_none() && bad.get("name").is_none());
        assert_eq!(bad["action"]["params"]["instructions"][0], "Mobilize The…");
        assert_eq!(ok["action"]["params"]["decision_id"], "SOV_ok", "compatible binding untouched");
        // Unprovable ownership (complex condition): only a truncated, foreign-tag
        // match is withdrawn; an exact-name foreign decision is a legitimate control.
        let index = crate::import_catalog::Index::new(&json!({"entries":[
            {"kind":"decision","id":"GER_mobilize_x","name":"Mobilize X","country_condition":{"op":"unknown"}},
            {"kind":"decision","id":"NOR_repair_mines","name":"Repair Mines","country_condition":{"op":"unknown"}}]})).unwrap();
        let mut weak = step("word-0008", "GER_mobilize_x");
        let mut exact = step("word-0010", "NOR_repair_mines");
        exact["action"]["params"]["instructions"] = json!(["Repair Swedish mines"]);
        weak["key"] = json!("word-0008");
        let mut g = json!({"id":"g","applies_to":{"country":"SOV"},"tracks":{"operations":[weak, exact]}});
        withdraw_incompatible_decisions(&mut g, &index).unwrap();
        assert_eq!(g["tracks"]["operations"][0]["action"]["params"]["binding_withdrawn"]["reason"], "weak_foreign_decision_binding");
        assert_eq!(g["tracks"]["operations"][1]["action"]["params"]["decision_id"], "NOR_repair_mines");
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
