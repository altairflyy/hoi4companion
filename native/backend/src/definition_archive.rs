//! Immutable, content-addressed guide definition archives (format 1).
//!
//! An archive freezes exactly the definition the Rust evaluator used when a
//! run was created: the statically prepared AND presented guide, because
//! presentation also writes catalog-derived semantic fields (prerequisites,
//! continuous focus mode, facility buildings, operational criteria). A pinned
//! run is therefore evaluated without re-preparing, re-presenting or
//! re-resolving names against a newer catalog. Only the volatile runtime stamps
//! (`runtime_definition_revision`, `runtime_engine`) are excluded.
//!
//! Identity: `gd1_` + SHA-256 of the canonical envelope bytes (object keys
//! sorted, arrays/strings/numbers preserved). The envelope never contains its
//! own ID. Contracts are part of the hash: a definition compiled or evaluated
//! under different rules is a different definition.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;

pub const ARCHIVE_FORMAT: u64 = 1;
/// Static preparation/decomposition contract of NEW archives. Contract 2 adds
/// `decomposition::decompose` (multiline containers -> `--line-N` children) on
/// top of the contract-1 compile. Bump again whenever preparation can give an
/// existing source a different step structure or key meaning.
pub const COMPILER_CONTRACT: u64 = crate::decomposition::CONTRACT;
/// Contract under which legacy (never archived) runs are evaluable.
pub const LEGACY_COMPILER_CONTRACT: u64 = 1;
/// Contract of the in-memory registry (`Application::guides`): the plain
/// contract-1 compile, kept so legacy runs keep their original step keys.
pub const REGISTRY_COMPILER_CONTRACT: u64 = 1;
/// Evaluation rule contract supported by this `guide-core` build.
pub const EVALUATOR_CONTRACT: u64 = 1;
const SUPPORTED_COMPILER: &[u64] = &[1, 2, 3, 4];
const SUPPORTED_EVALUATOR: &[u64] = &[1];

pub(crate) fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), sorted(v)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        v => v.clone(),
    }
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Definition content as evaluated, without volatile runtime stamps.
fn semantic(definition: &Value) -> Result<Value> {
    let mut definition = definition.clone();
    let map = definition.as_object_mut().ok_or("Invalid guide definition")?;
    map.remove("runtime_definition_revision");
    map.remove("runtime_engine");
    Ok(definition)
}

/// A canonical archive envelope and its identity.
#[derive(Clone)]
pub struct Archive {
    pub id: String,
    pub payload: String,
}
/// Build the archive for the definition currently evaluated for `guide_id`.
/// `source` is the normalized acquired guide before associations/preparation;
/// its digest records provenance of the exact input that was compiled.
pub fn build(guide_id: &str, definition: &Value, source: &Value) -> Result<Archive> {
    let definition = semantic(definition)?;
    if definition["id"] != guide_id {
        return Err("Archived definition does not match its guide".into());
    }
    let source_sha256 =
        digest(&serde_json::to_vec(&sorted(source)).map_err(|e| e.to_string())?);
    let envelope = sorted(&json!({
        "archive_format": ARCHIVE_FORMAT,
        "guide_id": guide_id,
        "compiler_contract": COMPILER_CONTRACT,
        "evaluator_contract": EVALUATOR_CONTRACT,
        "source_sha256": source_sha256,
        // Stable operation identities and source occurrences, persisted once
        // with the definition (archives written before 2026-11-15 hold null).
        "occurrence_manifest": crate::occurrence_manifest::build(&definition)?,
        "definition": definition,
    }));
    let payload = serde_json::to_string(&envelope).map_err(|e| e.to_string())?;
    Ok(Archive {
        id: format!("gd1_{}", digest(payload.as_bytes())),
        payload,
    })
}
/// Verify a stored archive byte-for-byte against its ID and the contracts this
/// build can evaluate. Any mismatch fails closed; nothing is repaired.
pub fn open(id: &str, payload: &str) -> Result<Value> {
    if !hoi4companion_storage::definition_id(id) || format!("gd1_{}", digest(payload.as_bytes())) != id {
        return Err("Archived guide definition is corrupt".into());
    }
    let envelope: Value = serde_json::from_str(payload).map_err(|e| e.to_string())?;
    if serde_json::to_string(&sorted(&envelope)).map_err(|e| e.to_string())? != payload {
        return Err("Archived guide definition is not canonical".into());
    }
    if envelope["archive_format"].as_u64() != Some(ARCHIVE_FORMAT)
        || !envelope["definition"].is_object()
        || envelope["definition"]["id"] != envelope["guide_id"]
    {
        return Err("Unsupported guide definition archive".into());
    }
    let compiler = envelope["compiler_contract"].as_u64().unwrap_or(0);
    let evaluator = envelope["evaluator_contract"].as_u64().unwrap_or(0);
    if !SUPPORTED_COMPILER.contains(&compiler) || !SUPPORTED_EVALUATOR.contains(&evaluator) {
        return Err("Archived guide definition requires an unsupported contract: progress is unverifiable".into());
    }
    // A null manifest is an older archive: still evaluable, identities unavailable.
    if !envelope["occurrence_manifest"].is_null() {
        crate::occurrence_manifest::validate(&envelope["occurrence_manifest"], &envelope["definition"])?;
    }
    Ok(envelope)
}
/// Canonical bytes and identity of an envelope (verification/tools/tests).
pub fn seal(envelope: &Value) -> Result<Archive> {
    let payload = serde_json::to_string(&sorted(envelope)).map_err(|e| e.to_string())?;
    Ok(Archive { id: format!("gd1_{}", digest(payload.as_bytes())), payload })
}
pub fn pin(id: &str) -> Value {
    json!({"status":"pinned","id":id,"archive_format":ARCHIVE_FORMAT})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn guide(title: &str) -> Value {
        json!({"id":"g","title":title,"tracks":{"operations":[{"key":"k","title":"Ünïcode ✓"}]},
            "runtime_definition_revision":"volatile","runtime_engine":"rust-guide-core"})
    }
    #[test]
    fn identity_is_deterministic_canonical_and_content_sensitive() {
        let source = json!({"id":"g","b":1,"a":[2,1]});
        let a = build("g", &guide("A"), &source).unwrap();
        // Key order and runtime stamps never change identity.
        let mut reordered = json!({"tracks":guide("A")["tracks"],"title":"A","id":"g"});
        reordered["runtime_definition_revision"] = json!("other");
        let b = build("g", &reordered, &json!({"a":[2,1],"b":1,"id":"g"})).unwrap();
        assert_eq!((a.id.clone(), a.payload.clone()), (b.id, b.payload));
        assert_ne!(a.id, build("g", &guide("B"), &source).unwrap().id);
        assert_ne!(a.id, build("g", &guide("A"), &json!({"id":"g","a":[1,2],"b":1})).unwrap().id, "array order is meaning");
        let opened = open(&a.id, &a.payload).unwrap();
        assert_eq!(opened["definition"]["tracks"]["operations"][0]["title"], "Ünïcode ✓");
        assert!(opened["definition"].get("runtime_definition_revision").is_none());
        assert!(build("other", &guide("A"), &source).is_err());
    }
    #[test]
    fn corrupt_noncanonical_or_unsupported_archives_fail_closed() {
        let a = build("g", &guide("A"), &json!({})).unwrap();
        assert!(open(&a.id, &a.payload.replace("\"A\"", "\"Z\"")).is_err());
        let mut forged: Value = serde_json::from_str(&a.payload).unwrap();
        forged["evaluator_contract"] = json!(99);
        let text = serde_json::to_string(&sorted(&forged)).unwrap();
        assert!(open(&format!("gd1_{}", digest(text.as_bytes())), &text).unwrap_err().contains("unsupported contract"));
        let pretty = serde_json::to_string_pretty(&forged).unwrap();
        assert!(open(&format!("gd1_{}", digest(pretty.as_bytes())), &pretty).is_err());
    }
    #[test]
    fn archives_carry_a_valid_manifest_and_older_null_manifests_still_open() {
        let a = build("g", &guide("A"), &json!({})).unwrap();
        let envelope = open(&a.id, &a.payload).unwrap();
        assert_eq!(envelope["occurrence_manifest"]["operations"][0]["step_key"], "k");
        let mut older = envelope.clone();
        older["occurrence_manifest"] = Value::Null;
        let text = serde_json::to_string(&sorted(&older)).unwrap();
        assert!(open(&format!("gd1_{}", digest(text.as_bytes())), &text).is_ok());
        let mut tampered = envelope;
        tampered["occurrence_manifest"]["operations"] = json!([]);
        let text = serde_json::to_string(&sorted(&tampered)).unwrap();
        assert!(open(&format!("gd1_{}", digest(text.as_bytes())), &text).unwrap_err().contains("manifest"));
    }
}
