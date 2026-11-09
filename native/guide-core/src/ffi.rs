//! Bounded C ABI. Handles retain only immutable compiled definitions, never facts.
use super::*;
use std::ffi::{CString, c_char};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
static GUIDES: OnceLock<Mutex<HashMap<u64, Arc<CompiledGuide>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
const MAX_INPUT: usize = 16 * 1024 * 1024;
fn guides() -> &'static Mutex<HashMap<u64, Arc<CompiledGuide>>> {
    GUIDES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn reply(value: Result<Value, Error>) -> *mut c_char {
    let value = match value {
        Ok(value) => json!({"schema_version":1,"result":value}),
        Err(error) => json!({"schema_version":1,"error":error}),
    };
    CString::new(value.to_string())
        .expect("JSON has no unescaped nul")
        .into_raw()
}
fn guarded(f: impl FnOnce() -> Result<Value, Error> + std::panic::UnwindSafe) -> *mut c_char {
    reply(
        std::panic::catch_unwind(f)
            .unwrap_or_else(|_| Err("Guide core rejected malformed input".into())),
    )
}
unsafe fn decode<T: serde::de::DeserializeOwned>(ptr: *const u8, len: usize) -> Result<T, Error> {
    if ptr.is_null() || len == 0 || len > MAX_INPUT {
        return Err("Invalid guide-core input size".into());
    }
    // SAFETY: the caller must supply a readable len-byte buffer for this call.
    serde_json::from_slice(unsafe { std::slice::from_raw_parts(ptr, len) })
        .map_err(|e| e.to_string())
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_guide_schema_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_guide_info() -> *mut c_char {
    reply(Ok(engine_info()))
}
/// # Safety
/// ptr points to len readable bytes until this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_guide_compile(ptr: *const u8, len: usize) -> *mut c_char {
    guarded(|| {
        let guide: Guide = unsafe { decode(ptr, len) }?;
        let compiled = CompiledGuide::compile(guide)?;
        let mut registry = guides().lock().map_err(|_| "Guide registry unavailable")?;
        if registry.len() >= 64 {
            return Err("Compiled guide handle limit exceeded".into());
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        registry.insert(id, Arc::new(compiled));
        Ok(json!({"handle":id}))
    })
}
/// # Safety
/// ptr points to len readable bytes until this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_guide_evaluate(
    handle: u64,
    ptr: *const u8,
    len: usize,
) -> *mut c_char {
    guarded(|| {
        let guide = guides()
            .lock()
            .map_err(|_| "Guide registry unavailable")?
            .get(&handle)
            .cloned()
            .ok_or("Unknown compiled guide handle")?;
        let input: Input = unsafe { decode(ptr, len) }?;
        guide.evaluate(&input)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_guide_release(handle: u64) -> bool {
    guides()
        .lock()
        .is_ok_and(|mut r| r.remove(&handle).is_some())
}
/// # Safety
/// ptr is null or an outstanding JSON reply returned by this DLL, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_guide_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_null_and_oversized_input() {
        assert!(unsafe { decode::<Value>(std::ptr::null(), 0) }.is_err());
        assert!(unsafe { decode::<Value>(std::ptr::dangling(), MAX_INPUT + 1) }.is_err());
    }
    #[test]
    fn missing_handles_are_rejected() {
        let data = b"{}";
        let ptr = unsafe { hoi4_guide_evaluate(u64::MAX, data.as_ptr(), data.len()) };
        let value: Value =
            serde_json::from_slice(unsafe { std::ffi::CStr::from_ptr(ptr) }.to_bytes()).unwrap();
        assert!(value.get("error").is_some());
        unsafe { hoi4_guide_free(ptr) };
    }
}
