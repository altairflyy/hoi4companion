//! Bounded possible identity labels, never execution of scripted localization.
use crate::{entity_catalog, recipient};
use regex::Regex;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::OnceLock,
};
pub struct Expander<'a> {
    definitions: &'a Value,
    labels: &'a Value,
    adjectives: BTreeMap<String, BTreeSet<String>>,
}
impl<'a> Expander<'a> {
    pub fn new(definitions: &'a Value, labels: &'a Value) -> Self {
        Self {
            definitions,
            labels,
            adjectives: entity_catalog::adjectives(labels, false),
        }
    }
    fn alternatives(&self, scope: Option<&str>, name: &str, countries: &[&str]) -> Vec<String> {
        if let Some(scope) = scope
            && !scope.eq_ignore_ascii_case("root")
            && !recipient::tag(scope)
            && !scope.bytes().all(|b| b.is_ascii_digit())
        {
            return vec![];
        }
        if name == "GetAdjective"
            && let Some(scope) = scope
        {
            let tags = if scope.eq_ignore_ascii_case("root") {
                countries.to_vec()
            } else {
                vec![scope]
            };
            return tags
                .into_iter()
                .filter(|s| recipient::tag(s))
                .flat_map(|tag| self.adjectives.get(tag).into_iter().flatten())
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .take(64)
                .collect();
        }
        if let Some(scope) = scope
            && scope.bytes().all(|b| b.is_ascii_digit())
        {
            return if name == "GetName" {
                self.labels[format!("STATE_{scope}")]
                    .as_str()
                    .map(|s| vec![s.to_owned()])
                    .unwrap_or_default()
            } else {
                vec![]
            };
        }
        self.definitions[name]
            .as_array()
            .into_iter()
            .flatten()
            .take(64)
            .filter_map(Value::as_str)
            .filter_map(|key| self.labels[key].as_str())
            .map(str::to_owned)
            .collect()
    }
    pub fn expand(&self, text: &str, countries: &[&str]) -> Vec<String> {
        static TOKEN: OnceLock<Regex> = OnceLock::new();
        let token=TOKEN.get_or_init(||Regex::new(r"\[(?:(?P<scope>[A-Za-z_][A-Za-z0-9_]*|[0-9]+)\.)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\]").unwrap());
        struct Pass<'a, 'b> {
            expander: &'a Expander<'b>,
            countries: &'a [&'a str],
            token: &'a Regex,
            left: usize,
        }
        impl Pass<'_, '_> {
            fn run(&mut self, text: &str, seen: &HashSet<String>, depth: usize) -> Vec<String> {
                if depth > 8 || text.chars().count() > 4096 || self.left == 0 {
                    return vec![];
                }
                self.left -= 1;
                let Some(capture) = self.token.captures(text) else {
                    return if text.contains(['[', ']']) {
                        vec![]
                    } else {
                        vec![text.into()]
                    };
                };
                let name = &capture["name"];
                if seen.contains(name) {
                    return vec![];
                }
                let mut next = seen.clone();
                next.insert(name.into());
                let full = capture.get(0).unwrap();
                let scope = capture.name("scope").map(|s| s.as_str());
                let mut result = BTreeSet::new();
                for label in self.expander.alternatives(scope, name, self.countries) {
                    for expanded in self.run(&label, &next, depth + 1) {
                        let combined = format!(
                            "{}{}{}",
                            &text[..full.start()],
                            expanded,
                            &text[full.end()..]
                        );
                        for item in self.run(&combined, seen, depth + 1) {
                            result.insert(item);
                            if result.len() >= 64 {
                                return result.into_iter().collect();
                            }
                        }
                    }
                }
                result.into_iter().collect()
            }
        }
        if !token.is_match(text) {
            return vec![];
        }
        Pass {
            expander: self,
            countries,
            token,
            left: 65536,
        }
        .run(text, &HashSet::new(), 0)
    }
}
