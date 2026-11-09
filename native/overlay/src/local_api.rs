//! Bounded loopback HTTP and the adjacent Rust vision library. No helper
//! interpreter, shell, screenshot file, base64 transport or game injection.
use serde_json::{Value, json};
use std::{
    ffi::{CStr, c_char},
    path::PathBuf,
};
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::{
            GetProcAddress, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            LoadLibraryExW,
        },
    },
    core::{PCSTR, PCWSTR},
};
type Result<T> = std::result::Result<T, String>;
#[path = "../../common/loopback.rs"]
mod loopback;
pub use loopback::Api;
type Request = unsafe extern "C" fn(u64, *const u8, usize) -> *mut c_char;
type Frame = unsafe extern "C" fn(u64, *const u8, usize, *const u8, usize) -> *mut c_char;
type Free = unsafe extern "C" fn(*mut c_char);
pub struct Library {
    module: HMODULE,
    request: Request,
    frame: Frame,
    free: Free,
}
impl Library {
    pub fn open() -> Result<Self> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let parent = exe.parent().ok_or("HUD executable has no directory")?;
        let mut paths = vec![];
        if let Some(path) = std::env::var_os("HOI4_BACKEND_LIBRARY") {
            paths.push(PathBuf::from(path));
        }
        paths.push(parent.join("hoi4companion_backend.dll"));
        if let Some(parent) = parent.parent() {
            paths.push(parent.join("hoi4companion_backend.dll"));
        }
        paths.push(PathBuf::from(
            "src/hoi4companion/_native/hoi4companion_backend.dll",
        ));
        let path = paths
            .into_iter()
            .find(|p| p.is_file())
            .ok_or("Native vision library missing; rebuild the backend")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let module = unsafe {
            LoadLibraryExW(
                PCWSTR(wide.as_ptr()),
                None,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        }
        .map_err(|e| e.to_string())?;
        let result = (|| -> Result<Self> {
            let request =
                unsafe { GetProcAddress(module, PCSTR(c"hoi4_backend_request".as_ptr().cast())) }
                    .ok_or("Native backend request ABI missing")?;
            let frame =
                unsafe { GetProcAddress(module, PCSTR(c"hoi4_backend_hud_frame".as_ptr().cast())) }
                    .ok_or("Native vision frame ABI missing; rebuild the backend")?;
            let free =
                unsafe { GetProcAddress(module, PCSTR(c"hoi4_backend_free".as_ptr().cast())) }
                    .ok_or("Native backend release ABI missing")?;
            Ok(Self {
                module,
                request: unsafe {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, Request>(request)
                },
                frame: unsafe {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, Frame>(frame)
                },
                free: unsafe {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, Free>(free)
                },
            })
        })();
        if result.is_err() {
            let _ = unsafe { FreeLibrary(module) };
        }
        result
    }
    fn reply(&self, pointer: *mut c_char) -> Result<Value> {
        if pointer.is_null() {
            return Err("Empty native vision response".into());
        }
        let bytes = unsafe { CStr::from_ptr(pointer) }.to_bytes();
        let result = if bytes.len() > 16_000_000 {
            Err("Native vision response exceeds limit".into())
        } else {
            serde_json::from_slice::<Value>(bytes).map_err(|e| e.to_string())
        };
        unsafe { (self.free)(pointer) };
        let value = result?;
        if value["schema_version"] != 1 {
            return Err("Unsupported native vision response".into());
        }
        if let Some(error) = value.get("error") {
            return Err(error.as_str().unwrap_or("Native vision failed").into());
        }
        Ok(value["result"].clone())
    }
    pub fn request(&self, op: &str, args: Value) -> Result<Value> {
        let bytes = serde_json::to_vec(&json!({"schema_version":1,"op":op,"args":args}))
            .map_err(|e| e.to_string())?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("Native visual definitions exceed limit".into());
        }
        self.reply(unsafe { (self.request)(0, bytes.as_ptr(), bytes.len()) })
    }
    pub fn frame(&self, handle: u64, args: &Value, raw: &[u8]) -> Result<Value> {
        let bytes = serde_json::to_vec(args).map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 || raw.len() > 80_000_000 {
            return Err("Native frame request exceeds limit".into());
        }
        self.reply(unsafe {
            (self.frame)(handle, bytes.as_ptr(), bytes.len(), raw.as_ptr(), raw.len())
        })
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        let _ = unsafe { FreeLibrary(self.module) };
    }
}
