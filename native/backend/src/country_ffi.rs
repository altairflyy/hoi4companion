use crate::{
    country,
    live_ffi::{Callbacks, Class, Read},
    reply,
};
use std::ffi::{c_char, c_void};
/// # Safety
/// Request bytes and the read-only callbacks/context stay live until return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_country(
    data: *const u8,
    size: usize,
    read: Option<Read>,
    class: Option<Class>,
    context: *mut c_void,
) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            if data.is_null() || size == 0 || size > 65536 {
                return Err("Richiesta paese non valida".into());
            }
            let request = serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
                .map_err(|e| e.to_string())?;
            let mut memory = Callbacks {
                read: read.ok_or("Lettore memoria assente")?,
                class: class.ok_or("Lettore RTTI assente")?,
                context,
                executable: None,
                error: None,
                discovery: None,
            };
            country::collect(&mut memory, request)
        })
        .unwrap_or_else(|_| Err("Lettura paese rifiutata".into())),
    )
}

/// # Safety
/// Request/context and all read-only callbacks stay live until return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_country_v2(
    data: *const u8,
    size: usize,
    read: Option<Read>,
    class: Option<Class>,
    executable: Option<Read>,
    error: Option<Read>,
    context: *mut c_void,
) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            if data.is_null() || size == 0 || size > 65536 {
                return Err("Richiesta paese non valida".into());
            }
            let request = serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
                .map_err(|e| e.to_string())?;
            let mut memory = Callbacks {
                read: read.ok_or("Lettore memoria assente")?,
                class: class.ok_or("Lettore RTTI assente")?,
                executable,
                error,
                discovery: None,
                context,
            };
            country::collect(&mut memory, request)
        })
        .unwrap_or_else(|_| Err("Lettura paese rifiutata".into())),
    )
}

/// # Safety
/// Request/context and bounded read-only callbacks stay live until return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_country_v3(
    data: *const u8,
    size: usize,
    read: Option<Read>,
    class: Option<Class>,
    executable: Option<Read>,
    error: Option<Read>,
    discovery: Option<Read>,
    context: *mut c_void,
) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            if data.is_null() || size == 0 || size > 8 * 1024 * 1024 {
                return Err("Richiesta paese non valida".into());
            }
            let request = serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
                .map_err(|e| e.to_string())?;
            let mut memory = Callbacks {
                read: read.ok_or("Lettore memoria assente")?,
                class: class.ok_or("Lettore RTTI assente")?,
                executable,
                error,
                discovery,
                context,
            };
            country::collect(&mut memory, request)
        })
        .unwrap_or_else(|_| Err("Lettura paese rifiutata".into())),
    )
}
