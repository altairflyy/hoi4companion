//! Where the player acts in the game, for the HUD card (UX R9). Same rules as
//! `category()` in `web/action-plan.js` (UX R1), checked by both against
//! `tests/fixtures/display-category-contract.json`. Presentation only: it never
//! changes `params.category`, bindings, status or completion.
//!
//! JavaScript regexes without the `u` flag use ASCII `\b`, `\w` and `\d`; the
//! patterns below spell them out the same way. JavaScript's `war(?!\s+industrialist)`
//! has no lookahead here: that phrase is masked before the diplomacy test.
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

fn ascii(pattern: &str) -> String {
    format!("(?i){}", pattern.replace("\\b", "(?-u:\\b)").replace("\\w", "[0-9A-Za-z_]").replace("\\d", "[0-9]"))
}

struct Rules {
    build: Regex,
    army: Regex,
    production: Regex,
    war_industrialist: Regex,
    topics: Vec<(&'static str, Regex)>,
}

fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| {
        let re = |p: &str| Regex::new(&ascii(p)).expect("display category pattern");
        Rules {
            build: re(r"^(?:BUILD|CONSTRUCT)(?:\s*:|\s)"),
            army: re(r"^(?:ARMY|NAVY|AIR\s*FORCE|RECRUIT|TRAIN)(?:\s*:|\s)"),
            production: re(r"^(?:MIL|NAV|PRODUCTION)\s*:"),
            war_industrialist: re(r"\bwar(\s+industrialist)"),
            topics: vec![
                ("production", re(r"\b(?:design|produce|production|MIOs?)\b|->")),
                ("construction", re(r"\b(?:build|construct)\b|\b(?:infra\w*|civs?|mils?|land facilit(?:y|ies))\b[^.;]*\bin\b|\b(?:dockyards?|refiner(?:y|ies)|air ?bases?|naval bases?|forts?|bunkers?|infrastructures?)\b")),
                ("diplomacy", re(r"\b(?:justif\w*|war|wars|attack|invade|invasion|annex|capitulate|cap|defeat|puppet|peace conference|wargoals?|declare|liberate|conquer)\b")),
                ("diplomacy", re(r"\b(?:factions?|invite|join|guarantee|volunteers?|vol|attach[eé]|lend[- ]?lease|spy|agency|operatives?|alliance|non-aggression|embargo)\b")),
                ("trade", re(r"\b(?:trade|import|export|oil|steel|rubber|tungsten|chromium|alumin(?:i)?um|resources?)\b")),
                ("politics", re(r"\b\d+\s*PP\b|\(\s*\d+\s*PP|\b(?:laws?|advisors?|ministers?|expert|decisions?|elections?|conscription)\b")),
                ("army", re(r"\b(?:train|training|deploy|recruit|divisions?|templates?|generals?|admirals?|marshals?|armies|army|navy|navies|fleets?|planes?|air ?wings?|doctrines?|garrison|drills?|exercises?|INF|OFFs?|DEF|ALP)\b|\b\d+w\b|\b\d+\s*(?:AE|NE|AXP)\b")),
            ],
        }
    })
}

/// The display category of a prepared or evaluated step.
pub fn category(step: &Value) -> String {
    // The player's own reading (UX R13) wins: it was chosen for this very line.
    if let Some(c) = step["interpretation"]["category"].as_str().filter(|c| crate::interpretations::CATEGORIES.contains(c)) {
        return c.to_owned();
    }
    let p = &step["action"]["params"];
    let bound = p["category"].as_str().filter(|c| !c.is_empty());
    if let Some(c) = bound.filter(|c| *c != "checkpoints") {
        return c.to_owned();
    }
    let text = p["instructions"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join("\n");
    let text = text.trim();
    let r = rules();
    if r.build.is_match(text) {
        return "construction".into();
    }
    if r.army.is_match(text) {
        return "army".into();
    }
    if r.production.is_match(text) {
        return "production".into();
    }
    if step["action"]["kind"] == "special_project" {
        return "projects".into();
    }
    let title = ["name", "title"].iter().filter_map(|k| step[*k].as_str()).find(|s| !s.trim().is_empty()).unwrap_or("");
    let all = [title, text].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n");
    if !all.is_empty() {
        let masked = r.war_industrialist.replace_all(&all, "w_r$1");
        for (topic, pattern) in &r.topics {
            let subject = if *topic == "diplomacy" { masked.as_ref() } else { all.as_str() };
            if pattern.is_match(subject) {
                return (*topic).into();
            }
        }
    }
    bound.unwrap_or("checkpoints").to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_the_shared_contract() {
        let fixture: Value = serde_json::from_str(include_str!("../../../tests/fixtures/display-category-contract.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            assert_eq!(category(&case["step"]), case["category"].as_str().unwrap(), "{}", case["name"]);
        }
    }
}
