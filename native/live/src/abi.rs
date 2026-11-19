//! Patch-tolerant admission by field-bearing code and primary RTTI, never by
//! version strings. A failed or ambiguous optional group disables that domain.
use crate::pe::{Pe, number};
use regex::bytes::RegexBuilder;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
type Result<T> = std::result::Result<T, String>;
const ABI: &str = include_str!("../../../src/hoi4companion/live-abi-v1.json");
// Same native field layout, relocated for the exact HOI4 1.20.0.0 PE: globals,
// branch/RIP displacements, jump tables and frame offsets re-read from that build.
const ABI_1_20: &str = include_str!("../../../src/hoi4companion/live-abi-v1-hoi4-1.20.json");
const ABIS: [&str; 2] = [ABI, ABI_1_20];
// Optional captured globals may be absent from the older reference layout.
// Declare only reviewed root slots; set() must never create arbitrary paths.
const OPTIONAL_CAPTURE_GLOBALS: &[&str] = &[
    "ideas_database_rva",
    "modifier_definitions_rva",
    "commander_reference_registry_rva",
    "equipment_database_rva",
    "logistics_days_rva",
    "decisions_database_rva",
    "decision_cost_cheat_rva",
];

fn optional_slots(profile: &mut Value) {
    for key in [
        "building_database_rva",
        "resources_database_rva",
        "token_table_rva",
        "political_power_offset",
    ].into_iter().chain(OPTIONAL_CAPTURE_GLOBALS.iter().copied()) {
        if profile.get(key).is_none() {
            profile[key] = Value::Null;
        }
    }
}

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
    let slot = value
        .pointer_mut(path)
        .ok_or_else(|| format!("Invalid ABI profile path: {path}"))?;
    if path.strip_prefix('/').is_some_and(|key| OPTIONAL_CAPTURE_GLOBALS.contains(&key))
        && (new.as_u64().is_none() || !slot.is_null() && slot.as_u64().is_none())
    {
        return Err(format!("Invalid ABI captured global: {path}"));
    }
    *slot = new;
    Ok(())
}
/// Qualifies against every relocation set. Exactly one may admit the image;
/// two admitting definitions are ambiguous evidence and reject acquisition.
pub fn qualify(data: &[u8], manifest: &Value) -> Result<Value> {
    let mut admitted = vec![];
    let mut errors = vec![];
    for text in ABIS {
        match qualify_with(data, manifest, text) {
            Ok(profile) => admitted.push(profile),
            Err(error) => errors.push(error),
        }
    }
    match admitted.len() {
        1 => Ok(admitted.pop().unwrap()),
        0 => Err(errors.join("; ")),
        _ => Err("Ambiguous ABI relocation sets".into()),
    }
}
fn qualify_with(data: &[u8], manifest: &Value, text: &str) -> Result<Value> {
    if data.get(..2) != Some(b"MZ") {
        return Err("Invalid PE image".into());
    }
    let pe = Pe::new(data)?;
    let definition: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
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
    optional_slots(&mut profile);
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
    profile["compatibility"] = json!({"method":"structural","layout_id":definition["layout_id"],"relocation_source":definition["provenance"]["source_sha256"],"groups":qualified,"live_validated":false});
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_executable_qualifies_every_group_when_provided() {
        // Opt-in static evidence: HOI4_QUALIFY_EXE names a read-only copy of a game PE.
        let Ok(path)=std::env::var("HOI4_QUALIFY_EXE") else {return};
        let data=std::fs::read(path).unwrap();
        let manifest:Value=serde_json::from_str(include_str!("../../../src/hoi4companion/live-profiles-v1.json")).unwrap();
        let profile=qualify(&data,&manifest).unwrap();
        let groups=profile["compatibility"]["groups"].as_object().unwrap();
        let failed=groups.iter().filter(|(_,v)|**v!=true).collect::<Vec<_>>();
        assert!(failed.is_empty(),"{failed:?}");
        // The campaign token slot is a static token object whose initializer loads
        // its id with `mov edx,[rip+id]` shortly before `lea rcx,[rip+slot]`; that
        // id must be the save key 0x3b80 used next to the campaign UUID (world+0x728).
        let token=profile["campaign_token_rva"].as_u64().unwrap();
        let pe=Pe::new(&data).unwrap();
        let ids=sections(&data,&pe,true).unwrap().into_iter().flat_map(|(base,code)|{
            (7..code.len().saturating_sub(7)).filter(|&i|code[i..i+3]==[0x48,0x8d,0x0d]
                && (base+i as u64+7).wrapping_add(i32::from_le_bytes(code[i+3..i+7].try_into().unwrap()) as i64 as u64)==token)
                .filter_map(|i|(i.saturating_sub(0x30)..i).rev().find(|&j|code[j..j+2]==[0x8b,0x15]).map(|j|
                    (base+j as u64+6).wrapping_add(i32::from_le_bytes(code[j+2..j+6].try_into().unwrap()) as i64 as u64)))
                .collect::<Vec<_>>()}).map(|id|number(pe.bytes(id,4).unwrap(),0,4).unwrap()).collect::<Vec<_>>();
        assert_eq!(ids,vec![0x3b80],"campaign token slot {token:#x}");
        eprintln!("qualified by {} with {} groups",profile["compatibility"]["relocation_source"],groups.len());
    }
    #[test]
    fn relocation_sets_share_one_field_layout() {
        // Readers check evidence counts per group; a relocation set may change
        // bytes and globals only, never the groups, rows or what they set.
        let shape=|text:&str|{let d:Value=serde_json::from_str(text).unwrap();
            let groups=d["groups"].as_object().unwrap().iter().map(|(g,rows)|(g.clone(),rows.as_array().unwrap().iter().map(|r|{
                let mut r=r.clone();let o=r.as_object_mut().unwrap();
                for k in ["bytes","mask"]{assert_eq!(o[k].as_str().unwrap().len(),o["bytes"].as_str().unwrap().len());}
                o.remove("bytes");o.remove("mask");
                if let Some(g)=o.get_mut("globals"){for x in g.as_array_mut().unwrap(){x.as_array_mut().unwrap().pop();}}
                r}).collect::<Vec<_>>())).collect::<Vec<_>>();
            (d["layout_id"].clone(),d["reference_profile"].clone(),d["rtti"].clone(),d["sections"].clone(),d["construction_layout"].clone(),groups)};
        let reference=shape(ABI);
        for text in ABIS{assert_eq!(shape(text),reference);}
        let sources=ABIS.iter().map(|t|serde_json::from_str::<Value>(t).unwrap()["provenance"]["source_sha256"].clone()).collect::<std::collections::HashSet<_>>();
        assert_eq!(sources.len(),ABIS.len());
    }
    #[test]
    fn compiled_capture_paths_are_admitted_by_the_reference_layout() {
      for text in ABIS {
        let definition:Value=serde_json::from_str(text).unwrap();
        let manifest:Value=serde_json::from_str(include_str!("../../../src/hoi4companion/live-profiles-v1.json")).unwrap();
        let mut profile=manifest["profiles"][definition["reference_profile"].as_str().unwrap()].clone();
        optional_slots(&mut profile);
        for (group,rows) in definition["groups"].as_object().unwrap() {
            for row in rows.as_array().unwrap() {
                if row["capture"].is_array() {
                    let path=row["set"].as_str().unwrap();
                    set(&mut profile,path,json!(300)).unwrap_or_else(|error|panic!("{group} {path}: {error}"));
                }
            }
        }
      }
    }
    #[test]
    fn reviewed_optional_captures_have_slots_without_arbitrary_path_creation() {
        let mut profile=json!({"world_rva":100,"ideas_database_rva":200});
        optional_slots(&mut profile);
        assert_eq!(profile["world_rva"],100);
        assert_eq!(profile["ideas_database_rva"],200);
        for key in OPTIONAL_CAPTURE_GLOBALS {
            set(&mut profile,&format!("/{key}"),json!(300)).unwrap();
            assert_eq!(profile[*key],300);
        }
        let before=profile.clone();
        for path in ["/unreviewed_database_rva","/unreviewed/nested","/ideas_database_rva/nested"] {
            assert!(set(&mut profile,path,json!(400)).is_err());
            assert_eq!(profile,before);
        }
    }
    #[test]
    fn reviewed_captured_global_slots_accept_only_unsigned_addresses() {
        let mut profile=json!({});optional_slots(&mut profile);
        for invalid in [Value::Null,json!(-1),json!("300"),json!({"address":300})] {
            assert!(set(&mut profile,"/ideas_database_rva",invalid).is_err());
            assert!(profile["ideas_database_rva"].is_null());
        }
        profile["ideas_database_rva"]=json!("not an address");
        assert!(set(&mut profile,"/ideas_database_rva",json!(300)).is_err());
        assert_eq!(profile["ideas_database_rva"],"not an address");
    }
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
