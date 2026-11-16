//! Every real game entity a free-text reference could name, for the playing
//! nation: decisions, events and event answers, focuses, laws, ideas,
//! technologies. Uses the catalog's own name/alias/word-prefix matcher with
//! the guide's country and focus tree, plus a prefix matcher over event
//! answers for the author's abbreviations ("A New Bismarck…" for "A New
//! Bismarckjugend should be top priority": cut mid-word, accents and
//! punctuation ignored). Entries the catalog proves unavailable to the country
//! are dropped, unprovable ones are kept and marked. Nothing is chosen here:
//! callers bind only a single compatible candidate and otherwise keep the
//! ambiguity visible.
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
fn fold(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' | 'ą' => 'a',
        'ç' | 'č' | 'ć' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'ę' | 'ě' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'ı' => 'i',
        'ñ' | 'ń' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'ø' | 'ő' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'ű' | 'ů' => 'u',
        'ý' | 'ÿ' => 'y',
        'ł' => 'l',
        'ś' | 'š' => 's',
        'ź' | 'ż' | 'ž' => 'z',
        'ř' => 'r',
        'đ' | 'ď' => 'd',
        'ť' => 't',
        'ğ' => 'g',
        c => c,
    }
}
/// Lowercase accent-free words joined by single spaces. Apostrophes vanish
/// ("That’s" = "thats"); every other non-alphanumeric character separates.
pub fn normalize(text: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in text.chars().flat_map(char::to_lowercase).map(fold) {
        if matches!(c, '\'' | '’' | '‘' | '`' | '´') {
            continue;
        }
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out
}
fn truncated(text: &str) -> bool {
    let t = text.trim_end();
    t.ends_with('…') || t.ends_with("...")
}

/// Normalized event-answer names (name, localized name, aliases) by catalog row.
pub struct AnswerNames(Vec<(String, usize)>);
impl AnswerNames {
    pub fn new(index: &Index) -> Self {
        let mut names = vec![];
        for (i, row) in index.rows().iter().enumerate().filter(|(_, r)| r["kind"] == "event_option") {
            let all = [&row["name"], &row["localized_name"]]
                .into_iter()
                .chain(row["aliases"].as_array().into_iter().flatten());
            for name in all.filter_map(Value::as_str) {
                let n = normalize(name);
                if !n.is_empty() {
                    names.push((n, i));
                }
            }
        }
        names.sort();
        names.dedup();
        Self(names)
    }
    /// Rows whose answer text starts with (truncated) or equals the reference.
    fn rows<'a>(&self, index: &'a Index, text: &str) -> Vec<&'a Value> {
        let q = normalize(&query(text));
        let cut = truncated(text);
        // A full-text reference must be a real sentence, not "Accept".
        if q.chars().filter(|c| c.is_alphanumeric()).count() < 4 || !cut && q.split(' ').count() < 3 {
            return vec![];
        }
        // Sorted names: every match sits in one run starting at the query.
        let start = self.0.partition_point(|(name, _)| name.as_str() < q.as_str());
        let mut out: Vec<usize> = vec![];
        for (_, i) in self.0[start..].iter().take_while(|(name, _)| if cut { name.starts_with(&q) } else { *name == q }) {
            if !out.contains(i) {
                out.push(*i);
            }
        }
        out.into_iter().filter_map(|i| index.rows().get(i)).collect()
    }
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

/// One author reference inside an instruction line.
#[derive(Debug, PartialEq)]
pub struct Reference {
    pub text: String,
    /// The author's "(NNPP)" note, when it belongs to this reference alone.
    pub cost: Option<f64>,
}
/// Timing lead-ins whose right side is the instruction ("On Expand…done: X").
fn timing(text: &str) -> bool {
    crate::import_catalog::regex(r"(?i)^(on|when|at|after|before|asap|around|if|once|shortly|by|until|from|during|in)\b").is_match(text)
}
/// The references an author line names, in the shapes the guides use:
/// "Mobilize The…", "- Recall Kocs…(25 CP)", "They will not… event (50PP)",
/// "Which By God…: CW setup*" (answer, then what to do), "On Expand…done: We
/// will acc…(15PP)" (when, then the answer), "Absolutely Not… + There is a…"
/// (several answers), and an answer written out in full on its own line.
pub fn references(line: &str) -> Vec<Reference> {
    let re = crate::import_catalog::regex;
    let mut text = re(r"^\s*(?:[-•*–]\s+)").replace(line.trim(), "").trim().to_owned();
    if text.contains('\n') {
        return vec![];
    }
    if let Some(colon) = text.find(':') {
        let (left, right) = (text[..colon].trim(), text[colon + 1..].trim());
        if truncated(left) && !timing(left) {
            text = left.to_owned();
        } else if timing(left) && !right.is_empty() {
            text = right.to_owned();
        } else if !right.is_empty() {
            return vec![];
        }
    }
    let cost_note = re(r"(?i)\((\d{1,4})\s*PP\)");
    let costs: Vec<f64> = cost_note.captures_iter(&text).filter_map(|c| c[1].parse().ok()).collect();
    let text = re(r"(?:\s*\([^()]*\))+\s*$").replace(&text, "").trim().to_owned();
    let text = re(r"(?i)\s+event$").replace(&text, "").trim().to_owned();
    // Several answers: split after each truncation mark followed by more text.
    let mut pieces = vec![];
    let mut rest = text.clone();
    while let Some(m) = re(r"(?:…|\.\.\.)").find(&rest) {
        let piece = rest[..m.end()].trim().to_owned();
        let after = re(r"^(?:\(.*?\)\s*)?(?:\+|/|,|&|and\b|or\b)?\s*")
            .replace(rest[m.end()..].trim_start(), "")
            .into_owned();
        pieces.push(piece);
        // Text after the last mark that is not itself a reference ends the
        // list; after a single abbreviation it is other work, not an answer.
        if !after.is_empty() && !re(r"(?:…|\.\.\.)").is_match(&after) {
            if pieces.len() == 1 && after.chars().any(char::is_alphanumeric) {
                return vec![];
            }
            rest.clear();
            break;
        }
        rest = after;
    }
    if pieces.is_empty() {
        let full = rest.trim();
        // A full answer only as the whole line: never a fragment of a sentence.
        if !full.is_empty() && !full.contains(',') && full.chars().count() <= 120 {
            pieces.push(full.to_owned());
        }
    }
    let single = pieces.len() == 1 && costs.len() == 1;
    pieces
        .into_iter()
        .filter(|p| p.chars().count() <= 90)
        .map(|p| Reference { text: p, cost: if single { costs.first().copied() } else { None } })
        .collect()
}

/// A single event answer the nation can give, chosen from the candidates.
fn choose(found: &[Value], cost: Option<f64>, index: &Index, tag: &str) -> Result<Option<(Value, &'static str)>> {
    let compatible: Vec<&Value> = found.iter().filter(|c| c["compatible"] != false).collect();
    let options: Vec<&Value> = compatible.iter().copied().filter(|c| c["kind"] == "event_option").collect();
    // Prefer what the nation can provably receive; the author's cost note must
    // match the answer's real cost; else a single candidate whose event
    // namespace names the nation.
    let confirmed: Vec<&Value> = options.iter().copied().filter(|c| c["compatible"] == true).collect();
    let costed: Vec<&Value> = cost
        .map(|c| options.iter().copied().filter(|o| o["political_power_cost"].as_f64() == Some(c)).collect())
        .unwrap_or_default();
    if let [one] = costed.as_slice() {
        return Ok(Some(((*one).clone(), "political_power_cost_matches")));
    }
    if let [one] = confirmed.as_slice() {
        return Ok(Some(((*one).clone(), "event_received_by_nation")));
    }
    if confirmed.is_empty() && compatible.len() == 1 && options.len() == 1 {
        let option = options[0];
        if event_affinity(option["event_id"].as_str().unwrap_or(""), index, tag)? {
            return Ok(Some((option.clone(), "event_namespace_names_nation")));
        }
    }
    // Among several unconfirmed answers, exactly one whose event names the nation.
    if confirmed.is_empty() && options.len() > 1 {
        let mut named = vec![];
        for option in &options {
            if event_affinity(option["event_id"].as_str().unwrap_or(""), index, tag)? {
                named.push(*option);
            }
        }
        if let [one] = named.as_slice() {
            return Ok(Some(((*one).clone(), "event_namespace_names_nation")));
        }
    }
    Ok(None)
}
fn bind(option: &Value, index: &Index, country: &Value) -> Result<Option<Value>> {
    let event_id = option["event_id"].as_str().unwrap_or("");
    if event_id.is_empty() {
        return Ok(None);
    }
    let probe = json!({"action":{"kind":"manual","params":{"category":"politics","event_id":event_id,"event_option_id":option["id"]}}});
    let result = crate::political_binding::bind(&probe, index, country, false)?;
    Ok((result["reason"] == "event_visible_option").then_some(result))
}

/// Ground the author's event answers ("Mobilize The…", "On Expand…done: We
/// will acc…(15PP)", "Absolutely Not… + There is a…") that earlier passes left
/// unbound: bind a single nation-compatible event answer through the existing
/// political binding (live observation of the event popup, HUD option
/// highlight) and keep everything else a manually tracked instruction with
/// its candidates visible. With several answers on one line the first one
/// found is bound and the others are listed with it.
pub fn ground_references(guide: &mut Value, index: &Index) -> Result<()> {
    let country = guide["applies_to"]["country"].clone();
    let scope = guide["applies_to"]["tree_id"].clone();
    let Some(tag) = country.as_str().map(str::to_owned) else { return Ok(()) };
    let names = AnswerNames::new(index);
    let tracks = guide["tracks"].as_object_mut().ok_or("Missing guide tracks")?;
    for row in tracks.values_mut().filter_map(Value::as_array_mut).flatten() {
        let params = &row["action"]["params"];
        let Some(line) = params["instructions"].as_array().filter(|a| a.len() == 1).and_then(|a| a[0].as_str()) else { continue };
        if row["action"]["kind"] != "manual" || bound(params) || !params["reference_resolution"].is_null() {
            continue;
        }
        let refs = references(line);
        if refs.is_empty() {
            continue;
        }
        let mut answers = vec![];
        let mut unresolved = vec![];
        for reference in &refs {
            let found = candidates_with(&reference.text, index, &country, &scope, Some(&names))?;
            let found = found.as_array().cloned().unwrap_or_default();
            match choose(&found, reference.cost, index, &tag)? {
                Some((option, basis)) => {
                    if let Some(result) = bind(&option, index, &country)? {
                        answers.push((reference, option, basis, result));
                        continue;
                    }
                    unresolved.push((reference, found));
                }
                None => unresolved.push((reference, found)),
            }
        }
        if let Some((reference, option, basis, result)) = answers.first() {
            let mut params = result["step"]["action"]["params"].clone();
            for keep in ["instructions", "source_action"] {
                if !row["action"]["params"][keep].is_null() {
                    params[keep] = row["action"]["params"][keep].clone();
                }
            }
            let also: Vec<Value> = answers[1..].iter().map(|(r, o, b, _)| json!({"text":r.text,"basis":b,"event_id":o["event_id"],"option_id":o["id"],"option_text":o["name"]})).collect();
            params["reference_resolution"] = json!({"basis":basis,"text":reference.text,"event_id":option["event_id"],"option_id":option["id"],"option_text":option["name"]});
            if !also.is_empty() {
                params["reference_resolution"]["also"] = json!(also);
            }
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
        // Nothing bound: show what the first truncated reference could mean.
        if let Some((reference, found)) = unresolved.into_iter().find(|(r, f)| truncated(&r.text) && !f.is_empty()) {
            let compatible: Vec<Value> = found.iter().filter(|c| c["compatible"] != false).cloned().collect();
            row["action"]["params"]["reference_candidates"] = if compatible.is_empty() {
                json!({"status":"no_compatible_candidate","text":reference.text,"candidates":found.iter().take(6).cloned().collect::<Vec<_>>()})
            } else {
                json!({"status":if compatible.len() == 1 {"single_unconfirmed"} else {"ambiguous"},"text":reference.text,
                    "candidates":compatible.into_iter().take(6).collect::<Vec<_>>()})
            };
        }
    }
    Ok(())
}

pub fn candidates(text: &str, index: &Index, country: &Value, scope: &Value) -> Result<Value> {
    candidates_with(text, index, country, scope, None)
}
fn candidates_with(text: &str, index: &Index, country: &Value, scope: &Value, names: Option<&AnswerNames>) -> Result<Value> {
    let q = query(text);
    if q.chars().filter(|c| c.is_alphanumeric()).count() < 4 {
        return Ok(json!([]));
    }
    let actor = country.as_str();
    let built;
    let names = match names {
        Some(names) => names,
        None => {
            built = AnswerNames::new(index);
            &built
        }
    };
    // Event answers the abbreviation is a prefix of come first: the author's
    // own convention; the catalog matcher adds other kinds of entries.
    let mut rows: Vec<Value> = names.rows(index, text).into_iter().cloned().collect();
    if truncated(text) {
        rows.extend(index.matches(&q, KINDS, country, scope, false)?);
    }
    let mut out = vec![];
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        let (kind, id) = (row["kind"].as_str().unwrap_or(""), row["id"].as_str().unwrap_or(""));
        if id.is_empty() || !seen.insert((kind.to_owned(), id.to_owned())) {
            continue;
        }
        let mut ownership = if row["country_condition"].is_null() {
            None
        } else {
            crate::decision_admission::ownership(&row["country_condition"], actor, None, 0)
        };
        // An option's own trigger ("true") says nothing about the nation; the
        // nation is decided by who can receive the event (event_reach).
        let event_id = if kind == "event" { id } else { row["event_id"].as_str().unwrap_or("") };
        if ["event", "event_option"].contains(&kind) {
            if ownership == Some(true) {
                ownership = None;
            }
            if let (Some(actor), false) = (actor, event_id.is_empty()) {
                let parents = index.entries(json!({"op":"matches","query":event_id,"kinds":["event"],"country":country,"explicit":true}))?;
                let receives = parents.iter().filter_map(|p| crate::event_reach::receives(&p["receivers"], &p["receivers_unknown"], actor)).collect::<Vec<_>>();
                if !receives.is_empty() && receives.iter().all(|r| !r) {
                    continue;
                }
                if receives.contains(&true) {
                    ownership = Some(true);
                }
            }
        }
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
        out.push(json!({"kind":kind,"id":id,"name":row["name"],"event_id":row["event_id"],"political_power_cost":row["political_power_cost"],
            "compatible":match ownership { Some(true) => json!(true), _ => if foreign { json!(false) } else { Value::Null } },
            "foreign_tag":foreign}));
    }
    Ok(json!(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn texts(line: &str) -> Vec<(String, Option<f64>)> {
        references(line).into_iter().map(|r| (r.text, r.cost)).collect()
    }
    #[test]
    fn author_line_shapes_yield_their_answers() {
        assert_eq!(texts("Mobilize The…"), [("Mobilize The…".into(), None)]);
        assert_eq!(texts("We will seize…(25PP) (if possible)"), [("We will seize…".into(), Some(25.0))]);
        assert_eq!(texts("- Recall Kocs…(25 CP)"), [("Recall Kocs…".into(), None)]);
        assert_eq!(texts("They will not… event (50PP)"), [("They will not…".into(), Some(50.0))]);
        assert_eq!(texts("Which By God…: CW setup*,spy Madrid"), [("Which By God…".into(), None)]);
        assert_eq!(texts("On Expand…done: We will acc…(15PP)"), [("We will acc…".into(), Some(15.0))]);
        assert_eq!(texts("On event, wait 10 days, then: Clearly The Opposition…"), [("Clearly The Opposition…".into(), None)]);
        assert_eq!(texts("Absolutely Not… + There is a… (150PP)"), [("Absolutely Not…".into(), None), ("There is a…".into(), None)]);
        assert_eq!(texts("Refuse to scrap… / Do nothing…"), [("Refuse to scrap…".into(), None), ("Do nothing…".into(), None)]);
        assert_eq!(texts("We cannot…, We shall…"), [("We cannot…".into(), None), ("We shall…".into(), None)]);
        assert_eq!(texts("Reject The Saltsjobaden Agreement"), [("Reject The Saltsjobaden Agreement".into(), None)]);
        // An abbreviation followed by other work is not an answer list.
        assert!(texts("Good luck…, pick Army offense (50PP)").is_empty());
        assert!(texts("On Power…: Economist (150PP)").iter().all(|(t, _)| t == "Economist"));
        assert!(texts("Line one\nLine two").is_empty());
    }
    #[test]
    fn normalization_ignores_case_accents_and_punctuation() {
        assert_eq!(normalize("Which, by God, …"), "which by god");
        assert_eq!(normalize("That’s Capitalism!"), "thats capitalism");
        assert_eq!(normalize("Österreich"), "osterreich");
        assert!(normalize("A New Bismarckjugend should be top priority").starts_with(&normalize(&query("A New Bismarck…"))));
    }
}
