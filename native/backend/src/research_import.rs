//! Source-declared research lanes and choices; no campaign observations are invented.
use crate::import_catalog::{Index, language, regex};
use hoi4companion_catalog_core::{casefold, nfkd, normal_name, unicode_alnum, unicode_decimal};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
type Tokens = BTreeSet<String>;
type Techs = Map<String, Value>;
fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn string_list(v: &Value) -> Vec<String> {
    arr(v)
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
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
pub fn tokens(value: &str, vocabulary: &Value) -> Tokens {
    let source = vocabulary["research_families"][casefold(value.trim())]
        .as_str()
        .unwrap_or(value);
    let value = casefold(&nfkd(source));
    let mut words = value
        .split(|c| !unicode_alnum(c) || unicode_decimal(c))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect::<Tokens>();
    if words.remove("equip") {
        words.insert("equipment".into());
    }
    if words.remove("aa") {
        words.extend(["anti".into(), "air".into()]);
    }
    words
}
pub fn expanded(query: &str, vocabulary: &Value) -> String {
    let query = regex(r"([A-Za-z])(\d+)$")
        .replace(query, "$1 $2")
        .to_string();
    let Some(m) = regex(r"(?i)^\s*(.+?)(\s+(?:\d+|I|II|III|IV|V|VI))?\s*$").captures(&query) else {
        return query;
    };
    vocabulary["research_families"][casefold(&m[1])]
        .as_str()
        .map(|family| format!("{family}{}", m.get(2).map(|m| m.as_str()).unwrap_or("")))
        .unwrap_or(query)
}
fn named(query: &str, context: &str, vocabulary: &Value) -> Option<Value> {
    if query.contains('_') {
        return None;
    }
    let qualifier = regex(r"(?i)\(\s*(?:navy|naval)\s*\)");
    let naval = qualifier.is_match(&format!("{query} {context}"));
    let abbreviated = query.contains('…') || query.contains("...");
    let base = qualifier.replace_all(query, "");
    let base = regex(r"(?i)\s+(?:UP|UPGRADES)\s*$")
        .replace(base.trim(), "")
        .replace('…', "")
        .replace("...", "")
        .trim()
        .to_owned();
    vocabulary["named_research_progressions"]
        .as_object()?
        .values()
        .find(|d| {
            arr(&d["aliases"])
                .iter()
                .any(|v| normal_name(&base) == normal_name(s(v)))
                && (!truth(&d["requires_naval_qualifier_or_ellipsis"]) || naval || abbreviated)
        })
        .cloned()
}
pub(crate) fn chains(
    matches: &[String],
    by_id: &Techs,
    family_ids: Option<&BTreeSet<String>>,
) -> Option<Vec<String>> {
    let mut parents: HashMap<&str, Vec<&str>> = HashMap::new();
    for (key, item) in by_id {
        for target in arr(&item["upgrade_successors"]) {
            parents.entry(s(target)).or_default().push(key);
        }
    }
    let mut roots = BTreeSet::new();
    for start in matches {
        let mut key = start.as_str();
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(key) {
                return None;
            }
            let previous = parents
                .get(key)
                .into_iter()
                .flatten()
                .filter(|p| family_ids.is_none_or(|f| f.contains(**p)))
                .copied()
                .collect::<Vec<_>>();
            if previous.is_empty() {
                roots.insert(key.to_owned());
                break;
            }
            if previous.len() != 1 {
                return None;
            }
            key = previous[0];
        }
    }
    if roots.len() != 1 {
        return None;
    }
    let mut key = roots.first()?.to_owned();
    let mut chain = vec![];
    while let Some(item) = by_id.get(&key) {
        if chain.contains(&key) {
            return None;
        }
        chain.push(key.clone());
        let mut links = string_list(&item["upgrade_successors"]);
        if links.iter().any(|p| !by_id.contains_key(p)) {
            return None;
        }
        if let Some(family) = family_ids {
            links.retain(|p| family.contains(p));
        }
        if links.is_empty() {
            return Some(chain);
        }
        if links.len() != 1 {
            return None;
        }
        key = links.remove(0);
    }
    None
}
fn detached(
    matches: &[String],
    by_id: &Techs,
    family: &Tokens,
    vocabulary: &Value,
) -> Option<Vec<String>> {
    if !(2..=128).contains(&matches.len()) {
        return None;
    }
    let mut ranks = BTreeMap::new();
    let mut scheme = None;
    for key in matches {
        let item = &by_id[key];
        if truth(&item["upgrade_successors"]) {
            return None;
        }
        let name = s(&item["name"]).trim();
        let numbered = regex(r"(?i)^(.+?)\s+(\d+|VIII|VII|VI|IV|IX|III|II|I|V|X)$");
        let qualified = regex(r"(?i)^(Basic|Improved|Advanced)\s+(.+)$");
        let (stem, rank, kind) = if let Some(m) = numbered.captures(name) {
            let rank = m[2].parse::<usize>().ok().or_else(|| {
                ["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x"]
                    .iter()
                    .position(|v| *v == casefold(&m[2]))
                    .map(|i| i + 1)
            })?;
            (m[1].to_owned(), rank, "numbered")
        } else {
            let m = qualified.captures(name)?;
            (
                m[2].to_owned(),
                ["basic", "improved", "advanced"]
                    .iter()
                    .position(|v| *v == casefold(&m[1]))?
                    + 1,
                "qualified",
            )
        };
        if &tokens(&stem, vocabulary) != family
            || ranks.contains_key(&rank)
            || scheme.is_some_and(|old| old != kind)
        {
            return None;
        }
        ranks.insert(rank, key.clone());
        scheme = Some(kind);
    }
    if ranks.keys().copied().ne(1..=ranks.len()) {
        return None;
    }
    Some(ranks.into_values().collect())
}
pub fn relative_chain(query: &str, by_id: &Techs, vocabulary: &Value) -> Option<Vec<String>> {
    if let Some(named) = named(query, "", vocabulary) {
        let members = string_list(&named["members"]);
        return members
            .iter()
            .all(|key| by_id.contains_key(key))
            .then_some(members);
    }
    let family = tokens(query, vocabulary);
    if family.is_empty() {
        return None;
    }
    let matches = by_id
        .iter()
        .filter(|(key, item)| {
            std::iter::once(key.as_str())
                .chain(std::iter::once(s(&item["name"])))
                .chain(arr(&item["aliases"]).iter().map(s))
                .any(|label| family.is_subset(&tokens(label, vocabulary)))
        })
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    let levels = [
        "i", "ii", "iii", "iv", "v", "vi", "basic", "improved", "advanced", "upgrade",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect::<Tokens>();
    let canonical = matches
        .iter()
        .filter(|key| {
            tokens(s(&by_id[*key]["name"]), vocabulary)
                .difference(&levels)
                .cloned()
                .collect::<Tokens>()
                == family
        })
        .cloned()
        .collect::<Vec<_>>();
    let tight = if !canonical.is_empty() {
        canonical
    } else {
        matches
            .iter()
            .filter(|key| {
                arr(&by_id[*key]["aliases"]).iter().any(|label| {
                    tokens(s(label), vocabulary)
                        .difference(&levels)
                        .cloned()
                        .collect::<Tokens>()
                        == family
                })
            })
            .cloned()
            .collect()
    };
    let selected = if tight.is_empty() { &matches } else { &tight };
    let mut family_ids = matches.iter().cloned().collect::<BTreeSet<_>>();
    if family == Tokens::from(["machine".into(), "tools".into()]) {
        family_ids.insert("assembly_line_production".into());
    }
    chains(selected, by_id, Some(&family_ids))
        .or_else(|| detached(selected, by_id, &family, vocabulary))
}
fn first_useful(
    family: &Tokens,
    by_id: &Techs,
    context: &Value,
    vocabulary: &Value,
) -> (Option<String>, Option<String>, &'static str) {
    if !truth(context) {
        return (None, None, "upgrade_baseline_unknown");
    }
    let matches = by_id
        .iter()
        .filter(|(k, t)| family.is_subset(&tokens(&format!("{k} {}", s(&t["name"])), vocabulary)))
        .map(|(k, _)| k.clone())
        .collect::<Vec<_>>();
    let levels = [
        "i", "ii", "iii", "iv", "v", "vi", "basic", "improved", "advanced", "upgrade",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect::<Tokens>();
    let tight = matches
        .iter()
        .filter(|k| {
            tokens(s(&by_id[*k]["name"]), vocabulary)
                .difference(&levels)
                .cloned()
                .collect::<Tokens>()
                == *family
        })
        .cloned()
        .collect::<Vec<_>>();
    let matches = if tight.is_empty() { matches } else { tight };
    let mut parents: HashMap<String, Vec<String>> = HashMap::new();
    for (k, t) in by_id {
        for target in string_list(&t["upgrade_successors"]) {
            parents.entry(target).or_default().push(k.clone());
        }
    }
    let mut roots = BTreeSet::new();
    for mut key in matches {
        let mut seen = HashSet::new();
        while seen.insert(key.clone()) {
            let previous = parents.get(&key).cloned().unwrap_or_default();
            if previous.is_empty() {
                roots.insert(key);
                break;
            }
            if previous.len() != 1 {
                return (None, None, "upgrade_branch_ambiguous");
            }
            key = previous[0].clone();
        }
    }
    if roots.len() != 1 {
        return (
            None,
            None,
            if roots.is_empty() {
                "upgrade_baseline_unknown"
            } else {
                "upgrade_branch_ambiguous"
            },
        );
    }
    let mut key = roots.first().unwrap().clone();
    let mut seen = HashSet::new();
    let mut anchor = None;
    while let Some(item) = by_id.get(&key) {
        if !seen.insert(key.clone()) {
            break;
        }
        if arr(&context["unknown"]).iter().any(|x| x == &key) {
            return (None, anchor, "upgrade_baseline_unknown");
        }
        if !arr(&context["researched"]).iter().any(|x| x == &key) {
            return (Some(key), anchor, "upgrade_resolved");
        }
        let links = string_list(&item["upgrade_successors"]);
        if links.len() != 1 {
            return (
                None,
                Some(key),
                if links.is_empty() {
                    "upgrade_end_of_line"
                } else {
                    "upgrade_branch_ambiguous"
                },
            );
        }
        anchor = Some(key);
        key = links[0].clone();
    }
    (None, anchor, "upgrade_target_missing")
}
fn compile_choices(guide: &mut Value, by_id: &Techs, index: &Index) -> Result<(), String> {
    let vocabulary = index.vocabulary();
    let defs = vocabulary["research_branch_choices"]
        .as_object()
        .ok_or("Invalid research vocabulary")?;
    for lane in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
    {
        let mut declared = Map::new();
        for step in lane.as_array_mut().ok_or("Invalid guide track")? {
            if step["action"]["kind"] != "research" {
                continue;
            }
            let query = step["research_choice"]
                .get("query")
                .or_else(|| step["relative_research"].get("query"))
                .unwrap_or(&step["action"]["entity"]["id"]);
            let query = s(query).to_owned();
            if query.contains('_') {
                continue;
            }
            let re = regex(r"(?i)^\s*(.+?)(?:\s+(UP|UPGRADES|\d+|III|II|I|IV|V))?\s*$");
            let Some(m) = re.captures(&query) else {
                continue;
            };
            let stem = &m[1];
            let suffix = m.get(2).map(|v| v.as_str());
            let is_up = suffix.is_some_and(|s| ["up", "upgrades"].contains(&casefold(s).as_str()));
            let explicit = defs.iter().find(|(_, d)| {
                arr(&d["aliases"])
                    .iter()
                    .any(|x| normal_name(stem) == normal_name(s(x)))
            });
            let inherited = declared.iter().find(|(_, d)| {
                d["families"]
                    .as_object()
                    .into_iter()
                    .flat_map(|m| m.values())
                    .any(|x| normal_name(&expanded(stem, vocabulary)) == normal_name(s(x)))
            });
            let selected = explicit.or(inherited);
            let Some((key, data)) = selected else {
                if !query.contains('/') {
                    continue;
                }
                let parts = query.split('/').map(str::trim).collect::<Vec<_>>();
                if !(2..=8).contains(&parts.len()) || parts.iter().any(|p| p.is_empty()) {
                    continue;
                }
                let mut branches = Map::new();
                let mut failed = false;
                for (i, part) in parts.iter().enumerate() {
                    let up = regex(r"(?i)^(.+?)\s+(?:UP|UPGRADES)$");
                    let chain = if let Some(m) = up.captures(part) {
                        relative_chain(&m[1], by_id, vocabulary)
                    } else {
                        let matches = index.matches(
                            part,
                            &["technology"],
                            &Value::Null,
                            &Value::Null,
                            false,
                        )?;
                        if matches.len() == 1 {
                            Some(vec![s(&matches[0]["id"]).into()])
                        } else {
                            None
                        }
                    };
                    if let Some(chain) = chain {
                        branches.insert(format!("option_{}", i + 1), json!(chain));
                    } else {
                        failed = true;
                        break;
                    }
                }
                if failed {
                    continue;
                }
                let ids = branches.values().flat_map(arr).map(s).collect::<Vec<_>>();
                if ids.iter().copied().collect::<HashSet<_>>().len() != ids.len() {
                    continue;
                }
                let target = branches.values().next().unwrap()[0].clone();
                step["research_choice"] = json!({"group":format!("step:{}",s(&step["key"])),"query":query,"branches":branches,"mode":"once"});
                step.as_object_mut().unwrap().remove("relative_research");
                step.as_object_mut().unwrap().remove("research_upgrade");
                step["action"]["entity"]["id"] = target;
                continue;
            };
            let key = key.clone();
            let data = data.clone();
            let mut branches = Map::new();
            let mut failed = false;
            for (branch, family) in data["families"]
                .as_object()
                .ok_or("Invalid choice vocabulary")?
            {
                if let Some(chain) = relative_chain(s(family), by_id, vocabulary) {
                    branches.insert(branch.clone(), json!(chain));
                } else {
                    failed = true;
                    break;
                }
            }
            if failed {
                continue;
            }
            let mut choice = json!({"group":key,"query":query,"branches":branches});
            let mut level = 1;
            if let Some(suffix) = suffix
                && !is_up
            {
                level = suffix
                    .parse::<usize>()
                    .ok()
                    .or_else(|| {
                        ["i", "ii", "iii", "iv", "v"]
                            .iter()
                            .position(|x| *x == casefold(suffix))
                            .map(|i| i + 1)
                    })
                    .ok_or("Invalid research choice level")?;
                choice["level"] = json!(level);
                if branches
                    .values()
                    .any(|chain| level == 0 || level > arr(chain).len())
                {
                    continue;
                }
            }
            let target = branches.values().next().unwrap()[level - 1].clone();
            step["research_choice"] = choice;
            step.as_object_mut().unwrap().remove("relative_research");
            step.as_object_mut().unwrap().remove("research_upgrade");
            step["action"]["entity"]["id"] = target;
            declared.insert(key, data);
        }
    }
    Ok(())
}
struct History {
    key: String,
    target: Option<String>,
    chain: Option<Vec<String>>,
    tokens: Tokens,
    family: Tokens,
}
fn follows(start: &str, end: &str, by_id: &Techs) -> bool {
    let mut key = start;
    let mut seen = HashSet::new();
    while let Some(item) = by_id.get(key) {
        if !seen.insert(key) {
            break;
        }
        if key == end {
            return true;
        }
        let links = arr(&item["upgrade_successors"]);
        if links.len() != 1 {
            break;
        }
        key = s(&links[0]);
    }
    false
}
pub fn compile(guide: &Value, index: &Index, context: &Value) -> Result<Value, String> {
    let vocabulary = index.vocabulary();
    let mut guide = guide.clone();
    let mut by_id = Map::new();
    for item in index.entries(json!({"op":"rows","kinds":["technology"]}))? {
        by_id.insert(s(&item["id"]).into(), item);
    }
    compile_choices(&mut guide, &by_id, index)?;
    let country = &guide["applies_to"]["country"];
    if truth(context) && truth(country) && country != &context["country"] {
        return Err("Contesto iniziale ricerca di una nazione diversa dalla guida".into());
    }
    let baseline = guide.get("research_baseline").cloned().unwrap_or(json!([]));
    if !baseline.is_array()
        || arr(&baseline)
            .iter()
            .any(|key| key.as_str().is_none_or(|k| !by_id.contains_key(k)))
    {
        return Err("research_baseline deve contenere ID tecnologici presenti nel catalogo".into());
    }
    let has_baseline = guide.get("research_baseline").is_some();
    let mut resolutions = json!({});
    for lane in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
    {
        let mut history: Vec<History> = vec![];
        let mut families: BTreeMap<Tokens, Option<String>> = BTreeMap::new();
        for step in lane.as_array_mut().ok_or("Invalid guide track")? {
            if step["action"]["kind"] != "research" {
                continue;
            }
            let key = s(&step["key"]).to_owned();
            if truth(&step["research_choice"]) {
                crate::guide_schema::choice(step)?;
                if step["research_choice"]["branches"]
                    .as_object()
                    .unwrap()
                    .values()
                    .flat_map(arr)
                    .any(|x| !by_id.contains_key(s(x)))
                {
                    return Err(
                        "Un ramo della scelta di ricerca non è presente nel catalogo compatibile"
                            .into(),
                    );
                }
                resolutions[&key] = json!({"query":step["research_choice"]["query"],"interpretation":"branch_choice","reason":"upgrade_live","upgrade_from":null,"candidates":[]});
                continue;
            }
            let query = s(step["relative_research"]
                .get("query")
                .unwrap_or(&step["action"]["entity"]["id"]))
            .to_owned();
            let relative_re = regex(r"(?i)^\s*(.+?)\s+(?:UP|UPGRADES)\s*(?:\([^)]*\))?\s*$");
            let relative = relative_re.captures(&query);
            let params = step["action"]["params"].clone();
            let mut family_query = relative
                .as_ref()
                .map(|m| m[1].to_owned())
                .or_else(|| (params["interpretation"] == "next_upgrade").then_some(query.clone()));
            if let Some(family) = family_query.as_mut() {
                let unqualified = regex(r"(?i)^(?:basic|improved|advanced)\s+")
                    .replace(family, "")
                    .to_string();
                if unqualified != *family && expanded(&unqualified, vocabulary) != unqualified {
                    *family = expanded(&unqualified, vocabulary);
                }
            }
            let named = named(
                &query,
                &format!("{} {}", s(&step["title"]), s(&step["note"])),
                vocabulary,
            );
            if named.is_some() && family_query.is_none() {
                family_query = Some(query.clone());
            }
            if family_query.is_none()
                && !query.contains('_')
                && !regex(r"(?i)\b(?:\d+|i|ii|iii|iv|v)\s*$").is_match(&query)
            {
                let matches =
                    index.matches(&query, &["technology"], &Value::Null, &Value::Null, true)?;
                if matches.len() != 1 && relative_chain(&query, &by_id, vocabulary).is_some() {
                    family_query = Some(query.clone());
                }
            }
            if let Some(family_query) = family_query.filter(|s| !s.is_empty()) {
                let mut chain = if let Some(named) = &named {
                    let members = string_list(&named["members"]);
                    members
                        .iter()
                        .all(|k| by_id.contains_key(k))
                        .then_some(members)
                } else {
                    relative_chain(&family_query, &by_id, vocabulary)
                };
                if params.get("upgrade_from").is_some() && (chain.is_some() || named.is_none()) {
                    let explicit = params["upgrade_from"].as_str();
                    let previous = explicit.and_then(|e| history.iter().rev().find(|r| r.key == e));
                    let anchor = if let Some(previous) = previous {
                        previous.target.clone()
                    } else {
                        explicit
                            .filter(|e| by_id.contains_key(*e))
                            .map(str::to_owned)
                    };
                    let prior = previous.and_then(|r| r.chain.clone());
                    if let Some(prior) = prior {
                        if chain.as_ref().is_some_and(|c| c != &prior) {
                            chain = None;
                        } else {
                            chain = Some(prior);
                        }
                    } else if let Some(anchor) = anchor {
                        if chain.as_ref().is_some_and(|c| !c.contains(&anchor)) {
                            chain = None;
                        } else if chain.is_none() {
                            chain = chains(&[anchor], &by_id, None);
                        }
                    } else {
                        chain = None;
                    }
                } else if chain.is_none() && named.is_none() {
                    let family = tokens(&family_query, vocabulary);
                    if let Some(earlier) = history
                        .iter()
                        .rev()
                        .find(|r| family.is_subset(&r.tokens) && r.target.is_some())
                    {
                        chain = chains(&[earlier.target.clone().unwrap()], &by_id, None);
                    }
                }
                if let Some(chain) = &chain {
                    step["relative_research"] = json!({"query":query,"chain":chain});
                    step["action"]["entity"]["id"] = json!(chain[0]);
                    step.as_object_mut().unwrap().remove("research_upgrade");
                    resolutions[&key] = json!({"query":query,"interpretation":"next_upgrade","upgrade_from":null,"reason":"upgrade_live","candidates":[]});
                } else {
                    step.as_object_mut().unwrap().remove("relative_research");
                    resolutions[&key] = json!({"query":query,"interpretation":"next_upgrade","upgrade_from":null,"reason":"upgrade_branch_ambiguous","candidates":[]});
                }
                history.push(History {
                    key,
                    target: None,
                    chain,
                    tokens: tokens(&query, vocabulary),
                    family: tokens(&family_query, vocabulary),
                });
                continue;
            }
            let explicit = &params["upgrade_from"];
            let matched =
                index.matches(&query, &["technology"], &Value::Null, &Value::Null, true)?;
            let mut target = if matched.len() == 1 {
                Some(s(&matched[0]["id"]).to_owned())
            } else {
                None
            };
            let family = tokens(
                relative.as_ref().map(|m| &m[1]).unwrap_or(&query),
                vocabulary,
            );
            if truth(context)
                && relative.is_none()
                && target.as_deref() != Some(query.as_str())
                && (families.contains_key(&family)
                    || target.as_ref().is_some_and(|t| {
                        arr(&context["researched"]).iter().any(|x| x == t)
                            && normal_name(&query) != normal_name(s(&by_id[t]["name"]))
                    }))
            {
                target = None;
            }
            if let Some(target) = &target {
                for anchor in families.values_mut() {
                    if anchor
                        .as_ref()
                        .is_some_and(|a| follows(a, target, &by_id) || follows(target, a, &by_id))
                    {
                        *anchor = Some(target.clone());
                    }
                }
            }
            if target.is_none()
                && (relative.is_some() || params.get("upgrade_from").is_some() || truth(context))
            {
                let mut anchor = None;
                let mut use_start = false;
                if params.get("upgrade_from").is_some() {
                    if let Some(explicit) = explicit.as_str() {
                        anchor = if let Some(previous) =
                            history.iter().rev().find(|r| r.key == explicit)
                        {
                            previous.target.clone()
                        } else {
                            by_id.contains_key(explicit).then(|| explicit.to_owned())
                        };
                    }
                } else {
                    let previous = history
                        .iter()
                        .rev()
                        .filter(|r| family.is_subset(&r.tokens))
                        .collect::<Vec<_>>();
                    let direct = previous.iter().find(|r| r.family == family);
                    if let Some(known) = families.get(&family) {
                        anchor = known.clone();
                    } else if let Some(direct) = direct {
                        anchor = direct.target.clone();
                    } else if let Some(first) = previous.first() {
                        if let Some(proposed) = &first.target
                            && previous.iter().all(|r| {
                                r.target.as_ref().is_some_and(|t| {
                                    follows(proposed, t, &by_id) || follows(t, proposed, &by_id)
                                })
                            })
                        {
                            anchor = Some(proposed.clone());
                        }
                    } else {
                        let candidates = arr(&baseline)
                            .iter()
                            .filter(|k| {
                                family.is_subset(&tokens(
                                    &format!("{} {}", s(k), s(&by_id[s(k)]["name"])),
                                    vocabulary,
                                ))
                            })
                            .collect::<Vec<_>>();
                        if candidates.len() == 1 {
                            anchor = Some(s(candidates[0]).into());
                        } else if truth(context) && !has_baseline {
                            use_start = true;
                        }
                    }
                }
                let mut reason = "upgrade_baseline_unknown";
                let successors = anchor
                    .as_ref()
                    .and_then(|a| by_id.get(a))
                    .map(|a| string_list(&a["upgrade_successors"]))
                    .unwrap_or_default();
                if use_start {
                    (target, anchor, reason) = first_useful(&family, &by_id, context, vocabulary);
                } else if let Some(a) = &anchor {
                    reason = if successors.is_empty() {
                        "upgrade_end_of_line"
                    } else if successors.len() != 1 {
                        "upgrade_branch_ambiguous"
                    } else {
                        "upgrade_target_missing"
                    };
                    if successors.len() == 1 && by_id.contains_key(&successors[0]) {
                        target = Some(successors[0].clone());
                        if target.as_ref() == Some(a)
                            || arr(&baseline)
                                .iter()
                                .any(|x| x.as_str() == target.as_deref())
                            || history.iter().any(|r| r.target == target)
                        {
                            target = None;
                            reason = "upgrade_cycle_or_repeated_target";
                        }
                    }
                }
                resolutions[&key] = json!({"query":query,"interpretation":"next_upgrade","upgrade_from":anchor,"reason":if target.is_some(){"upgrade_resolved"}else{reason},"candidates":successors.iter().filter_map(|key|by_id.get(key)).collect::<Vec<_>>()});
                families.insert(family.clone(), target.clone());
                if let Some(target) = &target {
                    step["action"]["entity"]["id"] = json!(target);
                    step["research_upgrade"] = json!({"query":query,"from_id":anchor,"basis":"catalog_paths_and_guide_order"});
                    if use_start {
                        step["research_upgrade"]["basis"] =
                            json!("country_history_and_catalog_paths");
                        step["research_upgrade"]["country"] = context["country"].clone();
                        step["research_upgrade"]["date"] = context["date"].clone();
                    }
                }
            }
            let mut labels = format!("{query} {}", s(&step["research_upgrade"]["query"]));
            if let Some(title) = step["title"].as_str() {
                labels.push(' ');
                labels.push_str(title);
            }
            if let Some(target) = &target {
                labels.push_str(&format!(" {target} {}", s(&by_id[target]["name"])));
            }
            history.push(History {
                key,
                target,
                chain: None,
                tokens: tokens(&labels, vocabulary),
                family,
            });
        }
    }
    Ok(json!({"guide":guide,"resolutions":resolutions}))
}
pub fn chain_request(args: &Value) -> Result<Value, String> {
    let vocabulary = args.get("vocabulary").unwrap_or_else(|| language());
    let by_id = args["by_id"]
        .as_object()
        .ok_or("Missing technology definitions")?;
    Ok(json!(relative_chain(s(&args["query"]), by_id, vocabulary)))
}
