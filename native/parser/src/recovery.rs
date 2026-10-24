//! Reviewed, content-pinned adaptations. Unknown malformed sources stay errors.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

#[derive(Clone, Deserialize, Serialize)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub replacement: String,
    pub kind: String,
}

#[derive(Deserialize)]
struct Entry {
    file: String,
    decoded_sha256: String,
    source_sha256: String,
    edits: Vec<Edit>,
    expected_ast_sha256: String,
    original_error: String,
}

#[derive(Deserialize)]
struct Manifest {
    schema_version: u32,
    entries: Vec<Entry>,
}

#[derive(Serialize)]
pub struct Evidence {
    pub policy: &'static str,
    pub reviewed_file: String,
    pub decoded_sha256: String,
    pub reviewed_source_sha256: String,
    pub edits: Vec<Edit>,
    pub original_error: String,
    pub original_lines: usize,
    pub omitted_properties: bool,
    pub status: &'static str,
}

pub fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn adapt(source: &str, entry: &Entry) -> Result<String, String> {
    let mut output = String::with_capacity(source.len() + 10);
    let mut previous = 0;
    for edit in &entry.edits {
        if edit.start < previous || edit.start > edit.end
            || source.get(edit.start..edit.end) != Some(edit.original.as_str())
        {
            return Err("Invalid reviewed recovery span".into());
        }
        // These operations preserve all original token positions. They may
        // close only an audited EOF, blank an excess final brace or omit a
        // specifically reviewed valueless property. No invented scalar values.
        let allowed = match edit.kind.as_str() {
            "close_reviewed_eof_block" => edit.start == source.len()
                && edit.end == source.len() && edit.replacement == "\n}",
            "remove_unmatched_final_close" => edit.original == "}"
                && edit.replacement == " " && source[edit.end..].trim().is_empty(),
            "omit_assignment_without_value" => edit.original.trim_end().ends_with('=')
                && edit.replacement.bytes().all(|b| b == b' ')
                && edit.replacement.len() == edit.original.len(),
            _ => false,
        };
        if !allowed { return Err("Unsupported reviewed recovery operation".into()); }
        output.push_str(&source[previous..edit.start]);
        output.push_str(&edit.replacement);
        previous = edit.end;
    }
    output.push_str(&source[previous..]);
    let node = crate::parse(output.as_bytes())?;
    let canonical = serde_json::to_vec(&serde_json::to_value(node).map_err(|e|e.to_string())?)
        .map_err(|e|e.to_string())?;
    if digest(&canonical) != entry.expected_ast_sha256 {
        return Err("Reviewed recovery AST fingerprint differs".into());
    }
    Ok(output)
}

pub fn recover(source: &str) -> Result<Option<(String, Evidence)>, String> {
    static MANIFEST: OnceLock<Result<Manifest,String>> = OnceLock::new();
    let manifest = MANIFEST.get_or_init(|| {
        let manifest: Manifest=serde_json::from_str(include_str!("source-recoveries.json"))
            .map_err(|e|e.to_string())?;
        if manifest.schema_version != 1 {return Err("Unsupported recovery manifest".into());}
        Ok(manifest)
    }).as_ref().map_err(Clone::clone)?;
    let hash=digest(source.as_bytes());
    let Some(entry)=manifest.entries.iter().find(|e| e.decoded_sha256==hash) else {return Ok(None);};
    // Do not silently transform a source accepted by the strict parser.
    if crate::parse(source.as_bytes()).is_ok() {return Err("Reviewed source no longer needs recovery".into());}
    let adapted=adapt(source,entry)?;
    Ok(Some((adapted, Evidence {
        policy:"reviewed-content-only-v1",reviewed_file:entry.file.clone(),decoded_sha256:hash,
        reviewed_source_sha256:entry.source_sha256.clone(),edits:entry.edits.clone(),
        original_error:entry.original_error.clone(),original_lines:source.lines().count().max(1),
        omitted_properties:entry.edits.iter().any(|e|e.kind=="omit_assignment_without_value"),
        status:"recovered_static_definition",
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(source:&str,replacement:&str)->Entry {
        let candidate=format!("{source}{replacement}");
        let node=crate::parse(candidate.as_bytes()).unwrap();
        Entry {file:"fixture.txt".into(),decoded_sha256:digest(source.as_bytes()),source_sha256:String::new(),
            original_error:"unclosed block".into(),expected_ast_sha256:digest(&serde_json::to_vec(&serde_json::to_value(node).unwrap()).unwrap()),
            edits:vec![Edit {start:source.len(),end:source.len(),original:String::new(),replacement:replacement.into(),kind:"close_reviewed_eof_block".into()}]}
    }
    #[test]
    fn recovery_preserves_duplicates_operators_and_scalar_values() {
        let source="ideas={country={A={modifier={x>=001 x=002} name=\"Città\"}} # EOF";
        let entry=fixture(source,"\n}");
        let candidate=adapt(source,&entry).unwrap();
        assert!(candidate.starts_with(source));
        assert!(candidate.contains("x>=001 x=002"));
        assert!(recover("unreviewed={x=1").unwrap().is_none());
    }
    #[test]
    fn incorrect_spans_changed_ast_and_invented_values_fail_closed() {
        let source="x={a=1";
        let mut entry=fixture(source,"\n}");
        entry.edits[0].replacement="a=99\n}".into();
        assert!(adapt(source,&entry).is_err());
        entry=fixture(source,"\n}"); entry.expected_ast_sha256="wrong".into();
        assert!(adapt(source,&entry).is_err());
        entry=fixture(source,"\n}");entry.edits[0].start=usize::MAX;
        assert!(adapt(source,&entry).is_err());
    }
}
