use super::{live, reply};
use std::ffi::{c_char, c_void};
pub(crate) type Read = unsafe extern "C" fn(*mut c_void, u64, *mut u8, usize) -> i32;
pub(crate) type Class = unsafe extern "C" fn(*mut c_void, u64, *mut u8, usize) -> i32;
pub(crate) struct Callbacks {
    pub(crate) read: Read,
    pub(crate) class: Class,
    pub(crate) context: *mut c_void,
    pub(crate) executable: Option<Read>,
    pub(crate) error: Option<Read>,
    pub(crate) discovery: Option<Read>,
}
impl Callbacks {
    fn error_message(&self, code: i32, fallback: &str) -> String {
        let Some(error) = self.error else {
            return fallback.into();
        };
        let mut bytes = vec![0; 4096];
        let index = code.unsigned_abs().saturating_sub(1) as u64;
        let n = unsafe { error(self.context, index, bytes.as_mut_ptr(), bytes.len()) };
        if n <= 0 || n as usize > bytes.len() {
            return fallback.into();
        }
        String::from_utf8(bytes[..n as usize].to_vec()).unwrap_or_else(|_| fallback.into())
    }
}
impl live::Memory for Callbacks {
    fn discovery_read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, String> {
        let mut bytes = vec![0; size];
        let code = unsafe {
            self.discovery.unwrap_or(self.read)(self.context, address, bytes.as_mut_ptr(), size)
        };
        if code != 0 {
            return Err(self.error_message(code, "GUI discovery read failed"));
        }
        Ok(bytes)
    }
    fn fixture_transport(&self) -> bool {
        true
    }
    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>, String> {
        let mut data = vec![0; size];
        let code = unsafe { (self.read)(self.context, address, data.as_mut_ptr(), size) };
        if code != 0 {
            return Err(self.error_message(code, "Lettura memoria non riuscita"));
        }
        Ok(data)
    }
    fn class_name(&mut self, rva: u64) -> Result<String, String> {
        let mut data = [0; 256];
        let size = unsafe { (self.class)(self.context, rva, data.as_mut_ptr(), data.len()) };
        if size <= 0 || size as usize > data.len() {
            return Err(self.error_message(size, "RTTI non riconosciuto"));
        }
        String::from_utf8(data[..size as usize].to_vec()).map_err(|e| e.to_string())
    }
    fn executable_bytes(&mut self, rva: u64, size: usize) -> Result<Vec<u8>, String> {
        let function = self.executable.ok_or("Executable bytes unavailable")?;
        if size > 65536 {
            return Err("Executable read exceeds limit".into());
        }
        let mut data = vec![0; size];
        let code = unsafe { function(self.context, rva, data.as_mut_ptr(), size) };
        if code != 0 {
            return Err(self.error_message(code, "Executable bytes unavailable"));
        }
        Ok(data)
    }
}
/// # Safety
/// data points to size readable bytes. Callbacks/context remain live until
/// return and write only within the supplied output buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_read_core(
    data: *const u8,
    size: usize,
    read: Option<Read>,
    class: Option<Class>,
    context: *mut c_void,
) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            if data.is_null() || size == 0 || size > 65536 {
                return Err("Profilo lettura non valido".into());
            }
            let mut memory = Callbacks {
                read: read.ok_or("Lettore memoria assente")?,
                class: class.ok_or("Lettore RTTI assente")?,
                context,
                executable: None,
                error: None,
                discovery: None,
            };
            let profile = serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
                .map_err(|e| e.to_string())?;
            live::collect(&mut memory, profile)
        })
        .unwrap_or_else(|_| Err("Decodifica stato rifiutata".into())),
    )
}
