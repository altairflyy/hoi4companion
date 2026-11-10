//! Patch-tolerant admission by field-bearing code and primary RTTI, never by
//! version strings. A failed or ambiguous optional group disables that domain.
use crate::pe::{Pe, number};
use regex::bytes::RegexBuilder;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
type Result<T> = std::result::Result<T, String>;
const ABI: &str = include_str!("../../../src/hoi4companion/live-abi-v1.json");

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn decode(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.len() > 131072 {
        return Err("Invalid ABI signature".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
fn sections<'a>(data: &'a [u8], pe: &Pe<'_>, executable: bool) -> Result<Vec<(u64, &'a [u8])>> {
    let header = number(data, 0x3c, 4)? as usize;
    let optional = number(data, header + 20, 2)? as usize;
    let mut out = vec![];
    for (i, &(rva, length, offset)) in pe.sections.iter().enumerate() {
        let flags = number(data, header + 24 + optional + 40 * i + 36, 4)?;
        if (flags & 0x20000000 != 0) == executable {
            out.push((
                rva,
                data.get(
                    offset
                        ..offset
                            .checked_add(length as usize)
                            .ok_or("Invalid ABI section")?,
                )
                .ok_or("Invalid ABI section")?,
            ));
        }
    }
    Ok(out)
}
fn matches(row: &Value, areas: &[(u64, &[u8])]) -> Result<u64> {
    let bytes = decode(row["bytes"].as_str().ok_or("Missing ABI bytes")?)?;
    let mask = decode(row["mask"].as_str().ok_or("Missing ABI mask")?)?;
    if bytes.len() < 16
        || mask.len() != bytes.len()
        || mask.iter().filter(|b| **b == 255).count() < 12
        || mask.iter().any(|b| ![0, 255].contains(b))
    {
        return Err("Weak ABI signature".into());
    }
    let pattern = bytes
        .iter()
        .zip(&mask)
        .map(|(b, m)| {
            if *m == 0 {
                ".".into()
            } else {
                format!("\\x{b:02x}")
            }
        })
        .collect::<String>();
    let expression = RegexBuilder::new(&pattern)
        .unicode(false)
        .dot_matches_new_line(true)
        .size_limit(32 * 1024 * 1024)
        .build()
        .map_err(|e| e.to_string())?;
    let mut found = None;
    for &(start, data) in areas {
        // Overlapping candidates are significant: never select an arbitrary one.
        let mut offset = 0;
        while let Some(m) = expression.find_at(data, offset) {
            let address = start + m.start() as u64;
            let valid = row["globals"].as_array().is_none_or(|rows| {
                rows.iter().all(|g| {
                    let Some(at) = g[0].as_u64() else {
                        return false;
                    };
                    let Some(end) = g[1].as_u64() else {
                        return false;
                    };
                    let Some(target) = g[2].as_u64() else {
                        return false;
                    };
                    data.get(m.start() + at as usize..m.start() + at as usize + 4)
                        .is_some_and(|v| {
                            (address as i128
                                + end as i128
                                + i32::from_le_bytes(v.try_into().unwrap()) as i128)
                                == target as i128
                        })
                })
            });
            if valid {
                if found.is_some() {
                    return Err("Ambiguous ABI signature".into());
                }
                found = Some(address);
            }
            offset = m.start() + 1;
        }
    }
    found.ok_or_else(|| "ABI signature not found".into())
}
fn primary_vtable(pe: &Pe<'_>, areas: &[(u64, &[u8])], name: &str) -> Result<u64> {
    let wanted = format!(".?AV{name}@@");
    let mut found = None;
    for &(start, bytes) in areas {
        for (i, chunk) in bytes.chunks_exact(8).enumerate() {
            let pointer = u64::from_le_bytes(chunk.try_into().unwrap());
            if pointer < pe.base {
                continue;
            }
            let rva = start + (i * 8) as u64 + 8;
            if pe.class_name(rva).is_ok_and(|s| s == wanted) {
                if found.is_some() {
                    return Err(format!("Ambiguous RTTI: {name}"));
                }
                found = Some(rva);
            }
        }
    }
    found.ok_or_else(|| format!("Missing RTTI: {name}"))
}
fn set(value: &mut Value, path: &str, new: Value) -> Result<()> {
    *value
        .pointer_mut(path)
        .ok_or_else(|| format!("Invalid ABI profile path: {path}"))? = new;
    Ok(())
}
pub(crate) fn qualify(data: &[u8], manifest: &Value) -> Result<Value> {
    if data.get(..2) != Some(b"MZ") {
        return Err("Invalid PE image".into());
    }
    let pe = Pe::new(data)?;
    let definition: Value = serde_json::from_str(ABI).map_err(|e| e.to_string())?;
    let mut profile = manifest["profiles"][definition["reference_profile"]
        .as_str()
        .ok_or("Missing ABI reference")?]
    .clone();
    if !profile.is_object() {
        return Err("Missing ABI reference layout".into());
    }
    let code = sections(data, &pe, true)?;
    let readonly = sections(data, &pe, false)?;
    let mut qualified = serde_json::Map::new();
    profile["abi_functions"] = json!({});
    profile["abi_action_signatures"] = json!({});
    profile["abi_design_signatures"] = json!([]);
    for key in [
        "building_database_rva",
        "resources_database_rva",
        "token_table_rva",
        "political_power_offset",
    ] {
        if profile.get(key).is_none() {
            profile[key] = Value::Null;
        }
    }
    for (group, rows) in definition["groups"]
        .as_object()
        .ok_or("Missing ABI groups")?
    {
        let trial = (|| -> Result<Value> {
            let mut result = profile.clone();
            for row in rows.as_array().ok_or("Invalid ABI group")? {
                let at = matches(row, &code)?;
                if let Some(path) = row["set"].as_str() {
                    let value = if row["capture"].is_array() {
                        let displacement = number(
                            pe.bytes(
                                at + row["capture"][0]
                                    .as_u64()
                                    .ok_or("Invalid ABI displacement")?,
                                4,
                            )?,
                            0,
                            4,
                        )? as u32 as i32;
                        let target = at as i128
                            + row["capture"][1]
                                .as_u64()
                                .ok_or("Invalid ABI instruction")?
                                as i128
                            + displacement as i128;
                        let header = number(data, 0x3c, 4)? as usize;
                        if target < 0 || target >= number(data, header + 80, 4)? as i128 {
                            return Err("ABI global outside image".into());
                        }
                        target as u64
                    } else {
                        row["constant"].as_u64().unwrap_or(at)
                    };
                    set(&mut result, path, json!(value))?;
                }
                if let Some(path) = row["signature"].as_str() {
                    let size = row["signature_size"]
                        .as_u64()
                        .unwrap_or(row["bytes"].as_str().unwrap().len() as u64 / 2)
                        as usize;
                    set(&mut result, path, json!(hex(pe.bytes(at, size)?)))?;
                }
                for (field, key) in [
                    ("function", "abi_functions"),
                    ("action", "abi_action_signatures"),
                    ("design", "abi_design_signatures"),
                ] {
                    let spec = &row[field];
                    if !spec.is_array() {
                        continue;
                    }
                    let size = spec[if field == "design" { 1 } else { 2 }]
                        .as_u64()
                        .ok_or("Invalid ABI evidence size")?
                        as usize;
                    let bytes = pe.bytes(at, size)?;
                    let evidence = if field == "function" {
                        json!([at, size, format!("{:x}", Sha256::digest(bytes))])
                    } else {
                        json!([at, hex(bytes)])
                    };
                    if field == "design" {
                        result[key].as_array_mut().unwrap().push(evidence)
                    } else {
                        let section = spec[0].as_str().ok_or("Invalid ABI evidence group")?;
                        if result[key][section].is_null() {
                            result[key][section] = json!([])
                        }
                        result[key][section].as_array_mut().unwrap().push(evidence);
                    }
                }
            }
            Ok(result)
        })();
        match trial {
            Ok(next) => {
                profile = next;
                qualified.insert(group.clone(), json!(true));
            }
            Err(error) if group == "core" => return Err(format!("Core ABI: {error}")),
            Err(error) => {
                qualified.insert(group.clone(), json!({"available":false,"reason":error}));
            }
        }
    }
    for (key, name) in definition["rtti"].as_object().ok_or("Missing ABI RTTI")? {
        profile[key] = json!(primary_vtable(
            &pe,
            &readonly,
            name.as_str().ok_or("Invalid RTTI name")?
        )?);
    }
    for (group, key) in [
        ("gui", "gui_layout"),
        ("military", "military_layout"),
        ("foreign", "foreign_interactions_layout"),
        ("metrics", "country_metrics_layout"),
    ] {
        if qualified[group] != true {
            profile.as_object_mut().unwrap().remove(key);
        }
    }
    profile["layout_id"] = definition["layout_id"].clone();
    if qualified["construction"] == true {
        profile["construction_layout"] = definition["construction_layout"].clone();
    }
    if qualified["country"] == true {
        let sections = definition["sections"]
            .as_array()
            .ok_or("Missing country capabilities")?;
        let enabled = profile["country_sections"]["$tuple"]
            .as_array_mut()
            .ok_or("Invalid country capabilities")?;
        for section in sections {
            if !enabled.contains(section) {
                enabled.push(section.clone());
            }
        }
    }
    profile["compatibility"] = json!({"method":"structural","layout_id":definition["layout_id"],"groups":qualified,"live_validated":false});
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relocation_ignores_only_explicit_masked_bytes() {
        let mut bytes = vec![0x90; 32];
        bytes[4..8].copy_from_slice(&[1, 2, 3, 4]);
        let mut mask = vec![255; 32];
        mask[4..8].fill(0);
        let row = json!({"bytes":hex(&bytes),"mask":hex(&mask)});
        bytes[4..8].copy_from_slice(&[8, 7, 6, 5]);
        assert_eq!(matches(&row, &[(0x2000, &bytes)]).unwrap(), 0x2000);
        bytes[12] = 0x91;
        assert!(matches(&row, &[(0x2000, &bytes)]).is_err());
    }
    #[test]
    fn ambiguity_and_weak_signatures_are_rejected() {
        let row = json!({"bytes":"90".repeat(32),"mask":"ff".repeat(32)});
        assert!(
            matches(&row, &[(0x1000, &[0x90; 33])])
                .unwrap_err()
                .contains("Ambiguous")
        );
        let weak = json!({"bytes":"90".repeat(32),"mask":"00".repeat(32)});
        assert!(matches(&weak, &[(0x1000, &[0x90; 32])]).is_err());
    }
    #[test]
    fn changed_writable_global_rejects_match() {
        let mut bytes = vec![0x90; 32];
        bytes[4..8].fill(0);
        let target = 0x1008u64;
        let row = json!({"bytes":hex(&bytes),"mask":"ff".repeat(32),"globals":[[4,8,target]]});
        assert!(matches(&row, &[(0x1000, &bytes)]).is_ok());
        let mut wrong = row.clone();
        wrong["globals"][0][2] = json!(target + 1);
        assert!(matches(&wrong, &[(0x1000, &bytes)]).is_err());
    }
}
