//! Own only processes created here. No PID attachment or arbitrary termination.
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ffi::{CString, c_char};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
#[cfg(windows)]
mod process;
#[cfg(windows)]
use process::Tool;

#[derive(Deserialize)]
pub struct Spawn {
    schema_version: u32,
    argv: Vec<String>,
    stdout: String,
    stderr: String,
    #[serde(default)]
    cwd: Option<String>,
    max_millis: u64,
    max_output_bytes: u64,
}
impl Spawn {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.argv.is_empty()
            || self.argv.len() > 4096
            || self.argv[0].is_empty()
            || self.argv.iter().any(|s| s.contains('\0'))
            || self
                .argv
                .iter()
                .map(|s| s.encode_utf16().count() + 3)
                .sum::<usize>()
                > 30000
            || self.stdout.is_empty()
            || self.stderr.is_empty()
            || self.stdout.contains('\0')
            || self.stderr.contains('\0')
            || self.cwd.as_ref().is_some_and(|s| s.contains('\0'))
            || self.max_millis == 0
            || self.max_millis > 24 * 3600 * 1000
            || self.max_output_bytes == 0
            || self.max_output_bytes > 1024 * 1024 * 1024
        {
            return Err("Invalid owned tool specification".into());
        }
        Ok(())
    }
}
#[cfg(not(windows))]
struct Tool;
#[cfg(not(windows))]
impl Tool {
    fn spawn(_: Spawn) -> Result<Arc<Self>, String> {
        Err("Windows job objects are required".into())
    }
    fn status(&self) -> Result<Value, String> {
        Err("Windows job objects are required".into())
    }
    fn cancel(&self) -> Result<Value, String> {
        Err("Windows job objects are required".into())
    }
    fn wait(&self, _: u32) -> Result<Value, String> {
        Err("Windows job objects are required".into())
    }
}
/// Direct Rust ownership uses the same kill-on-close Windows job as the C ABI.
/// Dropping this owner ends only its own process tree, including utility children.
pub struct OwnedTool {
    inner: Arc<Tool>,
}
impl OwnedTool {
    pub fn spawn(value: Value) -> Result<Self, String> {
        let spec: Spawn = serde_json::from_value(value).map_err(|e| e.to_string())?;
        spec.validate()?;
        Ok(Self {
            inner: Tool::spawn(spec)?,
        })
    }
    pub fn status(&self) -> Result<Value, String> {
        self.inner.status()
    }
    pub fn wait(&self, millis: u32) -> Result<Value, String> {
        self.inner.wait(millis.min(30000))
    }
    pub fn cancel(&self) -> Result<Value, String> {
        self.inner.cancel()
    }
}
impl Drop for OwnedTool {
    fn drop(&mut self) {
        let _ = self.inner.cancel();
    }
}
static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<Tool>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Arc<Tool>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
fn reply(value: Result<Value, String>) -> *mut c_char {
    let value = match value {
        Ok(result) => json!({"schema_version":1,"result":result}),
        Err(error) => json!({"schema_version":1,"error":error}),
    };
    CString::new(value.to_string())
        .expect("JSON escaped")
        .into_raw()
}
fn guard(f: impl FnOnce() -> Result<Value, String> + std::panic::UnwindSafe) -> *mut c_char {
    reply(std::panic::catch_unwind(f).unwrap_or_else(|_| Err("Owned tool request failed".into())))
}
fn tool(handle: u64) -> Result<Arc<Tool>, String> {
    registry()
        .lock()
        .map_err(|_| "Tool registry unavailable")?
        .get(&handle)
        .cloned()
        .ok_or_else(|| "Unknown owned tool handle".into())
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_tools_abi_version() -> u32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_tools_info() -> *mut c_char {
    reply(Ok(
        json!({"engine":"rust-tools","schema_version":1,"abi_version":1,"owned_processes_only":true,"shell":false,"game_access":false}),
    ))
}
/// # Safety
/// data points to size readable bytes until this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_tools_spawn(data: *const u8, size: usize) -> *mut c_char {
    guard(|| {
        if data.is_null() || size == 0 || size > 1024 * 1024 {
            return Err("Invalid tool input size".into());
        }
        let spec: Spawn = serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
            .map_err(|e| e.to_string())?;
        spec.validate()?;
        let mut registry = registry().lock().map_err(|_| "Tool registry unavailable")?;
        if registry.len() >= 32 {
            return Err("Owned tool limit exceeded".into());
        }
        let tool = Tool::spawn(spec)?;
        let handle = NEXT.fetch_add(1, Ordering::Relaxed);
        let status = tool.status()?;
        registry.insert(handle, tool);
        Ok(json!({"handle":handle,"status":status}))
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_tools_status(handle: u64) -> *mut c_char {
    guard(|| tool(handle)?.status())
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_tools_wait(handle: u64, millis: u32) -> *mut c_char {
    guard(|| tool(handle)?.wait(millis.min(30_000)))
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_tools_cancel(handle: u64) -> *mut c_char {
    guard(|| tool(handle)?.cancel())
}
#[unsafe(no_mangle)]
pub extern "C" fn hoi4_tools_release(handle: u64) -> *mut c_char {
    guard(|| {
        let value = registry()
            .lock()
            .map_err(|_| "Tool registry unavailable")?
            .remove(&handle);
        if let Some(tool) = value {
            tool.cancel()?;
            Ok(json!(true))
        } else {
            Ok(json!(false))
        }
    })
}
/// # Safety
/// data is null or one live JSON result allocated by this library, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_tools_free(data: *mut c_char) {
    if !data.is_null() {
        drop(unsafe { CString::from_raw(data) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_and_unowned_requests_never_spawn_or_attach() {
        let spec = Spawn {
            schema_version: 1,
            argv: vec![],
            stdout: "a".into(),
            stderr: "b".into(),
            cwd: None,
            max_millis: 1,
            max_output_bytes: 1,
        };
        assert!(spec.validate().is_err());
        assert!(tool(u64::MAX).is_err());
        let p = unsafe { hoi4_tools_spawn(std::ptr::null(), 0) };
        let value: Value =
            serde_json::from_slice(unsafe { std::ffi::CStr::from_ptr(p) }.to_bytes()).unwrap();
        assert!(value.get("error").is_some());
        unsafe { hoi4_tools_free(p) };
    }
}
