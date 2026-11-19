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
/// A game name without its inline icons and colour codes.
fn strip_markup(text: &str) -> String {
    crate::import_catalog::regex(r"£[^\s£]*£?|§[A-Za-z!]").replace_all(text, " ").into_owned()
}
/// Lowercase accent-free words joined by single spaces. Apostrophes vanish
/// ("That’s" = "thats"); every other non-alphanumeric character separates.
pub fn normalize(text: &str) -> String {
    // Game markup is not part of a name: inline icons ("£BoP_right_texticon
    // Take Over Ministry of Corporations") and colour codes ("§Y…§!").
    let text = strip_markup(text);
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

/// Entity kinds besides event answers that an author abbreviation may name
/// and that an existing binding observes live. Focuses are listed only so a
/// focus with the same name keeps the reference ambiguous.
pub const NAMED_KINDS: &[&str] = &["decision", "advisor", "technology", "idea", "focus"];

/// Normalized names (name, localized name, aliases) by catalog row, for the
/// given entity kinds (event answers by default).
pub struct AnswerNames(Vec<(String, usize)>);
impl AnswerNames {
    pub fn new(index: &Index) -> Self {
        Self::of_kinds(index, &["event_option"])
    }
    pub fn of_kinds(index: &Index, kinds: &[&str]) -> Self {
        let mut names = vec![];
        for (i, row) in index.rows().iter().enumerate().filter(|(_, r)| r["kind"].as_str().is_some_and(|k| kinds.contains(&k))) {
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
        self.spelled_rows(index, text).into_iter().map(|(row, _)| row).collect()
    }
    /// The same rows, each with whether it matches the reference's own words
    /// (true) or only one of their other spellings (false, `spellings`).
    fn spelled_rows<'a>(&self, index: &'a Index, text: &str) -> Vec<(&'a Value, bool)> {
        let q = normalize(&query(text));
        let cut = truncated(text);
        // A full-text reference must be a real sentence, not "Accept".
        if q.chars().filter(|c| c.is_alphanumeric()).count() < 4 || !cut && q.split(' ').count() < 3 {
            return vec![];
        }
        let mut out: Vec<(usize, bool)> = vec![];
        for (n, q) in spellings(&q).into_iter().enumerate() {
            // Sorted names: every match sits in one run starting at the query.
            let start = self.0.partition_point(|(name, _)| name.as_str() < q.as_str());
            for (_, i) in self.0[start..].iter().take_while(|(name, _)| if cut { name.starts_with(&q) } else { *name == q }) {
                match out.iter_mut().find(|(j, _)| j == i) {
                    Some((_, literal)) => *literal |= n == 0,
                    None => out.push((*i, n == 0)),
                }
            }
        }
        out.into_iter().filter_map(|(i, literal)| index.rows().get(i).map(|row| (row, literal))).collect()
    }
}
/// Whether an answer's text starts with (a truncated reference) or equals the
/// reference's words, in any of their spellings: the test the search uses.
pub fn answer_matches(reference: &str, answer: &str) -> bool {
    let q = normalize(&query(reference));
    let name = normalize(answer);
    let cut = truncated(reference);
    !q.is_empty() && !name.is_empty() && spellings(&q).iter().any(|q| if cut { name.starts_with(q.as_str()) } else { name == *q })
}
/// The same words in their other spellings, for a normalized query: "cannot",
/// "can not" and "can't" (apostrophes are already gone: "cant"). Only whole
/// words change; the query itself comes first.
fn spellings(q: &str) -> Vec<String> {
    let words: Vec<&str> = q.split(' ').collect();
    let mut out = vec![q.to_owned()];
    for (i, word) in words.iter().enumerate() {
        let (len, found) = match (*word, words.get(i + 1)) {
            ("can", Some(&"not")) => (2, true),
            ("cannot" | "cant", _) => (1, true),
            _ => (0, false),
        };
        if !found {
            continue;
        }
        for spelling in ["cannot", "can not", "cant"] {
            let variant = [&words[..i], &[spelling], &words[i + len..]].concat().join(" ");
            if !out.contains(&variant) {
                out.push(variant);
            }
        }
        break;
    }
    out
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
    ["decision_id", "event_option_id", "advisor_id", "law_id", "idea_id", "operation_id", "production_targets", "building_id", "technology_id"]
        .iter()
        .any(|k| !params[*k].is_null())
}

/// An author cost note: amount and currency (PP political power, CP command
/// power, AE/NE army/navy experience).
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Cost {
    pub amount: f64,
    pub unit: &'static str,
}
impl Cost {
    /// The catalog field holding this currency's real cost, when the catalog has one.
    fn field(&self) -> Option<&'static str> {
        match self.unit {
            "PP" => Some("political_power_cost"),
            "CP" => Some("command_power_cost"),
            _ => None,
        }
    }
    fn matches(&self, row: &Value) -> bool {
        self.field().and_then(|f| row[f].as_f64()) == Some(self.amount)
    }
}
/// One author reference inside an instruction line.
#[derive(Debug, PartialEq)]
pub struct Reference {
    pub text: String,
    /// The author's cost note ("(25PP)", "(25 CP)"), when it belongs to this reference.
    pub cost: Option<Cost>,
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
    split_references(line).0
}
/// The references of a line, and whether they are the whole of its work: false
/// when the pieces are alternatives ("A… / B…", "A… or B…"), when other work
/// follows them ("A… + B… & SPA war*") or when a piece was dropped. Only a whole
/// list may complete the step, and only once every piece is observed.
fn split_references(line: &str) -> (Vec<Reference>, bool) {
    let re = crate::import_catalog::regex;
    let mut whole = true;
    let mut text = re(r"^\s*(?:[-•*–]\s+)").replace(line.trim(), "").trim().to_owned();
    if text.contains('\n') {
        return (vec![], true);
    }
    // "On X…: action" / "On X… action": only the action names references.
    if let Some((_, action)) = trigger(&text) {
        text = action;
    }
    if let Some(colon) = text.find(':') {
        let (left, right) = (text[..colon].trim(), text[colon + 1..].trim());
        if truncated(left) && !timing(left) {
            text = left.to_owned();
        } else if timing(left) && !right.is_empty() {
            text = right.to_owned();
        } else if !right.is_empty() {
            return (vec![], true);
        }
    }
    // Trailing notes ("(25PP) (if possible)"): the first cost note counts.
    let trailing = re(r"(?:\s*\([^()]*\))+\s*$").find(&text).map(|m| m.as_str().to_owned()).unwrap_or_default();
    let tail_cost = re(r"\([^()]*\)").find_iter(&trailing).find_map(|m| note_cost(m.as_str()));
    let text = re(r"(?:\s*\([^()]*\))+\s*$").replace(&text, "").trim().to_owned();
    let text = re(r"(?i)\s+event$").replace(&text, "").trim().to_owned();
    // Several answers: split after each truncation mark followed by more text.
    let mut pieces = vec![];
    // The cost note written right after a piece ("Liturgical…(70PP) + …").
    let mut own: Vec<Option<(Cost, u32)>> = vec![];
    let mut rest = text.clone();
    while let Some(m) = re(r"(?:…|\.\.\.)").find(&rest) {
        let piece = rest[..m.end()].trim().to_owned();
        let tail = rest[m.end()..].trim_start();
        own.push(re(r"^\([^()]*\)").find(tail).and_then(|n| note_cost(n.as_str())));
        let separator = re(r"^(?:\(.*?\)\s*)?(\+|/|,|&|and\b|or\b)?").captures(tail).and_then(|c| c.get(1)).map(|m| m.as_str().to_owned());
        let after = re(r"^(?:\(.*?\)\s*)?(?:\+|/|,|&|and\b|or\b)?\s*")
            .replace(tail, "")
            .into_owned();
        pieces.push(piece);
        if !after.is_empty() && matches!(separator.as_deref(), Some("/" | "or")) {
            whole = false;
        }
        // Text after the last mark that is not itself a reference ends the
        // list; after a single abbreviation it is other work, not an answer.
        if !after.is_empty() && !re(r"(?:…|\.\.\.)").is_match(&after) {
            if pieces.len() == 1 && after.chars().any(char::is_alphanumeric) {
                return (vec![], true);
            }
            if after.chars().any(char::is_alphanumeric) {
                whole = false;
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
    // Which piece a cost note belongs to: a note right after a piece is its
    // own; the trailing note belongs to a single piece, to the last piece when
    // every other piece has its own note, or to each of N pieces when it says
    // "xN" ("Prepare For…& Ignite… (50PPx2)"). A note after several pieces
    // without "xN" may be their sum ("A… + B… (150PP)") and decides nothing.
    let n = pieces.len();
    own.resize(n, None);
    let mut costs: Vec<Option<Cost>> = own.iter().map(|c| c.and_then(|(c, times)| (times == 1).then_some(c))).collect();
    if let Some((cost, times)) = tail_cost {
        if n == 1 && times == 1 {
            costs[0] = Some(cost);
        } else if n > 1 && times as usize == n && own.iter().all(Option::is_none) {
            costs = vec![Some(cost); n];
        } else if n > 1 && times == 1 && own[n - 1].is_none() && own[..n - 1].iter().all(Option::is_some) {
            costs[n - 1] = Some(cost);
        }
    }
    if pieces.iter().any(|p| p.chars().count() > 90) {
        whole = false;
    }
    let refs = pieces
        .into_iter()
        .zip(costs)
        .filter(|(p, _)| p.chars().count() <= 90)
        .map(|(p, cost)| Reference { text: p, cost })
        .collect();
    (refs, whole)
}
/// "(25PP)", "(25 CP)", "(50PPx2)", "(10 AE)": amount, currency, multiplier.
fn note_cost(note: &str) -> Option<(Cost, u32)> {
    let hit = crate::import_catalog::regex(r"(?i)^\(\s*(\d{1,4})\s*(PP|CP|AE|NE)\s*(?:x\s*(\d))?\s*\)$").captures(note.trim())?;
    let unit = match hit[2].to_uppercase().as_str() {
        "PP" => "PP",
        "CP" => "CP",
        "AE" => "AE",
        _ => "NE",
    };
    let times = hit.get(3).and_then(|t| t.as_str().parse().ok()).unwrap_or(1);
    Some((Cost { amount: hit[1].parse().ok()?, unit }, times))
}

/// A single event answer the nation can give, chosen from the candidates.
fn choose(found: &[Value], cost: Option<Cost>, index: &Index, tag: &str) -> Result<Option<(Value, &'static str)>> {
    let compatible: Vec<&Value> = found.iter().filter(|c| c["compatible"] != false).collect();
    let options: Vec<&Value> = compatible.iter().copied().filter(|c| c["kind"] == "event_option").collect();
    // Prefer what the nation can provably receive; the author's cost note must
    // match the answer's real cost; else a single candidate whose event
    // namespace names the nation.
    let confirmed: Vec<&Value> = options.iter().copied().filter(|c| c["compatible"] == true).collect();
    let costed: Vec<&Value> = cost
        .map(|c| options.iter().copied().filter(|o| c.matches(o)).collect())
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
/// Among several candidates, the only event answer whose event a focus the
/// guide picks, or another nation's focus on the AI historical path, leads to
/// (the same traced chains as the suite's events page, `event_routes`).
/// Candidates the catalog proves the nation cannot receive never count.
fn reached(found: &[Value], routes: &crate::event_routes::Chains, tag: &str, guide: &[String], historical: &std::collections::HashSet<&str>) -> Option<(Value, &'static str)> {
    let compatible: Vec<&Value> = found.iter().filter(|c| c["compatible"] != false).collect();
    if compatible.len() < 2 {
        return None;
    }
    let reach = |event: &str| {
        if routes.from_guide(event, guide) {
            Some("event_reached_from_guide_focus")
        } else if routes.historical_roots(event, tag, historical).next().is_some() {
            Some("event_reached_from_historical_focus")
        } else {
            None
        }
    };
    let kept: Vec<(&Value, &'static str)> = compatible
        .into_iter()
        .filter(|c| c["kind"] == "event_option")
        .filter_map(|c| reach(c["event_id"].as_str().unwrap_or("")).map(|basis| (c, basis)))
        .collect();
    match kept.as_slice() {
        [(one, basis)] => Some(((*one).clone(), basis)),
        _ => None,
    }
}
/// Decisions, advisors, technologies and ideas whose real name the truncated
/// abbreviation is a prefix of ("Recall Kocs…", "Albert Speer…", "Small
/// Caliber Semi…"), with the same nation rules as `candidates_with`: entries
/// the catalog proves unavailable to the nation are dropped, an advisor of
/// another nation's character file is dropped, a country-tagged ID of
/// another nation is dropped. `compatible` is true only when the catalog
/// proves the nation can use it. Focuses of the guide's own tree are kept
/// (kind "focus") so the caller can see a same-named focus.
fn named_candidates(text: &str, index: &Index, names: &AnswerNames, tag: &str, scope: &Value) -> Vec<Value> {
    if !truncated(text) {
        return vec![];
    }
    let mut out: Vec<Value> = vec![];
    for row in names.rows(index, text) {
        let (kind, id) = (row["kind"].as_str().unwrap_or(""), row["id"].as_str().unwrap_or(""));
        if id.is_empty() || out.iter().any(|o| o["kind"] == kind && o["id"] == id) {
            continue;
        }
        if kind == "focus" {
            if scope.is_string() && (row["scope"] == *scope || row["tree_id"] == *scope) {
                out.push(json!({"kind":kind,"id":id,"name":row["name"],"compatible":true}));
            }
            continue;
        }
        let ownership = if row["country_condition"].is_null() {
            None
        } else {
            crate::decision_admission::ownership(&row["country_condition"], Some(tag), None, 0)
        };
        if ownership == Some(false) {
            continue;
        }
        let prefix = id.split(['_', '.']).next().unwrap_or("");
        let tagged = prefix.len() == 3 && prefix.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        if tagged && prefix != tag && kind != "technology" {
            continue;
        }
        let mut compatible = ownership == Some(true) || (tagged && prefix == tag);
        if kind == "advisor" {
            let countries: Vec<&str> = row["countries"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            if !countries.is_empty() && !countries.contains(&tag) {
                continue;
            }
            compatible |= countries.contains(&tag);
        }
        let name = row["name"].as_str().map(|n| strip_markup(n).split_whitespace().collect::<Vec<_>>().join(" "));
        out.push(json!({"kind":kind,"id":id,"name":name,"political_power_cost":row["political_power_cost"],
            "command_power_cost":row["command_power_cost"],"targeted":row["targeted"],
            "compatible":if compatible { json!(true) } else { Value::Null }}));
    }
    out
}
/// A single named entity the nation can use: never beside a same-named
/// focus of the guide's tree; the author's cost note must not contradict the
/// real cost and, when several names fit, picks exactly one; otherwise one
/// candidate the catalog proves usable (technologies are common to every
/// nation). Anything else stays a list of candidates.
fn choose_named(found: &[Value], cost: Option<Cost>) -> Option<(Value, &'static str)> {
    if found.iter().any(|c| c["kind"] == "focus") {
        return None;
    }
    let costed = |c: &Value| ["political_power_cost", "command_power_cost"].iter().any(|f| c[*f].is_number());
    let contradicted = |c: &Value| cost.is_some_and(|k| k.field().is_some() && costed(c) && !k.matches(c));
    let fit: Vec<&Value> = found.iter().filter(|c| !contradicted(c)).collect();
    if let Some(k) = cost {
        let exact: Vec<&Value> = fit.iter().copied().filter(|c| k.matches(c)).collect();
        if let [one] = exact.as_slice() {
            return Some(((*one).clone(), "cost_matches"));
        }
    }
    if found.len() != 1 || fit.len() != 1 {
        return None;
    }
    let one = fit[0];
    if one["compatible"] == true {
        Some((one.clone(), "unique_name_for_nation"))
    } else if one["kind"] == "technology" {
        Some((one.clone(), "unique_technology_name"))
    } else {
        None
    }
}
/// Bind a named entity through the existing bindings: decisions, advisors
/// and ideas via `political_binding` (live decision/advisor/idea facts),
/// technologies via the research facts. Returns the bound step, only when it
/// carries a live-observable condition.
fn bind_named(row: &Value, entity: &Value, index: &Index, country: &Value) -> Result<Option<Value>> {
    let id = entity["id"].clone();
    let mut probe = row.clone();
    if probe["action"].get("params").is_none() {
        probe["action"]["params"] = json!({});
    }
    let expected = match entity["kind"].as_str() {
        Some("technology") => {
            probe["action"]["params"]["technology_id"] = id.clone();
            probe["started_when"] = json!({"op":"fact","key":"research.active","test":"contains","value":id});
            probe["completion"] = json!({"op":"fact","key":"research.completed","test":"contains","value":id});
            return Ok(Some(probe));
        }
        Some("decision") => ("decision_id", "decision_identity_resolved"),
        Some("advisor") => ("advisor_id", "politics_live_advisor"),
        Some("idea") => ("idea_id", "politics_live_idea"),
        _ => return Ok(None),
    };
    probe["action"]["params"][expected.0] = id;
    let result = crate::political_binding::bind(&probe, index, country, false)?;
    let step = &result["step"];
    let live = step["completion"].is_object() || step["started_when"].is_object();
    Ok((result["reason"] == expected.1 && live).then(|| step.clone()))
}

/// The trigger of a "trigger: action" line ("On Expand…done: …", "On
/// Picking Reaffirm…: …", "When … / After … / Around …: …"), with its action.
///
/// The trigger ends at the colon, or, without a colon right after it, at its
/// first truncation mark ("On Coal… Develop Ersatz… (30PP)"), keeping a
/// closing quote, a "done" and a parenthetical that follow the mark ("On The
/// National… (or when JAP war on CHI): …" keeps the alternative inside the
/// trigger, so it is not mistaken for a plain focus condition).
pub fn trigger(line: &str) -> Option<(String, String)> {
    let re = crate::import_catalog::regex;
    let text = re(r"^\s*(?:[-•*–]\s+)").replace(line.trim(), "").trim().to_owned();
    if text.contains('\n') || !timing(&text) {
        return None;
    }
    let colon = text.find(':');
    let mark = re(r"…|\.\.\.").find(&text).map(|m| m.end());
    let by_colon = match (colon, mark) {
        (Some(c), Some(m)) if m <= c => {
            let between = &text[m..c];
            !re(r"…|\.\.\.").is_match(between) && between.chars().count() <= 30
        }
        (Some(_), _) => true,
        _ => false,
    };
    let (left, right) = if by_colon {
        let c = colon?;
        (text[..c].trim().to_owned(), text[c + 1..].trim().to_owned())
    } else {
        let m = mark?;
        let tail = re(r#"^(?:\s*[”"’'])?(?:\s*done\b)?(?:\s*\([^()]*\))?"#).find(&text[m..]).map_or(0, |t| t.end());
        let end = m + tail;
        (text[..end].trim().to_owned(), text[end..].trim().trim_start_matches(':').trim().to_owned())
    };
    (!right.is_empty() && left.chars().count() <= 80).then_some((left, right))
}
/// The step's only instruction, or the first line of a multi-line block
/// (true): the block's lead line, whose trigger scopes the whole block.
/// Community country abbreviations the game's names do not contain.
fn country_aliases(text: &str) -> String {
    let re = crate::import_catalog::regex;
    // "the USSR", "the UK": the article is not part of the name.
    let text = re(r"(?i)\bthe\s+(USSR|UK|US)\b").replace_all(text, "$1");
    let text = re(r"\bUSSR\b").replace_all(&text, "SOV");
    let text = re(r"\bUS\b").replace_all(&text, "USA");
    re(r"\bUK\b").replace_all(&text, "ENG").into_owned()
}
/// What a written-out action asks, once its trigger and bullet are removed.
fn action_text(line: &str) -> String {
    let text = trigger(line).map(|t| t.1).unwrap_or_else(|| crate::import_catalog::regex(r"^\s*(?:[-•*–]\s+)").replace(line.trim(), "").trim().to_owned());
    country_aliases(text.trim())
}
/// Actions the game state read live cannot show: recognized with their
/// targets so the card says what is asked, never tracked.
fn unobservable_action(text: &str, index: &Index) -> Result<Option<Value>> {
    let re = crate::import_catalog::regex;
    // Trailing notes ("(50PP)", "(if needed)") do not change what is asked;
    // the line is only recognized here, never tracked.
    let text = re(r"(?:\s*\([^()]*\))+\s*$").replace(text, "").trim().to_owned();
    let Some(hit) = re(r"(?i)^(justify(?:\s+(?:a\s+)?war\s*goals?)?|annex|integrate|invade|collabs?(?:oration)?(?:\s+government)?)\s+(?:(?:on|against|in)\s+)?(.+)$").captures(&text) else {
        return Ok(None);
    };
    let verb = hit[1].to_lowercase();
    let action = match verb.split_whitespace().next().unwrap_or("") {
        "justify" => "justify_war_goal",
        "annex" => "annex",
        "integrate" => "integrate",
        "invade" => "invade",
        _ => "collaboration_government",
    };
    let mut targets = vec![];
    for part in re(r"\s*(?:,|&|\band\b)\s*").split(&hit[2]) {
        let found = index.query(json!({"op":"resolve_country","query":part}))?;
        let Some(tag) = found.as_str().filter(|t| !t.is_empty()) else { return Ok(None) };
        targets.push(tag.to_owned());
    }
    Ok((!targets.is_empty()).then(|| json!({"status":"not_observable","action":action,"targets":targets,"text":text})))
}
/// "REL ENG, FRA, GER (40PP)": one action on several countries, rewritten as
/// one clause per country ("REL ENG, REL FRA, REL GER (40PP)") so the foreign
/// parser reads it. Every item after the verb must be a country.
fn one_action_on_several(text: &str, index: &Index) -> Result<Option<String>> {
    let re = crate::import_catalog::regex;
    let cost = re(r"\s*\(\d+\s*PP\)\s*$").find(text).map(|m| m.as_str().trim().to_owned());
    let body = re(r"\s*\(\d+\s*PP\)\s*$").replace(text, "").trim().to_owned();
    let Some(first) = re(r"\s*(?:,|&|\band\b)\s*").find(&body) else { return Ok(None) };
    let head = body[..first.start()].trim();
    let Some(space) = head.rfind(char::is_whitespace) else { return Ok(None) };
    let verb = head[..space].trim();
    let mut targets = vec![head[space..].trim().to_owned()];
    targets.extend(re(r"\s*(?:,|&|\band\b)\s*").split(&body[first.end()..]).map(|t| t.trim().to_owned()));
    for t in &targets {
        if !index.query(json!({"op":"resolve_country","query":t}))?.as_str().is_some_and(|s| !s.is_empty()) {
            return Ok(None);
        }
    }
    let mut out = targets.iter().map(|t| format!("{verb} {t}")).collect::<Vec<_>>().join(", ");
    if let Some(cost) = cost {
        out = format!("{out} {cost}");
    }
    Ok(Some(out))
}
/// A written-out foreign action on an existing single-line step ("REL ENG,
/// FRA (40PP)", "war on GRE"): bound through the foreign-interaction binding
/// when the whole line parses, every target resolving to a country; several
/// targets complete only when each one is observed, each kept as its own
/// durable milestone. Unobservable actions are recognized only.
fn ground_action(row: &mut Value, line: &str, index: &Index) -> Result<()> {
    if row.get("completion").is_some() {
        return Ok(());
    }
    let text = action_text(line);
    if text.is_empty() {
        return Ok(());
    }
    let parsed = match crate::command_families::foreign(&text, index)? {
        Some(items) => Some(items),
        None => match one_action_on_several(&text, index)? {
            Some(expanded) => crate::command_families::foreign(&expanded, index)?,
            None => None,
        },
    };
    if let Some(items) = parsed {
        let items = items.as_array().cloned().unwrap_or_default();
        let mut parts = vec![];
        for item in &items {
            let mut probe = row.clone();
            for (k, v) in item["params"].as_object().into_iter().flatten() {
                probe["action"]["params"][k] = v.clone();
            }
            let result = crate::political_binding::foreign(&probe, index)?;
            if result["bound"] != true {
                return Ok(());
            }
            parts.push(result["step"].clone());
        }
        let describe: Vec<Value> = parts
            .iter()
            .map(|s| json!({"action":s["action"]["params"]["mission_id"].as_str().unwrap_or_else(|| s["action"]["params"]["operation_id"].as_str().unwrap_or("")),"target":s["action"]["params"]["target_country"]}))
            .collect();
        let resolution = json!({"status":"bound","actions":describe,"text":text});
        if let [single] = parts.as_slice() {
            *row = single.clone();
            row["action"]["params"]["action_resolution"] = resolution;
        } else if !parts.is_empty() {
            row["completion"] = json!({"op":"all","args":parts.iter().map(|s| s["completion"].clone()).collect::<Vec<_>>()});
            row["started_when"] = json!({"op":"any","args":parts.iter().map(|s| s["started_when"].clone()).collect::<Vec<_>>()});
            row["action"]["params"]["foreign_parts"] = json!(parts.iter().map(|s| s["action"]["params"].clone()).collect::<Vec<_>>());
            row["action"]["params"]["action_resolution"] = resolution;
        }
        if let Some(map) = row.as_object_mut() {
            map.remove("tracking");
        }
        return Ok(());
    }
    if let Some(recognized) = unobservable_action(&text, index)? {
        row["action"]["params"]["action_resolution"] = recognized;
    }
    Ok(())
}
pub(crate) fn first_line(row: &Value) -> Option<(String, bool)> {
    let text = row["action"]["params"]["instructions"].as_array().filter(|a| a.len() == 1).and_then(|a| a[0].as_str())?;
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next()?.to_owned();
    Some((first, lines.next().is_some()))
}
/// A real `when` the step already has (not promote's unknown placeholder).
fn real_when(row: &Value) -> bool {
    row.get("when").is_some_and(|w| !(w["op"] == "fact" && w["key"].as_str().is_some_and(|k| k.starts_with("guide.conditions."))))
}
/// Family 1: the author's trigger ("On Expand…done: …") resolved against the
/// guide's grounded focuses/technologies/decisions and its focus tree
/// (`entity_resolution`, the contract 3 rules). An observable condition
/// becomes the step's `when` (only where it has none or promote's unknown
/// placeholder); otherwise the step records why it stays unknown.
fn ground_trigger(row: &mut Value, candidates: &[crate::entity_resolution::Candidate], index: &Index) {
    let Some((line, block)) = first_line(row) else { return };
    // A block may open with a bare header ("On Organize…:") over its lines.
    let header = line.strip_suffix(':').map(str::trim).filter(|h| block && timing(h) && !h.contains(':'));
    let Some(left) = trigger(&line).map(|t| t.0).or(header.map(str::to_owned)) else { return };
    if !(left.contains('…') || left.contains("...")) || !row["action"]["params"]["trigger_resolution"].is_null() || real_when(row) {
        return;
    }
    let resolution = crate::entity_resolution::resolve_trigger(&left, candidates);
    let (condition, relation, verification) = crate::entity_resolution::trigger_condition(&resolution);
    let entity = &resolution["entity"];
    let name = index
        .rows()
        .iter()
        .find(|r| r["kind"] == entity["kind"] && r["id"] == entity["id"])
        .map(|r| r["name"].clone())
        .unwrap_or(Value::Null);
    let mut record = json!({"text":left,"status":if condition.is_some() { "compiled" } else { "unknown" },"relation":relation,
        "verification":verification,"resolution_status":resolution["status"]});
    if entity.is_object() {
        record["entity"] = json!({"kind":entity["kind"],"id":entity["id"],"name":name});
    }
    if resolution["candidates"].is_array() {
        record["candidates"] = resolution["candidates"].clone();
    }
    // An equal best match settled by the guide's own steps keeps the other
    // tied entities visible.
    if resolution["disambiguation"].is_string() {
        record["disambiguation"] = resolution["disambiguation"].clone();
        record["candidates"] = resolution["alternatives"].clone();
    }
    if let Some(condition) = condition {
        row["when"] = condition;
    }
    row["action"]["params"]["trigger_resolution"] = record;
}

pub(crate) fn bind(option: &Value, index: &Index, country: &Value) -> Result<Option<Value>> {
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
/// found is bound and the others are listed with it. Decisions, advisors,
/// technologies and ideas named by their abbreviation ("Recall Kocs…(25 CP)",
/// "Albert Speer…") are bound the same way when no event answer fits, and the
/// trigger of a "On X…: action" line becomes the step's observable `when`.
/// Several event answers are narrowed to the one whose event the guide's own
/// focuses or other nations' historical AI focuses (`historical`, catalog
/// source "historical_focuses") lead to.
pub fn ground_references(guide: &mut Value, index: &Index, historical: &Value) -> Result<()> {
    let country = guide["applies_to"]["country"].clone();
    let scope = guide["applies_to"]["tree_id"].clone();
    let Some(tag) = country.as_str().map(str::to_owned) else { return Ok(()) };
    let guide_focuses = crate::event_routes::guide_focuses(guide);
    let historical = crate::event_routes::historical_set(historical);
    // Traced over the whole catalog, only when a line needs it.
    let mut routes: Option<crate::event_routes::Chains> = None;
    let names = AnswerNames::new(index);
    let named = AnswerNames::of_kinds(index, NAMED_KINDS);
    // Trigger candidates: the guide's own grounded steps and its focus tree.
    let triggers = crate::entity_resolution::merge(
        crate::entity_resolution::guide_candidates(guide),
        crate::entity_resolution::catalog_candidates(index.rows(), guide),
    );
    let tracks = guide["tracks"].as_object_mut().ok_or("Missing guide tracks")?;
    // Triggers first, so every line can see the focus anchor that follows it.
    for row in tracks.values_mut().filter_map(Value::as_array_mut).flatten() {
        if row["action"]["kind"] == "manual" {
            ground_trigger(row, &triggers, index);
        }
    }
    let timing = crate::event_timing::Timing::new(index.rows());
    let plan = crate::event_timing::planned_days(&guide_focuses, &timing);
    let earliest = earliest_days(index.rows());
    for steps in tracks.values_mut().filter_map(Value::as_array_mut) {
    let anchors = line_anchors(steps, &plan);
    for (position, row) in steps.iter_mut().enumerate() {
        let params = &row["action"]["params"];
        let Some((line, block)) = first_line(row) else { continue };
        if row["action"]["kind"] != "manual" || bound(params) || !params["reference_resolution"].is_null() {
            continue;
        }
        let (refs, whole) = split_references(&line);
        if refs.is_empty() {
            // No abbreviation: the line may still be a written-out action
            // ("Justify HUN, YUG (60PP)", "REL UK, FRA (40PP)", "war on GRE").
            if !block {
                ground_action(row, &line, index)?;
            }
            continue;
        }
        // Each reference: an event answer first (the authors' main use), else
        // a decision, advisor, technology or idea named by its abbreviation.
        let mut grounded: Vec<(&Reference, Value, &'static str, Value)> = vec![];
        let mut unresolved = vec![];
        let mut timelines: Vec<(String, Value)> = vec![];
        let mut literals: Vec<(String, Value)> = vec![];
        for reference in &refs {
            let found = candidates_with(&reference.text, index, &country, &scope, Some(&names))?;
            let mut found = found.as_array().cloned().unwrap_or_default();
            let mut chosen = choose(&found, reference.cost, index, &tag)?;
            if chosen.is_none() && found.iter().filter(|c| c["kind"] == "event_option" && c["compatible"] != false).count() > 0 {
                let routes = routes.get_or_insert_with(|| crate::event_routes::chains(index.rows(), &tag));
                chosen = reached(&found, routes, &tag, &guide_focuses, &historical);
            }
            // Still several: answers with the guide's own words come before
            // those found only through another spelling ("can't" for "cannot").
            if chosen.is_none() {
                let dropped = spelling_only(&found);
                if !dropped.is_empty() {
                    found.retain(|c| !(c["kind"] == "event_option" && dropped.iter().any(|d| c["id"] == *d)));
                    let mut record = json!({"dropped":dropped});
                    let mut left = choose(&found, reference.cost, index, &tag)?;
                    if left.is_none() {
                        let routes = routes.get_or_insert_with(|| crate::event_routes::chains(index.rows(), &tag));
                        left = reached(&found, routes, &tag, &guide_focuses, &historical);
                    }
                    if let Some((mut one, then)) = left {
                        record["then"] = json!(then);
                        one["literal"] = record;
                        chosen = Some((one, "literal_text"));
                    } else {
                        literals.push((reference.text.clone(), record));
                    }
                }
            }
            // Still several: drop answers whose event cannot fire before the
            // day the guide plans to reach the next focus after this line.
            if chosen.is_none()
                && let Some((anchor, planned)) = &anchors[position]
            {
                let dropped = timeline(&found, &earliest, *planned);
                if !dropped.is_empty() {
                    found.retain(|c| !(c["kind"] == "event_option" && dropped.iter().any(|(d, _)| c["id"] == *d)));
                    let mut record = json!({"anchor_focus":anchor,"planned_day":planned,"focus_speed_factor":crate::event_timing::FOCUS_SPEED_FACTOR,
                        "dropped":dropped.iter().map(|(id, day)| json!({"id":id,"earliest_day":day})).collect::<Vec<_>>()});
                    // The existing rules again on what is left; else the only answer left.
                    let compatible: Vec<&Value> = found.iter().filter(|c| c["compatible"] != false).collect();
                    record["left"] = json!(compatible.len());
                    record["left_received"] = json!(compatible.iter().filter(|c| c["compatible"] == true).filter_map(|c| c["id"].as_str()).collect::<Vec<_>>());
                    let mut left = choose(&found, reference.cost, index, &tag)?;
                    if left.is_none() {
                        let routes = routes.get_or_insert_with(|| crate::event_routes::chains(index.rows(), &tag));
                        left = reached(&found, routes, &tag, &guide_focuses, &historical);
                    }
                    // Only an answer the nation provably receives: the plan alone
                    // never turns an unconfirmed answer into a binding.
                    if left.is_none() && let [one] = compatible.as_slice() && one["kind"] == "event_option" && one["compatible"] == true {
                        left = Some(((*one).clone(), "single_answer_left"));
                    }
                    if let Some((mut one, then)) = left {
                        record["then"] = json!(then);
                        record["earliest_day"] = json!(earliest_of(&earliest, one["event_id"].as_str().unwrap_or("")));
                        one["timeline"] = record;
                        chosen = Some((one, "guide_timeline"));
                    } else {
                        timelines.push((reference.text.clone(), record));
                    }
                }
            }
            if let Some((option, basis)) = chosen
                && let Some(result) = bind(&option, index, &country)?
            {
                grounded.push((reference, option, basis, result));
                continue;
            }
            let entities = named_candidates(&reference.text, index, &named, &tag, &scope);
            // A compatible event answer with the same words keeps it ambiguous.
            if !found.iter().any(|c| c["kind"] == "event_option" && c["compatible"] != false)
                && let Some((entity, basis)) = choose_named(&entities, reference.cost)
                && let Some(step) = bind_named(row, &entity, index, &country)?
            {
                grounded.push((reference, entity, basis, step));
                continue;
            }
            // Named prefix matches lead the list shown on the card.
            for entity in entities.into_iter().rev() {
                if !found.iter().any(|c| c["kind"] == entity["kind"] && c["id"] == entity["id"]) {
                    found.insert(0, entity);
                }
            }
            unresolved.push((reference, found));
        }
        let describe = |r: &Reference, e: &Value, basis: &str| {
            if e["kind"] == "event_option" {
                let mut out = json!({"text":r.text,"basis":basis,"kind":"event_option","id":e["id"],"name":e["name"],"event_id":e["event_id"],"option_id":e["id"],"option_text":e["name"]});
                // A plan, not a proof: the guide timeline that left this answer.
                if e["timeline"].is_object() {
                    out["timeline"] = e["timeline"].clone();
                }
                // The answers set aside because only another spelling matched.
                if e["literal"].is_object() {
                    out["literal"] = e["literal"].clone();
                }
                out
            } else {
                json!({"text":r.text,"basis":basis,"kind":e["kind"],"id":e["id"],"name":e["name"]})
            }
        };
        if let Some((reference, entity, basis, result)) = grounded.first() {
            let step = if entity["kind"] == "event_option" { &result["step"] } else { result };
            let mut params = step["action"]["params"].clone();
            for keep in ["instructions", "source_action", "trigger_resolution"] {
                if !row["action"]["params"][keep].is_null() {
                    params[keep] = row["action"]["params"][keep].clone();
                }
            }
            params["reference_resolution"] = describe(reference, entity, basis);
            let also: Vec<Value> = grounded[1..].iter().map(|(r, e, b, _)| describe(r, e, b)).collect();
            if !also.is_empty() {
                params["reference_resolution"]["also"] = json!(also);
            }
            // References of the same line that stay unresolved are listed too.
            let open: Vec<Value> = unresolved
                .iter()
                .filter(|(r, _)| truncated(&r.text))
                .map(|(r, f)| json!({"text":r.text,"candidates":f.iter().filter(|c| c["compatible"] != false).take(4).cloned().collect::<Vec<_>>()}))
                .collect();
            if !open.is_empty() {
                params["reference_resolution"]["unresolved"] = json!(open);
            }
            // The lead line of a block holding other work: recognized and
            // shown, never bound, so the block is not completed by it alone.
            // The same for a line of several actions unless every action is
            // grounded and is the whole of the line's work: alternatives
            // ("A… / B…"), an unresolved piece or trailing work ("& SPA war*")
            // must not let one observed action complete the step.
            let several = refs.len() > 1;
            if block || several && (!whole || !unresolved.is_empty()) {
                let mut recognized = params["reference_resolution"].clone();
                recognized["scope"] = json!(if block { "first_line" } else { "line" });
                recognized["tracked"] = json!(false);
                row["action"]["params"]["reference_resolution"] = recognized;
                continue;
            }
            let completion_of = |e: &Value, result: &Value| -> Value {
                if e["kind"] == "event_option" { result["step"]["completion"].clone() } else { result["completion"].clone() }
            };
            let completions: Vec<Value> = grounded.iter().map(|(_, e, _, result)| completion_of(e, result)).collect();
            // Every action of the line has to be observed: a missing
            // completion for any piece keeps the step manual.
            if several && completions.iter().any(Value::is_null) {
                let mut recognized = params["reference_resolution"].clone();
                recognized["scope"] = json!("line");
                recognized["tracked"] = json!(false);
                row["action"]["params"]["reference_resolution"] = recognized;
                continue;
            }
            row["action"]["params"] = params;
            if let Some(map) = row.as_object_mut() {
                map.remove("tracking");
            }
            for field in ["completion", "started_when"] {
                if !step[field].is_null() {
                    row[field] = step[field].clone();
                }
            }
            if several {
                row["completion"] = json!({"op":"all","args":completions});
            }
            continue;
        }
        // Nothing bound and no abbreviation: a written-out line that is not a
        // name ("war on GRE") may still be an action.
        if !block && refs.iter().all(|r| !truncated(&r.text)) {
            ground_action(row, &line, index)?;
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
            // What the guide timeline ruled out, when it narrowed the list.
            if let Some((_, record)) = timelines.iter().find(|(text, _)| *text == reference.text) {
                row["action"]["params"]["reference_candidates"]["timeline"] = record.clone();
            }
            if let Some((_, record)) = literals.iter().find(|(text, _)| *text == reference.text) {
                row["action"]["params"]["reference_candidates"]["literal"] = record.clone();
            }
            // Every event answer still possible (the list above shows a few):
            // what the HUD may recognize in a visible popup (`hud_targets`).
            let options: Vec<Value> = found.iter().filter(|c| c["kind"] == "event_option" && c["compatible"] != false)
                .map(|c| json!({"id":c["id"],"event_id":c["event_id"],"compatible":c["compatible"]})).collect();
            if !options.is_empty() && options.len() <= MAX_EVENT_OPTIONS {
                row["action"]["params"]["reference_candidates"]["event_options"] = json!(options);
            }
        }
    }
    }
    Ok(())
}

/// The guide focus each line comes before: the next step in the same track
/// that is a focus step or whose trigger compiled to a focus ("On Organize…"),
/// with the day the guide plans that focus to complete (`event_timing::planned_days`).
fn line_anchors(steps: &[Value], plan: &std::collections::HashMap<String, f64>) -> Vec<Option<(String, f64)>> {
    let anchor = |row: &Value| -> Option<String> {
        if row["action"]["entity"]["kind"] == "focus" {
            return row["action"]["entity"]["id"].as_str().map(str::to_owned);
        }
        let trigger = &row["action"]["params"]["trigger_resolution"];
        (trigger["status"] == "compiled" && trigger["entity"]["kind"] == "focus").then(|| trigger["entity"]["id"].as_str().map(str::to_owned)).flatten()
    };
    let mut out = vec![None; steps.len()];
    let mut next: Option<(String, f64)> = None;
    for (i, row) in steps.iter().enumerate().rev() {
        out[i] = next.clone();
        if let Some(focus) = anchor(row) {
            next = plan.get(&focus).map(|day| (focus, *day));
        }
    }
    out
}
/// Earliest possible day of each event, from the catalog (`earliest_day`,
/// `event_timing`): `None` when any definition of the event has no bound.
fn earliest_days(rows: &[Value]) -> std::collections::HashMap<&str, Option<f64>> {
    let mut out: std::collections::HashMap<&str, Option<f64>> = std::collections::HashMap::new();
    for row in rows.iter().filter(|r| r["kind"] == "event") {
        let day = row["earliest_day"].as_f64();
        let entry = out.entry(row["id"].as_str().unwrap_or("")).or_insert(day);
        *entry = match (*entry, day) {
            (Some(a), Some(b)) => Some(a.min(b)),
            _ => None,
        };
    }
    out
}
fn earliest_of(days: &std::collections::HashMap<&str, Option<f64>>, event: &str) -> Option<f64> {
    days.get(event).copied().flatten()
}
/// Most event answers a candidate line keeps in full (`event_options`); a
/// longer list is not kept, so nothing is recognized from it.
const MAX_EVENT_OPTIONS: usize = 128;
/// Event answers among several compatible ones found only through another
/// spelling of the reference's words, when some compatible answer matches
/// those words literally; none otherwise (the spellings are then all there is).
fn spelling_only(found: &[Value]) -> Vec<String> {
    let options: Vec<&Value> = found.iter().filter(|c| c["compatible"] != false && c["kind"] == "event_option").collect();
    if options.len() < 2 || !options.iter().any(|c| c["spelling_variant"] != true) {
        return vec![];
    }
    options.into_iter().filter(|c| c["spelling_variant"] == true).filter_map(|c| c["id"].as_str().map(str::to_owned)).collect()
}
/// Event answers among several compatible ones whose event certainly cannot
/// fire by `planned` (its earliest possible day is later), with that day.
/// Candidates whose timing is unknown, and other kinds, are never dropped.
fn timeline(found: &[Value], days: &std::collections::HashMap<&str, Option<f64>>, planned: f64) -> Vec<(String, f64)> {
    let compatible: Vec<&Value> = found.iter().filter(|c| c["compatible"] != false).collect();
    if compatible.iter().filter(|c| c["kind"] == "event_option").count() < 2 {
        return vec![];
    }
    compatible
        .into_iter()
        .filter(|c| c["kind"] == "event_option")
        .filter_map(|c| earliest_of(days, c["event_id"].as_str().unwrap_or("")).filter(|day| *day > planned).map(|day| (c["id"].as_str().unwrap_or("").to_owned(), day)))
        .collect()
}

/// Whether an event answer can show in a single-player game where the human
/// plays `actor`, from the catalog's `player_requirement` (what the option's
/// own trigger certainly requires, "ROOT" being the receiving nation): false
/// when another nation named by its tag must be human-played, or the actor
/// must be the AI, by its tag or as ROOT when the catalog proves the actor
/// receives the event (`received`; otherwise ROOT may be another nation).
/// No requirement read: true (unknown stays unknown).
pub fn single_player(requirement: &Value, actor: &str, received: bool) -> bool {
    let tags = |key: &str| requirement[key].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>();
    !tags("human").iter().any(|t| *t != "ROOT" && *t != actor) && !tags("ai").iter().any(|t| *t == actor || received && *t == "ROOT")
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
    let spelled = names.spelled_rows(index, text);
    // Rows found only through another spelling of the reference's words.
    let variants: std::collections::HashSet<(String, String)> = spelled.iter().filter(|(_, literal)| !literal)
        .map(|(r, _)| (r["kind"].as_str().unwrap_or("").to_owned(), r["id"].as_str().unwrap_or("").to_owned())).collect();
    let mut rows: Vec<Value> = spelled.into_iter().map(|(row, _)| row.clone()).collect();
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
                if kind == "event_option" && !single_player(&row["player_requirement"], actor, receives.contains(&true)) {
                    continue;
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
        let mut candidate = json!({"kind":kind,"id":id,"name":row["name"],"event_id":row["event_id"],"political_power_cost":row["political_power_cost"],
            "compatible":match ownership { Some(true) => json!(true), _ => if foreign { json!(false) } else { Value::Null } },
            "foreign_tag":foreign});
        if variants.contains(&(kind.to_owned(), id.to_owned())) {
            candidate["spelling_variant"] = json!(true);
        }
        out.push(candidate);
    }
    Ok(json!(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn texts(line: &str) -> Vec<(String, Option<f64>)> {
        references(line).into_iter().map(|r| (r.text, r.cost.map(|c| c.amount))).collect()
    }
    fn units(line: &str) -> Vec<Option<&'static str>> {
        references(line).into_iter().map(|r| r.cost.map(|c| c.unit)).collect()
    }
    #[test]
    fn several_actions_keep_their_own_cost_notes() {
        assert_eq!(texts("Liturgical…(70PP) + Convoke…(100PP)"), [("Liturgical…".into(), Some(70.0)), ("Convoke…".into(), Some(100.0))]);
        assert_eq!(texts("Prepare For…& Ignite… (50PPx2)"), [("Prepare For…".into(), Some(50.0)), ("Ignite…".into(), Some(50.0))]);
        // A note after several pieces may be their sum: it decides nothing.
        assert_eq!(texts("Absolutely Not… + There is a… (150PP)"), [("Absolutely Not…".into(), None), ("There is a…".into(), None)]);
        // "x3" over two pieces does not fit either.
        assert_eq!(texts("A thing… + Other… (50PPx3)"), [("A thing…".into(), None), ("Other…".into(), None)]);
        assert_eq!(units("- Recall Kocs…(25 CP)"), [Some("CP")]);
        assert_eq!(units("Take Over Ministry Of Corp… (50PP)"), [Some("PP")]);
        assert_eq!(texts("Take Over Ministry Of Corp… (50PP)"), [("Take Over Ministry Of Corp…".into(), Some(50.0))]);
    }
    #[test]
    fn trigger_lines_split_into_trigger_and_action() {
        assert_eq!(trigger("On Expand…done: We will acc…(15PP)"), Some(("On Expand…done".into(), "We will acc…(15PP)".into())));
        assert_eq!(trigger("- On Picking Reaffirm…: Albert Speer…").map(|t| t.0), Some("On Picking Reaffirm…".into()));
        assert_eq!(trigger("On “Appeal for…”: Liturgical…(70PP)").map(|t| t.1), Some("Liturgical…(70PP)".into()));
        assert_eq!(trigger("Around Expand Later…: Small Caliber Semi…").map(|t| t.0), Some("Around Expand Later…".into()));
        // Without a colon the trigger ends at its truncation mark.
        assert_eq!(trigger("On Coal… Develop Ersatz… (30PP)"), Some(("On Coal…".into(), "Develop Ersatz… (30PP)".into())));
        assert_eq!(texts("On Coal… Develop Ersatz… (30PP)"), [("Develop Ersatz…".into(), Some(30.0))]);
        assert_eq!(trigger("After The People’s… Force deploy").map(|t| t.0), Some("After The People’s…".into()));
        assert_eq!(trigger("On “Push Past…” War on: Costa Rica").map(|t| t.0), Some("On “Push Past…” War on".into()));
        // An alternative written after the mark stays part of the trigger.
        assert_eq!(trigger("On The National… (or when JAP war on CHI, if at least 160PP): Justify Gibraltar (24PP)").map(|t| t.0),
            Some("On The National… (or when JAP war on CHI, if at least 160PP)".into()));
        assert_eq!(trigger("On Military…done/ before claim expire: Invade FRA").map(|t| t.0), Some("On Military…done/ before claim expire".into()));
        // No trigger: an answer then other work, or a plain line.
        assert!(trigger("Which By God…: CW setup").is_none());
        assert!(trigger("Recall Kocs…(25 CP)").is_none());
        assert!(trigger("On X:").is_none());
    }
    fn names(entries: &[(&str, &str, &str)]) -> Vec<Value> {
        entries.iter().map(|(k, id, c)| json!({"kind":k,"id":id,"name":id,"compatible":if *c == "y" { json!(true) } else { Value::Null },
            "political_power_cost":if id.ends_with("50") { json!(50.0) } else { Value::Null }})).collect()
    }
    #[test]
    fn a_named_entity_is_bound_only_when_unique_and_usable() {
        let pp = |n: f64| Some(Cost { amount: n, unit: "PP" });
        assert_eq!(choose_named(&names(&[("decision", "ITA_liturgical", "y")]), None).map(|c| c.1), Some("unique_name_for_nation"));
        // Unprovable decisions and ideas stay candidates; technologies are common.
        assert!(choose_named(&names(&[("decision", "liturgical", "")]), None).is_none());
        assert_eq!(choose_named(&names(&[("technology", "small_caliber_semi", "")]), None).map(|c| c.1), Some("unique_technology_name"));
        // Two names: only the author's cost note can pick one.
        let two = names(&[("decision", "ITA_take_over_50", "y"), ("decision", "ITA_take_over_other", "y")]);
        assert!(choose_named(&two, None).is_none());
        assert_eq!(choose_named(&two, pp(50.0)).map(|c| c.0["id"].clone()), Some(json!("ITA_take_over_50")));
        // A cost note the real cost contradicts never binds.
        assert!(choose_named(&names(&[("decision", "ITA_take_over_50", "y")]), pp(25.0)).is_none());
        // A same-named focus of the guide's tree keeps it ambiguous.
        assert!(choose_named(&names(&[("decision", "ITA_expand_lateran", "y"), ("focus", "ITA_expand_lateran_treaty", "y")]), None).is_none());
    }
    #[test]
    fn author_line_shapes_yield_their_answers() {
        assert_eq!(texts("Mobilize The…"), [("Mobilize The…".into(), None)]);
        assert_eq!(texts("We will seize…(25PP) (if possible)"), [("We will seize…".into(), Some(25.0))]);
        assert_eq!(texts("- Recall Kocs…(25 CP)"), [("Recall Kocs…".into(), Some(25.0))]);
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
    fn triggers_names_and_several_actions_are_grounded_on_existing_steps() {
        let index = Index::new(&json!({"entries":[
            {"kind":"country","id":"HUN","name":"Hungary"},
            {"kind":"focus","id":"HUN_expand_the_army","name":"Expand the Army","scope":"hun_tree"},
            {"kind":"focus","id":"HUN_seek_allies","name":"Seek Allies","scope":"hun_tree"},
            {"kind":"focus","id":"HUN_seek_glory","name":"Seek Glory","scope":"hun_tree"},
            {"kind":"decision","id":"HUN_recall_kocsard_janky","name":"Recall Kocsárd Janky to Active Service","command_power_cost":25.0,"targeted":false},
            {"kind":"decision","id":"HUN_liturgical_reform","name":"Liturgical Reform","political_power_cost":70.0,"targeted":false},
            {"kind":"decision","id":"HUN_convoke_council","name":"Convoke the Council","political_power_cost":100.0,"targeted":false},
            {"kind":"decision","id":"HUN_take_over_ministry_a","name":"Take Over Ministry Of Corporations","political_power_cost":50.0,"targeted":false},
            {"kind":"decision","id":"HUN_take_over_ministry_b","name":"Take Over Ministry Of Corporate Affairs","political_power_cost":25.0,"targeted":false},
            {"kind":"decision","id":"ROM_recall_someone","name":"Recall Somebody","targeted":false},
            {"kind":"technology","id":"small_caliber_semi_auto","name":"Small Caliber Semi-Automatic Rifles"}
        ]})).unwrap();
        let row = |key: &str, line: &str| json!({"key":key,"title":line,"tracking":"reminder","action":{"kind":"manual","params":{"instructions":[line]}}});
        let mut guide = json!({"applies_to":{"country":"HUN","tree_id":"hun_tree"},"tracks":{"operations":[
            row("a", "On Expand…done: Recall Kocs…(25 CP)"),
            row("b", "Take Over Ministry Of Corp… (50PP)"),
            row("c", "Liturgical…(70PP) + Convoke…(100PP)"),
            row("d", "On Seek…: Small Caliber Semi…"),
            row("e", "Recall Some…"),
            row("f", "On Expand… & GER down: Liturgical…"),
            row("g", "Recall Kocs…(25 CP)
- Deploy & vol"),
            row("h", "On Expand…done:
- Deploy & vol
- Liturgical…"),
            row("i", "Liturgical…(70PP) + Convoke…(100PP) & GER war"),
            row("j", "Liturgical…(70PP) / Convoke…(100PP)"),
            row("k", "Liturgical…(70PP) + Recall Some…"),
        ]}});
        ground_references(&mut guide, &index, &Value::Null).unwrap();
        let rows = guide["tracks"]["operations"].as_array().unwrap().clone();
        let p = |i: usize| rows[i]["action"]["params"].clone();
        // 1: the trigger becomes the observable `when`; 2: the abbreviated
        // decision is bound through political_binding, its CP cost checked.
        assert_eq!(rows[0]["when"], json!({"op":"fact","key":"focus.completed","test":"contains","value":"HUN_expand_the_army"}));
        assert_eq!(p(0)["trigger_resolution"]["status"], "compiled");
        assert_eq!(p(0)["decision_id"], "HUN_recall_kocsard_janky");
        assert_eq!(p(0)["reference_resolution"]["basis"], "cost_matches");
        assert_eq!(rows[0]["completion"]["args"][0]["value"], "HUN_recall_kocsard_janky");
        assert!(rows[0].get("tracking").is_none());
        // Two names share the prefix: the author's cost note picks one.
        assert_eq!(p(1)["decision_id"], "HUN_take_over_ministry_a");
        // 3: each action of the line, with its own cost note; the step is
        // complete only once every action is observed.
        assert_eq!(p(2)["decision_id"], "HUN_liturgical_reform");
        assert_eq!(p(2)["reference_resolution"]["also"][0]["id"], "HUN_convoke_council");
        assert_eq!(rows[2]["completion"]["op"], "all");
        assert_eq!(rows[2]["completion"]["args"].as_array().map(Vec::len), Some(2));
        assert_eq!(rows[2]["completion"]["args"][1]["args"][0]["value"], "HUN_convoke_council");
        // Other work after the actions, alternatives, or an unresolved piece:
        // recognized and shown, never completed by one observed action.
        for i in [8, 9, 10] {
            assert_eq!(p(i)["reference_resolution"]["id"], "HUN_liturgical_reform", "row {i}");
            assert_eq!(p(i)["reference_resolution"]["tracked"], false, "row {i}");
            assert_eq!(p(i)["reference_resolution"]["scope"], "line", "row {i}");
            assert!(p(i)["decision_id"].is_null() && rows[i].get("completion").is_none(), "row {i}");
        }
        assert_eq!(p(10)["reference_resolution"]["unresolved"][0]["text"], "Recall Some…");
        // An ambiguous trigger stays unknown with its candidates; the action
        // is still grounded (technologies are common to every nation).
        assert!(rows[3].get("when").is_none());
        assert_eq!(p(3)["trigger_resolution"]["status"], "unknown");
        assert_eq!(p(3)["trigger_resolution"]["resolution_status"], "ambiguous");
        assert_eq!(p(3)["technology_id"], "small_caliber_semi_auto");
        assert_eq!(rows[3]["completion"]["key"], "research.completed");
        // Another nation's decision is never bound.
        assert!(p(4)["decision_id"].is_null());
        // A second condition in the trigger is not understood: no `when`.
        assert!(rows[5].get("when").is_none());
        assert_eq!(p(5)["trigger_resolution"]["verification"], "residual_condition_not_understood");
        // The lead line of a block with other work is recognized, not bound.
        assert_eq!(p(6)["reference_resolution"]["id"], "HUN_recall_kocsard_janky");
        assert_eq!(p(6)["reference_resolution"]["tracked"], false);
        assert!(p(6)["decision_id"].is_null() && rows[6].get("completion").is_none());
        assert_eq!(rows[6]["tracking"], "reminder");
        // A block's leading trigger scopes the whole block.
        assert_eq!(rows[7]["when"]["value"], "HUN_expand_the_army");
        // Idempotent: a second pass changes nothing.
        let once = guide.clone();
        ground_references(&mut guide, &index, &Value::Null).unwrap();
        assert_eq!(guide, once);
    }
    #[test]
    fn several_event_answers_are_narrowed_to_events_of_guide_or_historical_focuses() {
        // Every event is received by GER, so each line has several confirmed answers.
        let event = |id: &str, from: &str| json!({"kind":"event","id":id,"name":id,"receivers":["GER"],"receivers_unknown":false,"triggered_by":[from]});
        let option = |id: &str, name: &str| json!({"kind":"event_option","id":format!("{id}.a"),"event_id":id,"name":name});
        let index = Index::new(&json!({"entries":[
            {"kind":"country","id":"GER","name":"Germany"},
            {"kind":"focus","id":"GER_picked","name":"Picked","scope":"ger_tree"},
            {"kind":"focus","id":"GER_skipped","name":"Skipped","scope":"ger_tree"},
            {"kind":"event","id":"eng_ask.1","name":"English offer","receivers":["ENG"],"receivers_unknown":false,"triggered_by":["focus:ENG_historic"]},
            event("test_a.1", "focus:GER_picked"), option("test_a.1", "Alpha comes from the guide"),
            event("test_a.2", "focus:GER_skipped"), option("test_a.2", "Alpha comes from elsewhere"),
            // Reached through England's answer to its historical focus.
            event("test_b.1", "event:eng_ask.1"), option("test_b.1", "Bravo answers England"),
            event("test_b.2", "focus:ITA_offpath"), option("test_b.2", "Bravo answers Italy"),
            event("test_c.1", "focus:GER_picked"), option("test_c.1", "Charlie one"),
            event("test_c.2", "focus:ENG_historic"), option("test_c.2", "Charlie two"),
            event("test_d.1", "focus:GER_skipped"), option("test_d.1", "Delta one"),
            event("test_d.2", "focus:ITA_offpath"), option("test_d.2", "Delta two"),
        ]})).unwrap();
        let row = |key: &str, line: &str| json!({"key":key,"title":line,"tracking":"reminder","action":{"kind":"manual","params":{"instructions":[line]}}});
        let mut guide = json!({"applies_to":{"country":"GER","tree_id":"ger_tree"},"tracks":{
            "focus":[{"key":"f","action":{"kind":"select_focus","entity":{"kind":"focus","id":"GER_picked"}}}],
            "operations":[row("a", "Alpha…"), row("b", "Bravo…"), row("c", "Charlie…"), row("d", "Delta…")]}});
        ground_references(&mut guide, &index, &json!(["ENG_historic", "GER_skipped"])).unwrap();
        let rows = guide["tracks"]["operations"].as_array().unwrap().clone();
        let p = |i: usize| rows[i]["action"]["params"].clone();
        // One candidate reached from a focus the guide picks: bound.
        assert_eq!(p(0)["event_option_id"], "test_a.1.a");
        assert_eq!(p(0)["reference_resolution"]["basis"], "event_reached_from_guide_focus");
        // One reached from another nation's historical AI focus: bound.
        assert_eq!(p(1)["event_option_id"], "test_b.1.a");
        assert_eq!(p(1)["reference_resolution"]["basis"], "event_reached_from_historical_focus");
        // Two reached, or none reached (the nation's own historical focus
        // outside the guide does not count): candidates only.
        for i in [2, 3] {
            assert!(p(i)["event_option_id"].is_null(), "row {i}");
            assert_eq!(p(i)["reference_candidates"]["status"], "ambiguous", "row {i}");
        }
        // Without historical focuses England's answer is not reached.
        let mut plain = json!({"applies_to":{"country":"GER","tree_id":"ger_tree"},"tracks":{"operations":[row("b", "Bravo…")]}});
        ground_references(&mut plain, &index, &Value::Null).unwrap();
        assert!(plain["tracks"]["operations"][0]["action"]["params"]["event_option_id"].is_null());
    }
    #[test]
    fn answers_whose_event_cannot_fire_by_the_planned_day_are_dropped() {
        // Every event is received by GER; the catalog gives each event its
        // earliest possible day (`event_timing`, speed factor applied) or none.
        let event = |id: &str, day: Option<f64>| json!({"kind":"event","id":id,"name":id,"receivers":["GER"],"receivers_unknown":false,"earliest_day":day});
        let option = |id: &str, name: &str| json!({"kind":"event_option","id":format!("{id}.a"),"event_id":id,"name":name});
        let focus = |id: &str, cost: f64, pre: Value| json!({"kind":"focus","id":id,"name":id,"scope":"tree","cost":cost,"prerequisites":pre,"bypass_possible":false,"external_completion_possible":false});
        let index = Index::new(&json!({"entries":[
            {"kind":"country","id":"GER","name":"Germany"},
            focus("GER_a", 5.0, json!([])), focus("GER_b", 10.0, json!([["GER_a"]])),
            // Days from other nations' focuses: 70, 140 and 175 written, divided by 1.5.
            event("alpha.1", Some(70.0 / 1.5)), option("alpha.1", "Alpha early"),
            event("alpha.2", Some(175.0 / 1.5)), option("alpha.2", "Alpha late"),
            // No bound (callers the trace may not hold): kept.
            event("bravo.1", Some(70.0 / 1.5)), option("bravo.1", "Bravo early"),
            event("bravo.2", None), option("bravo.2", "Bravo unknown"),
            // Later than the plan as written (140), not with the speed factor: kept.
            event("charlie.1", Some(70.0 / 1.5)), option("charlie.1", "Charlie early"),
            event("charlie.2", Some(140.0 / 1.5)), option("charlie.2", "Charlie faster"),
            // Two early answers stay; the late one is dropped from the list.
            event("delta.1", Some(70.0 / 1.5)), option("delta.1", "Delta one"),
            event("delta.2", Some(70.0 / 1.5)), option("delta.2", "Delta two"),
            event("delta.3", Some(175.0 / 1.5)), option("delta.3", "Delta late"),
        ]})).unwrap();
        let row = |key: &str, line: &str| json!({"key":key,"title":line,"tracking":"reminder","action":{"kind":"manual","params":{"instructions":[line]}}});
        let step = |id: &str| json!({"key":id,"action":{"kind":"select_focus","entity":{"kind":"focus","id":id}}});
        let mut guide = json!({"applies_to":{"country":"GER","tree_id":"tree"},"tracks":{
            "focus":[step("GER_a"), step("GER_b")],
            // GER_b is planned for day 105 (5 + 10 cost units, 7 days each).
            "operations":[row("a", "Alpha…"), row("b", "Bravo…"), row("c", "Charlie…"), row("d", "Delta…"), step("GER_b"), row("e", "Alpha…")]}});
        ground_references(&mut guide, &index, &Value::Null).unwrap();
        let rows = guide["tracks"]["operations"].as_array().unwrap().clone();
        let p = |i: usize| rows[i]["action"]["params"].clone();
        assert_eq!(p(0)["event_option_id"], "alpha.1.a");
        assert_eq!(p(0)["reference_resolution"]["basis"], "guide_timeline");
        assert_eq!(p(0)["reference_resolution"]["timeline"]["anchor_focus"], "GER_b");
        assert_eq!(p(0)["reference_resolution"]["timeline"]["planned_day"], 105.0);
        assert_eq!(p(0)["reference_resolution"]["timeline"]["dropped"][0]["id"], "alpha.2.a");
        // What chose it once the late answer was gone: the nation receives it.
        assert_eq!(p(0)["reference_resolution"]["timeline"]["then"], "event_received_by_nation");
        // The binding observes the event like any other: no completion without it.
        assert_eq!(rows[0]["started_when"]["value"], "alpha.1:alpha.1.a");
        for i in [1, 2] {
            assert!(p(i)["event_option_id"].is_null(), "row {i}");
            assert_eq!(p(i)["reference_candidates"]["status"], "ambiguous", "row {i}");
            assert!(p(i)["reference_candidates"]["timeline"].is_null(), "row {i}");
        }
        assert!(p(3)["event_option_id"].is_null());
        let delta = p(3);
        let ids: Vec<&str> = delta["reference_candidates"]["candidates"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
        assert_eq!(ids, ["delta.1.a", "delta.2.a"]);
        assert_eq!(p(3)["reference_candidates"]["timeline"]["dropped"][0]["id"], "delta.3.a");
        // No focus anchor after the line: no plan, nothing dropped.
        assert!(p(5)["event_option_id"].is_null());
        assert_eq!(p(5)["reference_candidates"]["candidates"].as_array().unwrap().len(), 2);
    }
    #[test]
    fn cannot_is_also_searched_as_can_not_and_cant() {
        assert_eq!(spellings("we cannot"), ["we cannot", "we can not", "we cant"]);
        assert_eq!(spellings("we can not accept"), ["we can not accept", "we cannot accept", "we cant accept"]);
        assert_eq!(spellings("we can win"), ["we can win"]);
        let event = |id: &str| json!({"kind":"event","id":id,"name":id,"receivers":["GER"],"receivers_unknown":false});
        let option = |id: &str, name: &str| json!({"kind":"event_option","id":format!("{id}.a"),"event_id":id,"name":name});
        let index = Index::new(&json!({"entries":[
            {"kind":"country","id":"GER","name":"Germany"},
            event("v.1"), option("v.1", "We can't allow this"),
            event("v.2"), option("v.2", "We can not allow that"),
            event("v.3"), option("v.3", "We cannot allow it"),
        ]})).unwrap();
        let found = candidates("We cannot…", &index, &json!("GER"), &json!("tree")).unwrap();
        let mut ids: Vec<&str> = found.as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
        ids.sort();
        assert_eq!(ids, ["v.1.a", "v.2.a", "v.3.a"]);
    }
    #[test]
    fn literal_answers_beat_the_ones_found_only_through_other_spellings() {
        // Every event is received by GER: several confirmed answers per line.
        let event = |id: &str| json!({"kind":"event","id":id,"name":id,"receivers":["GER"],"receivers_unknown":false});
        let option = |id: &str, name: &str| json!({"kind":"event_option","id":format!("{id}.a"),"event_id":id,"name":name});
        let index = Index::new(&json!({"entries":[
            {"kind":"country","id":"GER","name":"Germany"},
            event("v.1"), option("v.1", "We can't allow this"),
            event("v.2"), option("v.2", "We can not allow that"),
            event("v.3"), option("v.3", "We cannot allow it"),
            event("v.4"), option("v.4", "We cannot stop now"),
            event("t.1"), option("t.1", "They cannot win"),
            event("t.2"), option("t.2", "They can't lose"),
        ]})).unwrap();
        let row = |key: &str, line: &str| json!({"key":key,"title":line,"tracking":"reminder","action":{"kind":"manual","params":{"instructions":[line]}}});
        let mut guide = json!({"applies_to":{"country":"GER","tree_id":"tree"},"tracks":{
            "operations":[row("a", "We cannot allow…"), row("b", "We cannot…"), row("c", "They can not…")]}});
        ground_references(&mut guide, &index, &Value::Null).unwrap();
        let rows = guide["tracks"]["operations"].as_array().unwrap().clone();
        let p = |i: usize| rows[i]["action"]["params"].clone();
        let ids = |i: usize| p(i)["reference_candidates"]["candidates"].as_array().unwrap().iter().filter_map(|c| c["id"].as_str().map(str::to_owned)).collect::<Vec<_>>();
        // One literal answer left: it binds, with its own basis and record.
        assert_eq!(p(0)["event_option_id"], "v.3.a");
        assert_eq!(p(0)["reference_resolution"]["basis"], "literal_text");
        assert_eq!(p(0)["reference_resolution"]["literal"]["then"], "event_received_by_nation");
        let first = p(0);
        let mut dropped: Vec<&str> = first["reference_resolution"]["literal"]["dropped"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        dropped.sort();
        assert_eq!(dropped, ["v.1.a", "v.2.a"]);
        // Two literal answers: the variants leave the list, the line stays open.
        assert!(p(1)["event_option_id"].is_null());
        let mut left = ids(1);
        left.sort();
        assert_eq!(left, ["v.3.a", "v.4.a"]);
        assert_eq!(p(1)["reference_candidates"]["literal"]["dropped"].as_array().unwrap().len(), 2);
        // No literal answer: the spellings are all there is, nothing dropped.
        assert!(p(2)["event_option_id"].is_null());
        assert_eq!(ids(2).len(), 2);
        assert!(p(2)["reference_candidates"]["literal"].is_null());
    }
    #[test]
    fn event_answers_a_single_player_game_cannot_show_are_dropped() {
        // Every event is received by GER: several confirmed answers per line.
        let event = |id: &str| json!({"kind":"event","id":id,"name":id,"receivers":["GER"],"receivers_unknown":false});
        let option = |id: &str, name: &str, players: Value| json!({"kind":"event_option","id":format!("{id}.a"),"event_id":id,"name":name,"player_requirement":players});
        let index = Index::new(&json!({"entries":[
            {"kind":"country","id":"GER","name":"Germany"},
            // poland.2102.b needs Danzig human-played: never in single player.
            event("test_a.1"), option("test_a.1", "Perhaps they deserve some autonomy.", json!({"human":["DNZ"],"ai":[]})),
            event("test_a.2"), option("test_a.2", "Perhaps they have a point", Value::Null),
            // The guide's own nation human-played: kept, so still two answers.
            event("test_b.1"), option("test_b.1", "Bravo by the player", json!({"human":["GER","ROOT"],"ai":[]})),
            event("test_b.2"), option("test_b.2", "Bravo by anyone", Value::Null),
            // The receiving nation must be the AI: dropped.
            event("test_c.1"), option("test_c.1", "Charlie for the AI", json!({"human":[],"ai":["ROOT"]})),
            event("test_c.2"), option("test_c.2", "Charlie for anyone", Value::Null),
            // A trigger not understood for certain (no requirement read): kept.
            event("test_d.1"), option("test_d.1", "Delta unsure", Value::Null),
            event("test_d.2"), option("test_d.2", "Delta other", Value::Null),
            // The receiver must be the AI, but the catalog does not prove the
            // guide's nation receives the event: ROOT may be another nation, kept.
            {"kind":"event","id":"test_e.1","name":"test_e.1"}, option("test_e.1", "Echo for the AI", json!({"human":[],"ai":["ROOT"]})),
            {"kind":"event","id":"test_e.2","name":"test_e.2"}, option("test_e.2", "Echo for anyone", Value::Null),
        ]})).unwrap();
        let row = |key: &str, line: &str| json!({"key":key,"title":line,"tracking":"reminder","action":{"kind":"manual","params":{"instructions":[line]}}});
        let mut guide = json!({"applies_to":{"country":"GER","tree_id":"ger_tree"},"tracks":{
            "operations":[row("a", "Perhaps They…"), row("b", "Bravo…"), row("c", "Charlie…"), row("d", "Delta…"), row("e", "Echo…")]}});
        ground_references(&mut guide, &index, &Value::Null).unwrap();
        let rows = guide["tracks"]["operations"].as_array().unwrap().clone();
        let p = |i: usize| rows[i]["action"]["params"].clone();
        assert_eq!(p(0)["event_option_id"], "test_a.2.a");
        assert_eq!(p(0)["reference_resolution"]["basis"], "event_received_by_nation");
        assert_eq!(p(2)["event_option_id"], "test_c.2.a");
        // Two answers left (unconfirmed ones for row 4): not bound.
        for i in [1, 3, 4] {
            assert!(p(i)["event_option_id"].is_null(), "row {i}");
            assert_eq!(p(i)["reference_candidates"]["status"], "ambiguous", "row {i}");
            assert_eq!(p(i)["reference_candidates"]["candidates"].as_array().map(Vec::len), Some(2), "row {i}");
        }
    }
    #[test]
    fn written_out_actions_are_bound_only_when_the_whole_line_is_understood() {
        let index = Index::new(&json!({"entries":[
            {"kind":"country","id":"ROM","name":"Romania"},
            {"kind":"country","id":"HUN","name":"Hungary"},
            {"kind":"country","id":"YUG","name":"Yugoslavia"},
            {"kind":"country","id":"GRE","name":"Greece"},
            {"kind":"country","id":"ENG","name":"United Kingdom"},
            {"kind":"country","id":"FRA","name":"France"},
            {"kind":"focus","id":"ROM_force_abdication","name":"Force Abdication","scope":"rom_tree"}
        ]})).unwrap();
        let row = |key: &str, line: &str| json!({"key":key,"title":line,"tracking":"reminder","action":{"kind":"manual","params":{"instructions":[line]}}});
        let mut guide = json!({"applies_to":{"country":"ROM","tree_id":"rom_tree"},"tracks":{"operations":[
            row("a", "On Force…: Justify HUN, YUG (60PP)"),
            row("b", "REL UK, FRA (40PP)"),
            row("c", "ASAP: war on GRE"),
            row("d", "War on GRE, HUN (at the same time)"),
            row("e", "On Force…: Annex YUG"),
            row("f", "Justify & Spy FRA (Leba)"),
            row("g", "war on GRE
- push to Athens"),
            row("h", "REL ENG & HUN (20PP)"),
            row("i", "Invade the UK (if needed)"),
            row("j", "War Bonds (50PP)"),
        ]}});
        ground_references(&mut guide, &index, &Value::Null).unwrap();
        let rows = guide["tracks"]["operations"].as_array().unwrap().clone();
        let p = |i: usize| rows[i]["action"]["params"].clone();
        // A war goal is not read live: recognized with its targets, manual.
        assert_eq!(p(0)["action_resolution"]["status"], "not_observable");
        assert_eq!(p(0)["action_resolution"]["action"], "justify_war_goal");
        assert_eq!(p(0)["action_resolution"]["targets"], json!(["HUN", "YUG"]));
        assert!(rows[0].get("completion").is_none());
        assert_eq!(rows[0]["when"]["args"][1]["value"], "ROM_force_abdication");
        // Several foreign actions: each its own durable part, all required.
        assert_eq!(rows[1]["completion"]["op"], "all");
        assert_eq!(p(1)["foreign_parts"].as_array().map(Vec::len), Some(2));
        assert_eq!(p(1)["foreign_parts"][0]["target_country"], "ENG");
        assert_eq!(p(1)["action_resolution"]["actions"][1], json!({"action":"improve_relations","target":"FRA"}));
        assert!(rows[1].get("tracking").is_none());
        // One war declared by the player (outgoing record), kept as a milestone.
        assert_eq!(p(2)["operation_id"], "declare_war");
        assert_eq!(p(2)["target_country"], "GRE");
        let war = &rows[2]["completion"]["args"][0];
        assert_eq!(war["args"][1], json!({"op":"fact","key":"diplomacy.outgoing","test":"contains","value":"war:GRE"}));
        assert_eq!(rows[2]["completion"]["args"][1]["key"], "guide.campaign.milestones");
        // Extra words, unresolved targets or a block: left as they were.
        for i in [3, 5, 6] {
            assert!(p(i)["action_resolution"].is_null() && rows[i].get("completion").is_none(), "row {i}");
            assert_eq!(rows[i]["tracking"], "reminder", "row {i}");
        }
        assert_eq!(p(4)["action_resolution"]["action"], "annex");
        assert!(rows[4].get("completion").is_none());
        // "&" lists targets too; "the UK" is ENG; a trailing note does not
        // stop a recognition that is never tracked; a law is not an action.
        assert_eq!(p(7)["foreign_parts"][1]["target_country"], "HUN");
        assert_eq!(p(8)["action_resolution"], json!({"status":"not_observable","action":"invade","targets":["ENG"],"text":"Invade ENG"}));
        assert!(p(9)["action_resolution"].is_null() && rows[9].get("completion").is_none());
        let once = guide.clone();
        ground_references(&mut guide, &index, &Value::Null).unwrap();
        assert_eq!(guide, once);
    }
    #[test]
    fn normalization_ignores_case_accents_and_punctuation() {
        assert_eq!(normalize("Which, by God, …"), "which by god");
        assert_eq!(normalize("That’s Capitalism!"), "thats capitalism");
        assert_eq!(normalize("Österreich"), "osterreich");
        assert_eq!(normalize("£BoP_right_texticon Take Over Ministry of Corporations"), "take over ministry of corporations");
        assert_eq!(normalize("§YRome§!"), "rome");
        assert_eq!(strip_markup("£BoP_right_texticon Take Over Ministry").trim(), "Take Over Ministry");
        assert!(normalize("A New Bismarckjugend should be top priority").starts_with(&normalize(&query("A New Bismarck…"))));
    }
}
