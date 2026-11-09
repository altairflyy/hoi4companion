//! Passive design/image library. Validates the entire input before publication;
//! never writes to the game, and pictures never become evidence of completion.
use crate::{import_catalog::regex, script_document as script, word_sources};
use image::{ImageFormat, ImageReader};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Cursor, Write},
    path::Path,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
type Result<T> = std::result::Result<T, String>;
fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
pub fn metadata(name: &str) -> Value {
    let parts = name.split('/').collect::<Vec<_>>();
    let parents = &parts[..parts.len().saturating_sub(1)];
    let patterns = [
        ("templates", r"(?i)^(?:division\s+)?templates?$"),
        ("armor", r"(?i)^(?:armor|armour|tank)(?:\s+designs?)?$"),
        ("air", r"(?i)^(?:air|aircraft|aviation)(?:\s+designs?)?$"),
        ("navy", r"(?i)^(?:navy|naval|ship)(?:\s+designs?)?$"),
        ("multiplayer", r"(?i)^multiplayer$"),
    ];
    let mut category = "other";
    let mut start = 0;
    for (i, part) in parents.iter().enumerate().rev() {
        let label = regex(r"\s*\([^)]*\)\s*").replace_all(part, " ");
        if let Some((key, _)) = patterns
            .iter()
            .find(|(_, p)| regex(p).is_match(label.trim()))
        {
            category = key;
            start = i;
            break;
        }
    }
    let tail = parents[start..].join(" ");
    let tags = ["situational", "outdated"]
        .into_iter()
        .filter(|tag| regex(&format!(r"(?i)\b{tag}\b")).is_match(&tail))
        .collect::<Vec<_>>();
    json!({"category":category,"tags":tags})
}
// Persisted design IDs predate the Rust backend. Preserve Python's sorted,
// ASCII-escaped JSON digest so existing guide references retain their identity.
pub(crate) fn legacy_json(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut keys = m.keys().collect::<Vec<_>>();
            keys.sort();
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|k| format!("{}: {}", legacy_json(&json!(k)), legacy_json(&m[k])))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(legacy_json).collect::<Vec<_>>().join(", ")
        ),
        Value::String(_) => {
            let mut out = String::new();
            for c in v.to_string().chars() {
                if c.is_ascii() {
                    out.push(c)
                } else {
                    let mut buf = [0; 2];
                    for unit in c.encode_utf16(&mut buf) {
                        out.push_str(&format!("\\u{unit:04x}"));
                    }
                }
            }
            out
        }
        _ => v.to_string(),
    }
}
fn plain(node: Option<&Value>) -> Value {
    let mut result = Map::new();
    if let Some(node) = node {
        for (k, v) in script::entries(node) {
            if !k.is_empty() && v.is_string() {
                result.insert(k.into(), v.clone());
            }
        }
    }
    Value::Object(result)
}
pub fn manifest(root: &Path) -> Value {
    crate::guide_repository::json_file(&root.join("manifest.json"), 32 * 1024 * 1024)
        .ok()
        .filter(|v| v["schema_version"] == 1 && v["designs"].is_array() && v["images"].is_array())
        .unwrap_or_else(|| json!({"schema_version":1,"designs":[],"images":[]}))
}
fn validate_image(raw: &[u8]) -> Result<()> {
    let format = image::guess_format(raw).map_err(|_| "Invalid reference image")?;
    if ![ImageFormat::Png, ImageFormat::Jpeg].contains(&format) {
        return Err("Invalid reference image".into());
    }
    let (w, h) = ImageReader::with_format(Cursor::new(raw), format)
        .into_dimensions()
        .map_err(|_| "Invalid reference image")?;
    if u64::from(w) * u64::from(h) > 40_000_000 {
        return Err("Invalid reference image".into());
    }
    let mut reader = ImageReader::with_format(Cursor::new(raw), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(180_000_000);
    reader.limits(limits);
    reader.decode().map_err(|_| "Invalid reference image")?;
    Ok(())
}
pub(crate) fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let temp = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(data).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
pub fn import(root: &Path, path: &Path) -> Result<Value> {
    static PUBLISH: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = PUBLISH
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?;
    let mut archive = word_sources::archive(std::fs::File::open(path).map_err(|e| e.to_string())?)?;
    let mut designs = Map::new();
    let mut images = Map::new();
    let mut blobs = HashMap::new();
    let mut budget = 180_000_000u64;
    for i in 0..archive.len() {
        let file = archive.by_index_raw(i).map_err(|e| e.to_string())?;
        if file.is_dir() || !word_sources::safe(file.name()) {
            continue;
        }
        let name = file.name().to_owned();
        let size = file.size();
        drop(file);
        let entry_path = Path::new(&name);
        let suffix = entry_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !["txt", "png", "jpg", "jpeg"].contains(&suffix.as_str()) {
            continue;
        }
        budget = budget
            .checked_sub(size)
            .ok_or("Reference archive exceeds its decoded size limit")?;
        let raw = word_sources::read_entry(&mut archive, &name, 16_000_000)?;
        if suffix == "txt" {
            let node = serde_json::to_value(hoi4companion_parser::read_reviewed(&raw)?)
                .map_err(|e| e.to_string())?;
            for (_, group) in script::entries(&node).filter(|(_, v)| v["entries"].is_array()) {
                for equipment in script::blocks(group, "equipment") {
                    let title = script::scalar(equipment, "name");
                    let variants = script::blocks(equipment, "target_variant").collect::<Vec<_>>();
                    if title.is_empty() || title.chars().count() > 200 || variants.len() != 1 {
                        continue;
                    }
                    let variant = variants[0];
                    let identifier = script::scalar(variant, "type");
                    if !regex(r"^[A-Za-z0-9_]+$").is_match(identifier) {
                        continue;
                    }
                    let row = json!({"name":title,"equipment_id":identifier,"category":script::scalar(group,"category"),"modules":plain(script::blocks(variant,"modules").next()),"upgrades":plain(script::blocks(variant,"upgrades").next())});
                    let id = digest(legacy_json(&row).as_bytes());
                    let mut result = json!({"id":id});
                    result
                        .as_object_mut()
                        .unwrap()
                        .extend(row.as_object().unwrap().clone());
                    designs.insert(id, result);
                }
            }
        } else {
            validate_image(&raw)?;
            let id = digest(&raw);
            let mut row = json!({"id":id,"name":entry_path.file_stem().and_then(|s|s.to_str()).unwrap_or(""),"section":entry_path.parent().and_then(|p|p.file_name()).and_then(|s|s.to_str()).unwrap_or(""),"mime":if suffix=="png"{"image/png"}else{"image/jpeg"}});
            let meta = metadata(&name);
            row.as_object_mut()
                .unwrap()
                .extend(meta.as_object().unwrap().clone());
            let key = json!([
                row["id"],
                row["name"],
                row["category"],
                row["section"],
                row["tags"]
            ])
            .to_string();
            images.insert(key, row);
            blobs.insert(id, raw);
        }
    }
    if designs.is_empty() && images.is_empty() {
        return Err("No equipment designs or reference images in this archive".into());
    }
    let result = json!({"schema_version":1,"designs":designs.into_values().collect::<Vec<_>>(),"images":images.into_values().collect::<Vec<_>>()});
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    for (id, raw) in blobs {
        let path = root.join(id);
        if std::fs::read(&path).ok().as_deref() != Some(raw.as_slice()) {
            atomic_write(&path, &raw)?;
        }
    }
    atomic_write(
        &root.join("manifest.json"),
        serde_json::to_string_pretty(&result)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )?;
    Ok(result)
}
pub fn image(root: &Path, id: &str) -> Result<(Vec<u8>, String)> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Invalid image reference".into());
    }
    let data = manifest(root);
    let item = data["images"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .ok_or("Reference image not found")?;
    let path = root.join(id);
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if size > 16_000_000 {
        return Err("Invalid reference image".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if digest(&bytes) != id {
        return Err("Reference image checksum mismatch".into());
    }
    Ok((
        bytes,
        item["mime"]
            .as_str()
            .unwrap_or("application/octet-stream")
            .into(),
    ))
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _lock = LOCK
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?;
    let root = Path::new(
        args["root"]
            .as_str()
            .ok_or("Missing reference library root")?,
    );
    match op {
        "reference_library_manifest" => Ok(manifest(root)),
        "reference_library_import" => import(
            root,
            Path::new(args["path"].as_str().ok_or("Missing reference archive")?),
        ),
        "reference_library_image" => {
            let (data, mime) = image(root, args["id"].as_str().ok_or("Invalid image reference")?)?;
            Ok(json!({"data":data,"mime":mime}))
        }
        "reference_library_image_path" => {
            let id = args["id"].as_str().ok_or("Invalid image reference")?;
            let (_, mime) = image(root, id)?;
            Ok(json!({"path":root.join(id),"mime":mime,"sha256":id}))
        }
        _ => Err("Unknown reference library operation".into()),
    }
}
