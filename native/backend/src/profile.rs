//! Immutable executable generations. Known hashes retain their qualified
//! layouts; other builds must independently satisfy structural ABI evidence.
use crate::pe::{Pe, number};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
const MANIFEST: &str = include_str!("../../../src/hoi4companion/live-profiles-v1.json");
type Result<T> = std::result::Result<T, String>;
pub(crate) struct Profile {
    pub(crate) path: PathBuf,
    pub(crate) data: Vec<u8>,
    metadata: Value,
    classes: Mutex<HashMap<u64, String>>,
}
static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<Profile>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Arc<Profile>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
pub(crate) fn get(handle: u64) -> Result<Arc<Profile>> {
    registry()
        .lock()
        .map_err(|_| "Profili eseguibile non disponibili")?
        .get(&handle)
        .cloned()
        .ok_or_else(|| "Profilo eseguibile non disponibile".into())
}
fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn display(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(value) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{value}")
    } else {
        text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
    }
}
impl Profile {
    pub(crate) fn metadata(&self) -> &Value {
        &self.metadata
    }
    #[cfg(test)]
    pub(crate) fn for_test(path: &Path) -> Self {
        let path = std::fs::canonicalize(path).unwrap();
        let data = std::fs::read(&path).unwrap();
        let header = number(&data, 0x3c, 4).unwrap() as usize;
        let metadata = json!({"image_size":number(&data,header+80,4).unwrap(),"timestamp":number(&data,header+8,4).unwrap()});
        Self {
            path,
            data,
            metadata,
            classes: Mutex::new(HashMap::new()),
        }
    }
    pub(crate) fn open(path: &Path, manifest_hash: &str) -> Result<Self> {
        if manifest_hash != digest(MANIFEST.as_bytes()) {
            return Err("Profili live modificati: ricompilare il backend Rust".into());
        }
        let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 512 * 1024 * 1024 {
            return Err("Eseguibile oltre il limite".into());
        }
        let data = std::fs::read(&path).map_err(|e| e.to_string())?;
        if data.len() > 512 * 1024 * 1024 {
            return Err("Eseguibile oltre il limite".into());
        }
        let sha = digest(&data);
        let manifest: Value = serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
        let selected = match manifest["profiles"].get(&sha) {
            Some(profile) => profile.clone(),
            None => crate::abi::qualify(&data,&manifest)
                .map_err(|reason|format!("Versione HOI4: struttura nativa non compatibile ({reason})"))?,
        };
        let pe = Pe::new(&data)?;
        let header = number(&data, 0x3c, 4)? as usize;
        let metadata = json!({"path":display(&path),"sha256":sha,"image_base":pe.base,"image_size":number(&data,header+80,4)?,"timestamp":number(&data,header+8,4)?,"sections":pe.sections,"selected":selected,"manifest_sha256":digest(MANIFEST.as_bytes())});
        Ok(Self {
            path,
            data,
            metadata,
            classes: Mutex::new(HashMap::new()),
        })
    }
    pub(crate) fn class_name(&self, rva: u64) -> Result<String> {
        let mut cache = self
            .classes
            .lock()
            .map_err(|_| "Cache RTTI non disponibile")?;
        if let Some(name) = cache.get(&rva) {
            return Ok(name.clone());
        }
        let name = Pe::new(&self.data)?.class_name(rva)?;
        if cache.len() < 4096 {
            cache.insert(rva, name.clone());
        }
        Ok(name)
    }
}
pub(crate) fn execute(op: &str, args: &Value) -> Result<Value> {
    if op == "profile_manifest" {
        return Ok(json!({"schema_version":1,"sha256":digest(MANIFEST.as_bytes())}));
    }
    if op == "profile_open" {
        let path = args["path"]
            .as_str()
            .filter(|s| !s.is_empty() && !s.contains('\0'))
            .ok_or("Percorso eseguibile non valido")?;
        let profile = Profile::open(
            Path::new(path),
            args["manifest_sha256"].as_str().unwrap_or(""),
        )?;
        let mut registry = registry()
            .lock()
            .map_err(|_| "Profili eseguibile non disponibili")?;
        if registry.len() >= 128 {
            return Err("Limite profili eseguibile raggiunto".into());
        }
        let handle = NEXT.fetch_add(1, Ordering::Relaxed);
        let result = json!({"handle":handle,"metadata":profile.metadata});
        registry.insert(handle, Arc::new(profile));
        return Ok(result);
    }
    let handle = args["handle"]
        .as_u64()
        .ok_or("Profilo eseguibile non valido")?;
    if op == "profile_release" {
        return Ok(json!(
            registry()
                .lock()
                .map_err(|_| "Profili eseguibile non disponibili")?
                .remove(&handle)
                .is_some()
        ));
    }
    let profile = get(handle)?;
    match op {
        "profile_class" => profile
            .class_name(args["rva"].as_u64().ok_or("Riferimento PE non mappato")?)
            .map(Value::String),
        "profile_bytes" => {
            let size = args["size"]
                .as_u64()
                .filter(|n| *n <= 65536)
                .ok_or("Riferimento PE non mappato")? as usize;
            let pe = Pe::new(&profile.data)?;
            let bytes = pe.bytes(
                args["rva"].as_u64().ok_or("Riferimento PE non mappato")?,
                size,
            )?;
            Ok(json!(
                bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
            ))
        }
        "profile_path" => Ok(json!(display(&profile.path))),
        _ => Err("Operazione profilo eseguibile non valida".into()),
    }
}
