//! Owned sprite definitions and opaque HTTP asset identities. Texture paths are
//! resolved only inside configured roots; a request token never becomes a path.
use crate::{
    installation,
    script_document::{self, all, entries},
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::UNIX_EPOCH,
};
type Result<T> = std::result::Result<T, String>;
fn pure(value: &str) -> String {
    value
        .replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect::<Vec<_>>()
        .join("/")
}
fn definitions(node: &Value, sprites: &mut Map<String, Value>) {
    for (key, value) in entries(node) {
        if !value.is_object() {
            continue;
        }
        if [
            "spritetype",
            "frameanimatedspritetype",
            "corneredtilespritetype",
        ]
        .contains(&key.to_lowercase().as_str())
        {
            let fields: HashMap<_, _> =
                entries(value).map(|(k, v)| (k.to_lowercase(), v)).collect();
            let name = fields.get("name").and_then(|v| v.as_str());
            let texture = fields.get("texturefile").and_then(|v| v.as_str());
            let frames = match fields.get("noofframes") {
                Some(v) => v
                    .as_str()
                    .and_then(|s| s.trim().replace('_', "").parse::<u16>().ok()),
                None => Some(1),
            };
            if let (Some(name), Some(texture), Some(frames @ 1..=256)) = (name, texture, frames) {
                sprites.insert(
                    name.into(),
                    json!({"name":name,"texture":texture,"frames":frames}),
                );
            }
        } else {
            definitions(value, sprites);
        }
    }
}
fn canonical(path: &Path) -> Result<PathBuf> {
    Ok(PathBuf::from(installation::display(
        &path.canonicalize().map_err(|e| e.to_string())?,
    )))
}
pub struct Assets {
    roots: Vec<PathBuf>,
    sprites: Map<String, Value>,
    replacements: HashMap<PathBuf, Vec<String>>,
    warnings: Vec<Value>,
    resolved: HashMap<(String, u64), Value>,
    records: HashMap<String, Value>,
    cache: Option<PathBuf>,
    failed: HashSet<String>,
}
impl Assets {
    pub fn open(args: &Value) -> Result<Self> {
        let game = PathBuf::from(args["game"].as_str().ok_or("Missing asset root")?);
        let extras = args["roots"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| {
                v.as_str()
                    .map(PathBuf::from)
                    .ok_or("Invalid asset root".into())
            })
            .collect::<Result<Vec<_>>>()?;
        let roots = std::iter::once(&game)
            .chain(extras.iter())
            .map(|root| canonical(root))
            .collect::<Result<Vec<_>>>()?;
        let (mut replacements, mut warnings) = (HashMap::new(), vec![]);
        for root in roots.iter().skip(1) {
            let descriptor = root.join("descriptor.mod");
            if !descriptor.is_file() {
                continue;
            }
            match script_document::read(&descriptor) {
                Ok(node) => {
                    replacements.insert(
                        root.clone(),
                        all(&node, "replace_path")
                            .filter_map(Value::as_str)
                            .map(|s| hoi4companion_catalog_core::casefold(&pure(s)))
                            .collect(),
                    );
                }
                Err(_) => warnings.push(json!(format!(
                    "Replace path delle texture non leggibile: {}",
                    descriptor.file_name().unwrap_or_default().to_string_lossy()
                ))),
            }
        }
        let files = installation::collect(&game, &extras, "interface", ".gfx")?;
        warnings.extend(files["warnings"].as_array().into_iter().flatten().cloned());
        let mut sprites = Map::new();
        for file in files["files"].as_array().into_iter().flatten() {
            let path = Path::new(file.as_str().ok_or("Invalid graphics path")?);
            match script_document::read(path) {
                Ok(node) => definitions(&node, &mut sprites),
                Err(error) => warnings.push(json!(format!(
                    "Sprite non letti ({}): {error}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ))),
            }
        }
        Ok(Self {
            roots,
            sprites,
            replacements,
            warnings,
            resolved: HashMap::new(),
            records: HashMap::new(),
            cache: args["cache"].as_str().map(PathBuf::from),
            failed: HashSet::new(),
        })
    }
    pub fn texture(&self, relative: &str) -> Option<PathBuf> {
        let relative = relative.replace('\\', "/");
        if relative.starts_with('/')
            || relative.contains(':')
            || relative.split('/').any(|s| s == "..")
        {
            return None;
        }
        let relative = pure(&relative);
        let path = Path::new(&relative);
        let extension = path.extension()?.to_string_lossy().to_lowercase();
        if !["dds", "png", "tga", "bmp"].contains(&extension.as_str())
            || path
                .components()
                .any(|p| !matches!(p, Component::Normal(_)))
        {
            return None;
        }
        let mut alternatives = vec![path.to_path_buf()];
        if extension != "dds" {
            alternatives.push(path.with_extension("dds"));
        }
        let folded = hoi4companion_catalog_core::casefold(&relative);
        for root in self.roots.iter().rev() {
            for candidate in &alternatives {
                if let Ok(file) = canonical(&root.join(candidate))
                    && file.starts_with(root)
                    && file.is_file()
                {
                    return Some(file);
                }
            }
            if self.replacements.get(root).is_some_and(|items| {
                items
                    .iter()
                    .any(|prefix| folded == *prefix || folded.starts_with(&(prefix.clone() + "/")))
            }) {
                return None;
            }
        }
        None
    }
    pub fn resolve(&mut self, name: &str, frame: u64) -> Result<Value> {
        let key = (name.to_owned(), frame);
        if let Some(value) = self.resolved.get(&key) {
            return Ok(value.clone());
        }
        let (mut sprite, mut asset) = (Value::Null, Value::Null);
        if let Some(definition) = self.sprites.get(name)
            && let Some(frames) = definition["frames"].as_u64()
            && (1..=frames).contains(&frame)
            && let Some(file) = self.texture(
                definition["texture"]
                    .as_str()
                    .ok_or("Invalid texture definition")?,
            )
        {
            let stat = std::fs::metadata(&file).map_err(|e| e.to_string())?;
            let modified = stat
                .modified()
                .map_err(|e| e.to_string())?
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos();
            let path = installation::display(&file);
            let identity = format!("{path}|{}|{modified}|{frames}|{frame}|png-v1", stat.len());
            let token = format!("{:x}", Sha256::digest(identity.as_bytes()));
            sprite = json!({"name":name,"url":format!("/api/sprite/{token}.png"),"frame":frame,"frames":frames,"source":definition["texture"],"override":self.roots.iter().skip(1).any(|root|file.starts_with(root))});
            asset = json!({"path":path,"frames":frames,"frame":frame});
            self.records.insert(token, asset.clone());
        }
        let result = json!({"sprite":sprite,"asset":asset});
        self.resolved.insert(key, result.clone());
        Ok(result)
    }
    pub fn info(&self) -> Value {
        json!({"roots":self.roots.iter().map(|p|installation::display(p)).collect::<Vec<_>>(),"sprites":self.sprites,"warnings":self.warnings,"failed":self.failed})
    }
    pub fn inline(&mut self, texts: &[String]) -> Result<Value> {
        let mut result = Map::new();
        for text in texts {
            let mut cursor = 0;
            while let Some(start) = text[cursor..].find('£').map(|i| i + cursor) {
                let from = start + '£'.len_utf8();
                let name_end = text[from..]
                    .bytes()
                    .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                    .count()
                    + from;
                if name_end == from {
                    cursor = from;
                    continue;
                }
                let mut end = name_end;
                let mut frame = Some(1);
                if text.as_bytes().get(end) == Some(&b'|') {
                    let digits = end + 1;
                    let last = digits
                        + text[digits..]
                            .bytes()
                            .take_while(u8::is_ascii_digit)
                            .count();
                    if last > digits {
                        frame = text[digits..last].parse::<u64>().ok();
                        end = last;
                    }
                }
                if text[end..].starts_with('£') {
                    end += '£'.len_utf8();
                }
                let name = &text[from..name_end];
                let name = if name.starts_with("GFX_") {
                    name.to_owned()
                } else {
                    format!("GFX_{name}")
                };
                if let Some(frame) = frame.filter(|f| *f > 0) {
                    let value = self.resolve(&name, frame)?["sprite"].clone();
                    if !value.is_null() {
                        result.insert(text[start..end].into(), value);
                    }
                }
                cursor = end;
            }
        }
        Ok(Value::Object(result))
    }
    pub fn theme(&mut self) -> Result<Value> {
        let mut result = Map::new();
        for (key, name) in [
            ("locked", "GFX_focus_unavailable"),
            ("unknown", "GFX_focus_unavailable"),
            ("verify", "GFX_focus_unavailable"),
            ("excluded", "GFX_focus_unavailable"),
            ("ready", "GFX_focus_can_start"),
            ("active", "GFX_focus_can_start"),
            ("completed", "GFX_focus_completed"),
            ("exclusive", "GFX_focus_link_exclusive"),
        ] {
            let value = self.resolve(name, 1)?["sprite"].clone();
            if !value.is_null() {
                result.insert(key.into(), value);
            }
        }
        Ok(Value::Object(result))
    }
    pub fn png_file(&mut self, token: &str) -> Result<Option<PathBuf>> {
        if self.failed.contains(token) {
            return Ok(None);
        }
        let Some(record) = self.records.get(token) else {
            return Ok(None);
        };
        let cache = self
            .cache
            .as_ref()
            .ok_or("Sprite cache is not configured")?;
        let absolute = std::path::absolute(cache).map_err(|e| e.to_string())?;
        let mut normalized = PathBuf::new();
        for component in absolute.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    if !normalized.pop() {
                        return Err("Invalid sprite cache traversal".into());
                    }
                }
                other => normalized.push(other.as_os_str()),
            }
        }
        let cache = &normalized;
        let mut ancestor = cache.as_path();
        while !ancestor.exists() {
            ancestor = ancestor.parent().ok_or("Invalid sprite cache")?;
        }
        let resolved = canonical(ancestor)?;
        if self.roots.iter().any(|root| resolved.starts_with(root)) {
            return Err("Sprite cache must be outside game and mod roots".into());
        }
        let target = cache.join(format!("{token}.png"));
        if target.is_file() {
            return Ok(Some(target));
        }
        let convert = (|| -> Result<PathBuf> {
            use std::io::{Read, Write};
            let path = Path::new(record["path"].as_str().ok_or("Invalid asset record")?);
            if path.metadata().map_err(|e| e.to_string())?.len() > 32_000_000 {
                return Err("Texture exceeds limit".into());
            }
            let mut raw = Vec::new();
            std::fs::File::open(path)
                .map_err(|e| e.to_string())?
                .take(32_000_001)
                .read_to_end(&mut raw)
                .map_err(|e| e.to_string())?;
            let data = crate::texture::png(
                &raw,
                record["frames"].as_u64().ok_or("Invalid frame count")? as u32,
                record["frame"].as_u64().ok_or("Invalid frame")? as u32,
            )?;
            std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
            let temporary = cache.join(format!("{token}.{}.tmp", crate::progress_store::new_id()?));
            let save = (|| -> Result<()> {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)
                    .map_err(|e| e.to_string())?;
                file.write_all(&data).map_err(|e| e.to_string())?;
                drop(file);
                std::fs::rename(&temporary, &target).map_err(|e| e.to_string())
            })();
            if let Err(error) = save {
                let _ = std::fs::remove_file(&temporary);
                return Err(error);
            }
            Ok(target)
        })();
        match convert {
            Ok(path) => Ok(Some(path)),
            Err(_) => {
                self.failed.insert(token.into());
                Ok(None)
            }
        }
    }
}
type Owner = Arc<Mutex<Assets>>;
static REGISTRY: OnceLock<Mutex<HashMap<u64, Owner>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Owner>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
pub(crate) fn with<T>(
    handle: Option<u64>,
    apply: impl FnOnce(Option<&mut Assets>) -> Result<T>,
) -> Result<T> {
    let Some(id) = handle else {
        return apply(None);
    };
    let owner = registry()
        .lock()
        .map_err(|_| "Asset registry unavailable")?
        .get(&id)
        .cloned()
        .ok_or("Unknown asset handle")?;
    let mut owner = owner.lock().map_err(|_| "Assets unavailable")?;
    apply(Some(&mut owner))
}
pub fn execute(op: &str, args: &Value) -> Result<Value> {
    if op == "assets_open" {
        let owner = Assets::open(args)?;
        let mut entries = registry()
            .lock()
            .map_err(|_| "Asset registry unavailable")?;
        if entries.len() >= 64 {
            return Err("Asset registry limit exceeded".into());
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        entries.insert(id, Arc::new(Mutex::new(owner)));
        return Ok(json!(id));
    }
    let id = args["handle"].as_u64().ok_or("Missing asset handle")?;
    if op == "assets_release" {
        return Ok(json!(
            registry()
                .lock()
                .map_err(|_| "Asset registry unavailable")?
                .remove(&id)
                .is_some()
        ));
    }
    let owner = registry()
        .lock()
        .map_err(|_| "Asset registry unavailable")?
        .get(&id)
        .cloned()
        .ok_or("Unknown asset handle")?;
    let mut owner = owner.lock().map_err(|_| "Assets unavailable")?;
    request(&mut owner, op, args)
}
pub fn request(owner: &mut Assets, op: &str, args: &Value) -> Result<Value> {
    match op {
        "assets_png" => Ok(json!(
            owner
                .png_file(args["token"].as_str().ok_or("Missing sprite token")?)?
                .map(|path| installation::display(&path))
        )),
        "assets_info" => Ok(owner.info()),
        "assets_texture" => Ok(json!(
            owner
                .texture(
                    args["texture"]
                        .as_str()
                        .ok_or("Missing texture reference")?
                )
                .map(|p| installation::display(&p))
        )),
        "assets_resolve" => owner.resolve(
            args["name"].as_str().ok_or("Missing sprite identity")?,
            args["frame"].as_u64().ok_or("Invalid sprite frame")?,
        ),
        _ => Err("Unknown asset operation".into()),
    }
}
