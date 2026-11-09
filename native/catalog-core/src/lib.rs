//! Static, nation-neutral identity resolution. No game/process/filesystem access.
mod ffi;
pub fn borrow_catalog(handle: u64) -> Result<std::sync::Arc<Catalog>, String> {
    ffi::borrow(handle)
}
pub fn request(op: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    ffi::request(op, args)
}
mod text;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Mutex;
pub use text::dlc_name;
pub use text::fold as casefold;
pub use text::nfkd;
pub use text::production_name;
pub use text::optional_focus;
pub use text::{
    alnum as unicode_alnum, alpha as unicode_alpha, decimal as unicode_decimal,
    space as unicode_space,
};
pub use text::{normal as normal_name, words as identity_words};
type Set = BTreeSet<usize>;
type PairIndex = HashMap<(String, String), Set>;
type PrefixIndex = HashMap<(String, Vec<String>), Set>;
type CountryAliasIndex = HashMap<(String, String, String), Set>;
type CountryPrefixIndex = HashMap<(String, String, Vec<String>), Set>;

#[derive(Deserialize)]
pub struct Build {
    schema_version: u32,
    entries: Vec<Value>,
    #[serde(default)]
    country_names: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    adjectives: Vec<String>,
    #[serde(default)]
    vocabulary: Value,
}
#[derive(Clone, Default, Deserialize, PartialEq, Eq)]
pub struct Context {
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Rows,
    Matches,
    FocusWords,
    Suggest,
    Excluded,
    Audit,
    ResolveCountry,
    CountryHint,
    Stats,
    Normalize,
}
#[derive(Deserialize)]
pub struct Query {
    schema_version: u32,
    op: Operation,
    #[serde(default)]
    query: String,
    #[serde(default)]
    kinds: Vec<String>,
    #[serde(flatten)]
    context: Context,
    #[serde(default)]
    explicit: bool,
    #[serde(default)]
    typos: bool,
    #[serde(default)]
    filters: BTreeMap<String, Value>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    guide: Value,
    #[serde(default)]
    texts: Vec<String>,
}
fn default_limit() -> usize {
    6
}
fn string(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn strings(v: &Value) -> Vec<&str> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}
fn nonempty(v: &Option<String>) -> Option<&str> {
    v.as_deref().filter(|s| !s.is_empty())
}
fn country_test(node: &Value, country: &str, depth: usize) -> Option<bool> {
    if country.is_empty() || !node.is_object() || depth > 32 {
        return None;
    }
    match string(&node["op"]) {
        "tag" => node["value"].as_str().map(|v| v == country),
        "true" => Some(true),
        "false" => Some(false),
        "not" => country_test(&node["arg"], country, depth + 1).map(|v| !v),
        "all" | "any" => {
            let args = node["args"].as_array()?;
            let values: Vec<_> = args
                .iter()
                .map(|n| country_test(n, country, depth + 1))
                .collect();
            if node["op"] == "all" {
                if values.contains(&Some(false)) {
                    Some(false)
                } else if values.contains(&None) {
                    None
                } else {
                    Some(true)
                }
            } else if values.contains(&Some(true)) {
                Some(true)
            } else if values.contains(&None) {
                None
            } else {
                Some(false)
            }
        }
        _ => None,
    }
}
fn same(a: &Value, b: &Value) -> bool {
    // Generic metadata filters retain Python equality, including bool vs 0/1.
    if a == b {
        return true;
    }
    let number = |v: &Value| {
        v.as_f64()
            .or_else(|| v.as_bool().map(|b| if b { 1. } else { 0. }))
    };
    if let Some((a, b)) = number(a).zip(number(b)) {
        return a == b;
    }
    match (a, b) {
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|other| same(v, other)))
        }
        _ => false,
    }
}
fn add<K: Eq + std::hash::Hash>(index: &mut HashMap<K, Set>, key: K, i: usize) {
    index.entry(key).or_default().insert(i);
}
fn normal_words(value: &str) -> Vec<String> {
    text::words(value, false)
        .iter()
        .map(|w| text::normal(w))
        .collect()
}
fn template(name: &str) -> Option<(String, String)> {
    for (start, _) in name.match_indices('[') {
        let rest = &name[start + 1..];
        let Some(end) = rest.find("]") else {
            continue;
        };
        let inner = &rest[..end];
        let Some(tag) = inner.strip_suffix(".GetAdjective") else {
            continue;
        };
        if tag == "ROOT"
            || tag.len() == 3
                && tag
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Some((name[..start].into(), rest[end + 1..].into()));
        }
    }
    None
}

pub struct Catalog {
    rows: Vec<Value>,
    vocabulary: Value,
    kinds: HashMap<String, Set>,
    ids: PairIndex,
    names: PairIndex,
    aliases: PairIndex,
    country_aliases: CountryAliasIndex,
    country_prefixes: CountryPrefixIndex,
    prefixes: PrefixIndex,
    event_options: HashMap<String, Set>,
    events: HashMap<String, Vec<usize>>,
    country_names: BTreeMap<String, Vec<String>>,
    adjectives: BTreeSet<String>,
    templates: Vec<(usize, String, String)>,
    suggestion_words: HashMap<String, Set>,
    labels: Vec<Vec<(String, String)>>,
    focus_labels: HashMap<usize, Vec<(String, Vec<String>)>>,
    focus_signatures: HashMap<(usize, Vec<String>), Set>,
    focus_full: HashMap<Vec<String>, Set>,
    focus_deleted: HashMap<Vec<String>, Set>,
    families: HashMap<String, String>,
    contexts: Mutex<VecDeque<(Vec<String>, Context, Set)>>,
}
impl Catalog {
    pub fn entries(&self) -> &[Value] {
        &self.rows
    }
    pub fn vocabulary(&self) -> &Value {
        &self.vocabulary
    }
    pub fn compile(build: Build) -> Result<Self, String> {
        if build.schema_version != 1 || build.entries.len() > 500_000 {
            return Err("Unsupported catalog version or size".into());
        }
        let mut c = Self {
            rows: build.entries,
            vocabulary: build.vocabulary.clone(),
            kinds: HashMap::new(),
            ids: HashMap::new(),
            names: HashMap::new(),
            aliases: HashMap::new(),
            country_aliases: HashMap::new(),
            country_prefixes: HashMap::new(),
            prefixes: HashMap::new(),
            event_options: HashMap::new(),
            events: HashMap::new(),
            country_names: build.country_names,
            adjectives: build
                .adjectives
                .iter()
                .filter(|s| !s.is_empty() && !s.contains('['))
                .map(|s| text::normal(s))
                .collect(),
            templates: Vec::new(),
            suggestion_words: HashMap::new(),
            labels: Vec::new(),
            focus_labels: HashMap::new(),
            focus_signatures: HashMap::new(),
            focus_full: HashMap::new(),
            focus_deleted: HashMap::new(),
            families: HashMap::new(),
            contexts: Mutex::new(VecDeque::new()),
        };
        if let Some(families) = build.vocabulary["research_families"].as_object() {
            c.families = families
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.into())))
                .collect();
        }
        for (i, row) in c.rows.iter().enumerate() {
            let kind = row["kind"]
                .as_str()
                .ok_or("Catalog kind must be a string")?
                .to_string();
            let id = row["id"]
                .as_str()
                .ok_or("Catalog ID must be a string")?
                .to_string();
            let name = row["name"]
                .as_str()
                .ok_or("Catalog name must be a string")?;
            if kind.len() > 512 || id.len() > 16_384 || name.len() > 65_536 {
                return Err("Catalog label exceeds limit".into());
            }
            add(&mut c.kinds, kind.clone(), i);
            add(&mut c.ids, (kind.clone(), id.clone()), i);
            let normalized = text::normal(name);
            add(&mut c.names, (kind.clone(), normalized.clone()), i);
            let mut aliases = vec![id.as_str(), string(&row["localized_name"])];
            aliases.extend(strings(&row["aliases"]));
            if let Some(vocabulary) = build.vocabulary["entity_names"][&kind].as_object() {
                for (alias, target) in vocabulary {
                    if target
                        .as_str()
                        .is_some_and(|t| t == id || text::normal(t) == normalized)
                    {
                        aliases.push(alias);
                    }
                }
            }
            let mut labels = vec![name];
            labels.extend(strings(&row["aliases"]));
            let labels: Vec<_> = labels
                .into_iter()
                .filter(|s| !s.is_empty())
                .map(|s| (s.to_string(), text::normal(s)))
                .collect();
            if kind == "focus" {
                if let Some((before, after)) = template(name) {
                    c.templates.push((i, before, after));
                }
                let words: Vec<_> = labels
                    .iter()
                    .map(|(label, _)| (label.clone(), normal_words(label)))
                    .collect();
                for (_, words) in &words {
                    if words.len() < 2 {
                        continue;
                    }
                    add(&mut c.focus_full, words.clone(), i);
                    for position in 0..words.len() {
                        let mut deleted = words.clone();
                        deleted.remove(position);
                        add(&mut c.focus_signatures, (position, deleted.clone()), i);
                        add(&mut c.focus_deleted, deleted, i);
                    }
                }
                c.focus_labels.insert(i, words);
                if let Some(alias) = text::strip_to(name) {
                    add(&mut c.aliases, (kind.clone(), text::normal(&alias)), i);
                }
            }
            c.labels.push(labels);
            for alias in &aliases {
                if !alias.is_empty() {
                    add(&mut c.aliases, (kind.clone(), text::normal(alias)), i);
                }
            }
            if let Some(countries) = row["country_aliases"].as_object() {
                for (country, values) in countries {
                    for alias in strings(values) {
                        add(
                            &mut c.country_aliases,
                            (kind.clone(), country.clone(), text::normal(alias)),
                            i,
                        );
                        let words = text::words(&text::fold(alias), true);
                        for end in 2..=words.len() {
                            add(
                                &mut c.country_prefixes,
                                (kind.clone(), country.clone(), words[..end].to_vec()),
                                i,
                            );
                        }
                        if numbered_prefix(&words) {
                            add(
                                &mut c.country_prefixes,
                                (kind.clone(), country.clone(), words[..1].to_vec()),
                                i,
                            );
                        }
                    }
                }
            }
            for label in std::iter::once(name).chain(aliases.iter().copied()) {
                let words = text::words(&text::fold(label), true);
                for word in &words {
                    if word.chars().count() >= 3 {
                        add(&mut c.suggestion_words, word.clone(), i);
                    }
                }
                for end in 2..=words.len() {
                    add(&mut c.prefixes, (kind.clone(), words[..end].to_vec()), i);
                }
                if numbered_prefix(&words) {
                    add(&mut c.prefixes, (kind.clone(), words[..1].to_vec()), i);
                }
            }
            if kind == "event" {
                c.events.entry(id).or_default().push(i);
            }
            if kind == "event_option" {
                add(&mut c.event_options, string(&row["event_id"]).into(), i);
            }
        }
        Ok(c)
    }
    fn allowed(&self, i: usize, context: &Context) -> bool {
        let row = &self.rows[i];
        if let Some(scope) = nonempty(&context.scope)
            && row["kind"] == "focus"
            && row["scope_independent"] != true
            && row["scope"].as_str() != Some(scope)
        {
            return false;
        }
        if let Some(country) = nonempty(&context.country) {
            let countries = strings(&row["countries"]);
            if row.get("country_condition").is_some() {
                let national = country_test(&row["country_condition"], country, 0);
                if national == Some(false)
                    || national.is_none() && !countries.is_empty() && !countries.contains(&country)
                {
                    return false;
                }
            } else if !countries.is_empty() && !countries.contains(&country) {
                return false;
            }
            if row["kind"] == "event_option"
                && let Some(parents) = self.events.get(string(&row["event_id"]))
                && !parents.is_empty()
                && parents.iter().all(|p| {
                    country_test(&self.rows[*p]["country_condition"], country, 0) == Some(false)
                })
            {
                return false;
            }
        }
        true
    }
    fn domain(&self, q: &Query) -> Set {
        let mut kinds = q.kinds.clone();
        kinds.sort();
        let mut cache = self.contexts.lock().unwrap_or_else(|e| e.into_inner());
        let mut indices = if let Some(position) = cache
            .iter()
            .position(|(k, c, _)| *k == kinds && *c == q.context)
        {
            let entry = cache.remove(position).expect("existing context");
            let selected = entry.2.clone();
            cache.push_back(entry);
            selected
        } else {
            let domain: Set = q
                .kinds
                .iter()
                .flat_map(|kind| self.kinds.get(kind).into_iter().flatten().copied())
                .filter(|i| self.allowed(*i, &q.context))
                .collect();
            if cache.len() >= 64 {
                cache.pop_front();
            }
            cache.push_back((kinds, q.context.clone(), domain.clone()));
            domain
        };
        drop(cache);
        if let Some(event) = q.filters.get("event_id") {
            indices.retain(|i| {
                self.event_options
                    .get(string(event))
                    .is_some_and(|s| s.contains(i))
            });
        }
        if !q.filters.is_empty() {
            indices.retain(|i| {
                q.filters
                    .iter()
                    .all(|(k, v)| same(self.rows[*i].get(k).unwrap_or(&Value::Null), v))
            });
        }
        indices
    }
    fn matches(&self, q: &Query, query: &str, depth: usize) -> Set {
        if query.is_empty() || depth > 8 {
            return Set::new();
        }
        let domain = self.domain(q);
        let ids: Set = q
            .kinds
            .iter()
            .flat_map(|kind| {
                self.ids
                    .get(&(kind.clone(), query.into()))
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect();
        if !ids.is_empty() || q.explicit {
            return ids.intersection(&domain).copied().collect();
        }
        let focus = q.kinds == ["focus"];
        let technology = q.kinds == ["technology"];
        if focus && let Some(optional) = text::optional_focus(query) {
            return self
                .matches(q, &optional, depth + 1)
                .into_iter()
                .filter(|i| self.rows[*i]["continuous"] == true)
                .collect();
        }
        let needle = text::normal(query);
        let words = text::words(&text::fold(query), true);
        let mut names = Set::new();
        let mut aliases = Set::new();
        let mut prefixes = Set::new();
        for kind in &q.kinds {
            names.extend(
                self.names
                    .get(&(kind.clone(), needle.clone()))
                    .into_iter()
                    .flatten()
                    .copied(),
            );
            aliases.extend(
                self.aliases
                    .get(&(kind.clone(), needle.clone()))
                    .into_iter()
                    .flatten()
                    .copied(),
            );
            prefixes.extend(
                self.prefixes
                    .get(&(kind.clone(), words.clone()))
                    .into_iter()
                    .flatten()
                    .copied(),
            );
            if let Some(country) = &q.context.country {
                aliases.extend(
                    self.country_aliases
                        .get(&(kind.clone(), country.clone(), needle.clone()))
                        .into_iter()
                        .flatten()
                        .copied(),
                );
                prefixes.extend(
                    self.country_prefixes
                        .get(&(kind.clone(), country.clone(), words.clone()))
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
        }
        for candidates in [&names, &aliases, &prefixes] {
            let selected: Set = candidates.intersection(&domain).copied().collect();
            if !selected.is_empty() {
                return selected;
            }
        }
        if focus && query.contains('/') {
            let folded = text::fold(query);
            let variants: Set = self
                .templates
                .iter()
                .filter_map(|(i, before, after)| {
                    if !domain.contains(i) {
                        return None;
                    }
                    let before = text::fold(before);
                    let after = text::fold(after);
                    let capture = folded.strip_prefix(&before)?.strip_suffix(&after)?;
                    let parts: Vec<_> = capture
                        .split('/')
                        .map(|s| text::normal(s.trim_matches(text::space)))
                        .collect();
                    (parts.len() >= 2
                        && parts.len() <= 8
                        && parts.iter().all(|s| self.adjectives.contains(s)))
                    .then_some(*i)
                })
                .collect();
            if !variants.is_empty() {
                return variants;
            }
        }
        if focus && let Some(other) = text::strip_to(query) {
            return self.matches(q, &other, depth + 1);
        }
        if technology {
            let expanded = text::expand(query, &self.families);
            if expanded != query {
                return self.matches(q, &expanded, depth + 1);
            }
        }
        if q.typos && !q.explicit && !query.contains('_') && needle.chars().count() >= 10 {
            for max_edits in [2, 3] {
                if max_edits == 3 && !(focus && nonempty(&q.context.scope).is_some()) {
                    break;
                }
                let source = text::spelling(query);
                let mut scored = Vec::new();
                for i in &domain {
                    let row = &self.rows[*i];
                    let mut labels = vec![string(&row["name"])];
                    labels.extend(strings(&row["aliases"]));
                    if let Some(cost) = labels
                        .iter()
                        .filter_map(|label| {
                            text::typo_cost(&source, &text::spelling(label), max_edits)
                        })
                        .min()
                    {
                        scored.push((cost, *i));
                    }
                }
                if let Some(best) = scored.iter().map(|(cost, _)| *cost).min() {
                    return scored
                        .into_iter()
                        .filter_map(|(cost, i)| (cost == best).then_some(i))
                        .collect();
                }
            }
        }
        Set::new()
    }
    fn focus_words(&self, q: &Query) -> Value {
        if nonempty(&q.context.country).is_none() && nonempty(&q.context.scope).is_none()
            || q.query.contains(['_', '/', '\\', '…'])
            || q.query.contains("...")
        {
            return json!([]);
        }
        let words = normal_words(&q.query);
        if words.len() < 2 {
            return json!([]);
        }
        let mut nearby = Set::new();
        for position in 0..words.len() {
            let mut deleted = words.clone();
            deleted.remove(position);
            nearby.extend(
                self.focus_signatures
                    .get(&(position, deleted.clone()))
                    .into_iter()
                    .flatten(),
            );
            nearby.extend(self.focus_full.get(&deleted).into_iter().flatten());
        }
        nearby.extend(self.focus_deleted.get(&words).into_iter().flatten());
        let domain = self.domain(q);
        let mut out = Vec::new();
        for i in nearby.intersection(&domain) {
            for (label, other) in self.focus_labels.get(i).into_iter().flatten() {
                let close = if words.len() == other.len() {
                    words.iter().zip(other).filter(|(a, b)| a != b).count() == 1
                } else {
                    let (longer, shorter) = if words.len() > other.len() {
                        (&words, other)
                    } else {
                        (other, &words)
                    };
                    longer.len() == shorter.len() + 1
                        && (0..longer.len()).any(|n| {
                            longer[..n]
                                .iter()
                                .chain(&longer[n + 1..])
                                .eq(shorter.iter())
                        })
                };
                if close {
                    out.push(json!([i, label]));
                }
            }
        }
        json!(out)
    }
    fn suggest(&self, q: &Query) -> Value {
        if q.query.trim_matches(text::space).is_empty() {
            return json!([]);
        }
        let domain = self.domain(q);
        let words = text::words(&text::fold(&q.query), true);
        let stop = ["the", "and", "for", "with", "from", "that", "this", "into"];
        let mut nearby: Set = words
            .iter()
            .filter(|w| w.chars().count() >= 3 && !stop.contains(&w.as_str()))
            .flat_map(|w| self.suggestion_words.get(w).into_iter().flatten().copied())
            .filter(|i| domain.contains(i))
            .collect();
        if nearby.is_empty() {
            nearby = domain;
        }
        let needle = text::normal(&q.query);
        let mut scored = Vec::new();
        for i in nearby {
            let best = self.labels[i]
                .iter()
                .map(|(label, normalized)| (text::similarity(&needle, normalized), label))
                .max_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(b.1)));
            if let Some((score, label)) = best
                && score >= 0.55
            {
                scored.push((score, i, label));
            }
        }
        scored.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| string(&self.rows[a.1]["id"]).cmp(string(&self.rows[b.1]["id"])))
                .then_with(|| {
                    string(&self.rows[a.1]["scope"]).cmp(string(&self.rows[b.1]["scope"]))
                })
                .then(a.1.cmp(&b.1))
        });
        json!(scored.into_iter().take(q.limit).map(|(score,i,label)|json!({"index":i,"suggestion":{"basis":"text_similarity_only","score":format!("{score:.4}").parse::<f64>().expect("formatted score"),"matched_label":label,"requires_review":true}})).collect::<Vec<_>>())
    }
    fn resolve_country(&self, query: &str) -> Option<String> {
        let query = query.trim_matches(text::space);
        if query.is_empty() {
            return None;
        }
        let tag = query.to_uppercase();
        if tag.len() == 3
            && tag
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && (self.country_names.is_empty() || self.country_names.contains_key(&tag))
        {
            return Some(tag);
        }
        let needle = text::normal(query);
        let found: Vec<_> = self
            .country_names
            .iter()
            .filter(|(_, names)| names.iter().any(|n| text::normal(n) == needle))
            .map(|(tag, _)| tag.clone())
            .collect();
        if found.len() == 1 {
            Some(found[0].clone())
        } else {
            None
        }
    }
    fn country_hint(&self, q: &Query) -> Option<String> {
        let title = text::fold(&q.query);
        let candidates: BTreeSet<_> = self
            .country_names
            .iter()
            .filter(|(_, names)| {
                names.iter().filter(|s| !s.is_empty()).any(|name| {
                    title.strip_prefix(&text::fold(name)).is_some_and(|suffix| {
                        suffix.is_empty()
                            || suffix.chars().next().is_some_and(|c| {
                                text::space(c) || matches!(c, '_' | '(' | ':' | '—' | '-')
                            })
                    })
                })
            })
            .map(|(tag, _)| tag.clone())
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let mut support = BTreeMap::<String, BTreeSet<String>>::new();
        for rows in q.guide["tracks"].as_object()?.values() {
            for row in rows.as_array()? {
                if row["action"]["kind"] != "select_focus" {
                    continue;
                }
                let entity = &row["action"]["entity"];
                let query = string(&entity["id"]);
                let request = Query {
                    schema_version: 1,
                    op: Operation::Matches,
                    query: query.into(),
                    kinds: vec!["focus".into()],
                    context: Context {
                        country: None,
                        scope: entity["scope"].as_str().map(str::to_owned),
                    },
                    explicit: false,
                    typos: true,
                    filters: BTreeMap::new(),
                    limit: 6,
                    guide: Value::Null,
                    texts: Vec::new(),
                };
                for i in self.matches(&request, query, 0) {
                    for tag in strings(&self.rows[i]["countries"]) {
                        support
                            .entry(tag.into())
                            .or_default()
                            .insert(text::normal(query));
                    }
                }
            }
        }
        let maximum = support.values().map(BTreeSet::len).max()?;
        let qualified: Vec<_> = support
            .iter()
            .filter(|(tag, values)| values.len() == maximum && candidates.contains(*tag))
            .map(|(tag, _)| tag.clone())
            .collect();
        if maximum >= 2 && qualified.len() == 1 {
            Some(qualified[0].clone())
        } else {
            None
        }
    }
    fn identity(&self, i: usize) -> Value {
        let mut out = serde_json::Map::new();
        for field in ["kind", "id", "scope", "name", "event_id", "slot", "group"] {
            if let Some(value) = self.rows[i].get(field) {
                out.insert(field.into(), value.clone());
            }
        }
        Value::Object(out)
    }
    pub fn query(&self, q: Query) -> Result<Value, String> {
        if q.schema_version != 1
            || q.query.len() > 65_536
            || q.kinds.len() > 64
            || q.limit > 65_536
            || q.texts.len() > 100_000
        {
            return Err("Unsupported catalog query version or size".into());
        }
        Ok(match q.op {
            Operation::Rows => json!(self.domain(&q)),
            Operation::Matches => json!(self.matches(&q, &q.query, 0)),
            Operation::FocusWords => self.focus_words(&q),
            Operation::Suggest => self.suggest(&q),
            Operation::ResolveCountry => json!(self.resolve_country(&q.query)),
            Operation::CountryHint => json!(self.country_hint(&q)),
            Operation::Normalize => {
                json!(q.texts.iter().map(|s| text::normal(s)).collect::<Vec<_>>())
            }
            Operation::Stats => {
                json!({"rows":self.rows.len(),"ids":self.ids.len(),"names":self.names.len(),"aliases":self.aliases.len(),"prefixes":self.prefixes.len(),"contexts":self.contexts.lock().unwrap_or_else(|e|e.into_inner()).len()})
            }
            Operation::Excluded | Operation::Audit => {
                let request = Query {
                    context: Context::default(),
                    filters: if matches!(q.op, Operation::Excluded) {
                        q.filters.clone()
                    } else {
                        BTreeMap::new()
                    },
                    typos: matches!(q.op, Operation::Excluded),
                    ..q.clone_for_query()
                };
                let all = self.matches(&request, &q.query, 0);
                let mut excluded = Vec::new();
                for i in all {
                    let reason = if !self.allowed(i, &q.context) {
                        Some("country_or_scope_conflict")
                    } else if matches!(q.op, Operation::Audit)
                        && q.filters
                            .iter()
                            .any(|(k, v)| !same(self.rows[i].get(k).unwrap_or(&Value::Null), v))
                    {
                        Some("declared_parent_or_role_conflict")
                    } else {
                        None
                    };
                    if let Some(reason) = reason {
                        excluded.push(if matches!(q.op, Operation::Audit) {
                            json!({"entity":self.identity(i),"reason":reason})
                        } else {
                            json!({"index":i,"reason":reason})
                        });
                    }
                }
                if matches!(q.op, Operation::Excluded) {
                    json!(excluded)
                } else {
                    json!({"query":q.query,"country":q.context.country,"scope":q.context.scope,"filters":q.filters,"candidates":self.matches(&q,&q.query,0).into_iter().map(|i|self.identity(i)).collect::<Vec<_>>(),"excluded_candidates":excluded})
                }
            }
        })
    }
}
impl Query {
    fn clone_for_query(&self) -> Self {
        Self {
            schema_version: self.schema_version,
            op: Operation::Matches,
            query: self.query.clone(),
            kinds: self.kinds.clone(),
            context: self.context.clone(),
            explicit: self.explicit,
            typos: self.typos,
            filters: self.filters.clone(),
            limit: self.limit,
            guide: Value::Null,
            texts: Vec::new(),
        }
    }
}
fn numbered_prefix(words: &[String]) -> bool {
    words.len() > 1
        && words[0].chars().count() >= 8
        && words[1..].iter().all(|w| w.chars().all(text::decimal))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_and_unknown_country_keep_ambiguity() {
        let c = Catalog::compile(
            serde_json::from_value(json!({"schema_version":1,"entries":[
            {"kind":"law","id":"a","name":"Law","country_condition":{"op":"unknown"}},
            {"kind":"law","id":"b","name":"Law","country_condition":{"op":"tag","value":"GER"}}]}))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(c.query(serde_json::from_value(json!({"schema_version":1,"op":"matches","query":"Law","kinds":["law"],"country":"GER"})).unwrap()).unwrap(),json!([0,1]));
        assert_eq!(c.query(serde_json::from_value(json!({"schema_version":1,"op":"matches","query":"b","kinds":["law"],"country":"POL"})).unwrap()).unwrap(),json!([]));
    }
    #[test]
    fn malformed_rows_versions_and_missing_identity_are_rejected() {
        assert!(
            Catalog::compile(
                serde_json::from_value(json!({"schema_version":2,"entries":[]})).unwrap()
            )
            .is_err()
        );
        assert!(
            Catalog::compile(
                serde_json::from_value(
                    json!({"schema_version":1,"entries":[{"kind":"focus","name":"Missing ID"}]})
                )
                .unwrap()
            )
            .is_err()
        );
    }
}
