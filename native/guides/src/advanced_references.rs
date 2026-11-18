//! Authored reference material and native MIO queues. No campaign mutation,
//! arbitrary script execution, inferred rankings or automatic trait purchases.
use crate::{import_catalog::regex, script_document as script, word_sources};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, io::Cursor};
type Result<T> = std::result::Result<T, String>;
pub fn id(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn is_advanced(path: &str) -> bool {
    path.split('/')
        .any(|p| regex(r"(?i)^advanced guides\b").is_match(p))
        || path.to_lowercase().ends_with("/mio_queues.txt")
}
pub fn category(path: &str) -> &'static str {
    let p = path.to_lowercase();
    if p.contains("mio") {
        "mios"
    } else if p.contains("tier list") || p.contains("companies") {
        "companies"
    } else if p.contains("doctrine") || p.contains("spirit") {
        "doctrines"
    } else if p.contains("special project") {
        "projects"
    } else if p.contains("trait") || p.contains("commander") || p.contains("/hq") {
        "commanders"
    } else if p.contains("production") || p.contains("concentrated") || p.contains("dispersed") {
        "production"
    } else if p.contains("suppl") {
        "logistics"
    } else if p.contains("research") {
        "research"
    } else if p.contains("invade") {
        "strategy"
    } else if p.contains("stats") {
        "stats"
    } else {
        "general"
    }
}
pub fn mio_queues(raw: &[u8]) -> Result<Vec<Value>> {
    if raw.len() > 2_000_000 {
        return Err("MIO queue file exceeds 2 MB".into());
    }
    let node = serde_json::to_value(hoi4companion_parser::read_reviewed(raw)?)
        .map_err(|e| e.to_string())?;
    if script::scalar(&node, "version") != "0" {
        return Err("Unsupported MIO queue format version".into());
    }
    let mut rows = vec![];
    let mut seen = HashSet::new();
    for (org, group) in script::entries(&node).filter(|(k, _)| *k != "version") {
        if !regex(r"^[A-Za-z0-9_]{1,200}$").is_match(org) || !group["entries"].is_array() {
            return Err("Invalid MIO organization".into());
        }
        for (key, preset) in script::entries(group) {
            if !key.is_empty() || !preset["entries"].is_array() {
                return Err("Unknown MIO preset field".into());
            }
            if script::entries(preset).any(|(k, _)| !["name", "queued_trait"].contains(&k)) {
                return Err("Unknown MIO preset field".into());
            }
            let name = script::scalar(preset, "name");
            if name.is_empty() || name.chars().count() > 200 {
                return Err("Invalid MIO preset name".into());
            }
            let mut traits = vec![];
            for queue in script::blocks(preset, "queued_trait") {
                for (key, item) in script::entries(queue) {
                    if !key.is_empty() || script::entries(item).count() != 1 {
                        return Err("Invalid MIO trait entry".into());
                    }
                    let trait_id = script::scalar(item, "trait");
                    if !regex(r"^[A-Za-z0-9_]{1,200}$").is_match(trait_id)
                        || traits.iter().any(|v| v == trait_id)
                    {
                        return Err("Invalid or repeated MIO trait".into());
                    }
                    traits.push(trait_id.to_owned());
                }
            }
            if traits.is_empty() || traits.len() > 512 {
                return Err("Invalid MIO trait queue size".into());
            }
            let identity = id(serde_json::to_string(&(org, name, &traits))
                .unwrap()
                .as_bytes());
            if seen.insert(identity.clone()) {
                let country = org
                    .split('_')
                    .next()
                    .filter(|p| p.len() == 3 && p.bytes().all(|b| b.is_ascii_uppercase()));
                rows.push(json!({"id":identity,"organization_id":org,"name":name,"traits":traits,"country_hint":country,"evidence":"authored_preset","availability":"not_verified"}));
            }
        }
    }
    Ok(rows)
}
pub fn export_mios(rows: &[Value], existing: Option<&[u8]>) -> Result<Value> {
    let mut combined = if let Some(raw) = existing {
        mio_queues(raw)?
    } else {
        vec![]
    };
    let previous = combined.len();
    let mut added = 0;
    let mut renamed = 0;
    for row in rows {
        if combined
            .iter()
            .any(|r| r["organization_id"] == row["organization_id"] && r["traits"] == row["traits"])
        {
            continue;
        }
        let mut row = row.clone();
        if combined
            .iter()
            .any(|r| r["organization_id"] == row["organization_id"] && r["name"] == row["name"])
        {
            row["name"] = json!(format!(
                "{} [Companion {}]",
                row["name"].as_str().unwrap_or("MIO"),
                &id(row.to_string().as_bytes())[..8]
            ));
            renamed += 1;
        }
        combined.push(row);
        added += 1;
    }
    let mut text = String::from("version=0\n");
    let mut orgs = std::collections::BTreeMap::<&str, Vec<&Value>>::new();
    for row in &combined {
        orgs.entry(
            row["organization_id"]
                .as_str()
                .ok_or("Invalid MIO preset")?,
        )
        .or_default()
        .push(row)
    }
    for (org, rows) in orgs {
        text.push_str(&format!("{org}={{\n"));
        for row in rows {
            let name = serde_json::to_string(row["name"].as_str().ok_or("Invalid MIO name")?)
                .map_err(|e| e.to_string())?;
            text.push_str(&format!("  {{\n    name={name}\n    queued_trait={{\n"));
            for item in row["traits"].as_array().ok_or("Invalid MIO queue")? {
                text.push_str(&format!(
                    "      {{ trait={} }}\n",
                    serde_json::to_string(item.as_str().ok_or("Invalid MIO trait")?).unwrap()
                ))
            }
            text.push_str("    }\n  }\n");
        }
        text.push_str("}\n");
    }
    // Export only a document that the same native parser can read back.
    let verified = mio_queues(text.as_bytes())?;
    if verified.len() != combined.len() {
        return Err("MIO export roundtrip mismatch".into());
    }
    Ok(
        json!({"filename":"mio_queues.txt","text":text,"existing":previous,"added":added,"renamed":renamed,"count":verified.len(),"relative_path":"session_persistence/mio_queues.txt"}),
    )
}
pub fn export_from_documents(docs: Option<&str>, rows: &[Value]) -> Result<Value> {
    use std::io::Read;
    let mut existing = None;
    if let Some(docs) = docs.filter(|s| !s.is_empty()) {
        let root = std::path::Path::new(docs)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let path = root.join("session_persistence/mio_queues.txt");
        if path.exists() {
            let path = path.canonicalize().map_err(|e| e.to_string())?;
            if !path.starts_with(&root) {
                return Err("MIO preset path outside configured documents".into());
            }
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let before = file.metadata().map_err(|e| e.to_string())?;
            let mut raw = vec![];
            file.take(2_000_001)
                .read_to_end(&mut raw)
                .map_err(|e| e.to_string())?;
            let after = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if raw.len() > 2_000_000
                || before.len() != after.len()
                || before.modified().ok() != after.modified().ok()
            {
                return Err("MIO file changed or exceeds limit; retry after closing HOI4".into());
            }
            existing = Some(raw);
        }
    }
    let mut result = export_mios(rows, existing.as_deref())?;
    result["existing_file_observed"] = json!(existing.is_some());
    Ok(result)
}
pub fn document(name: &str, raw: &[u8]) -> Result<Value> {
    let (text, _, _) = word_sources::content(raw)?;
    let mut tiers = vec![];
    let mut active: Option<Value> = None;
    for line in text.lines() {
        if let Some(c) =
            regex(r"(?i)^([SABCDF])\s+Tier\s*(?:[–—:-]\s*)?(.*)$").captures(line.trim())
        {
            if let Some(v) = active.take() {
                tiers.push(v)
            }
            active = Some(json!({"tier":c[1].to_uppercase(),"title":line,"entries":[]}));
        } else if let Some(v) = active.as_mut() {
            if !line.starts_with('[') && !line.trim().is_empty() {
                v["entries"].as_array_mut().unwrap().push(json!(line))
            }
        }
    }
    if let Some(v) = active {
        tiers.push(v)
    }
    let title = std::path::Path::new(name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    Ok(
        json!({"id":id(raw),"name":title,"category":category(name),"kind":"document","text":text,"tiers":tiers,"images":[],"evidence":"authored_reference"}),
    )
}
pub fn embedded_images(raw: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut archive = word_sources::archive(Cursor::new(raw))?;
    let names = archive
        .file_names()
        .filter_map(|n| n.ok().map(|s| s.into_owned()))
        .filter(|n| n.starts_with("word/media/") && regex(r"(?i)\.(png|jpe?g)$").is_match(n))
        .collect::<Vec<_>>();
    let mut result = vec![];
    for name in names {
        result.push(word_sources::read_entry(&mut archive, &name, 16_000_000)?)
    }
    Ok(result)
}
fn xml(source: &str) -> Result<roxmltree::Document<'_>> {
    if source.to_uppercase().contains("<!DOCTYPE") || source.to_uppercase().contains("<!ENTITY") {
        return Err("Unsupported spreadsheet XML".into());
    }
    roxmltree::Document::parse_with_options(
        source,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 1_000_000,
        },
    )
    .map_err(|e| e.to_string())
}
pub fn workbook(name: &str, raw: &[u8]) -> Result<Value> {
    let mut archive = word_sources::archive(Cursor::new(raw))?;
    let mut shared = vec![];
    if archive
        .file_names()
        .any(|n| n.as_deref().unwrap_or("") == "xl/sharedStrings.xml")
    {
        let bytes = word_sources::read_entry(&mut archive, "xl/sharedStrings.xml", 8_000_000)?;
        let doc = xml(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)?;
        for si in doc.descendants().filter(|n| n.tag_name().name() == "si") {
            shared.push(
                si.descendants()
                    .filter(|n| n.tag_name().name() == "t")
                    .filter_map(|n| n.text())
                    .collect::<String>(),
            )
        }
    }
    let names = archive
        .file_names()
        .filter_map(|n| n.ok().map(|s| s.into_owned()))
        .filter(|n| regex(r"^xl/worksheets/sheet\d+\.xml$").is_match(n))
        .collect::<Vec<_>>();
    if names.len() > 32 {
        return Err("Too many spreadsheet sheets".into());
    }
    let mut sections = vec![];
    let mut remaining = 10000usize;
    for sheet in names {
        let bytes = word_sources::read_entry(&mut archive, &sheet, 8_000_000)?;
        let doc = xml(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)?;
        let mut lines = vec![];
        for row in doc.descendants().filter(|n| n.tag_name().name() == "row") {
            let mut cells = vec![];
            for c in row.children().filter(|n| n.tag_name().name() == "c") {
                remaining = remaining
                    .checked_sub(1)
                    .ok_or("Spreadsheet cell limit exceeded")?;
                let v = c
                    .children()
                    .find(|n| n.tag_name().name() == "v")
                    .and_then(|n| n.text())
                    .unwrap_or("");
                let value = if c.attribute("t") == Some("s") {
                    shared
                        .get(v.parse::<usize>().map_err(|_| "Invalid shared string")?)
                        .ok_or("Invalid shared string")?
                        .clone()
                } else if c.attribute("t") == Some("inlineStr") {
                    c.descendants()
                        .filter(|n| n.tag_name().name() == "t")
                        .filter_map(|n| n.text())
                        .collect()
                } else {
                    v.to_owned()
                };
                if !value.is_empty() {
                    cells.push(format!("{}: {value}", c.attribute("r").unwrap_or("?")))
                }
            }
            if !cells.is_empty() {
                lines.push(cells.join(" | "))
            }
        }
        sections.push(json!({"name":sheet,"text":lines.join("\n")}));
    }
    Ok(
        json!({"id":id(raw),"name":std::path::Path::new(name).file_stem().unwrap_or_default().to_string_lossy(),"category":category(name),"kind":"spreadsheet","sections":sections,"text":sections.iter().map(|s|s["text"].as_str().unwrap_or("")).collect::<Vec<_>>().join("\n"),"cached_values_only":true,"evidence":"authored_reference"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    const QUEUE:&[u8]=br#"version=0 FIN_test={ { name="Infantry" queued_trait={ {trait="first"} {trait="second"} } } } ENG_empty={}"#;
    #[test]
    fn mio_order_empty_organizations_and_safe_export() {
        let rows = mio_queues(QUEUE).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["traits"], json!(["first", "second"]));
        let result = export_mios(&rows, Some(QUEUE)).unwrap();
        assert_eq!(result["added"], 0);
        assert_eq!(
            mio_queues(result["text"].as_str().unwrap().as_bytes()).unwrap(),
            rows
        );
        let mut other = rows.clone();
        other[0]["traits"] = json!(["other"]);
        let exported = export_mios(&other, Some(QUEUE)).unwrap();
        assert_eq!(exported["renamed"], 1);
        assert_eq!(exported["count"], 2);
        assert_eq!(
            mio_queues(exported["text"].as_str().unwrap().as_bytes()).unwrap()[0]["traits"],
            rows[0]["traits"]
        );
    }
    #[test]
    fn mio_rejects_unknown_versions_duplicate_traits_and_unexpected_fields() {
        for raw in [
            b"version=9".as_slice(),
            br#"version=0 X={ { name="X" queued_trait={ {trait="a"} {trait="a"} } } }"#,
            br#"version=0 X={ { name="X" effect={ foo=yes } queued_trait={ {trait="a"} } } }"#,
        ] {
            assert!(mio_queues(raw).is_err())
        }
        assert!(is_advanced("ADVANCED GUIDES (NEW)/TIER.docx"));
        assert!(!is_advanced("COUNTRY GUIDES/Finland.docx"));
    }
    #[test]
    #[ignore = "explicit supplied archive, not a CI fixture"]
    fn supplied_advanced_archive_roundtrips_without_changing_other_references() {
        let archive = std::env::var("HOI4_ADVANCED_ARCHIVE").expect("explicit archive required");
        let root =
            std::env::temp_dir().join(format!("hoi4-advanced-validation-{}", std::process::id()));
        assert!(!root.exists());
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let incoming = root.join("incoming");
        let library =
            crate::reference_library::import(&incoming, std::path::Path::new(&archive)).unwrap();
        let docs = library["documents"].as_array().unwrap();
        assert_eq!(docs.len(), 31);
        assert_eq!(
            docs.iter().filter(|d| d["kind"] == "spreadsheet").count(),
            2
        );
        assert!(
            docs.iter()
                .any(|d| d["tiers"].as_array().is_some_and(|v| !v.is_empty()))
        );
        assert!(library["images"].as_array().unwrap().len() >= 70);
        let queues = library["mio_queues"].as_array().unwrap();
        assert!(!queues.is_empty());
        let export = export_mios(queues, None).unwrap();
        assert_eq!(
            mio_queues(export["text"].as_str().unwrap().as_bytes())
                .unwrap()
                .len(),
            queues.len()
        );
        for doc in docs {
            let (raw, _) =
                crate::reference_library::document_file(&incoming, doc["id"].as_str().unwrap())
                    .unwrap();
            assert_eq!(id(&raw), doc["id"]);
        }
        let saved = root.join("saved");
        std::fs::create_dir(&saved).unwrap();
        crate::reference_library::atomic_write(&saved.join("manifest.json"),br#"{"schema_version":1,"designs":[{"id":"existing","name":"User design"}],"images":[]}"#).unwrap();
        let merged = crate::reference_library::merge(&saved, &incoming).unwrap();
        assert!(
            merged["designs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["id"] == "existing")
        );
        assert_eq!(
            crate::reference_library::merge(&saved, &incoming).unwrap(),
            merged
        );
        println!(
            "ADVANCED_ARCHIVE_PASS documents={} mio_presets={} images={} designs={}",
            docs.len(),
            queues.len(),
            library["images"].as_array().unwrap().len(),
            library["designs"].as_array().unwrap().len()
        );
    }
}
