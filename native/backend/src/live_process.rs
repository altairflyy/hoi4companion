//! Native bulk acquisition through an already qualified, borrowed VM_READ
//! handle. Immutable PE bytes supply RTTI, never the mutable process image.
use super::{live, reply};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::ffi::{c_char, c_void};
use windows::Win32::{Foundation::HANDLE, System::Diagnostics::Debug::ReadProcessMemory};
type Result<T> = std::result::Result<T, String>;
use crate::pe::Pe;
pub(crate) struct Memory<'a> {
    handle: HANDLE,
    pe: Pe<'a>,
    classes: HashMap<u64, String>,
    pub(crate) witnesses: BTreeMap<(u64, usize), Vec<u8>>,
    used: usize,
    budget: usize,
    discovery_used: usize,
}
impl<'a> Memory<'a> {
    pub(crate) fn new(game: &'a crate::game_process::Game, budget: usize) -> Result<Self> {
        Ok(Self {
            handle: game.raw_handle(),
            pe: Pe::new(&game.profile.data)?,
            classes: HashMap::new(),
            witnesses: BTreeMap::new(),
            used: 0,
            budget,
            discovery_used: 0,
        })
    }
}
impl live::Memory for Memory<'_> {
    fn discovery_read(&mut self, address: u64, size: usize) -> Result<Vec<u8>> {
        self.discovery_used = self
            .discovery_used
            .checked_add(size)
            .ok_or("GUI discovery exceeds read limit")?;
        if self.discovery_used > 64 * 1024 * 1024
            || size > 65536
            || address < 0x10000
            || address
                .checked_add(size as u64)
                .is_none_or(|v| v > 0x7fff_ffff_ffff)
        {
            return Err("GUI discovery exceeds read limit".into());
        }
        let mut bytes = vec![0; size];
        let mut count = 0;
        if unsafe {
            ReadProcessMemory(
                self.handle,
                address as *const c_void,
                bytes.as_mut_ptr().cast(),
                size,
                Some(&mut count),
            )
        }
        .is_err()
            || count != size
        {
            return Err("Memoria del gioco non leggibile".into());
        }
        Ok(bytes)
    }
    fn executable_bytes(&mut self, rva: u64, size: usize) -> Result<Vec<u8>> {
        Ok(self.pe.bytes(rva, size)?.to_vec())
    }
    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>> {
        self.used = self
            .used
            .checked_add(size)
            .ok_or("Limite di lettura raggiunto")?;
        if self.used > self.budget {
            return Err("Limite di lettura raggiunto".into());
        }
        let mut bytes = vec![0; size];
        let mut count = 0;
        if unsafe {
            ReadProcessMemory(
                self.handle,
                address as *const c_void,
                bytes.as_mut_ptr().cast(),
                size,
                Some(&mut count),
            )
        }
        .is_err()
            || count != size
        {
            return Err("Memoria del gioco non leggibile".into());
        }
        if let Some(old) = self.witnesses.get(&(address, size)) {
            if old != &bytes {
                return Err("Stato cambiato durante la lettura".into());
            }
        } else {
            self.witnesses.insert((address, size), bytes.clone());
        }
        Ok(bytes)
    }
    fn class_name(&mut self, rva: u64) -> Result<String> {
        if let Some(value) = self.classes.get(&rva) {
            return Ok(value.clone());
        }
        let value = self.pe.class_name(rva)?;
        self.classes.insert(rva, value.clone());
        Ok(value)
    }
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize]);
        out.push(DIGITS[(b & 15) as usize]);
    }
    String::from_utf8(out).expect("hex ASCII")
}

/// # Safety
/// handle is a live, borrowed, qualified read-only process handle. data and pe
/// point to their declared readable lengths until return; no pointers retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_read_process(
    handle: *mut c_void,
    data: *const u8,
    size: usize,
    pe: *const u8,
    pe_size: usize,
    budget: usize,
) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            if handle.is_null()
                || handle as usize == usize::MAX
                || data.is_null()
                || size == 0
                || size > 65536
                || pe.is_null()
                || !(64..=512 * 1024 * 1024).contains(&pe_size)
                || budget == 0
                || budget > 8 * 1024 * 1024
            {
                return Err("Profilo lettura non valido".into());
            }
            let pe = Pe::new(unsafe { std::slice::from_raw_parts(pe, pe_size) })?;
            let mut memory = Memory {
                handle: HANDLE(handle),
                pe,
                classes: HashMap::new(),
                witnesses: BTreeMap::new(),
                used: 0,
                budget,
                discovery_used: 0,
            };
            let profile = serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
                .map_err(|e| e.to_string())?;
            let mut result = live::collect(&mut memory, profile)?;
            result["bytes_used"] = json!(memory.used);
            result["witnesses"] = Value::Array(
                memory
                    .witnesses
                    .iter()
                    .map(|((a, n), bytes)| json!([a, n, hex(bytes)]))
                    .collect(),
            );
            Ok(result)
        })
        .unwrap_or_else(|_| Err("Lettura nativa rifiutata".into())),
    )
}

/// # Safety
/// process is a qualified borrowed read-only handle; data points to size live
/// readable request bytes. The immutable profile generation is retained here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_read_profile_process(
    profile_handle: u64,
    process: *mut c_void,
    data: *const u8,
    size: usize,
    budget: usize,
) -> *mut c_char {
    match crate::profile::get(profile_handle) {
        Ok(profile) => unsafe {
            hoi4_backend_read_process(
                process,
                data,
                size,
                profile.data.as_ptr(),
                profile.data.len(),
                budget,
            )
        },
        Err(error) => reply(Err(error)),
    }
}

/// # Safety
/// profile/process are qualified live handles. Request bytes remain readable
/// until return. No pointer or borrowed process handle is retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoi4_backend_country_profile_process(
    profile_handle: u64,
    process: *mut c_void,
    data: *const u8,
    size: usize,
    budget: usize,
) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            if process.is_null()
                || process as usize == usize::MAX
                || data.is_null()
                || size == 0
                || size > 8 * 1024 * 1024
                || budget == 0
                || budget > 8 * 1024 * 1024
            {
                return Err("Richiesta paese non valida".into());
            }
            let profile = crate::profile::get(profile_handle)?;
            let pe = Pe::new(&profile.data)?;
            let mut memory = Memory {
                handle: HANDLE(process),
                pe,
                classes: HashMap::new(),
                witnesses: BTreeMap::new(),
                used: 0,
                budget,
                discovery_used: 0,
            };
            let mut request: crate::country::Request =
                serde_json::from_slice(unsafe { std::slice::from_raw_parts(data, size) })
                    .map_err(|e| e.to_string())?;
            // Real process qualification comes from the owned immutable profile,
            // never from caller-supplied fixture signatures or build offsets.
            if !request.attributes.is_object() {
                return Err("Invalid country attributes".into());
            }
            for key in [
                "world_rva",
                "building_database_rva",
                "ideas_database_rva",
                "equipment_database_rva",
                "logistics_days_rva",
                "modifier_definitions_rva",
                "decisions_database_rva",
                "decision_cost_cheat_rva",
                "commander_reference_registry_rva",
                "production_owner_offset",
                "production_totals",
                "military_layout",
                "country_metrics_layout",
                "foreign_interactions_layout",
                "campaign_signature",
                "campaign_signature_rva",
                "campaign_token_rva",
                "layout_id",
                "abi_functions",
                "abi_action_signatures",
                "abi_design_signatures",
                "resources_database_rva",
                "token_table_rva",
                "gui_layout",
                "political_power_offset",
                "construction_layout",
            ] {
                request.attributes.as_object_mut().unwrap().remove(key);
                if let Some(value) = profile.metadata()["selected"].get(key) {
                    request.attributes[key] = profile_value(value);
                }
            }
            request.attributes["sha256"] = profile.metadata()["sha256"].clone();
            request.attributes["image_base"] = profile.metadata()["image_base"].clone();
            request.attributes["executable_bytes"] = json!(true);
            let mut result = crate::country::collect(&mut memory, request)?;
            result["bytes_used"] = json!(memory.used);
            result["witnesses"] = Value::Array(
                memory
                    .witnesses
                    .iter()
                    .map(|((a, n), bytes)| json!([a, n, hex(bytes)]))
                    .collect(),
            );
            Ok(result)
        })
        .unwrap_or_else(|_| Err("Lettura paese rifiutata".into())),
    )
}

pub(crate) fn profile_value(value: &Value) -> Value {
    match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("$bytes") => map["$bytes"].clone(),
        Value::Object(map) if map.len() == 1 && map.contains_key("$tuple") => {
            profile_value(&map["$tuple"])
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), profile_value(v)))
                .collect(),
        ),
        Value::Array(rows) => Value::Array(rows.iter().map(profile_value).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(data: &mut [u8], offset: usize, value: u64, n: usize) {
        data[offset..offset + n].copy_from_slice(&value.to_le_bytes()[..n]);
    }
    #[test]
    fn immutable_pe_rtti_mapping_rejects_malformed_locator() {
        let mut data = vec![0; 0x2000];
        put(&mut data, 0x3c, 0x80, 4);
        data[0x80..0x84].copy_from_slice(b"PE\0\0");
        put(&mut data, 0x84, 0x8664, 2);
        put(&mut data, 0x86, 1, 2);
        put(&mut data, 0x94, 0xf0, 2);
        put(&mut data, 0x98, 0x20b, 2);
        put(&mut data, 0xb0, 0x140000000, 8);
        let section = 0x80 + 24 + 0xf0;
        put(&mut data, section + 12, 0x1000, 4);
        put(&mut data, section + 16, 0x1000, 4);
        put(&mut data, section + 20, 0x400, 4);
        put(&mut data, 0x4f8, 0x140001200, 8);
        put(&mut data, 0x600, 1, 4);
        put(&mut data, 0x60c, 0x1300, 4);
        put(&mut data, 0x614, 0x1200, 4);
        let name = b".?AVCCurrentGameState@@";
        data[0x710..0x710 + name.len()].copy_from_slice(name);
        assert_eq!(
            Pe::new(&data).unwrap().class_name(0x1100).unwrap(),
            ".?AVCCurrentGameState@@"
        );
        put(&mut data, 0x614, 0x1300, 4);
        assert!(Pe::new(&data).unwrap().class_name(0x1100).is_err());
    }
}
