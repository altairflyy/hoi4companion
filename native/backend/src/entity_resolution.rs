//! Game-grounded entity resolution for free-text references (compiler contract 3).
//!
//! Recognition is span-based and wording-independent: any contiguous token span
//! of a reference may match a candidate's full name; words outside the span
//! ("On", "After", "Finish", "when ... done") never decide WHAT is referenced.
//! Supported evidence, all generic:
//! - exact tokens, case/diacritics/punctuation/quotes folded;
//! - incomplete names: a token followed by `…`/`...` may be a prefix of the
//!   name token and the name may continue after it ("Seek…", "Translei…");
//! - abbreviated tokens: a token of >= 3 letters that is a prefix of the name
//!   token ("Org" -> "Organize"), except short all-caps tokens, which are
//!   country/unit labels and must match exactly;
//! - several names per candidate: author title, localized name, ID words.
//! Candidates come only from real game data: the guide's own grounded steps
//! (nation, focus tree, chronology) and catalog rows of the same tree. A
//! reference is `resolved` only when one entity has the unique best match;
//! equal best matches of different entities (focus vs event, two focuses) are
//! `ambiguous` and listed; otherwise `unresolved` with a reason. Nothing is
//! guessed, and intent is decided separately (`trigger_condition`).
use crate::import_catalog::regex;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub kind: String,
    pub id: String,
    pub names: Vec<Vec<String>>,
    /// Word-table ordinal of the step that grounded it (chronology, display only).
    pub ordinal: Option<u64>,
    pub source: &'static str,
}

/// Fold to lowercase ASCII-ish words; diacritics removed where trivial.
pub fn words(text: &str) -> Vec<String> {
    let folded: String = text
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            c => c,
        })
        .collect::<String>()
        .to_lowercase();
    regex(r"[a-z0-9]+").find_iter(&folded).map(|m| m.as_str().to_owned()).collect()
}
fn ordinal(key: &str) -> Option<u64> {
    regex(r"^word-(\d+)").captures(key).and_then(|c| c[1].parse().ok())
}
fn kind_of(step: &Value) -> Option<&'static str> {
    match (step["action"]["kind"].as_str()?, step["action"]["entity"]["kind"].as_str()) {
        ("select_focus", _) | (_, Some("focus")) => Some("focus"),
        ("research", _) | (_, Some("technology")) => Some("technology"),
        (_, Some("event")) => Some("event"),
        (_, Some("decision")) => Some("decision"),
        _ => None,
    }
}
/// ID words without the leading country tag / mod prefix ("wuw_HUN_end_x" -> end x).
fn id_words(id: &str, country: &str) -> Vec<String> {
    let mut parts: Vec<&str> = id.split('_').filter(|p| !p.is_empty()).collect();
    let tag = |p: &str| p.len() == 3 && p.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    // Only the country tag and a lowercase mod prefix before a tag ("wuw_HUN")
    // are prefixes; short real words ("new", "the") stay part of the name.
    while parts.len() > 2
        && (parts[0].eq_ignore_ascii_case(country) || tag(parts[0])
            || (parts[0].chars().all(|c| c.is_ascii_lowercase()) && parts[0].len() <= 4 && tag(parts[1])))
    {
        parts.remove(0);
    }
    parts.iter().flat_map(|p| words(p)).collect()
}
fn strip_note(title: &str) -> &str {
    title.split(['(', '/']).next().unwrap_or(title).trim()
}

/// Grounded candidates from the guide's own steps (already resolved to real IDs).
pub fn guide_candidates(guide: &Value) -> Vec<Candidate> {
    let country = guide["applies_to"]["country"].as_str().unwrap_or("");
    let mut by_id: BTreeMap<(String, String), Candidate> = BTreeMap::new();
    for step in guide["tracks"].as_object().into_iter().flat_map(|m| m.values()).flat_map(|v| v.as_array().into_iter().flatten()) {
        let Some(kind) = kind_of(step) else { continue };
        let Some(id) = step["action"]["entity"]["id"].as_str().filter(|s| !s.is_empty()) else { continue };
        let entry = by_id.entry((kind.to_owned(), id.to_owned())).or_insert_with(|| Candidate {
            kind: kind.into(), id: id.into(), names: vec![], ordinal: ordinal(step["key"].as_str().unwrap_or("")), source: "guide",
        });
        for name in [step["title"].as_str(), step["name"].as_str(), step["target"].as_str().filter(|t| !t.contains('_'))].into_iter().flatten() {
            let w = words(strip_note(name));
            if !w.is_empty() && !entry.names.contains(&w) {
                entry.names.push(w);
            }
        }
        let w = id_words(id, country);
        if !w.is_empty() && !entry.names.contains(&w) {
            entry.names.push(w);
        }
    }
    by_id.into_values().collect()
}
/// Catalog rows of the guide's focus tree (localized name + aliases).
pub fn catalog_candidates(rows: &[Value], guide: &Value) -> Vec<Candidate> {
    let tree = &guide["applies_to"]["tree_id"];
    if !tree.is_string() {
        return vec![];
    }
    rows.iter()
        .filter(|r| r["kind"] == "focus" && (r["scope"] == *tree || r["tree_id"] == *tree))
        .filter_map(|r| {
            let id = r["id"].as_str()?;
            let names: Vec<Vec<String>> = std::iter::once(&r["name"])
                .chain(r["aliases"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .map(words)
                .filter(|w| !w.is_empty())
                .collect();
            Some(Candidate { kind: "focus".into(), id: id.into(), names, ordinal: None, source: "catalog" })
        })
        .collect()
}
pub fn merge(mut a: Vec<Candidate>, b: Vec<Candidate>) -> Vec<Candidate> {
    for c in b {
        if let Some(existing) = a.iter_mut().find(|x| x.kind == c.kind && x.id == c.id) {
            for n in c.names {
                if !existing.names.contains(&n) {
                    existing.names.push(n);
                }
            }
        } else {
            a.push(c);
        }
    }
    a
}

struct Token {
    word: String,
    raw: String,
    truncated: bool,
    label: bool,
}
fn tokens(text: &str) -> Vec<Token> {
    let re = regex(r"[\p{L}\p{N}]+");
    let mut out: Vec<Token> = vec![];
    for m in re.find_iter(text) {
        let tail = &text[m.end()..];
        let truncated = tail.starts_with('…') || tail.starts_with("...");
        for word in words(m.as_str()) {
            out.push(Token {
                label: m.as_str().len() <= 3 && m.as_str().chars().all(|c| c.is_ascii_uppercase()),
                raw: m.as_str().into(),
                word,
                truncated: false,
            });
        }
        if let Some(last) = out.last_mut() {
            last.truncated = truncated;
        }
    }
    out
}

/// Best match of one name against any span of the reference.
/// Returns (score, span start, span end, kind of match).
fn match_name(reference: &[Token], name: &[String]) -> Option<(u32, usize, usize, &'static str)> {
    let mut best: Option<(u32, usize, usize, &'static str)> = None;
    for start in 0..reference.len() {
        let (mut j, mut score, mut exact, mut partial, mut cut) = (0usize, 0u32, 0usize, false, false);
        while j < name.len() && start + j < reference.len() {
            let r = &reference[start + j];
            let n = &name[j];
            let prefix_ok = !r.label && r.word.len() >= 3 && n.starts_with(&r.word);
            if r.word == *n {
                exact += 1;
                score += 3 + n.len() as u32;
            } else if (r.truncated && n.starts_with(&r.word)) || prefix_ok {
                cut |= r.truncated;
                score += 1 + r.word.len() as u32;
            } else {
                break;
            }
            j += 1;
            if r.truncated && j < name.len() {
                partial = true;
                break;
            }
        }
        let complete = j == name.len();
        if j == 0 || !(complete || partial) {
            continue;
        }
        // Minimum evidence: enough characters, and at least one real word.
        let chars: usize = reference[start..start + j].iter().map(|t| t.word.len()).sum();
        if chars < 4 || (exact == 0 && j == 1 && !partial && !cut) {
            continue;
        }
        let kind = if partial || cut { "truncated" } else if exact == j { "exact" } else { "abbreviated" };
        // Full names outrank truncations; more matched words outrank fewer.
        let score = score + if complete { 50 } else { 0 } + 4 * j as u32;
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, start, start + j, kind));
        }
    }
    best
}

pub fn resolve(text: &str, candidates: &[Candidate]) -> Value {
    let reference = tokens(text);
    let mut scored: Vec<(u32, &Candidate, usize, usize, &'static str)> = vec![];
    for c in candidates {
        if let Some((score, s, e, how)) = c.names.iter().filter_map(|n| match_name(&reference, n)).max_by_key(|m| m.0) {
            scored.push((score, c, s, e, how));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.id.cmp(&b.1.id)));
    let list = |items: &[(u32, &Candidate, usize, usize, &'static str)]| -> Value {
        json!(items.iter().take(6).map(|(score, c, ..)| json!({"kind":c.kind,"id":c.id,"score":score,"ordinal":c.ordinal,"source":c.source})).collect::<Vec<_>>())
    };
    let Some(top) = scored.first() else {
        return json!({"status":"unresolved","reason":"no_grounded_candidate","text":text});
    };
    let tied: Vec<_> = scored.iter().filter(|s| s.0 == top.0).cloned().collect();
    let distinct: std::collections::HashSet<_> = tied.iter().map(|s| (&s.1.kind, &s.1.id)).collect();
    let (score, c, s, e, how) = top.clone();
    let residual: Vec<String> = reference.iter().enumerate().filter(|(i, _)| *i < s || *i >= e).map(|(_, t)| t.word.clone()).collect();
    if distinct.len() > 1 {
        return json!({"status":"ambiguous","reason":"equal_best_candidates","text":text,"candidates":list(&tied),"residual_words":residual});
    }
    let matched: Vec<&str> = reference[s..e].iter().map(|t| t.raw.as_str()).collect();
    json!({"status":"resolved","text":text,"entity":{"kind":c.kind,"id":c.id},"match":how,"score":score,
        "matched_text":matched.join(" "),"residual_words":residual,"source":c.source,"ordinal":c.ordinal,
        "alternatives":list(&scored[1..])})
}

/// Closed-class words that may surround an entity reference in trigger
/// position without adding meaning of their own (temporal/selection
/// connectives, articles, auxiliaries, entity-kind words). Any other leftover
/// word (a second condition "& GER down", an alternative "or ASAP", a
/// different predicate "trained", a quantity, a negation) means the trigger is
/// NOT fully understood.
const FUNCTION_WORDS: &[&str] = &[
    "on", "upon", "after", "before", "when", "once", "at", "as", "soon", "right", "just", "shortly",
    "the", "a", "an", "of", "is", "are", "was", "has", "have", "had", "been", "being", "it", "its",
    "pick", "picks", "picking", "picked", "select", "selecting", "selected", "take", "taking", "taken",
    "start", "starting", "started", "get", "getting", "got", "reach", "reaching", "reached",
    "finish", "finishing", "finished", "complete", "completing", "completed", "done", "end", "ended",
    "research", "researching", "researched", "tech", "technology", "focus", "national",
];
const COMPLETED_WORDS: &[&str] = &["after", "once", "finish", "finishing", "finished", "complete", "completing", "completed", "done", "end", "ended", "researched"];
/// Whether everything outside the matched entity is function words.
pub fn fully_understood(resolution: &Value) -> bool {
    resolution["residual_words"].as_array().is_some_and(|w| w.iter().filter_map(Value::as_str).all(|w| FUNCTION_WORDS.contains(&w)))
}

/// Intent for a reference in TRIGGER position (a header or the scope part of
/// an inline `scope: action`): when does the scoped instruction apply?
/// Function words decide the relation: completion words -> "completed",
/// "before" -> "before" (negated), otherwise "reached" (started or completed).
/// Returns the observable condition, or None with the reason why automatic
/// verification is withheld.
pub fn trigger_condition(resolution: &Value) -> (Option<Value>, &'static str, &'static str) {
    if resolution["status"] != "resolved" {
        return (None, "none", "reference_not_resolved");
    }
    if !fully_understood(resolution) {
        return (None, "none", "residual_condition_not_understood");
    }
    let residual: Vec<&str> = resolution["residual_words"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let before = residual.contains(&"before");
    let completed = residual.iter().any(|w| COMPLETED_WORDS.contains(w));
    if before && completed {
        return (None, "none", "conflicting_temporal_relation");
    }
    let relation = if before { "before" } else if completed { "completed" } else { "reached" };
    let id = &resolution["entity"]["id"];
    let (active, done) = match resolution["entity"]["kind"].as_str() {
        Some("focus") => ("focus.active", "focus.completed"),
        Some("technology") => ("research.active", "research.completed"),
        Some("event") => return (None, relation, "event_firing_not_persistently_observable"),
        _ => return (None, relation, "entity_kind_not_observable"),
    };
    let done = json!({"op":"fact","key":done,"test":"contains","value":id});
    let reached = json!({"op":"any","args":[{"op":"fact","key":active,"test":"contains","value":id},done]});
    let condition = match relation {
        "completed" => done,
        "before" => json!({"op":"not","arg":reached}),
        _ => reached,
    };
    (Some(condition), relation, "observable")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn focus(id: &str, title: &str, ordinal: u64) -> Candidate {
        Candidate { kind: "focus".into(), id: id.into(), names: vec![words(title)], ordinal: Some(ordinal), source: "guide" }
    }
    fn pool() -> Vec<Candidate> {
        vec![
            focus("ITA_seek_peace", "Seek Peace With The Allies", 10),
            focus("ITA_expand_lateran", "Expand Lateran Treaty", 12),
            focus("ITA_organize_army", "Organize the Military", 3),
            focus("POL_pan_slavic", "Pan-Slavic Congress", 20),
            focus("GER_rhineland", "Rhineland", 2),
            focus("GER_reassert_east", "Reassert the East", 30),
        ]
    }
    fn id(r: &Value) -> &str {
        r["entity"]["id"].as_str().unwrap_or("")
    }

    #[test]
    fn recognition_is_independent_of_leading_wording() {
        let p = pool();
        for text in ["On Seek…", "After Seek…", "Seek…", "When Seek… is done", "finish “Seek…”"] {
            assert_eq!(id(&resolve(text, &p)), "ITA_seek_peace", "{text}");
        }
        assert_eq!(id(&resolve("Pan-Slavic…", &p)), "POL_pan_slavic");
        assert_eq!(id(&resolve("right after the Rhineland", &p)), "GER_rhineland");
        assert_eq!(resolve("Reassert…", &p)["match"], "truncated");
    }

    #[test]
    fn abbreviations_and_incomplete_names_need_real_evidence() {
        let p = pool();
        let r = resolve("Org the Mil", &p);
        assert_eq!((id(&r), r["match"].as_str().unwrap()), ("ITA_organize_army", "abbreviated"));
        // Mid-word truncation of the last word.
        assert_eq!(id(&resolve("On Expand Later…", &p)), "ITA_expand_lateran");
        // Too little evidence or an unfinished name without truncation mark.
        assert_eq!(resolve("On Exp", &p)["status"], "unresolved");
        assert_eq!(resolve("Expand Lateran", &p)["status"], "unresolved", "incomplete name without … is not assumed");
    }

    #[test]
    fn country_labels_are_not_abbreviations() {
        let mut p = pool();
        p.push(focus("X_finland_pact", "Finland Pact", 5));
        assert_eq!(resolve("After FIN down", &p)["status"], "unresolved");
        assert_eq!(resolve("Rhine…", &p)["status"], "resolved");
    }

    #[test]
    fn equal_best_matches_stay_ambiguous_across_entities_and_kinds() {
        let mut p = pool();
        p.push(focus("ITA_seek_glory", "Seek Glory", 40));
        let r = resolve("On Seek…", &p);
        assert_eq!(r["status"], "ambiguous");
        assert_eq!(r["candidates"].as_array().unwrap().len(), 2);
        let mut q = pool();
        q.push(Candidate { kind: "event".into(), id: "ita.15".into(), names: vec![words("Rhineland")], ordinal: None, source: "catalog" });
        assert_eq!(resolve("On Rhineland:", &q)["status"], "ambiguous", "focus vs event with the same name");
    }

    #[test]
    fn intent_is_decided_from_trigger_position_and_residual_words() {
        let p = pool();
        let (c, relation, why) = trigger_condition(&resolve("On Seek…", &p));
        assert_eq!((relation, why), ("reached", "observable"));
        assert_eq!(c.unwrap()["op"], "any");
        let (c, relation, _) = trigger_condition(&resolve("After Seek…", &p));
        assert_eq!(relation, "completed");
        assert_eq!(c.unwrap(), json!({"op":"fact","key":"focus.completed","test":"contains","value":"ITA_seek_peace"}));
        let event = vec![Candidate { kind: "event".into(), id: "ita.15".into(), names: vec![words("Spanish Civil War")], ordinal: None, source: "catalog" }];
        let (c, _, why) = trigger_condition(&resolve("On Spanish Civil War", &event));
        assert!(c.is_none());
        assert_eq!(why, "event_firing_not_persistently_observable");
        assert_eq!(trigger_condition(&resolve("When GER close to cap", &p)).2, "reference_not_resolved");
    }

    #[test]
    fn leftover_meaning_withholds_automatic_conditions() {
        let mut p = pool();
        p.push(Candidate { kind: "technology".into(), id: "paratroopers".into(), names: vec![words("Paratroopers")], ordinal: None, source: "guide" });
        // Same entity, but extra conditions/alternatives/predicates are not understood.
        for text in ["On Reassert… & GER down", "On Seek…, or ASAP afterwards", "When Paratroopers Trained", "On Seek… if not at war"] {
            let r = resolve(text, &p);
            assert_eq!(r["status"], "resolved", "{text}: entity is still recognized");
            let (c, _, why) = trigger_condition(&r);
            assert!(c.is_none(), "{text}");
            assert_eq!(why, "residual_condition_not_understood");
        }
        // Kind words and selection verbs are not extra meaning.
        assert!(trigger_condition(&resolve("When picking “Seek…”", &p)).0.is_some());
        assert!(trigger_condition(&resolve("On Paratroopers research", &p)).0.is_some());
        // "Before" keeps the polarity: applicable until the focus is reached.
        let (c, relation, _) = trigger_condition(&resolve("Shortly before Rhineland", &p));
        assert_eq!(relation, "before");
        assert_eq!(c.unwrap()["op"], "not");
        assert_eq!(trigger_condition(&resolve("Before Seek… is done", &p)).2, "conflicting_temporal_relation");
    }

    #[test]
    fn guide_steps_provide_grounded_candidates() {
        let guide = json!({"applies_to":{"country":"HUN"},"tracks":{"focus":[
            {"key":"word-0004","title":"End The Regency","name":"Fine della reggenza","action":{"kind":"select_focus","entity":{"kind":"focus","id":"wuw_HUN_end_the_regency"}}},
            {"key":"word-0009","title":"Electronic Mechanical Engineering","action":{"kind":"research","entity":{"kind":"technology","id":"electronic_mechanical_engineering"}}}]}});
        let c = guide_candidates(&guide);
        assert_eq!(c.len(), 2);
        assert!(c[0].names.contains(&words("fine della reggenza")), "localized name kept");
        assert_eq!(id(&resolve("On End The…", &c)), "wuw_HUN_end_the_regency");
        assert_eq!(id(&resolve("After Electronic Mech…", &c)), "electronic_mechanical_engineering");
    }
}
