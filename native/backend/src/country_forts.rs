//! Provincial fort levels and controllers; queue disappearance is no proof.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};
impl<M: Memory> Reader<'_, M> {
    pub(super) fn fortifications(&mut self) -> Result<Value> {
        let rva = self.attributes["building_database_rva"]
            .as_u64()
            .unwrap_or(0x332ee28);
        let registry = self.d.u(self.d.base + rva)?;
        self.typed(registry, "CBuildingDatabase")?;
        let mut definitions = BTreeMap::new();
        for (ordinal, definition) in self.array(registry + 0x40, 512)?.into_iter().enumerate() {
            if definition == 0 || self.d.kind(definition)?.1 != ".?AVCBuildingTemplate@@" {
                continue;
            }
            let key = self.d.string(definition + 0x270)?;
            if key == "bunker" || key == "coastal_bunker" || self.attributes["building_scopes"][&key] == "province" {
                if definitions.contains_key(&key) {
                    return Err("Duplicate provincial fort definition".into());
                }
                let slot = self.integer(definition + 0x2e0, 0, 511)?;
                if slot != ordinal as i64 {
                    return Err("Fort definition registry mismatch".into());
                }
                definitions.insert(key, (definition, slot as usize));
            }
        }
        if definitions.is_empty() {
            return Err("Provincial fort definitions unavailable".into());
        }
        let (data, count) =
            self.fort_registry(self.world + 0x2b0, 20000, "Invalid province registry")?;
        let (state_data, state_count) =
            self.fort_registry(self.world + 0x2c8, 4096, "Invalid state registry")?;
        let tags = self.pointer(self.world + 0x358)?;
        let tag_count = self.integer(self.world + 0x364, 1, 8192)? as usize;
        let mut names = BTreeMap::<i64, Option<String>>::from([(0, None)]);
        let mut states = BTreeMap::<u64, (i64, i64, i64)>::new();
        let mut controllers = serde_json::Map::new();
        let mut items = vec![];
        let mut slots = BTreeMap::new();
        for offset in (0..count).step_by(8192) {
            let raw = self
                .d
                .read(data + offset as u64 * 8, (count - offset).min(8192) * 8)?;
            for (relative, part) in raw.as_chunks::<8>().0.iter().enumerate() {
                let province = u64::from_le_bytes(*part);
                let identifier = offset + relative;
                if province == 0 {
                    continue;
                }
                self.typed(province, "CProvince")?;
                if self.d.i(province + 0xa4)? as i64 != identifier as i64 {
                    return Err("Province registry identity mismatch".into());
                }
                let state = self.d.u(province + 0xc0)?;
                if state == 0 {
                    continue;
                }
                if let std::collections::btree_map::Entry::Vacant(entry) = states.entry(state) {
                    self.typed(state, "CState")?;
                    let id = self.d.i(state + 0x58)? as i64;
                    if id <= 0
                        || id >= state_count as i64
                        || self.pointer(state_data + id as u64 * 8)? != state
                    {
                        return Err("Province/state registry mismatch".into());
                    }
                    entry.insert((
                        id,
                        self.integer(state + 0xc8, 0, 8191)?,
                        self.integer(state + 0xcc, 0, 8191)?,
                    ));
                }
                let (state_id, owner, state_controller) = states[&state];
                let controller = self.d.i(province + 0x188)? as i64;
                if !(0..=8191).contains(&controller) {
                    return Err("Invalid province controller index".into());
                }
                slots.insert(identifier, province + 0x188);
                if let std::collections::btree_map::Entry::Vacant(entry) = names.entry(controller) {
                    if controller <= 0 || controller >= tag_count as i64 {
                        return Err("Invalid province controller".into());
                    }
                    let name = self.d.string(tags + controller as u64 * 32)?;
                    if name.len() != 3
                        || !name
                            .bytes()
                            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
                    {
                        return Err("Invalid controller country tag".into());
                    }
                    entry.insert(Some(name));
                }
                controllers.insert(identifier.to_string(), json!(names[&controller]));
                if owner != self.tag as i64
                    || state_controller != self.tag as i64
                    || controller != self.tag as i64
                {
                    continue;
                }
                let status = self.typed(province + 0x190, "CBuildingStatus")?;
                let raw = self.track(status + 0x68, 12)?;
                let flags = i32::from_le_bytes(raw[..4].try_into().unwrap());
                let location = i32::from_le_bytes(raw[4..8].try_into().unwrap());
                let national = i32::from_le_bytes(raw[8..].try_into().unwrap());
                if flags < 0
                    || flags & 1 == 0
                    || location as i64 != identifier as i64
                    || national != 0
                {
                    return Err("Provincial building container identity mismatch".into());
                }
                let map = self.track(status + 0x20, 16)?;
                let members = self.d.read(status + 0x38, 16)?;
                let (mapping, map_capacity, map_count) = fort_header(&map);
                let (members, member_capacity, member_count) = fort_header(&members);
                if map_count < 0
                    || map_count > map_capacity
                    || map_capacity > 512
                    || map_count > 0 && mapping == 0
                    || member_count < 0
                    || member_count > member_capacity
                    || member_capacity > 512
                    || member_count > 0 && members == 0
                {
                    return Err("Invalid provincial building container".into());
                }
                let mut row = json!({"id":identifier,"state_id":state_id,"buildings":{}});
                let mut indices = HashSet::new();
                for (key, (definition, ordinal)) in &definitions {
                    if *ordinal >= map_count as usize {
                        return Err("Incomplete province building map".into());
                    }
                    let index = i32::from_le_bytes(
                        self.track(mapping + *ordinal as u64 * 4, 4)?
                            .try_into()
                            .unwrap(),
                    );
                    if index == -1 {
                        if ["bunker", "coastal_bunker"].contains(&key.as_str()) { row[key] = json!(0); }
                        row["buildings"][key] = json!(0);
                        continue;
                    }
                    if index < 0 || index >= member_count || !indices.insert(index) {
                        return Err("Invalid province building index".into());
                    }
                    let building = self.object_pointer(members + index as u64 * 8, "CBuilding")?;
                    let raw = self.track(building + 0x1d8, 16)?;
                    if u64::from_le_bytes(raw[..8].try_into().unwrap()) != status
                        || u64::from_le_bytes(raw[8..].try_into().unwrap()) != *definition
                    {
                        return Err("Provincial fort owner/template mismatch".into());
                    }
                    let level =
                        i16::from_le_bytes(self.track(building + 0x40, 2)?.try_into().unwrap());
                    if !(0..=1000).contains(&level) {
                        return Err("Invalid provincial fort level".into());
                    }
                    if ["bunker", "coastal_bunker"].contains(&key.as_str()) { row[key] = json!(level); }
                    row["buildings"][key] = json!(level);
                }
                items.push(row);
            }
        }
        let mut watched = items
            .iter()
            .map(|row| row["id"].as_u64().unwrap() as usize)
            .collect::<BTreeSet<_>>();
        if let Some(topology) = self.attributes["province_neighbors"].as_object() {
            for row in &items {
                if let Some(neighbors) = topology
                    .get(&row["id"].to_string())
                    .and_then(Value::as_array)
                {
                    watched.extend(
                        neighbors
                            .iter()
                            .filter_map(Value::as_u64)
                            .map(|v| v as usize),
                    );
                }
            }
        }
        for id in watched {
            if let Some(address) = slots.get(&id) {
                self.track(*address, 4)?;
            }
        }
        Ok(
            json!({"status":"observed","items":items,"controllers":controllers,"reason":"Built provincial levels; exact mapped absence is zero. Queue removal is not completion."}),
        )
    }
    fn fort_registry(&mut self, address: u64, maximum: i32, error: &str) -> Result<(u64, usize)> {
        let raw = self.track(address, 16)?;
        let (data, capacity, count) = fort_header(&raw);
        if count <= 0 || count > capacity || capacity > maximum || data == 0 {
            return Err(error.into());
        }
        Ok((data, count as usize))
    }
}
fn fort_header(raw: &[u8]) -> (u64, i32, i32) {
    (
        u64::from_le_bytes(raw[..8].try_into().unwrap()),
        i32::from_le_bytes(raw[8..12].try_into().unwrap()),
        i32::from_le_bytes(raw[12..].try_into().unwrap()),
    )
}
