//! Owned source generation snapshots for HTTP import work. Compilation runs
//! outside the live-reader/application locks; no campaign facts enter a packet.
use crate::{
    authoring, catalog_generation::Catalog, entity_catalog, guide_schema, import_catalog::Index,
    import_document, import_preview, reference_library, research_start,
};
use hoi4companion_catalog_core::normal_name;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
pub struct Publication {
    path: PathBuf,
    id: String,
    digest: String,
    committed: bool,
}
impl Publication {
    pub fn commit(mut self) -> Value {
        self.committed = true;
        json!({"id":self.id,"path":self.path,"href":format!("/walkthrough?guide={}",self.id)})
    }
}
impl Drop for Publication {
    fn drop(&mut self) {
        if !self.committed
            && let Ok(data) = std::fs::read(&self.path)
        {
            use sha2::{Digest, Sha256};
            if format!("{:x}", Sha256::digest(data)) == self.digest {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}
pub fn publish(guide: &Value, directory: &Path, bundled: &Path) -> Result<Publication> {
    use sha2::{Digest, Sha256};
    let id = guide["id"]
        .as_str()
        .filter(|s| crate::guide_repository::identity(s))
        .ok_or("Invalid guide identity")?;
    if bundled.join(format!("{id}.json")).exists() {
        return Err("ID già usato da una guida inclusa: scegli un nuovo ID".into());
    }
    let payload = serde_json::to_string_pretty(guide).map_err(|e| e.to_string())? + "\n";
    if payload.len() > 32 * 1024 * 1024 {
        return Err("Guide data exceeds size limit".into());
    }
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let temporary = directory.join(format!("{}.tmp", crate::progress_store::new_id()?));
    let path = directory.join(format!("{id}.json"));
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(payload.as_bytes())
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::hard_link(&temporary, &path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                "Guida già presente: scegli un nuovo ID per una revisione".into()
            } else {
                e.to_string()
            }
        })
    })();
    let _ = std::fs::remove_file(&temporary);
    written?;
    Ok(Publication {
        path,
        id: id.into(),
        digest: format!("{:x}", Sha256::digest(payload.as_bytes())),
        committed: false,
    })
}
pub struct Snapshot {
    index: Index,
    research: Value,
    compatibility: Option<Value>,
}
impl Catalog {
    pub(crate) fn import_index(&mut self) -> Result<Index> {
        if self.import_index_cache.is_none() {
            let mut args = self.identity_data()?;
            args["vocabulary"] = self.vocabulary.clone();
            self.import_index_cache = Some(Index::new(&args)?);
        }
        Ok(self.import_index_cache.as_ref().unwrap().clone())
    }
    fn research_for_import(&mut self, guide: &Value) -> Result<Value> {
        if self.sources["demo"] == true || self.sources["game_root"].as_str().is_none() {
            return Ok(Value::Null);
        }
        let scope = guide
            .get("applies_to")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let Some(scope) = scope.as_object() else {
            return Ok(Value::Null);
        };
        let country = scope.get("country").unwrap_or(&guide["country"]);
        if !country.is_string()
            || guide
                .get("start_date")
                .is_some_and(|d| !d.is_null() && !d.is_string())
        {
            return Ok(Value::Null);
        }
        let key = json!([country, guide["start_date"]]).to_string();
        if let Some(context) = self.research_context_cache.get(&key) {
            return Ok(context.clone());
        }
        let context = research_start::read(
            &json!({"game":self.sources["game_root"],"mods":self.sources["mod_roots"],"country":country,"start_date":guide["start_date"],"dlcs":self.sources["dlcs"],"scripts":self.sources["scripts"]}),
        )?;
        if self.research_context_cache.len() >= 128 {
            self.research_context_cache.clear();
        }
        self.research_context_cache.insert(key, context.clone());
        Ok(context)
    }
    pub(crate) fn import_snapshot(&mut self, guide: &Value) -> Result<Snapshot> {
        let research = self.research_for_import(guide)?;
        let index = self.import_index()?;
        let compatibility = guide_schema::normalize(guide)
            .ok()
            .map(|g| self.guide_compatibility(&g))
            .transpose()?;
        Ok(Snapshot {
            index,
            research,
            compatibility,
        })
    }
}
pub fn availability(report: &Value, compatibility: Option<&Value>) -> Value {
    let blocked = compatibility.is_some_and(|c| c["blocked"] == true);
    let valid = report["valid"] == true;
    json!({"status":if blocked{"dlc_unavailable"}else if valid{"ready"}else{"needs_revision"},"requires_review":!blocked&&!valid,"issue_count":if blocked{0}else{report["errors"].as_array().map(Vec::len).unwrap_or(0)}})
}
impl Snapshot {
    pub fn preview(&self, guide: &Value, sources: &Value, library: &Value) -> Value {
        let mut result =
            import_preview::preview(guide, sources, &self.index, library, &self.research);
        if !result["guide"].is_null()
            && let Some(compatibility) = &self.compatibility
        {
            result["compatibility"] = compatibility.clone();
        }
        result["availability"] = availability(&result, result.get("compatibility"));
        result
    }
}
pub fn status() -> Value {
    let key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    let model = std::env::var("HOI4_IMPORT_MODEL").unwrap_or_default();
    json!({"import_schema_version":1,"ai_configured":!key.is_empty()&&!model.is_empty(),"model":model,"provider":"OpenAI Responses","modes":["external_json","openai_api"],"formats":["hoi4-guide-authoring","hoi4-guide-import"],"authoring_schema_version":1})
}
pub fn packet(
    sources: &Value,
    entries: &[Value],
    reference: &Value,
    tree: Option<&str>,
) -> Result<Value> {
    import_document::validate_sources(sources)?;
    let tree = tree.filter(|s| !s.is_empty());
    if tree.is_some_and(|id| !entries.iter().any(|r| r["scope"] == id)) {
        return Err("Albero nazionale non presente nel catalogo".into());
    }
    let entries = entries
        .iter()
        .filter(|r| tree.is_none_or(|id| r["kind"] != "focus" || r["scope"] == id))
        .collect::<Vec<_>>();
    let political = [
        "law",
        "idea",
        "advisor",
        "event",
        "event_option",
        "decision",
    ];
    let text = normal_name(
        &sources
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["text"].as_str())
            .collect::<Vec<_>>()
            .join(" "),
    );
    let selected = entries
        .iter()
        .copied()
        .filter(|r| {
            political.contains(&r["kind"].as_str().unwrap_or(""))
                && std::iter::once(&r["id"])
                    .chain(std::iter::once(&r["name"]))
                    .chain(r["aliases"].as_array().into_iter().flatten())
                    .filter_map(Value::as_str)
                    .any(|name| {
                        let needle = normal_name(name);
                        needle.chars().count() >= 5 && text.contains(&needle)
                    })
        })
        .collect::<Vec<_>>();
    let events = selected
        .iter()
        .filter(|r| [Some("event"), Some("event_option")].contains(&r["kind"].as_str()))
        .map(|r| r.get("event_id").unwrap_or(&r["id"]).to_string())
        .collect::<HashSet<_>>();
    let rows = entries
        .into_iter()
        .filter(|r| {
            !political.contains(&r["kind"].as_str().unwrap_or(""))
                || selected.contains(r)
                || r["kind"] == "event" && events.contains(&r["id"].to_string())
                || r["kind"] == "event_option" && events.contains(&r["event_id"].to_string())
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut result = json!({"import_schema_version":1,"instructions":authoring::prompt(),"sources":sources,"catalog":entity_catalog::export(&json!(rows),reference)?,"localization_policy":"local_lookup","alias_policy":"english_reference_only"});
    if let Some(tree) = tree {
        result["context"] = json!({"tree_id":tree});
    }
    if reference_library::legacy_json(&result).len() > 2_000_000 {
        return Err(
            "Pacchetto troppo grande: seleziona un albero nazionale o riduci le fonti".into(),
        );
    }
    Ok(result)
}
