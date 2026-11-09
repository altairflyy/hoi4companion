//! Whole-guide static route constraints. Identity proof never observes completion.
use crate::{focus_import, import_catalog::Index};
use hoi4companion_catalog_core::{casefold, identity_words, normal_name};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
type Id = (Option<String>, String);
type Ids = BTreeSet<Id>;
type Graph = BTreeMap<Id, Value>;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.),
    }
}
fn id(v: &Value) -> Id {
    (
        v["scope"].as_str().map(str::to_owned),
        s(&v["id"]).to_owned(),
    )
}
fn value(k: &Id) -> Value {
    json!({"scope":k.0,"id":k.1})
}
fn related(k: &Id, id: &Value) -> Id {
    (k.0.clone(), s(id).to_owned())
}
fn names(ids: &Ids) -> Vec<String> {
    let mut result = ids.iter().map(|k| k.1.clone()).collect::<Vec<_>>();
    result.sort();
    result
}
fn ordered(ids: &Ids) -> Vec<&Id> {
    let mut keys = ids.iter().collect::<Vec<_>>();
    keys.sort_by(|a, b| a.0.is_none().cmp(&b.0.is_none()).then_with(|| a.cmp(b)));
    keys
}
fn scope<'a>(row: &'a Value, guide: &'a Value) -> &'a Value {
    row["action"]["entity"]
        .get("scope")
        .filter(|v| truth(v))
        .unwrap_or(&guide["applies_to"]["tree_id"])
}
fn optional(q: &str) -> bool {
    let parts = q.split('/').map(str::trim).collect::<Vec<_>>();
    parts.len() == 2
        && ((!parts[0].is_empty()
            && ["nothing", "none", "no focus"].contains(&casefold(parts[1]).as_str()))
            || (!parts[1].is_empty()
                && ["nothing", "none", "no focus"].contains(&casefold(parts[0]).as_str())))
}
pub fn same_qualifiers(query: &str, label: &str) -> bool {
    let levels = crate::import_catalog::regex(
        r"(?i)\b(?:\d+(?:st|nd|rd|th)?|VIII|VII|VI|IV|IX|III|II|I|V|X)\b",
    );
    let get = |s: &str| {
        levels
            .find_iter(s)
            .map(|m| normal_name(m.as_str()))
            .collect::<Vec<_>>()
    };
    if get(query) != get(label) {
        return false;
    }
    let bearing = |s: &str| {
        identity_words(&casefold(s), true)
            .iter()
            .filter_map(|w| match w.as_str() {
                "east" | "eastern" | "eastward" => Some("east"),
                "west" | "western" | "westward" => Some("west"),
                "north" | "northern" | "northward" => Some("north"),
                "south" | "southern" | "southward" => Some("south"),
                _ => None,
            })
            .collect::<BTreeSet<_>>()
    };
    bearing(query) == bearing(label)
}
pub fn word_variant(query: &str, label: &str) -> Option<Value> {
    if query.contains('_') || ["/", "\\", "…", "..."].iter().any(|m| query.contains(m)) {
        return None;
    }
    let words = |s: &str| {
        identity_words(s, false)
            .iter()
            .map(|w| normal_name(w))
            .collect::<Vec<_>>()
    };
    let left = words(query);
    let right = words(label);
    if left.len().min(right.len()) < 2
        || left.len().abs_diff(right.len()) > 1
        || !same_qualifiers(query, label)
    {
        return None;
    }
    let (different, shared, method) = if left.len() == right.len() {
        let different = left
            .iter()
            .zip(&right)
            .filter(|(a, b)| a != b)
            .map(|(a, b)| (a.clone(), b.clone()))
            .collect::<Vec<_>>();
        if different.len() != 1
            || different
                .iter()
                .any(|(a, b)| a.chars().count() < 2 || b.chars().count() < 2)
        {
            return None;
        }
        let shared = left
            .iter()
            .zip(&right)
            .filter(|(a, b)| a == b)
            .map(|(a, _)| a.clone())
            .collect::<Vec<_>>();
        (different, shared, "one_positional_word_difference")
    } else {
        let (long, short) = if left.len() > right.len() {
            (&left, &right)
        } else {
            (&right, &left)
        };
        let remove = (0..long.len()).find(|i| {
            long.iter()
                .enumerate()
                .filter(|(j, _)| j != i)
                .map(|(_, w)| w)
                .eq(short.iter())
        })?;
        let different = if left.len() > right.len() {
            (long[remove].clone(), String::new())
        } else {
            (String::new(), long[remove].clone())
        };
        (
            vec![different],
            short.clone(),
            "one_word_insertion_or_omission",
        )
    };
    let protected = [
        "basic",
        "improved",
        "advanced",
        "modern",
        "medium",
        "small",
        "large",
        "light",
        "heavy",
        "democratic",
        "communist",
        "communism",
        "fascist",
        "fascism",
        "monarchist",
        "neutrality",
        "not",
        "no",
        "never",
        "without",
        "against",
        "defense",
        "defence",
        "offense",
        "offence",
        "attack",
        "defend",
        "land",
        "naval",
        "air",
    ];
    if different.iter().flat_map(|(a, b)| [a, b]).any(|w| {
        protected.contains(&w.as_str())
            || !w.is_empty() && w.chars().all(hoi4companion_catalog_core::unicode_decimal)
    }) {
        return None;
    }
    let stop = [
        "the", "and", "from", "with", "that", "this", "into", "for", "of", "to", "a", "an", "in",
        "on", "at", "by", "or", "as",
    ];
    let meaningful = shared
        .iter()
        .filter(|w| !stop.contains(&w.as_str()))
        .collect::<Vec<_>>();
    if !meaningful.iter().any(|w| w.chars().count() >= 4)
        && meaningful.iter().filter(|w| w.chars().count() >= 3).count() < 2
    {
        return None;
    }
    Some(
        json!({"method":method,"matched_label":label,"changed_words":different.iter().map(|(a,b)|json!({"source":a,"catalog":b})).collect::<Vec<_>>()}),
    )
}
struct Route<'a> {
    nodes: &'a Graph,
    required: Ids,
    opposites: BTreeMap<Id, Ids>,
    conflicts: Vec<Value>,
    cache: RefCell<BTreeMap<Id, (Option<bool>, Ids)>>,
    guaranteed_cache: RefCell<BTreeMap<Id, Ids>>,
    acyclic: Ids,
}
impl<'a> Route<'a> {
    fn new(nodes: &'a Graph, anchors: &Ids) -> Self {
        fn acyclic(
            key: &Id,
            nodes: &Graph,
            visiting: &mut Ids,
            memo: &mut BTreeMap<Id, bool>,
        ) -> bool {
            if let Some(value) = memo.get(key) {
                return *value;
            }
            if visiting.len() > 512 || !visiting.insert(key.clone()) {
                return false;
            }
            let answer = nodes.get(key).is_none_or(|node| {
                arr(&node["prerequisites"])
                    .iter()
                    .flat_map(arr)
                    .all(|p| acyclic(&related(key, p), nodes, visiting, memo))
            });
            visiting.remove(key);
            memo.insert(key.clone(), answer);
            answer
        }
        let mut memo = BTreeMap::new();
        for key in nodes.keys() {
            acyclic(key, nodes, &mut Ids::new(), &mut memo);
        }
        let mut route = Self {
            nodes,
            required: anchors.clone(),
            opposites: BTreeMap::new(),
            conflicts: vec![],
            cache: RefCell::new(BTreeMap::new()),
            guaranteed_cache: RefCell::new(BTreeMap::new()),
            acyclic: memo
                .into_iter()
                .filter_map(|(k, ok)| ok.then_some(k))
                .collect(),
        };
        for (k, n) in nodes {
            for other in arr(&n["mutually_exclusive"]) {
                let other = related(k, other);
                route
                    .opposites
                    .entry(k.clone())
                    .or_default()
                    .insert(other.clone());
                route.opposites.entry(other).or_default().insert(k.clone());
            }
        }
        loop {
            let mut extra = Ids::new();
            for key in ordered(&route.required) {
                let (status, witness) = route.check(key);
                if status == Some(false) {
                    route
                        .conflicts
                        .push(json!({"entity":value(key),"conflicting_focus_ids":names(&witness)}));
                } else {
                    extra.extend(route.guaranteed(key));
                }
            }
            extra = extra.difference(&route.required).cloned().collect();
            if !route.conflicts.is_empty() || extra.is_empty() {
                break;
            }
            route.required.extend(extra);
            route.cache.borrow_mut().clear();
            route.guaranteed_cache.borrow_mut().clear();
        }
        route
    }
    fn check(&self, key: &Id) -> (Option<bool>, Ids) {
        if let Some(value) = self.cache.borrow().get(key) {
            return value.clone();
        }
        let value = self.check_inner(key, &mut Ids::new());
        self.cache.borrow_mut().insert(key.clone(), value.clone());
        value
    }
    fn check_inner(&self, key: &Id, stack: &mut Ids) -> (Option<bool>, Ids) {
        let opposed = self
            .opposites
            .get(key)
            .map(|o| o.intersection(&self.required).cloned().collect::<Ids>())
            .unwrap_or_default();
        if !opposed.is_empty() {
            return (Some(false), opposed);
        }
        let Some(node) = self.nodes.get(key) else {
            return (None, Ids::new());
        };
        if node.get("prerequisites").is_none() || stack.contains(key) || stack.len() > 512 {
            return (None, Ids::new());
        }
        stack.insert(key.clone());
        let mut unknown = false;
        for group in arr(&node["prerequisites"]) {
            let mut choices = vec![];
            for p in arr(group) {
                let parent = related(key, p);
                let mut choice = self.check_inner(&parent, stack);
                if self.nodes.get(&parent).is_some_and(|n| {
                    truth(&n["bypass_possible"]) || truth(&n["external_completion_possible"])
                }) {
                    choice = (Some(true), Ids::new());
                }
                choices.push(choice);
            }
            if !choices.iter().any(|(b, _)| *b == Some(true)) {
                if choices.iter().any(|(b, _)| b.is_none()) {
                    unknown = true;
                } else {
                    stack.remove(key);
                    return (
                        Some(false),
                        choices.into_iter().flat_map(|(_, w)| w).collect(),
                    );
                }
            }
        }
        stack.remove(key);
        (if unknown { None } else { Some(true) }, Ids::new())
    }
    fn guaranteed(&self, key: &Id) -> Ids {
        self.guaranteed_inner(key, &mut Ids::new())
    }
    fn guaranteed_inner(&self, key: &Id, stack: &mut Ids) -> Ids {
        if let Some(value) = self.guaranteed_cache.borrow().get(key) {
            return value.clone();
        }
        let mut required = Ids::from([key.clone()]);
        let Some(node) = self.nodes.get(key) else {
            return required;
        };
        if stack.contains(key)
            || stack.len() > 512
            || truth(&node["bypass_possible"])
            || truth(&node["external_completion_possible"])
        {
            return required;
        }
        stack.insert(key.clone());
        for group in arr(&node["prerequisites"]) {
            if arr(group).iter().any(|p| {
                self.nodes.get(&related(key, p)).is_some_and(|n| {
                    truth(&n["bypass_possible"]) || truth(&n["external_completion_possible"])
                })
            }) {
                continue;
            }
            let viable = arr(group)
                .iter()
                .map(|p| related(key, p))
                .filter(|p| self.check(p).0 != Some(false))
                .collect::<Vec<_>>();
            let mut alternatives = viable.iter().map(|p| self.guaranteed_inner(p, stack));
            if let Some(first) = alternatives.next() {
                let intersection =
                    alternatives.fold(first, |a, b| a.intersection(&b).cloned().collect());
                required.extend(intersection);
            }
        }
        stack.remove(key);
        if self.acyclic.contains(key) {
            self.guaranteed_cache
                .borrow_mut()
                .insert(key.clone(), required.clone());
        }
        required
    }
    fn summary(&self) -> Value {
        json!({"status":if !self.conflicts.is_empty(){"conflict"}else if self.required.iter().any(|k|self.check(k).0.is_none()){"incomplete"}else{"no_conflict_detected"},"required_focuses":ordered(&self.required).into_iter().map(value).collect::<Vec<_>>(),"conflicts":self.conflicts})
    }
}
fn conditional(
    key: &str,
    rows: &serde_json::Map<String, Value>,
    phases: &HashMap<String, &Value>,
    stack: &mut HashSet<String>,
) -> bool {
    if !stack.insert(key.into()) {
        return true;
    }
    let Some(row) = rows.get(key) else {
        return true;
    };
    let result = row.get("when").is_some()
        || row.get("skip_when").is_some()
        || phases
            .get(s(&row["phase"]))
            .is_some_and(|p| p.get("when").is_some())
        || ["depends_on", "depends_on_started"]
            .iter()
            .flat_map(|f| arr(&row[*f]))
            .any(|p| conditional(s(p), rows, phases, stack));
    stack.remove(key);
    result
}
struct Context<'a> {
    guide: &'a Value,
    rows: &'a serde_json::Map<String, Value>,
    mandatory: &'a HashMap<String, bool>,
    nodes: &'a Graph,
}
fn source_branch(
    ctx: &Context,
    key: &str,
    matches: &[Value],
    domains: &serde_json::Map<String, Value>,
    anchors: &Ids,
) -> Option<(Value, Value)> {
    let Context {
        guide,
        rows,
        mandatory,
        nodes,
    } = ctx;
    let row = &rows[key];
    if matches.len() < 2
        || matches
            .iter()
            .map(|e| normal_name(s(&e["name"])))
            .collect::<BTreeSet<_>>()
            .len()
            != 1
        || truth(&row["focus_resolution"]["locked"])
    {
        return None;
    }
    let files = arr(&guide["sources"])
        .iter()
        .filter(|src| {
            src["type"] == "file"
                && s(&src["text"]).contains("[Table row]")
                && s(&src["text"]).contains("[Column 3]")
        })
        .map(|src| s(&src["id"]))
        .collect::<HashSet<_>>();
    if !arr(&row["evidence"])
        .iter()
        .any(|e| files.contains(s(&e["source_id"])))
    {
        return None;
    }
    let bypass = crate::import_catalog::regex(r"(?i)\bbypass(?:ed)?\b");
    if rows
        .values()
        .any(|r| bypass.is_match(&format!("{} {}", s(&r["note"]), s(&r["title"]))))
    {
        return None;
    }
    let mut visited = HashMap::new();
    let mut stack = matches.iter().map(|e| (id(e), false)).collect::<Vec<_>>();
    while let Some((identity, leaving)) = stack.pop() {
        if leaving {
            visited.insert(identity, 2);
            continue;
        }
        if visited.get(&identity) == Some(&2) {
            continue;
        }
        let entry = nodes.get(&identity)?;
        if visited.get(&identity) == Some(&1)
            || visited.len() > 1200
            || entry.get("prerequisites").is_none()
        {
            return None;
        }
        visited.insert(identity.clone(), 1);
        stack.push((identity.clone(), true));
        stack.extend(
            arr(&entry["prerequisites"])
                .iter()
                .flat_map(arr)
                .map(|p| (related(&identity, p), false)),
        );
    }
    let normal = nodes
        .iter()
        .map(|(k, n)| {
            let mut n = n.clone();
            n["bypass_possible"] = json!(false);
            n["external_completion_possible"] = json!(false);
            (k.clone(), n)
        })
        .collect::<Graph>();
    let route = Route::new(&normal, anchors);
    if !route.conflicts.is_empty() {
        return None;
    }
    let values = matches
        .iter()
        .map(|e| route.check(&id(e)).0)
        .collect::<Vec<_>>();
    if values.iter().filter(|v| **v == Some(true)).count() != 1 || values.contains(&None) {
        return None;
    }
    let candidate = matches[values.iter().position(|v| *v == Some(true))?].clone();
    let mut previous = Ids::new();
    for (k, entries) in domains {
        if !mandatory[k]
            || rows[k]["_focus_position"].as_u64().unwrap_or(0)
                >= row["_focus_position"].as_u64().unwrap_or(0)
        {
            continue;
        }
        let bound = arr(entries)
            .iter()
            .map(id)
            .filter(|k| anchors.contains(k))
            .collect::<Ids>();
        if bound.len() == 1 {
            previous.extend(bound);
        }
    }
    let parents = arr(&candidate["prerequisites"])
        .iter()
        .flat_map(arr)
        .map(|p| related(&id(&candidate), p))
        .collect::<Ids>();
    let support = parents.intersection(&previous).cloned().collect::<Ids>();
    if support.is_empty() {
        return None;
    }
    let mut excluded = matches
        .iter()
        .zip(values)
        .filter(|(_, v)| *v == Some(false))
        .map(|(e, _)| s(&e["id"]).to_owned())
        .collect::<Vec<_>>();
    excluded.sort();
    Some((
        candidate,
        json!({"method":"declared_source_branch","supporting_focus_ids":names(&support),"basis":"same_label_direct_predecessor_and_normal_route_exclusions","bypasses_are_still_possible":true,"excluded_focus_ids":excluded}),
    ))
}
fn suggestions(
    index: &Index,
    query: &str,
    country: &Value,
    scope: &Value,
    limit: usize,
) -> Result<Vec<Value>, String> {
    let result=index.query(json!({"op":"suggest","query":query,"kinds":["focus"],"country":country,"scope":scope,"limit":limit}))?;
    arr(&result)
        .iter()
        .map(|r| {
            let mut entry = index
                .rows()
                .get(r["index"].as_u64().ok_or("Invalid suggestion")? as usize)
                .cloned()
                .ok_or("Invalid suggestion index")?;
            entry["suggestion"] = r["suggestion"].clone();
            Ok(entry)
        })
        .collect()
}
pub fn infer_scope(guide: &Value, index: &Index) -> Result<Value, String> {
    if truth(&guide["applies_to"]["tree_id"]) {
        return Ok(Value::Null);
    }
    let mut allowed: Option<BTreeSet<String>> = None;
    let mut names = BTreeSet::new();
    for row in guide["tracks"]
        .as_object()
        .ok_or("Missing guide tracks")?
        .values()
        .flat_map(arr)
        .filter(|r| r["action"]["kind"] == "select_focus")
    {
        let query = s(&row["action"]["entity"]["id"]);
        let found = index.matches(
            query,
            &["focus"],
            &guide["applies_to"]["country"],
            &row["action"]["entity"]["scope"],
            false,
        )?;
        let candidates = found
            .iter()
            .filter_map(|e| e["scope"].as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        if candidates.is_empty() {
            continue;
        }
        names.insert(normal_name(query));
        allowed = Some(
            allowed
                .map(|a| a.intersection(&candidates).cloned().collect())
                .unwrap_or(candidates),
        );
        if allowed.as_ref().is_some_and(BTreeSet::is_empty) {
            return Ok(Value::Null);
        }
    }
    Ok(
        if names.len() >= 2 && allowed.as_ref().is_some_and(|a| a.len() == 1) {
            json!(allowed.unwrap().first())
        } else {
            Value::Null
        },
    )
}
fn resolve_names(guide: &Value, index: &Index) -> Result<(Value, Value, Vec<String>), String> {
    let rows = guide["tracks"]
        .as_object()
        .ok_or("Missing guide tracks")?
        .values()
        .flat_map(arr)
        .map(|r| (s(&r["key"]).to_owned(), r.clone()))
        .collect::<serde_json::Map<_, _>>();
    let phases = arr(&guide["phases"])
        .iter()
        .map(|p| (s(&p["id"]).to_owned(), p))
        .collect::<HashMap<_, _>>();
    let nodes = index
        .entries(json!({"op":"rows","kinds":["focus"]}))?
        .into_iter()
        .map(|e| (id(&e), e))
        .collect::<Graph>();
    let other_kinds = index
        .rows()
        .iter()
        .filter_map(|e| e["kind"].as_str())
        .filter(|k| *k != "focus")
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let identity_graph = Route::new(&nodes, &Ids::new());
    let mut domains = serde_json::Map::new();
    let mut mandatory = HashMap::new();
    let mut word_choices: HashMap<String, BTreeMap<Id, Value>> = HashMap::new();
    let language = index.vocabulary();
    let parentheses = crate::import_catalog::regex(r"\(([^()]*)\)");
    let government_hints = parentheses
        .captures_iter(s(&guide["title"]))
        .filter_map(|m| {
            language["government_names"][casefold(m[1].trim())]
                .as_str()
                .map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();
    let government = if government_hints.len() == 1 {
        government_hints.first().cloned()
    } else {
        None
    };
    let mut government_choices = json!({});
    let mut government_excluded = json!({});
    let articles = crate::import_catalog::regex(r"(?i)\b(?:a|an|the)\s+");
    for (key, row) in &rows {
        if row["action"]["kind"] != "select_focus" {
            continue;
        }
        let query = s(&row["action"]["entity"]["id"]);
        let country = &guide["applies_to"]["country"];
        let scope = scope(row, guide);
        let mut matches = index.matches(query, &["focus"], country, scope, true)?;
        let competing = if matches.is_empty() {
            let mut found = false;
            for name in [query, articles.replace_all(query, "").trim()] {
                if !index
                    .matches(name, &other_kinds, country, &Value::Null, false)?
                    .is_empty()
                {
                    found = true;
                    break;
                }
            }
            found
        } else {
            false
        };
        if matches.is_empty() && !competing {
            let mut variants = BTreeMap::new();
            let candidates=index.query(json!({"op":"focus_words","query":query,"kinds":["focus"],"country":country,"scope":scope}))?;
            for pair in arr(&candidates) {
                let entry =
                    &index.rows()[pair[0].as_u64().ok_or("Invalid focus word index")? as usize];
                if let Some(evidence) = word_variant(query, s(&pair[1]))
                    && !variants.contains_key(&id(entry))
                    && identity_graph.check(&id(entry)).0 == Some(true)
                {
                    matches.push(entry.clone());
                    variants.insert(id(entry), evidence);
                }
            }
            if !variants.is_empty() {
                word_choices.insert(key.clone(), variants);
            }
        }
        if matches.len() > 1
            && let Some(gov) = &government
        {
            let excluded = matches
                .iter()
                .filter(|e| arr(&e["excluded_governments"]).iter().any(|x| x == gov))
                .cloned()
                .collect::<Vec<_>>();
            matches.retain(|e| !excluded.contains(e));
            government_excluded[key]=json!(excluded.iter().map(|e|json!({"entity":e,"reason":"declared_guide_government_conflict","government":gov,"basis":"guide_title"})).collect::<Vec<_>>());
            if !excluded.is_empty() && matches.len() == 1 {
                government_choices[key] = json!({"government":gov,"basis":"guide_title"});
            }
        }
        domains.insert(key.clone(), json!(matches));
        mandatory.insert(
            key.clone(),
            !conditional(key, &rows, &phases, &mut HashSet::new()) && !optional(query),
        );
    }
    let mut anchors = domains
        .iter()
        .filter(|(k, v)| mandatory[*k] && arr(v).len() == 1)
        .map(|(_, v)| id(&v[0]))
        .collect::<Ids>();
    let mut recovery = json!({});
    let name_matches = domains
        .iter()
        .map(|(k, v)| (k.clone(), !arr(v).is_empty()))
        .collect::<HashMap<_, _>>();
    let mut suggested = HashMap::new();
    let route = loop {
        let route = Route::new(&nodes, &anchors);
        let mut extra = Ids::new();
        if !route.conflicts.is_empty() {
            break route;
        }
        let ctx = Context {
            guide,
            rows: &rows,
            mandatory: &mandatory,
            nodes: &nodes,
        };
        for key in domains.keys().cloned().collect::<Vec<_>>() {
            let matches = arr(&domains[&key]);
            if !mandatory[&key] || matches.len() < 2 {
                continue;
            }
            let viable = matches
                .iter()
                .filter(|e| route.check(&id(e)).0 != Some(false))
                .collect::<Vec<_>>();
            if viable.len() == 1 && route.check(&id(viable[0])).0 == Some(true) {
                extra.insert(id(viable[0]));
            } else if viable.len() > 1
                && let Some((candidate, evidence)) =
                    source_branch(&ctx, &key, matches, &domains, &anchors)
            {
                extra.insert(id(&candidate));
                domains[&key] = json!([candidate]);
                recovery[&key] = evidence;
            }
        }
        for key in domains.keys().cloned().collect::<Vec<_>>() {
            let query = s(&rows[&key]["action"]["entity"]["id"]);
            if !arr(&domains[&key]).is_empty()
                || !mandatory[&key]
                || query.contains('_')
                || normal_name(query).chars().count() < 10
            {
                continue;
            }
            if !suggested.contains_key(&key) {
                suggested.insert(
                    key.clone(),
                    suggestions(
                        index,
                        query,
                        &guide["applies_to"]["country"],
                        scope(&rows[&key], guide),
                        128,
                    )?,
                );
            }
            let proposed = &suggested[&key];
            if proposed.len() >= 128 {
                continue;
            }
            let plausible = proposed
                .iter()
                .filter(|item| {
                    item["suggestion"]["score"].as_f64().unwrap_or(0.) >= 0.8
                        && same_qualifiers(query, s(&item["suggestion"]["matched_label"]))
                        && route.check(&id(item)).0 != Some(false)
                })
                .collect::<Vec<_>>();
            if plausible.len() != 1 {
                continue;
            }
            let candidate = plausible[0];
            let identity = id(candidate);
            if !route.required.contains(&identity)
                || anchors.contains(&identity)
                || route.check(&identity).0 != Some(true)
            {
                continue;
            }
            let support = anchors
                .iter()
                .filter(|a| {
                    **a != identity
                        && route.check(a).0 == Some(true)
                        && route.guaranteed(a).contains(&identity)
                })
                .cloned()
                .collect::<Ids>();
            if support.is_empty() {
                continue;
            }
            domains[&key] = json!([nodes[&identity]]);
            recovery[&key] = json!({"method":"unique_name_and_required_ancestor","supporting_focus_ids":names(&support),"matched_label":candidate["suggestion"]["matched_label"]});
            extra.insert(identity);
        }
        extra = extra.difference(&anchors).cloned().collect();
        if extra.is_empty() {
            break route;
        }
        anchors.extend(extra);
    };
    let mut result = json!({});
    let mut errors = vec![];
    if !route.conflicts.is_empty() {
        errors.push(format!(
            "Percorso della guida incompatibile: {}",
            route
                .conflicts
                .iter()
                .map(|c| format!(
                    "{} / {}",
                    s(&c["entity"]["id"]),
                    arr(&c["conflicting_focus_ids"])
                        .iter()
                        .map(s)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    for (key, domain) in &domains {
        let matches = arr(domain);
        let mut excluded = vec![];
        let mut viable = vec![];
        let mut support = BTreeSet::new();
        let mut audit = matches.to_vec();
        if matches.len() == 1 {
            for sibling in index.matches(
                s(&matches[0]["name"]),
                &["focus"],
                &Value::Null,
                &matches[0]["scope"],
                false,
            )? {
                if id(&sibling) != id(&matches[0]) {
                    audit.push(sibling);
                }
            }
        }
        for entry in audit {
            let (status, witness) = if mandatory[key] && route.conflicts.is_empty() {
                route.check(&id(&entry))
            } else {
                (None, Ids::new())
            };
            if status == Some(false) {
                excluded.push(json!({"entity":entry,"reason":"route_mutual_exclusion","conflicting_focus_ids":names(&witness)}));
                support.extend(names(&witness));
            } else if matches.contains(&entry) {
                viable.push(entry);
            }
        }
        let mut selected = matches.to_vec();
        let mut unresolved = false;
        if matches.len() > 1 && mandatory[key] && route.conflicts.is_empty() {
            selected = viable.clone();
            if viable.len() == 1 && route.check(&id(&viable[0])).0.is_none() {
                selected.clear();
                unresolved = true;
            }
        }
        if word_choices.contains_key(key)
            && selected.len() == 1
            && route.check(&id(&selected[0])).0 != Some(true)
        {
            selected.clear();
            unresolved = true;
        }
        if let Some(r) = recovery.get(key) {
            support.extend(
                arr(&r["supporting_focus_ids"])
                    .iter()
                    .map(|v| s(v).to_owned()),
            );
        }
        let mut excluded_all = arr(&government_excluded[key]).to_vec();
        excluded_all.extend(excluded);
        let mut item = json!({"matches":selected,"candidate_domain":matches,"excluded_candidates":excluded_all,"supporting_focus_ids":support,"unresolved_route":unresolved,"has_name_matches":name_matches[key],"viable_candidates":viable,"inferred":recovery.get(key).is_some()||matches.len()>1&&selected.len()==1});
        if let Some(g) = government_choices.get(key) {
            item["government_context"] = g.clone();
        }
        if let Some(variants) = word_choices.get(key)
            && selected.len() == 1
        {
            item["word_variant"] = variants[&id(&selected[0])].clone();
        }
        if let Some(r) = recovery.get(key) {
            item["recovery"] = r.clone();
        }
        result[key] = item;
    }
    Ok((result, route.summary(), errors))
}
pub fn resolve(guide: &Value, index: &Index) -> Result<Value, String> {
    let source = focus_import::source_guide(guide)?;
    let (mut result, mut route, mut errors) = resolve_names(&source, index)?;
    let rows = source["tracks"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(arr)
        .filter(|r| r["action"]["kind"] == "select_focus")
        .map(|r| (s(&r["key"]).to_owned(), r.clone()))
        .collect::<serde_json::Map<_, _>>();
    for (key, row) in &rows {
        let query = s(&row["action"]["entity"]["id"]);
        let exact = index.matches(
            query,
            &["focus"],
            &source["applies_to"]["country"],
            scope(row, &source),
            false,
        )?;
        if source["focus_sequence"]["coverage"] == "complete"
            && exact.is_empty()
            && !truth(&row["focus_resolution"]["locked"])
        {
            let candidates = result[key]["candidate_domain"].as_array_mut().unwrap();
            for item in suggestions(
                index,
                query,
                &source["applies_to"]["country"],
                scope(row, &source),
                32,
            )? {
                if item["suggestion"]["score"].as_f64().unwrap_or(0.) >= 0.8
                    && same_qualifiers(query, s(&item["suggestion"]["matched_label"]))
                    && !candidates.iter().any(|x| id(x) == id(&item))
                {
                    candidates.push(item);
                }
            }
        }
    }
    let sequence_result = focus_import::reconstruct(&source, index, &result, 2_000_000)?;
    let forced = sequence_result["forced"].as_object().unwrap();
    let mut sequence = sequence_result["report"].clone();
    let changed = forced
        .iter()
        .filter(|(key, item)| {
            let d = arr(&result[*key]["candidate_domain"]);
            d.len() != 1 || id(&d[0]) != id(item)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<serde_json::Map<_, _>>();
    if !changed.is_empty() {
        let mut qualified = source.clone();
        for row in qualified["tracks"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .flat_map(|v| v.as_array_mut().into_iter().flatten())
        {
            if let Some(item) = forced.get(s(&row["key"])) {
                let mut entity = json!({});
                for field in ["kind", "id", "scope"] {
                    if let Some(v) = item.get(field) {
                        entity[field] = v.clone();
                    }
                }
                row["action"]["entity"] = entity;
            }
        }
        (result, route, errors) = resolve_names(&qualified, index)?;
        for (key, item) in &changed {
            result[key]["matches"] = json!([item]);
            result[key]["inferred"] = json!(true);
            result[key]["recovery"] = json!({"method":"unique_global_sequence","source_position":rows[key]["_focus_position"],"coverage":"complete","counterexample_found":false});
        }
        sequence["reconstructed_count"] = json!(changed.len());
        sequence["corrections"] = json!(
            changed
                .iter()
                .map(
                    |(k, v)| json!({"key":k,"query":rows[k]["action"]["entity"]["id"],"id":v["id"]})
                )
                .collect::<Vec<_>>()
        );
    }
    let probable = arr(&sequence["ambiguous_slots"])
        .iter()
        .map(|r| (s(&r["key"]).to_owned(), r["candidates"].clone()))
        .collect::<HashMap<_, _>>();
    let mut counts = BTreeMap::from([
        ("direct", 0),
        ("reconstructed", 0),
        ("probable", 0),
        ("unresolved", 0),
    ]);
    for (key, item) in result.as_object_mut().unwrap() {
        let row = &rows[key];
        let single = arr(&item["matches"]).len() == 1;
        let classification = if changed.contains_key(key)
            || item["recovery"]["method"] == "declared_source_branch"
        {
            "reconstructed"
        } else if single {
            "direct"
        } else if truth(&item["candidate_domain"]) || probable.get(key).is_some_and(truth) {
            "probable"
        } else {
            "unresolved"
        };
        if let Some(p) = probable.get(key)
            && !single
        {
            item["probable_candidates"] = p.clone();
        }
        item["classification"] = json!(classification);
        item["source_position"] = row["_focus_position"].clone();
        item["source_query"] = row["focus_resolution"]
            .get("query")
            .unwrap_or(&row["action"]["entity"]["id"])
            .clone();
        item["locked"] = row["focus_resolution"]
            .get("locked")
            .cloned()
            .unwrap_or(json!(false));
        *counts.get_mut(classification).unwrap() += 1;
    }
    sequence["classification_counts"] = json!(counts);
    sequence["reconstructed_count"] = json!(counts["reconstructed"]);
    route["sequence"] = sequence;
    for e in arr(&sequence_result["errors"]).iter().map(s) {
        if !errors.iter().any(|x| x == e) {
            errors.push(e.into());
        }
    }
    Ok(json!({"resolutions":result,"route":route,"errors":errors}))
}
