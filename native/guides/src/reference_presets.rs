//! Saved recipe inventory is file evidence only, never proof of campaign models.
use crate::{guide_prepare::canonical, script_document as script};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    io::Read,
    path::Path,
    sync::{Mutex, OnceLock},
};
type Result<T> = std::result::Result<T, String>;
fn fields(variant: &Value, key: &str) -> Result<Value> {
    let blocks = script::blocks(variant, key).collect::<Vec<_>>();
    if blocks.len() > 1 {
        return Err("Repeated saved design fields".into());
    }
    let mut values = Map::new();
    if let Some(block) = blocks.first() {
        for (k, v) in script::entries(block) {
            if k.is_empty() || !v.is_string() || values.insert(k.into(), v.clone()).is_some() {
                return Err("Ambiguous saved design fields".into());
            }
        }
    }
    Ok(Value::Object(values))
}
fn recipes(raw: &[u8]) -> Result<Vec<Value>> {
    let node = serde_json::to_value(hoi4companion_parser::read_reviewed(raw)?)
        .map_err(|e| e.to_string())?;
    if script::scalar(&node, "version") != "0" {
        return Err("Unsupported saved design version".into());
    }
    let mut result = vec![];
    for (_, group) in script::entries(&node).filter(|(_, v)| v["entries"].is_array()) {
        let category = script::scalar(group, "category");
        if !["land", "naval", "air"].contains(&category) {
            return Err("Unknown saved design category".into());
        }
        for entry in script::blocks(group, "equipment") {
            let variants = script::blocks(entry, "target_variant").collect::<Vec<_>>();
            let name = script::scalar(entry, "name");
            if name.is_empty() || variants.len() != 1 {
                return Err("Incomplete saved design".into());
            }
            let variant = variants[0];
            let equipment = script::scalar(variant, "type");
            if equipment.is_empty() {
                return Err("Saved design has no equipment type".into());
            }
            result.push(json!({"name":name,"category":category,"equipment_id":equipment,"modules":fields(variant,"modules")?,"upgrades":fields(variant,"upgrades")?,"folder":script::scalar(entry,"folder")}));
        }
    }
    Ok(result)
}
fn identity(row: &Value) -> String {
    let mut value = Map::new();
    for key in ["name", "category", "equipment_id", "modules", "upgrades"] {
        value.insert(
            key.into(),
            row.get(key).cloned().unwrap_or_else(|| {
                if ["modules", "upgrades"].contains(&key) {
                    json!({})
                } else {
                    json!("")
                }
            }),
        );
    }
    canonical(&Value::Object(value)).to_string()
}
fn file_key(file: &std::fs::File) -> Result<Value> {
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
        };
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
            .map_err(|e| e.to_string())?;
        Ok(json!([
            metadata.len(),
            info.dwVolumeSerialNumber,
            info.nFileIndexHigh,
            info.nFileIndexLow,
            info.ftLastWriteTime.dwHighDateTime,
            info.ftLastWriteTime.dwLowDateTime
        ]))
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(json!([
            metadata.len(),
            metadata.dev(),
            metadata.ino(),
            metadata.mtime(),
            metadata.mtime_nsec()
        ]))
    }
}
pub fn inspect(docs: Option<&str>, designs: &Value) -> Value {
    let result = (|| -> Result<Value> {
        let docs = docs
            .filter(|s| !s.is_empty())
            .ok_or("HOI4 documents directory is not configured")?;
        let root = Path::new(docs).canonicalize().map_err(|e| e.to_string())?;
        let unresolved = root.join("session_persistence/equipment_designs/equipment_designs.txt");
        if !unresolved.exists() {
            return Ok(json!({"status":"file_missing","designs":{}}));
        }
        let path = unresolved.canonicalize().map_err(|e| e.to_string())?;
        if !path.starts_with(&root) {
            return Err("Saved design path is outside HOI4 documents".into());
        }
        let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let key = file_key(&file)?;
        if file.metadata().map_err(|e| e.to_string())?.len() > 2_000_000 {
            return Err("Saved design file exceeds its inventory limit".into());
        }
        type Cache = HashMap<String, (Value, Vec<Value>, String)>;
        static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
        let mut cache = CACHE
            .get_or_init(Default::default)
            .lock()
            .map_err(|e| e.to_string())?;
        let cache_key = path.to_string_lossy().into_owned();
        let cached = cache.get(&cache_key).filter(|(k, _, _)| k == &key).cloned();
        let (rows, digest) = if let Some((_, rows, digest)) = cached {
            (rows, digest)
        } else {
            let mut raw = vec![];
            (&file)
                .take(2_000_001)
                .read_to_end(&mut raw)
                .map_err(|e| e.to_string())?;
            let after = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            if file_key(&after)? != key
                || file_key(&file)? != key
                || raw.len() as u64 != key[0].as_u64().unwrap_or(u64::MAX)
                || raw.len() > 2_000_000
            {
                return Err("Saved design file changed during inspection".into());
            }
            let rows = recipes(&raw)?;
            let digest = format!("{:x}", Sha256::digest(&raw));
            if cache.len() >= 16 {
                cache.clear();
            }
            cache.insert(cache_key, (key, rows.clone(), digest.clone()));
            (rows, digest)
        };
        let mut indexed: HashMap<String, BTreeSet<String>> = HashMap::new();
        let mut names = BTreeSet::new();
        for row in &rows {
            names.insert(row["name"].clone().to_string());
            indexed
                .entry(identity(row))
                .or_default()
                .insert(row["folder"].as_str().unwrap_or("").into());
        }
        let mut locations = Map::new();
        for row in designs.as_array().into_iter().flatten() {
            let folders = indexed.get(&identity(row)).cloned().unwrap_or_default();
            locations.insert(row["id"].as_str().ok_or("Invalid design identity")?.into(),json!({"status":if !folders.is_empty(){"present"}else if names.contains(&row["name"].to_string()){"different_recipe"}else{"missing"},"folders":folders.into_iter().collect::<Vec<_>>()}));
        }
        Ok(
            json!({"status":"file_observed","count":rows.len(),"distinct_recipes":indexed.len(),"sha256":digest,"designs":locations}),
        )
    })();
    result.unwrap_or_else(|error| json!({"status":"unavailable","reason":error,"designs":{}}))
}
