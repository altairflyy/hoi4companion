//! Quick references for the HUD card: the guide's own Word sections (army
//! setup, war plans, relevant events) as checkable rows. Same reconstruction
//! as the walkthrough's `guide-references.js` (sections, quickReferences);
//! `tests/fixtures/reference-sections.json` keeps both in step. Read-only:
//! never parses actions or changes a definition.
use crate::import_catalog::regex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Most rows one HUD section lists, and the longest row text.
const SECTION_ROWS: usize = 60;
const ROW_CHARS: usize = 240;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Block {
    Text(String),
    Heading(String),
    Row { cells: Vec<String>, header: bool },
}

#[derive(Clone, Debug)]
pub(crate) struct Section {
    pub title: String,
    pub context: String,
    pub blocks: Vec<Block>,
    text: String,
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn section_word(s: &str) -> bool {
    regex(r"(?i)\b(?:setup|builds|plans?|priority|outcomes|events|achievements|improvements|references|strategy)\b").is_match(s)
}
fn heading(s: &str) -> bool {
    let base = regex(r"\s*\([^)]*\)\s*$").replace(s, "").to_string();
    s.encode_utf16().count() <= 120
        && base.chars().any(char::is_alphabetic)
        && !s.starts_with('-')
        && !s.starts_with('•')
        && base == base.to_uppercase()
        && !s.ends_with(['.', '!', '?'])
}

/// Paragraphs and table rows, from the `[Table row]` / `[Column N]` markers
/// retained in imported Word sources.
pub(crate) fn document_blocks(text: &str) -> Vec<Block> {
    if !text.contains("[Table row]") {
        return text.split('\n').map(|t| Block::Text(t.to_owned())).collect();
    }
    let mut chunks = text.split("[Table row]");
    let mut out: Vec<Block> = chunks.next().unwrap_or("").split('\n').filter(|l| !l.is_empty()).map(|t| Block::Text(t.to_owned())).collect();
    let marker = regex(r"\[Column \d+\]\r?\n");
    for chunk in chunks {
        let starts: Vec<(usize, usize)> = marker.find_iter(chunk).map(|m| (m.start(), m.end())).collect();
        let mut cells: Vec<String> = starts.iter().enumerate().map(|(i, (_, end))| {
            let stop = starts.get(i + 1).map_or(chunk.len(), |next| next.0);
            chunk[*end..stop].trim().to_owned()
        }).collect();
        if cells.is_empty() {
            continue;
        }
        let last: Vec<String> = cells.last().unwrap().split('\n').map(str::to_owned).collect();
        let tail = last.iter().enumerate().position(|(i, line)| {
            i > 0 && heading(&clean(line)) && (section_word(line) || line.starts_with("PART"))
        });
        if let Some(tail) = tail {
            *cells.last_mut().unwrap() = last[..tail].join("\n").trim().to_owned();
        }
        out.push(Block::Row { cells, header: false });
        if let Some(tail) = tail {
            out.extend(last[tail..].iter().map(|t| Block::Text(t.clone())));
        }
    }
    out
}

/// Word sections by heading, each with its blocks (nested ones included).
pub(crate) fn sections(text: &str) -> Vec<Section> {
    struct Open { title: String, context: String, level: usize, blocks: Vec<Block>, parts: Vec<String> }
    let mut result: Vec<Open> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut priority = false;
    fn append(result: &mut [Open], stack: &[usize], block: &Block) {
        let part = match block {
            Block::Row { cells, .. } => cells.join("\n"),
            Block::Text(t) | Block::Heading(t) => t.clone(),
        };
        for i in stack {
            result[*i].blocks.push(block.clone());
            result[*i].parts.push(part.clone());
        }
    }
    let add = |title: &str, level: usize, result: &mut Vec<Open>, stack: &mut Vec<usize>| {
        let title = regex(r":\s*$").replace(title, "").to_string();
        while stack.last().is_some_and(|i| result[*i].level >= level) {
            stack.pop();
        }
        append(result, stack, &Block::Heading(title.clone()));
        let context = stack.iter().map(|i| result[*i].title.clone()).collect::<Vec<_>>().join(" › ");
        result.push(Open { title, context, level, blocks: vec![], parts: vec![] });
        stack.push(result.len() - 1);
    };
    let level_of = |title: &str, result: &[Open], stack: &[usize]| {
        if regex(r"(?i)^HQ\s+SETUP\b").is_match(title) && stack.iter().any(|i| regex(r"(?i)^ARMY$").is_match(&result[*i].title)) {
            3
        } else if section_word(title) || regex(r"^PART\b").is_match(title) {
            1
        } else {
            2
        }
    };
    for block in document_blocks(text) {
        match &block {
            Block::Text(raw) => {
                let plain = clean(raw);
                if let Some(markdown) = regex(r"^(#{1,6})\s+(.+?)\s*#*\s*$").captures(raw) {
                    priority = false;
                    add(&markdown[2], markdown[1].len(), &mut result, &mut stack);
                } else if heading(&plain) {
                    priority = false;
                    let level = level_of(&plain, &result, &stack);
                    add(&plain, level, &mut result, &mut stack);
                } else if !priority {
                    append(&mut result, &stack, &block);
                }
            }
            Block::Row { cells, .. } => {
                let first = cells[0].clone();
                let band = cells[1..].iter().all(String::is_empty);
                if cells.len() == 3 && regex(r"(?i)^ACTIONS$").is_match(&first) && regex(r"(?i)^RESEARCH PRIORITY\*?$").is_match(&cells[1]) {
                    priority = true;
                    stack.clear();
                    continue;
                }
                if priority {
                    if !band || !heading(&first) || first.contains('\n') {
                        continue;
                    }
                    priority = false;
                }
                if cells.iter().all(String::is_empty) {
                    continue;
                }
                if band && !first.contains('\n') && heading(&first) {
                    let level = level_of(&first, &result, &stack);
                    add(&first, level, &mut result, &mut stack);
                    continue;
                }
                let header = cells.len() > 1
                    && cells.iter().all(|c| heading(&clean(c)))
                    && cells.iter().any(|c| regex(r"\b(?:GENERAL|ROLE|ORDERS|NAME|MEANING|FLEET|COMMAND|UNITS|CODE|ADJUSTMENTS)\b").is_match(c));
                let row = Block::Row { cells: cells.clone(), header };
                append(&mut result, &stack, &row);
                if !header && cells.len() > 1 && !first.contains('\n') && heading(&first) {
                    let context = stack.iter().map(|i| result[*i].title.clone()).collect::<Vec<_>>().join(" › ");
                    result.push(Open { title: first.clone(), context, level: 2, blocks: vec![row], parts: vec![cells[1..].join("\n")] });
                }
            }
            Block::Heading(_) => {}
        }
    }
    result
        .into_iter()
        .map(|o| Section { text: o.parts.join("\n").trim().to_owned(), title: o.title, context: o.context, blocks: o.blocks })
        .filter(|s| !s.text.is_empty())
        .collect()
}

/// The guide's quick-reference sections, as on the walkthrough toolbar.
pub(crate) fn quick(all: &[Section]) -> Vec<(&'static str, Vec<&Section>)> {
    let base = |s: &Section| clean(&regex(r"\s*\([^)]*\)\s*$").replace(&s.title, ""));
    // The "Setup" alias of guide-references.js compile(), then setupCandidates
    // for an army context: ARMY, else HQ SETUP, else ARMY SETUP.
    let setup: Vec<&Section> = all.iter().filter(|s| {
        let b = base(s);
        regex(r"(?i)\bsetup$").is_match(&b)
            || regex(r"(?i)\bsetup\b").is_match(&s.context) && regex(r"(?i)^(?:ARMY|NAVY|AIR FORCE)$").is_match(&b)
            || regex(r"(?i)\b(?:setup)s$").is_match(&b) && clean(&regex(r"(?i)s$").replace(&b, "")).eq_ignore_ascii_case("setup")
    }).collect();
    let score = |s: &Section| {
        if regex(r"(?i)^ARMY$").is_match(&s.title) { 0 } else if regex(r"(?i)^HQ SETUP$").is_match(&s.title) { 1 } else if regex(r"(?i)^ARMY SETUP$").is_match(&s.title) { 2 } else { usize::MAX }
    };
    let best = setup.iter().map(|s| score(s)).min().unwrap_or(usize::MAX);
    let army = if best == usize::MAX { vec![] } else { setup.into_iter().filter(|s| score(s) == best).collect() };
    let pick = |pattern: &str| all.iter().filter(|s| regex(pattern).is_match(&s.title)).collect::<Vec<_>>();
    [("Army setup", army), ("War plans", pick(r"(?i)^WAR PLANS?$")), ("Relevant events", pick(r"(?i)^RELEVANT EVENTS$"))]
        .into_iter()
        .filter(|(_, c)| !c.is_empty())
        .collect()
}

/// HUD card sections from the guide's sources: one row per table row or
/// paragraph line, nested headings as non-checkable labels. Each row has a
/// stable id (label + text + occurrence) for the HUD's done marks.
pub(crate) fn card_sections(guide: &Value) -> Value {
    let mut all = vec![];
    for source in guide["sources"].as_array().into_iter().flatten() {
        if let (Some(_), Some(text)) = (source["id"].as_str(), source["text"].as_str()) {
            all.extend(sections(text));
        }
    }
    let mut out = vec![];
    for (label, candidates) in quick(&all) {
        let mut rows: Vec<Value> = vec![];
        let mut seen = std::collections::HashMap::<String, usize>::new();
        for section in candidates {
            for block in &section.blocks {
                let (text, heading) = match block {
                    Block::Heading(t) => (clean(t), true),
                    Block::Row { header: true, .. } => continue,
                    Block::Row { cells, .. } => (cells.iter().filter(|c| !c.trim().is_empty())
                        .map(|c| c.split('\n').map(clean).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(", "))
                        .collect::<Vec<_>>().join(" · "), false),
                    Block::Text(t) => (clean(t.trim_start_matches(['-', '•', ' '])), false),
                };
                if text.is_empty() || rows.len() >= SECTION_ROWS {
                    continue;
                }
                let text: String = text.chars().take(ROW_CHARS).collect();
                let n = seen.entry(text.clone()).or_default();
                *n += 1;
                let id = format!("{:x}", Sha256::digest(format!("{label}\u{1f}{text}\u{1f}{n}").as_bytes()));
                rows.push(json!({"id":&id[..16],"text":text,"heading":heading}));
            }
        }
        if rows.iter().any(|r| r["heading"] == false) {
            out.push(json!({"label":label,"title":candidates_title(&all, label),"rows":rows}));
        }
    }
    Value::Array(out)
}
fn candidates_title(all: &[Section], label: &str) -> Value {
    quick(all).into_iter().find(|(l, _)| *l == label).and_then(|(_, c)| c.first().map(|s| json!(s.title))).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/reference-sections.json");
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
    #[test]
    fn quick_references_match_the_walkthrough() {
        let fixture = fixture();
        let all = sections(fixture["source"].as_str().unwrap());
        let quick = quick(&all);
        let got: Vec<Value> = quick.iter().map(|(label, c)| json!({"label":label,"titles":c.iter().map(|s| &s.title).collect::<Vec<_>>(),
            "rows":c.iter().flat_map(|s| s.blocks.iter().filter_map(|b| match b { Block::Row { cells, header: false } => Some(json!(cells)), _ => None })).collect::<Vec<_>>()})).collect();
        assert_eq!(json!(got), fixture["quick"]);
    }
    #[test]
    fn card_rows_are_checkable_with_stable_ids_and_headings_are_labels() {
        let fixture = fixture();
        let guide = json!({"sources":[{"id":"doc","text":fixture["source"]}]});
        let cards = card_sections(&guide);
        let labels: Vec<&str> = cards.as_array().unwrap().iter().map(|c| c["label"].as_str().unwrap()).collect();
        assert_eq!(labels, ["Army setup", "War plans", "Relevant events"]);
        let war = &cards[1]["rows"];
        assert_eq!(war[0], json!({"id":war[0]["id"],"text":"START & NED","heading":true}));
        assert_eq!(war[1]["text"], "Prepare the landing.");
        let events = &cards[2]["rows"];
        assert_eq!(events[0]["text"], "JUL 1937 · JAP war on CHI");
        let army = &cards[0]["rows"];
        assert!(army.as_array().unwrap().iter().any(|r| r["text"] == "10 ALP, 11 ARM, 1 MOT · Konstantine R, Ivan K, ANY DEF · Invade NED., ARM pushes; MOT follows."));
        assert!(!army.as_array().unwrap().iter().any(|r| r["text"].as_str().unwrap().contains("GENERAL")), "column headers are not rows");
        assert_eq!(card_sections(&guide), cards, "ids are stable");
        assert_eq!(card_sections(&json!({"sources":[]})), json!([]));
    }
}
