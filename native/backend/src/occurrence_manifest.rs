//! Occurrence manifest (format 1): stable identity for every operation of one
//! immutable guide definition, with source provenance.
//!
//! Identity rules:
//! - `operation_id` = digest of (guide, compiler contract, all acquired source
//!   texts, step key, canonical step content). A positional key reused with a
//!   different meaning, source or compiler therefore gets a different ID, while
//!   re-preparing identical input yields identical IDs (preview/commit/reload).
//! - Source occurrences are half-open UTF-8 byte spans in the acquired text,
//!   matched only on whole lines. Repeated identical text is assigned in
//!   document order only when the number of matches equals the number of
//!   claimants; otherwise the span is `ambiguous`/`unlocated`, never guessed.
//! - Order lives in `order`, never inside an identity.
//!
//! The manifest is descriptive: it changes no evaluation or completion rule.
//! `shadow` proposes children for multiline reminder containers without
//! activating them; activation requires a new compiler contract.
use crate::definition_archive::{COMPILER_CONTRACT, sorted};
use crate::import_catalog::regex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
type Result<T> = std::result::Result<T, String>;

pub const MANIFEST_FORMAT: u64 = 1;
/// Compiler contract required before shadow children may become steps.
pub const DECOMPOSITION_COMPILER_CONTRACT: u64 = 2;

fn digest(parts: &[&[u8]]) -> String {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    format!("{:x}", hash.finalize())
}
fn canonical(v: &Value) -> Vec<u8> {
    serde_json::to_vec(&sorted(v)).unwrap_or_default()
}
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
/// Steps in track order (tracks sorted by name, then array order).
fn steps(definition: &Value) -> Vec<(String, usize, &Value)> {
    let mut tracks: Vec<_> = definition["tracks"].as_object().into_iter().flatten().collect();
    tracks.sort_by(|a, b| a.0.cmp(b.0));
    tracks
        .into_iter()
        .flat_map(|(track, lane)| {
            lane.as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .map(move |(i, step)| (track.clone(), i, step))
        })
        .collect()
}
/// Start offsets of `needle` in `hay` occupying whole lines only.
pub fn line_matches(hay: &str, needle: &str) -> Vec<usize> {
    if needle.is_empty() {
        return vec![];
    }
    let bytes = hay.as_bytes();
    hay.match_indices(needle)
        .map(|(i, _)| i)
        .filter(|&i| {
            (i == 0 || bytes[i - 1] == b'\n')
                && (i + needle.len() == hay.len() || matches!(bytes[i + needle.len()], b'\n' | b'\r'))
        })
        .collect()
}
/// Base Word key without promotion suffixes, and the direct parent key.
pub(crate) fn lineage(key: &str) -> (String, Option<String>, &'static str) {
    let base = regex(r"(?:--(?:operation|note|line|seq)-\d+)+$").replace(key, "").to_string();
    if base == key {
        return (base, None, "step");
    }
    let parent = regex(r"--(?:operation|note|line|seq)-\d+$").replace(key, "").to_string();
    let role = if regex(r"--note-\d+$").is_match(key) { "note" } else { "child" };
    (base, Some(parent), role)
}
pub(crate) fn instruction(step: &Value) -> Option<&str> {
    step["action"]["params"]["instructions"][0]
        .as_str()
        .or_else(|| step["action"]["instructions"][0].as_str())
        .filter(|s| !s.trim().is_empty())
}
/// Assign `claimants` (in order) to whole-line matches of the same text.
/// Unequal counts: nobody gets a span.
fn assign(matches: &[usize], claimants: usize) -> (Vec<Option<usize>>, &'static str) {
    match matches.len() {
        0 => (vec![None; claimants], "unlocated"),
        n if n == claimants => (matches.iter().map(|&m| Some(m)).collect(), "located"),
        _ => (vec![None; claimants], "ambiguous"),
    }
}

pub fn build(definition: &Value) -> Result<Value> {
    let guide = text(&definition["id"]);
    let sources: HashMap<&str, &str> = definition["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s["id"].as_str()?, s["text"].as_str()?)))
        .collect();
    let mut source_list: Vec<_> = sources
        .iter()
        .map(|(id, body)| json!({"source_id":id,"content_sha256":digest(&[body.as_bytes()])}))
        .collect();
    source_list.sort_by(|a, b| text(&a["source_id"]).cmp(text(&b["source_id"])));
    let scope = digest(&[&canonical(&json!(source_list))]);
    let contract = COMPILER_CONTRACT.to_string();
    let all = steps(definition);
    let mut seen_keys = HashSet::new();
    for (_, _, step) in &all {
        if !seen_keys.insert(text(&step["key"])) || text(&step["key"]).is_empty() {
            return Err("Guide definition has missing or duplicate step keys".into());
        }
    }

    // Blocks: one per base key whose evidence quotes a known source text.
    let mut groups: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (_, _, step) in &all {
        let (base, _, _) = lineage(text(&step["key"]));
        if groups.contains_key(&base) {
            continue;
        }
        if let Some(e) = step["evidence"].as_array().into_iter().flatten().find(|e| {
            sources.contains_key(text(&e["source_id"])) && !text(&e["quote"]).is_empty()
        }) {
            groups.insert(base, (text(&e["source_id"]).into(), text(&e["quote"]).into()));
        }
    }
    // Same quote in the same source: assign by Word ordinal order.
    let mut by_quote: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for (base, (source, quote)) in &groups {
        by_quote.entry((source.clone(), quote.clone())).or_default().push(base.clone());
    }
    let mut blocks = Map::new();
    let mut block_of: HashMap<String, (String, Option<usize>, String)> = HashMap::new();
    for ((source, quote), mut bases) in by_quote {
        bases.sort_by_key(|b| ordinal(b));
        let (spans, status) = assign(&line_matches(sources[source.as_str()], &quote), bases.len());
        for (n, (base, start)) in bases.iter().zip(spans).enumerate() {
            let id = format!(
                "blk1_{}",
                &digest(&[guide.as_bytes(), scope.as_bytes(), base.as_bytes(), quote.as_bytes()])[..32]
            );
            blocks.insert(id.clone(), json!({"block_id":id,"source_id":source,"base_key":base,
                "quote":quote,"occurrence":n,"status":status,
                "start_utf8":start,"end_utf8":start.map(|s| s + quote.len())}));
            block_of.insert(base.clone(), (id, start, quote.clone()));
        }
    }

    // Child instruction lines inside their block, repeated lines in step order.
    let mut line_claims: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut child_span: HashMap<String, (Option<usize>, &'static str)> = HashMap::new();
    for (_, _, step) in &all {
        let key = text(&step["key"]);
        let (base, parent, _) = lineage(key);
        if parent.is_none() {
            continue;
        }
        // Contract-2 children record their 1-based physical line: locate the
        // preserved text inside exactly that line (bullets excluded).
        if let (Some(n), Some(line), Some((_, start, quote))) = (
            step["action"]["params"]["decomposition"]["line"].as_u64(),
            instruction(step),
            block_of.get(&base),
        ) {
            let mut offset = 0;
            let mut found = None;
            for (i, raw) in quote.split('\n').enumerate() {
                if i + 1 == n as usize {
                    found = raw.find(line).map(|p| offset + p);
                    break;
                }
                offset += raw.len() + 1;
            }
            let span = start.zip(found).map(|(a, b)| a + b);
            child_span.insert(key.to_owned(), (span, match (start, found) {
                (None, _) => "unlocated",
                (_, None) => "derived",
                _ => "located",
            }));
            continue;
        }
        if let (Some(line), Some(_)) = (instruction(step), block_of.get(&base)) {
            line_claims.entry((base, line.to_owned())).or_default().push(key.to_owned());
        }
    }
    for ((base, line), keys) in &line_claims {
        let (_, start, quote) = &block_of[base];
        let found = line_matches(quote, line);
        let (spans, status) = assign(&found, keys.len());
        let status = match (start, status) {
            (_, "unlocated") => "derived",
            (None, _) => "unlocated",
            (_, s) => s,
        };
        for (key, rel) in keys.iter().zip(spans) {
            child_span.insert(key.clone(), (start.zip(rel).map(|(a, b)| a + b), status));
        }
    }

    let mut operations = vec![];
    let mut order: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut repeats: HashMap<String, usize> = HashMap::new();
    for (track, index, step) in &all {
        let key = text(&step["key"]);
        let (base, parent, role) = lineage(key);
        let id = format!(
            "op1_{}",
            &digest(&[guide.as_bytes(), contract.as_bytes(), scope.as_bytes(), key.as_bytes(), &canonical(step)])[..32]
        );
        let block = block_of.get(&base);
        let line = instruction(step);
        let (start, status, quote) = match (role, block) {
            (_, None) => (None, "unlocated", line.map(str::to_owned)),
            ("step", Some((_, start, quote))) => {
                (*start, if start.is_some() { "located" } else { "unlocated" }, Some(quote.clone()))
            }
            (_, Some(_)) => {
                let (s, status) = child_span.get(key).copied().unwrap_or((None, "derived"));
                (s, status, line.map(str::to_owned))
            }
        };
        // nth operation quoting identical text: distinguishes repeats in reports.
        let occurrence = quote.as_ref().map(|q| {
            let n = repeats.entry(q.clone()).or_default();
            *n += 1;
            *n - 1
        });
        let end = match (start, &quote) {
            (Some(s), Some(q)) if status == "located" => Some(s + q.len()),
            _ => None,
        };
        order.entry(track.clone()).or_default().push(json!(id));
        operations.push(json!({"operation_id":id,"step_key":key,"track":track,"index":index,
            "base_key":base,"parent_key":parent,"role":role,"block_id":block.map(|b| b.0.clone()),
            "quote":quote,"occurrence":occurrence,"span_status":status,
            "start_utf8":if end.is_some() {json!(start)} else {Value::Null},"end_utf8":end}));
    }
    Ok(json!({"manifest_format":MANIFEST_FORMAT,"compiler_contract":COMPILER_CONTRACT,
        "guide_id":guide,"source_scope_sha256":scope,"sources":source_list,
        "blocks":blocks.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
        "operations":operations,"order":order}))
}
fn ordinal(base: &str) -> (u64, String) {
    let n = regex(r"(\d+)$").captures(base).and_then(|c| c[1].parse().ok()).unwrap_or(u64::MAX);
    (n, base.to_owned())
}

/// Structural verification of a stored manifest against its definition.
/// Independent of the builder version: IDs are opaque and never recomputed.
pub fn validate(manifest: &Value, definition: &Value) -> Result<()> {
    let fail = |m: &str| Err(format!("Occurrence manifest invalid: {m}"));
    if manifest["manifest_format"].as_u64() != Some(MANIFEST_FORMAT) || manifest["guide_id"] != definition["id"] {
        return fail("format or guide");
    }
    let sources: HashMap<&str, &str> = definition["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s["id"].as_str()?, s["text"].as_str()?)))
        .collect();
    let keys: HashSet<&str> = steps(definition).iter().map(|(_, _, s)| text(&s["key"])).collect();
    let blocks: HashMap<&str, &Value> = manifest["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|b| (text(&b["block_id"]), b))
        .collect();
    let slice = |source: &str, start: &Value, end: &Value| -> Option<String> {
        let (s, e) = (start.as_u64()? as usize, end.as_u64()? as usize);
        sources.get(source)?.get(s..e).map(str::to_owned)
    };
    for block in blocks.values() {
        if block["status"] == "located"
            && slice(text(&block["source_id"]), &block["start_utf8"], &block["end_utf8"]).as_deref()
                != block["quote"].as_str()
        {
            return fail("block span");
        }
    }
    let mut ids = HashSet::new();
    let mut covered = HashSet::new();
    for op in manifest["operations"].as_array().into_iter().flatten() {
        let id = text(&op["operation_id"]);
        if !id.starts_with("op1_") || !ids.insert(id) {
            return fail("operation identity");
        }
        if !keys.contains(text(&op["step_key"])) || !covered.insert(text(&op["step_key"])) {
            return fail("operation step");
        }
        if op["span_status"] == "located" {
            let block = op["block_id"].as_str().and_then(|b| blocks.get(b)).ok_or("Occurrence manifest invalid: block")?;
            if slice(text(&block["source_id"]), &op["start_utf8"], &op["end_utf8"]).as_deref() != op["quote"].as_str() {
                return fail("operation span");
            }
        }
    }
    let ordered: Vec<&str> = manifest["order"].as_object().into_iter().flatten()
        .flat_map(|(_, v)| v.as_array().into_iter().flatten()).filter_map(Value::as_str).collect();
    if covered.len() != keys.len() || ordered.len() != ids.len() || !ordered.iter().all(|i| ids.contains(i)) {
        return fail("coverage");
    }
    Ok(())
}

/// Multiline manual reminder (the benchmark's "opaque multiline" container).
pub fn is_container(step: &Value) -> bool {
    step["tracking"] == "reminder"
        && step["action"]["kind"] == "manual"
        && step["action"]["params"]["instructions"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| text(t).lines().filter(|l| !l.trim().is_empty()).count() > 1)
}

/// Inactive decomposition proposals for multiline containers. Each nonempty
/// line becomes a candidate child with its own identity and exact span; a line
/// ending in ':' is kept as a raw scope header (condition unknown) that
/// applies to the following lines until the next header. Nothing here is
/// interpreted, verified or made completable.
pub fn shadow(definition: &Value, manifest: &Value) -> Result<Value> {
    let ops: HashMap<&str, &Value> = manifest["operations"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|o| (text(&o["step_key"]), o))
        .collect();
    let mut containers = vec![];
    for (_, _, step) in steps(definition) {
        if !is_container(step) {
            continue;
        }
        let op = ops.get(text(&step["key"])).ok_or("Container missing from manifest")?;
        let body = text(&step["action"]["params"]["instructions"][0]);
        let located = op["span_status"] == "located" && op["quote"].as_str() == Some(body);
        let base = op["start_utf8"].as_u64().map(|s| s as usize).filter(|_| located);
        let parent = text(&op["operation_id"]);
        let (mut children, mut scopes, mut current) = (vec![], vec![], None::<String>);
        let mut offset = 0;
        for (n, raw) in body.split('\n').enumerate() {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            let start = offset;
            offset += raw.len() + 1;
            if line.trim().is_empty() {
                continue;
            }
            let id = |prefix: &str| format!("{prefix}_{}", &digest(&[parent.as_bytes(), n.to_string().as_bytes(), line.as_bytes()])[..32]);
            let span = base.map(|b| json!({"start_utf8":b + start,"end_utf8":b + start + line.len()}));
            if line.trim_end().ends_with(':') {
                let scope = id("scp1");
                scopes.push(json!({"scope_id":scope,"line":n,"quote":line,"span":span,"condition":null,"condition_status":"unknown"}));
                current = Some(scope);
                continue;
            }
            children.push(json!({"proposed_id":id("pc1"),"parent_operation_id":parent,"line":n,"quote":line,
                "span":span,"scope_ids":current.iter().collect::<Vec<_>>(),
                "interpretation":{"status":"unrecognized"},"verification":{"mode":"unavailable"}}));
        }
        containers.push(json!({"operation_id":parent,"step_key":step["key"],"spans_located":base.is_some(),
            "children":children,"scopes":scopes}));
    }
    Ok(json!({"active":false,"requires_compiler_contract":DECOMPOSITION_COMPILER_CONTRACT,
        "container_count":containers.len(),"containers":containers}))
}

/// Progress correspondence between two manifests. Only identical operation
/// IDs (same guide, compiler contract, sources, key and step content) are
/// `same`; everything else is unmapped. An old operation whose block now hosts
/// different operations is `split_unmapped`: a parent never completes children.
pub fn correspondence(old: &Value, new: &Value) -> Value {
    let new_ids: HashSet<&str> = new["operations"].as_array().into_iter().flatten()
        .filter_map(|o| o["operation_id"].as_str()).collect();
    let new_blocks: HashMap<(&str, &str), usize> = new["operations"].as_array().into_iter().flatten()
        .filter_map(|o| Some((o["base_key"].as_str()?, o["quote"].as_str().unwrap_or(""))))
        .fold(HashMap::new(), |mut m, k| { *m.entry(k).or_default() += 1; m });
    let mut result = Map::new();
    for op in old["operations"].as_array().into_iter().flatten() {
        let id = text(&op["operation_id"]);
        let status = if new_ids.contains(id) {
            "same"
        } else if new_blocks.keys().any(|(b, _)| *b == text(&op["base_key"]))
            && new["operations"].as_array().into_iter().flatten()
                .filter(|o| o["base_key"] == op["base_key"]).count() > 1
        {
            "split_unmapped"
        } else {
            "unmapped"
        };
        result.insert(text(&op["step_key"]).to_owned(), json!({"operation_id":id,"status":status}));
    }
    Value::Object(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../../tests/fixtures/guide-definition-identity-cases.json")).unwrap()
    }
    fn definition(source: &str, steps: Value) -> Value {
        json!({"id":"g","sources":[{"id":"src-a","type":"file","text":source}],"tracks":{"operations":steps}})
    }
    fn step(key: &str, quote: &str, instruction: &str) -> Value {
        json!({"key":key,"tracking":"reminder","evidence":[{"source_id":"src-a","quote":quote}],
            "action":{"kind":"manual","params":{"instructions":[instruction]}}})
    }

    #[test]
    fn repeated_unicode_lines_get_distinct_identities_and_exact_spans() {
        let f = fixture();
        let source = f["source"]["text"].as_str().unwrap();
        let block = source.trim_end_matches('\n');
        let line = f["children"][0]["quote"].as_str().unwrap();
        // Parent container plus two promoted children quoting identical text.
        let def = definition(source, json!([step("word-0001", block, block),
            step("word-0001--operation-2", block, line), step("word-0001--operation-3", block, line)]));
        let m = build(&def).unwrap();
        validate(&m, &def).unwrap();
        let ops = m["operations"].as_array().unwrap();
        assert_eq!(ops[0]["span_status"], "located");
        for (op, child) in ops[1..].iter().zip(f["children"].as_array().unwrap()) {
            assert_eq!(op["span_status"], "located");
            assert_eq!((op["start_utf8"].clone(), op["end_utf8"].clone()), (child["start_utf8"].clone(), child["end_utf8"].clone()));
            assert_eq!(op["parent_key"], "word-0001");
        }
        assert_ne!(ops[1]["operation_id"], ops[2]["operation_id"]);
        assert_eq!((ops[1]["occurrence"].clone(), ops[2]["occurrence"].clone()), (json!(0), json!(1)));
        assert_eq!(build(&def).unwrap(), m, "deterministic");
        // Shadow: the header becomes an unknown scope over both repeated lines.
        let s = shadow(&def, &m).unwrap();
        assert_eq!(s["active"], false);
        let c = &s["containers"][0];
        assert_eq!(c["scopes"][0]["quote"], "When at war:");
        assert_eq!(c["scopes"][0]["condition_status"], "unknown");
        let kids = c["children"].as_array().unwrap();
        assert_eq!(kids.len(), 2);
        assert_ne!(kids[0]["proposed_id"], kids[1]["proposed_id"]);
        assert_eq!(kids[1]["span"], json!({"start_utf8":36,"end_utf8":58}));
        assert_eq!(kids[0]["scope_ids"], json!([c["scopes"][0]["scope_id"]]));
    }

    #[test]
    fn reused_keys_with_new_meaning_or_source_get_new_identities() {
        let a = definition("Build A\nBuild B\n", json!([step("word-0001", "Build A", "Build A")]));
        let b = definition("Build A\nBuild B\n", json!([step("word-0001", "Build B", "Build B")]));
        let c = definition("Build A\nBuild C\n", json!([step("word-0001", "Build A", "Build A")]));
        let id = |d: &Value| build(d).unwrap()["operations"][0]["operation_id"].clone();
        assert_ne!(id(&a), id(&b), "same key, different instruction");
        assert_ne!(id(&a), id(&c), "same step, different acquired source");
        let (ma, mb) = (build(&a).unwrap(), build(&b).unwrap());
        assert_eq!(correspondence(&ma, &ma)["word-0001"]["status"], "same");
        assert_eq!(correspondence(&ma, &mb)["word-0001"]["status"], "unmapped");
    }

    #[test]
    fn unequal_repeat_counts_are_ambiguous_not_guessed() {
        let def = definition("X\nX\nX\n", json!([step("word-0001", "X", "X"), step("word-0002", "X", "X")]));
        let m = build(&def).unwrap();
        validate(&m, &def).unwrap();
        for op in m["operations"].as_array().unwrap() {
            assert_eq!(op["span_status"], "unlocated");
            assert!(op["start_utf8"].is_null());
        }
        assert!(m["blocks"].as_array().unwrap().iter().all(|b| b["status"] == "ambiguous"));
        let missing = definition("Y\n", json!([step("word-0001", "X", "X")]));
        assert_eq!(build(&missing).unwrap()["blocks"][0]["status"], "unlocated");
    }

    #[test]
    fn decomposition_of_a_container_never_maps_parent_progress_to_children() {
        let block = "Do A\nDo B";
        let old = definition("Do A\nDo B\n", json!([step("word-0001", block, block)]));
        let new = definition("Do A\nDo B\n", json!([step("word-0001", block, "Do A"), step("word-0001--operation-2", block, "Do B")]));
        let map = correspondence(&build(&old).unwrap(), &build(&new).unwrap());
        assert_eq!(map["word-0001"]["status"], "split_unmapped");
    }

    #[test]
    fn tampered_manifests_fail_validation() {
        let def = definition("Do A\nDo B\n", json!([step("word-0001", "Do A", "Do A"), step("word-0002", "Do B", "Do B")]));
        let good = build(&def).unwrap();
        let mut span = good.clone();
        span["operations"][0]["end_utf8"] = json!(3);
        let mut dup = good.clone();
        dup["operations"][1]["operation_id"] = good["operations"][0]["operation_id"].clone();
        let mut missing = good.clone();
        missing["operations"].as_array_mut().unwrap().pop();
        for bad in [span, dup, missing] {
            assert!(validate(&bad, &def).is_err());
        }
        let mut dup_keys = def.clone();
        dup_keys["tracks"]["operations"][1]["key"] = json!("word-0001");
        assert!(build(&dup_keys).is_err());
    }
}
