//! Bounded source-slot proofs, not simulation of game availability or effects.
use crate::import_catalog::Index;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
type Set = BTreeSet<usize>;
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
pub fn source_guide(guide: &Value) -> Result<Value, String> {
    let mut guide = guide.clone();
    let mut position = 0;
    for lane in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
    {
        let lane = lane.as_array_mut().ok_or("Invalid guide track")?;
        let focus_lane = lane.iter().any(|r| r["action"]["kind"] == "select_focus");
        for row in lane {
            if row["action"]["kind"] != "select_focus"
                && !(focus_lane && row["action"]["kind"] == "free")
            {
                continue;
            }
            position += 1;
            let old = row["focus_resolution"].clone();
            if truth(&old) && old["entity"] == row["action"]["entity"] && !truth(&old["locked"]) {
                row["action"]["entity"]["id"] = old["query"].clone();
            }
            row["_focus_position"] = old.get("position").cloned().unwrap_or(json!(position));
        }
    }
    Ok(guide)
}
pub fn conditionals(guide: &Value, index: &Index) -> Result<Value, String> {
    let pattern = crate::import_catalog::regex(
        r"(?i)^\s*([^/()]+?)\s*\(\s*bypass\s*\)\s*/\s*([^/()]+?)(?:\s*\(\s*if\s+([^()]+?)\s*\))?\s*$",
    );
    let mut guide = guide.clone();
    let country = guide["applies_to"]["country"].clone();
    let tree = guide["applies_to"]["tree_id"].clone();
    let id = s(&guide["id"]).to_owned();
    let mut result = json!({});
    for row in guide["tracks"]
        .as_object_mut()
        .ok_or("Missing guide tracks")?
        .values_mut()
        .flat_map(|v| v.as_array_mut().into_iter().flatten())
    {
        if row["action"]["kind"] != "select_focus"
            || truth(&row["action"]["params"]["conditional_focus"])
        {
            continue;
        }
        let query = s(&row["action"]["entity"]["id"]).to_owned();
        let Some(first) = pattern.captures(&query) else {
            continue;
        };
        let mut parts = [first[1].trim().to_owned(), first[2].trim().to_owned()];
        let mut phrase = first.get(3).map(|m| m.as_str().trim().to_owned());
        let mut full = query.trim().to_owned();
        if phrase.is_none() {
            for field in ["note", "title"] {
                if let Some(m) = pattern.captures(s(&row[field]))
                    && m.get(3).is_some()
                    && [0, 1].iter().all(|i| {
                        hoi4companion_catalog_core::casefold(m[i + 1].trim())
                            == hoi4companion_catalog_core::casefold(&parts[*i])
                    })
                {
                    parts = [m[1].trim().to_owned(), m[2].trim().to_owned()];
                    phrase = Some(m[3].trim().to_owned());
                    full = m[0].trim().to_owned();
                    break;
                }
            }
        }
        let Some(phrase) = phrase else {
            continue;
        };
        let scope = row["action"]["entity"]
            .get("scope")
            .filter(|v| truth(v))
            .unwrap_or(&tree);
        let left = index.matches(&parts[0], &["focus"], &country, scope, true)?;
        let right = index.matches(&parts[1], &["focus"], &country, scope, true)?;
        if left.len() != 1 || right.len() != 1 {
            continue;
        }
        let (a, b) = (&left[0], &right[0]);
        if a["id"] == b["id"]
            || a["scope"] != b["scope"]
            || truth(&a["continuous"])
            || truth(&b["continuous"])
        {
            continue;
        }
        let token = format!(
            "{:x}",
            Sha256::digest(format!("{id}\0{}\0{phrase}", s(&row["key"])).as_bytes())
        );
        let guard = json!({"op":"fact","key":format!("guide.conditions.{}",&token[..24]),"test":"eq","value":true});
        if row.get("when").is_none() {
            row["when"] = guard.clone();
        }
        if row.get("completion").is_none() {
            row["completion"] = json!({"op":"any","args":[{"op":"fact","key":"focus.completed","test":"contains","value":a["id"]},{"op":"fact","key":"focus.completed","test":"contains","value":b["id"]}]});
        }
        let metadata = json!({"bypass_id":a["id"],"fallback_id":b["id"],"condition_text":phrase,"source_query":query,"condition_basis":if row["when"]!=guard{"declared"}else{"source_text_unverifiable"}});
        if row["action"].get("params").is_none() {
            row["action"]["params"] = json!({});
        }
        row["action"]["params"]["conditional_focus"] = metadata.clone();
        row["action"]["entity"]["id"] = b["id"].clone();
        if truth(&b["scope"]) {
            row["action"]["entity"]["scope"] = b["scope"].clone();
        }
        row["target"] = b["id"].clone();
        if row.get("note").is_none() {
            row["note"] = json!(full);
        }
        result[s(&row["key"])] = metadata;
    }
    Ok(json!({"guide":guide,"resolutions":result}))
}
fn conditional(
    row: &Value,
    rows: &HashMap<String, &Value>,
    phases: &HashMap<String, &Value>,
    seen: &mut HashSet<String>,
) -> bool {
    let key = s(&row["key"]);
    if !seen.insert(key.into()) {
        return true;
    }
    let result = row.get("when").is_some()
        || row.get("skip_when").is_some()
        || truth(&row["optional"])
        || truth(&row["action"]["params"]["focus_optional"])
        || phases
            .get(s(&row["phase"]))
            .is_some_and(|p| p.get("when").is_some())
        || ["depends_on", "depends_on_started"]
            .iter()
            .flat_map(|f| arr(&row[*f]))
            .any(|k| {
                rows.get(s(k))
                    .is_none_or(|r| conditional(r, rows, phases, seen))
            });
    seen.remove(key);
    result
}
struct Search {
    parents: Vec<Vec<Set>>,
    opposites: Vec<Set>,
    repeat: Set,
    bypass: Set,
    initial: Set,
    used: u64,
    limit: u64,
}
impl Search {
    fn spend(&mut self) -> Result<(), ()> {
        self.used = self.used.saturating_add(1);
        if self.used > self.limit {
            Err(())
        } else {
            Ok(())
        }
    }
    fn propagate(&mut self, mut ds: Vec<Set>) -> Result<Option<Vec<Set>>, ()> {
        loop {
            let mut changed = false;
            if ds.iter().any(Set::is_empty) {
                return Ok(None);
            }
            let fixed = ds
                .iter()
                .filter(|d| d.len() == 1)
                .map(|d| *d.first().unwrap())
                .collect::<Vec<_>>();
            let unique = fixed
                .iter()
                .filter(|k| !self.repeat.contains(k))
                .copied()
                .collect::<Vec<_>>();
            let uniques = unique.iter().copied().collect::<Set>();
            if uniques.len() != unique.len() || !uniques.is_disjoint(&self.initial) {
                return Ok(None);
            }
            let all_fixed = fixed
                .iter()
                .copied()
                .chain(self.initial.iter().copied())
                .collect::<Set>();
            if all_fixed
                .iter()
                .any(|k| !self.opposites[*k].is_disjoint(&all_fixed))
            {
                return Ok(None);
            }
            let mut prefix = self.initial.clone();
            for domain in &mut ds {
                let mut keep = Set::new();
                for k in domain.iter().copied() {
                    self.spend()?;
                    if !self.repeat.contains(&k)
                        && (self.initial.contains(&k) || domain.len() > 1 && all_fixed.contains(&k))
                    {
                        continue;
                    }
                    if self.opposites[k]
                        .iter()
                        .any(|q| *q != k && all_fixed.contains(q))
                    {
                        continue;
                    }
                    if self.bypass.contains(&k)
                        || self.parents[k].iter().all(|g| {
                            g.iter()
                                .any(|p| prefix.contains(p) || self.bypass.contains(p))
                        })
                    {
                        keep.insert(k);
                    }
                }
                if keep.is_empty() {
                    return Ok(None);
                }
                if keep != *domain {
                    *domain = keep;
                    changed = true;
                }
                prefix.extend(domain.iter().copied());
            }
            for i in 0..ds.len() {
                if ds[i].len() != 1 {
                    continue;
                }
                let k = *ds[i].first().unwrap();
                if self.bypass.contains(&k) {
                    continue;
                }
                for group in &self.parents[k] {
                    if group
                        .iter()
                        .any(|p| self.initial.contains(p) || self.bypass.contains(p))
                    {
                        continue;
                    }
                    let positions = (0..i)
                        .filter(|j| !ds[*j].is_disjoint(group))
                        .collect::<Vec<_>>();
                    if positions.is_empty() {
                        return Ok(None);
                    }
                    if positions.len() == 1 {
                        let j = positions[0];
                        let keep = ds[j].intersection(group).copied().collect();
                        if keep != ds[j] {
                            ds[j] = keep;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                return Ok(Some(ds));
            }
        }
    }
    fn find(&mut self, ds: Vec<Set>, depth: usize) -> Result<Option<Vec<usize>>, ()> {
        self.spend()?;
        if depth > 512 {
            return Err(());
        }
        let Some(ds) = self.propagate(ds)? else {
            return Ok(None);
        };
        let variable = (0..ds.len())
            .filter(|i| ds[*i].len() > 1)
            .min_by_key(|i| (ds[*i].len(), *i));
        let Some(i) = variable else {
            return Ok(Some(ds.iter().map(|d| *d.first().unwrap()).collect()));
        };
        for k in &ds[i] {
            let mut branch = ds.clone();
            branch[i] = Set::from([*k]);
            if let Some(solution) = self.find(branch, depth + 1)? {
                return Ok(Some(solution));
            }
        }
        Ok(None)
    }
}
fn visit(k: usize, parents: &[Vec<Set>], visiting: &mut Set, done: &mut Set, depth: usize) -> bool {
    if visiting.contains(&k) || depth > 512 {
        return false;
    }
    if done.contains(&k) {
        return true;
    }
    visiting.insert(k);
    for p in parents[k].iter().flatten() {
        if !visit(*p, parents, visiting, done, depth + 1) {
            return false;
        }
    }
    visiting.remove(&k);
    done.insert(k);
    true
}
pub fn reconstruct(
    guide: &Value,
    index: &Index,
    named: &Value,
    limit: u64,
) -> Result<Value, String> {
    let tracks = guide["tracks"].as_object().ok_or("Missing guide tracks")?;
    let lanes = tracks
        .values()
        .filter(|v| arr(v).iter().any(|r| r["action"]["kind"] == "select_focus"))
        .collect::<Vec<_>>();
    let rows = lanes
        .iter()
        .flat_map(|l| arr(l))
        .filter(|r| r["action"]["kind"] == "select_focus" || r["action"]["kind"] == "free")
        .collect::<Vec<_>>();
    let policy = guide
        .get("focus_sequence")
        .cloned()
        .unwrap_or(json!({"coverage":"partial"}));
    let mut report = json!({"schema_version":1,"coverage":policy["coverage"],"status":"partial","slot_count":rows.len(),"reconstructed_count":0,"corrections":[],"limitations":["availability_and_effects_not_evaluated"],"work":0});
    macro_rules! end{()=>{return Ok(json!({"forced":{},"report":report,"errors":[]}))};($status:expr,$lim:expr)=>{{report["status"]=json!($status);report["limitations"]=json!([$lim]);end!();}}}
    if policy["coverage"] != "complete" || rows.is_empty() {
        end!();
    }
    if rows.len() > 256 {
        end!("limit", "sequence_slot_limit");
    }
    let phases = arr(&guide["phases"])
        .iter()
        .map(|p| (s(&p["id"]).to_owned(), p))
        .collect::<HashMap<_, _>>();
    let all_rows = tracks
        .values()
        .flat_map(arr)
        .map(|r| (s(&r["key"]).to_owned(), r))
        .collect::<HashMap<_, _>>();
    if lanes.len() != 1
        || lanes
            .iter()
            .any(|l| arr(l).iter().any(|r| r["action"]["kind"] == "research"))
    {
        end!("incomplete", "multiple_focus_tracks");
    }
    let free = rows
        .iter()
        .filter(|r| r["action"]["kind"] == "free")
        .map(|r| s(&r["key"]))
        .collect::<HashSet<_>>();
    let optional = rows
        .iter()
        .filter(|r| {
            free.contains(s(&r["key"])) || conditional(r, &all_rows, &phases, &mut HashSet::new())
        })
        .map(|r| s(&r["key"]))
        .collect::<HashSet<_>>();
    report["conditional_slot_count"] = json!(optional.len() - free.len());
    report["free_slot_count"] = json!(free.len());
    if !optional.is_empty() {
        report["limitations"]
            .as_array_mut()
            .unwrap()
            .push(json!("conditional_steps_overapproximated"));
    }
    let country = &guide["applies_to"]["country"];
    let mut scope = guide["applies_to"]["tree_id"].clone();
    if !truth(&scope) {
        let scopes = rows
            .iter()
            .flat_map(|r| arr(&named[s(&r["key"])]["candidate_domain"]))
            .filter_map(|x| x["scope"].as_str())
            .filter(|s| !s.is_empty())
            .collect::<BTreeSet<_>>();
        if scopes.len() == 1 {
            scope = json!(scopes.first());
        }
    }
    if !truth(&scope) {
        end!("incomplete", "focus_tree_unknown");
    }
    let entries =
        index.entries(json!({"op":"rows","kinds":["focus"],"country":country,"scope":scope}))?;
    let mut nodes = BTreeMap::new();
    for e in entries {
        nodes.insert(
            (
                e["scope"].as_str().map(str::to_owned),
                s(&e["id"]).to_owned(),
            ),
            e,
        );
    }
    if nodes.len() > 1200 {
        end!("limit", "sequence_catalog_limit");
    }
    let ids = nodes
        .keys()
        .enumerate()
        .map(|(i, k)| (k.clone(), i))
        .collect::<BTreeMap<_, _>>();
    let nodes = nodes.into_values().collect::<Vec<_>>();
    let skip = nodes.len();
    let mut initial = Set::new();
    for id in arr(&policy["initial_focuses"]) {
        let Some(k) = ids.get(&(scope.as_str().map(str::to_owned), s(id).to_owned())) else {
            report["status"] = json!("conflict");
            report["limitations"] = json!(["initial_focus_not_in_catalog"]);
            return Ok(
                json!({"forced":{},"report":report,"errors":["Focus sequence: an initial focus is absent from the selected tree"]}),
            );
        };
        initial.insert(*k);
    }
    let mut search = Search {
        parents: vec![vec![]; skip + 1],
        opposites: vec![Set::new(); skip + 1],
        repeat: Set::from([skip]),
        bypass: Set::new(),
        initial,
        used: 0,
        limit,
    };
    for (k, n) in nodes.iter().enumerate() {
        if n.get("prerequisites").is_none() || n.get("mutually_exclusive").is_none() {
            end!("incomplete", "missing_focus_relations");
        }
        for group in arr(&n["prerequisites"]) {
            let ps = arr(group)
                .iter()
                .filter_map(|p| ids.get(&(n["scope"].as_str().map(str::to_owned), s(p).to_owned())))
                .copied()
                .collect::<Set>();
            if ps.is_empty() || ps.len() != arr(group).iter().map(s).collect::<HashSet<_>>().len() {
                end!("incomplete", "missing_focus_relations");
            }
            search.parents[k].push(ps);
        }
        if truth(&n["continuous"]) || truth(&n["repeatable"]) {
            search.repeat.insert(k);
        }
        if n["bypass_possible"] != false || truth(&n["external_completion_possible"]) {
            search.bypass.insert(k);
        }
        if n["sequence_effects_safe"] == false {
            end!("incomplete", "focus_tree_reset_effects");
        }
        for other in arr(&n["mutually_exclusive"]) {
            if let Some(q) = ids.get(&(n["scope"].as_str().map(str::to_owned), s(other).to_owned()))
            {
                search.opposites[k].insert(*q);
                search.opposites[*q].insert(k);
            }
        }
    }
    let mut done = Set::new();
    for k in 0..skip {
        if !visit(k, &search.parents, &mut Set::new(), &mut done, 0) {
            end!("incomplete", "cyclic_focus_relations");
        }
    }
    let mut domains = vec![];
    for row in &rows {
        let choices = arr(&named[s(&row["key"])]["candidate_domain"]);
        let explicit = truth(&row["focus_resolution"]["locked"])
            || s(&row["action"]["entity"]["id"]).contains('_');
        let mut domain = if !choices.is_empty() {
            choices
                .iter()
                .filter_map(|x| {
                    ids.get(&(
                        x["scope"].as_str().map(str::to_owned),
                        s(&x["id"]).to_owned(),
                    ))
                })
                .copied()
                .collect::<Set>()
        } else if explicit {
            Set::new()
        } else {
            (0..skip).collect()
        };
        let alternatives = &row["action"]["params"]["conditional_focus"];
        if truth(alternatives) {
            for field in ["bypass_id", "fallback_id"] {
                if let Some(k) = ids.get(&(
                    scope.as_str().map(str::to_owned),
                    s(&alternatives[field]).to_owned(),
                )) {
                    domain.insert(*k);
                }
            }
        }
        if optional.contains(s(&row["key"])) {
            domain.insert(skip);
        }
        domains.push(domain);
    }
    let solution = match search.find(domains.clone(), 0) {
        Ok(s) => s,
        Err(()) => {
            report["status"] = json!("limit");
            report["work"] = json!(search.used);
            report["limitations"] = json!(["sequence_search_limit"]);
            end!();
        }
    };
    let Some(solution) = solution else {
        report["status"] = json!("conflict");
        report["work"] = json!(search.used);
        let mut issues = vec![];
        let mut prefix = search.initial.clone();
        for (i, (row, domain)) in rows.iter().zip(&domains).enumerate() {
            if domain.len() == 1 {
                let k = *domain.first().unwrap();
                if !search.bypass.contains(&k) {
                    for group in &search.parents[k] {
                        if !group
                            .iter()
                            .any(|p| prefix.contains(p) || search.bypass.contains(p))
                        {
                            let mut required = group
                                .iter()
                                .map(|p| s(&nodes[*p]["id"]))
                                .collect::<Vec<_>>();
                            required.sort();
                            issues.push(json!({"key":row["key"],"position":row.get("_focus_position").cloned().unwrap_or(json!(i+1)),"id":nodes[k]["id"],"reason":"missing_earlier_prerequisite","required_any_of":required}));
                        }
                    }
                }
            }
            prefix.extend(domain);
        }
        issues.truncate(12);
        let mut errors = issues
            .iter()
            .map(|x| {
                format!(
                    "Focus sequence: source position {} ({}) requires one of {} before this slot",
                    x["position"],
                    s(&x["id"]),
                    arr(&x["required_any_of"])
                        .iter()
                        .map(s)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>();
        report["conflicts"] = json!(issues);
        if errors.is_empty() {
            errors.push("Focus sequence has no compatible ordering: check names, coverage, initial focuses and source positions".into());
        }
        let mut repairs = vec![];
        for (i, row) in rows.iter().enumerate() {
            let q = s(&row["action"]["entity"]["id"]);
            if row["action"]["kind"] == "free"
                || q.contains('_')
                || truth(&row["focus_resolution"]["locked"])
                || domains[i].len() != 1
                || domains[i]
                    .iter()
                    .any(|k| *k != skip && q == s(&nodes[*k]["id"]))
            {
                continue;
            }
            let mut relaxed = domains.clone();
            relaxed[i] = (0..skip).filter(|k| !domains[i].contains(k)).collect();
            if optional.contains(s(&row["key"])) {
                relaxed[i].insert(skip);
            }
            let alternative = match search.find(relaxed, 0) {
                Ok(a) => a,
                Err(()) => {
                    report["limitations"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!("repair_search_limit"));
                    break;
                }
            };
            if let Some(a) = alternative
                && a[i] != skip
            {
                let item = &nodes[a[i]];
                let position = row.get("_focus_position").cloned().unwrap_or(json!(i + 1));
                repairs.push(json!({"key":row["key"],"position":position,"query":q,"entity":item,"classification":"probable","exhaustive":false}));
                errors.push(format!("Focus sequence: review source position {position} ({q}); compatible candidate {} ({}), not applied",s(&item["name"]),s(&item["id"])));
            }
            if repairs.len() == 8 {
                break;
            }
        }
        report["probable_repairs"] = json!(repairs);
        report["work"] = json!(search.used);
        return Ok(json!({"forced":{},"report":report,"errors":errors}));
    };
    let mut forced = json!({});
    let mut witnesses = solution.iter().map(|k| Set::from([*k])).collect::<Vec<_>>();
    for (i, row) in rows.iter().enumerate() {
        let mut other = domains.clone();
        other[i].remove(&solution[i]);
        let alternative = if other[i].is_empty() {
            None
        } else {
            match search.find(other, 0) {
                Ok(a) => a,
                Err(()) => {
                    report["status"] = json!("limit");
                    report["work"] = json!(search.used);
                    report["limitations"] = json!(["sequence_search_limit"]);
                    end!();
                }
            }
        };
        if alternative.is_none()
            && solution[i] != skip
            && row["action"]["kind"] == "select_focus"
            && !truth(&row["action"]["params"]["conditional_focus"])
        {
            forced[s(&row["key"])] = nodes[solution[i]].clone();
        } else if let Some(a) = alternative {
            for (j, k) in a.iter().enumerate() {
                witnesses[j].insert(*k);
            }
        }
    }
    report["status"] = json!("validated");
    report["forced_count"] = json!(forced.as_object().unwrap().len());
    report["work"] = json!(search.used);
    report["bypass_uncertainty_count"] = json!(search.bypass.len());
    report["external_completion_count"] = json!(
        nodes
            .iter()
            .filter(|n| truth(&n["external_completion_possible"]))
            .count()
    );
    report["ambiguous_slots"]=json!(rows.iter().enumerate().filter(|(_,r)|forced.get(s(&r["key"])).is_none()&&r["action"]["kind"]=="select_focus").map(|(i,r)|json!({"key":r["key"],"candidates":witnesses[i].iter().filter(|k|**k!=skip).take(12).map(|k|nodes[*k].clone()).collect::<Vec<_>>(),"exhaustive":false})).collect::<Vec<_>>());
    Ok(json!({"forced":forced,"report":report,"errors":[]}))
}
