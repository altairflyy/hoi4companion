//! Handles retain immutable catalogs. Queries have no game, network or disk I/O.
use super::*;
use std::ffi::{CString, c_char};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicU64, Ordering},
};
const MAX_BYTES: usize = 64 * 1024 * 1024;
static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<Catalog>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Arc<Catalog>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
pub(super) fn borrow(handle: u64) -> Result<Arc<Catalog>, String> {
    registry()
        .lock()
        .map_err(|_| "Catalog registry unavailable")?
        .get(&handle)
        .cloned()
        .ok_or_else(|| "Unknown native catalog handle".into())
}
pub(super) fn request(op: &str, args: &Value) -> Result<Value, String> {
    match op {
        "identity_info" => Ok(
            json!({"engine":"rust-catalog-core","schema_version":1,"abi_version":1,"unicode_version":text::unicode_version(),"game_access":false}),
        ),
        "identity_open" => {
            let catalog =
                Catalog::compile(serde_json::from_value(args.clone()).map_err(|e| e.to_string())?)?;
            let mut entries = registry()
                .lock()
                .map_err(|_| "Catalog registry unavailable")?;
            if entries.len() >= 128 {
                return Err("Native catalog handle limit exceeded".into());
            }
            let handle = NEXT.fetch_add(1, Ordering::Relaxed);
            entries.insert(handle, Arc::new(catalog));
            Ok(json!({"handle":handle}))
        }
        "identity_query" => {
            let handle = args["handle"].as_u64().ok_or("Missing identity handle")?;
            let catalog = registry()
                .lock()
                .map_err(|_| "Catalog registry unavailable")?
                .get(&handle)
                .cloned()
                .ok_or("Unknown native catalog handle")?;
            catalog
                .query(serde_json::from_value(args["request"].clone()).map_err(|e| e.to_string())?)
        }
        "identity_release" => Ok(json!(
            registry()
                .lock()
                .map_err(|_| "Catalog registry unavailable")?
                .remove(&args["handle"].as_u64().ok_or("Missing identity handle")?)
                .is_some()
        )),
        _ => Err("Unknown identity service operation".into()),
    }
}
fn reply(value: Result<Value, String>) -> *mut c_char {
    let value = match value {
        Ok(result) => json!({"schema_version":1,"result":result}),
        Err(error) => json!({"schema_version":1,"error":error}),
    };
    CString::new(value.to_string())
        .expect("escaped JSON")
        .into_raw()
}
fn guard(f: impl FnOnce() -> Result<Value, String> + std::panic::UnwindSafe) -> *mut c_char {
    reply(
        std::panic::catch_unwind(f).unwrap_or_else(|_| Err("Native catalog rejected input".into())),
    )
}
unsafe fn decode<T: serde::de::DeserializeOwned>(p: *const u8, n: usize) -> Result<T, String> {
    if p.is_null() || n == 0 || n > MAX_BYTES {
        return Err("Invalid native catalog input size".into());
    }
    serde_json::from_slice(unsafe { std::slice::from_raw_parts(p, n) }).map_err(|e| e.to_string())
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_catalog_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_catalog_info() -> *mut c_char {
    reply(Ok(
        json!({"engine":"rust-catalog-core","schema_version":1,"abi_version":1,"unicode_version":text::unicode_version(),"game_access":false}),
    ))
}
/// # Safety
/// p points to n readable bytes until return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_catalog_compile(p: *const u8, n: usize) -> *mut c_char {
    guard(|| {
        let catalog = Catalog::compile(unsafe { decode(p, n) }?)?;
        let mut entries = registry()
            .lock()
            .map_err(|_| "Catalog registry unavailable")?;
        if entries.len() >= 128 {
            return Err("Native catalog handle limit exceeded".into());
        }
        let handle = NEXT.fetch_add(1, Ordering::Relaxed);
        entries.insert(handle, Arc::new(catalog));
        Ok(json!({"handle":handle}))
    })
}
/// # Safety
/// p points to n readable bytes until return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_catalog_query(handle: u64, p: *const u8, n: usize) -> *mut c_char {
    guard(|| {
        let catalog = registry()
            .lock()
            .map_err(|_| "Catalog registry unavailable")?
            .get(&handle)
            .cloned()
            .ok_or("Unknown native catalog handle")?;
        catalog.query(unsafe { decode(p, n) }?)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_catalog_release(handle: u64) -> bool {
    registry()
        .lock()
        .is_ok_and(|mut r| r.remove(&handle).is_some())
}
/// # Safety
/// p is null or a live reply allocated by this library, freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_catalog_free(p: *mut c_char) {
    if !p.is_null() {
        drop(unsafe { CString::from_raw(p) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_null_oversize_and_missing_handles() {
        assert!(unsafe { decode::<Value>(std::ptr::null(), 0) }.is_err());
        assert!(unsafe { decode::<Value>(std::ptr::dangling(), MAX_BYTES + 1) }.is_err());
        let p = unsafe { hoi4_catalog_query(u64::MAX, b"{}".as_ptr(), 2) };
        let result: Value =
            serde_json::from_slice(unsafe { std::ffi::CStr::from_ptr(p) }.to_bytes()).unwrap();
        assert!(result.get("error").is_some());
        unsafe { hoi4_catalog_free(p) };
    }
}
