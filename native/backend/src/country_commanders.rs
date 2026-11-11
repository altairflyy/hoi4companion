//! Current commander assignments require native role, owner and reciprocal target.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value, json};
use std::collections::HashSet;

impl<M: Memory> Reader<'_, M> {
    // CReference lookup for the small native type registries. The immutable ABI
    // profile qualifies the dispatch, hash and probe layout; no process call runs.
    fn commander_target(&mut self, handle: &[u8]) -> Result<u64> {
        let kind = u32::from_le_bytes(handle[..4].try_into().unwrap());
        let sequence = u32::from_le_bytes(handle[4..].try_into().unwrap());
        if kind == 0 || kind >= 100 || sequence == 0 {
            return Err("Invalid commander target identity".into());
        }
        let registry_rva = self.attributes["commander_reference_registry_rva"]
            .as_u64().filter(|n| *n > 0).ok_or("Commander reference registry not qualified")?;
        let registry = self.pointer(self.d.base + registry_rva + kind as u64 * 8)?;
        let head = self.track(registry + 8, 17)?;
        let data = u64::from_le_bytes(head[..8].try_into().unwrap());
        let mask = i32::from_le_bytes(head[12..16].try_into().unwrap());
        let spill = head[16] as u64;
        if !(0..=2_097_151).contains(&mask) || !(mask as u32 + 1).is_power_of_two() {
            return Err("Invalid commander reference registry".into());
        }
        let hash = ((kind as u64) << 32 | sequence as u64).wrapping_mul(0x49631a45);
        let bucket = (hash ^ (hash >> 32)) & mask as u64;
        let slots = mask as u64 + 1 + spill;
        for distance in 1..=255u64 {
            let slot = bucket + distance - 1;
            if slot >= slots { break; }
            let row = self.track(data + slot * 24, 24)?;
            if distance > row[4] as u64 { break; }
            if row[8..16] == *handle {
                let target = u64::from_le_bytes(row[16..24].try_into().unwrap());
                if self.track(target + 8, 8)? != handle {
                    return Err("Commander target reference identity mismatch".into());
                }
                return Ok(target);
            }
        }
        Err("Commander target reference unavailable".into())
    }

    pub(super) fn commander_assignments(&mut self) -> Result<Value> {
        self.function_signatures("commander_assignments", "Commander assignments are not qualified")?;
        if !self.attributes["abi_functions"]["commander_assignments"].as_array().is_some_and(|a| a.len() == 11) {
            return Err("Missing commander assignment evidence".into());
        }
        self.typed(self.country, "CCountry")?;
        if self.integer(self.country + 8, 1, 8191)? != self.tag as i64 {
            return Err("Commander country identity mismatch".into());
        }
        for (token, name) in [(0x34e2,"field_marshal"), (0x34c7,"corps_commander"), (0x30b7,"navy_leader")] {
            if self.token_name(token)? != name { return Err("Commander role token mismatch".into()); }
        }
        let manager = self.object_pointer(self.country + 0xff0, "CCountryCharacters")?;
        if self.pointer(manager + 8)? != self.country { return Err("Commander roster owner mismatch".into()); }
        let mut characters = HashSet::new();
        let mut leaders = HashSet::new();
        let mut targets = HashSet::new();
        let mut items = vec![];
        for row in self.inline(manager + 0x10, 16, 4096)? {
            let flag = self.track(row + 10, 1)?[0];
            if flag > 1 { return Err("Invalid commander role flag".into()); }
            if flag == 0 { continue; }
            let character = self.object_pointer(row, "CCharacter")?;
            if self.integer(character + 0x68, 1, 8191)? != self.tag as i64 { return Err("Commander belongs to another country".into()); }
            let token = self.integer(character + 0x18, 1, 1_000_000)?;
            let id = self.token_name(token)?;
            let leader = self.pointer(character + 0xa8)?;
            let role = self.integer(leader + 0xe7c, 0, 2)?;
            self.typed(leader, if role == 2 { "CNavyLeader" } else { "CArmyLeader" })?;
            if self.integer(leader + 0x120, 1, 8191)? != self.tag as i64 { return Err("Unit leader belongs to another country".into()); }
            let identity = self.track(leader + 8, 8)?;
            if identity[..4] == [0;4] || identity[4..] == [0;4] || !characters.insert(id.clone()) || !leaders.insert(identity.clone()) {
                return Err("Duplicate commander identity".into());
            }
            let handle = self.track(leader + if role == 2 { 0xf60 } else { 0x1048 }, 8)?;
            if handle == [0;8] { continue; }
            let target = self.commander_target(&handle)?;
            let (role_name, kind, label) = if role == 2 {
                self.typed(target, "CFleet")?;
                if self.pointer(target + 0xa8)? != self.country || self.track(target + 0xb0,8)? != identity {
                    return Err("Fleet commander owner or reciprocity mismatch".into());
                }
                ("navy_leader", "fleet", self.name(target + 0xe0)?)
            } else {
                self.typed(target, if role == 0 { "CArmyGroup" } else { "COrdersGroup" })?;
                if self.integer(target + 0x3c,1,8191)? != self.tag as i64 || self.pointer(target + 0x88)? != leader {
                    return Err("Army commander owner or reciprocity mismatch".into());
                }
                (if role == 0 {"field_marshal"} else {"corps_commander"}, if role == 0 {"army_group"} else {"army"}, self.name(target + 0x160)?)
            };
            if !targets.insert(handle.clone()) { return Err("Duplicate commander target assignment".into()); }
            items.push(json!({"commander_id":id,"role":role_name,"target_kind":kind,"instance_id":super::hex(&handle),"name":label}));
        }
        Ok(json!({"schema_version":1,"status":"observed","source":"native_reciprocal_commander_assignments","items":items}))
    }
}

#[cfg(test)]
#[path = "country_commanders_tests.rs"]
mod tests;
