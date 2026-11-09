//! Qualified military observations, doctrine membership and effective support.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::OnceLock;
fn functions() -> &'static Value {
    static PROFILE: OnceLock<Value> = OnceLock::new();
    PROFILE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../src/hoi4companion/country-functions-v1.json"
        ))
        .expect("bundled function profiles")
    })
}
fn decode_hex(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return Err("Invalid executable signature".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
impl<M: Memory> Reader<'_, M> {
    fn military_layout(&mut self) -> Result<Value> {
        let layout = self.attributes["military_layout"].clone();
        if !layout.is_object() {
            return Err("Military state layout is not qualified for this executable.".into());
        }
        for signature in layout["signatures"]
            .as_array()
            .ok_or("Invalid military signatures")?
        {
            let rva = signature[0].as_u64().ok_or("Invalid military signature")?;
            let expected = decode_hex(signature[1].as_str().ok_or("Invalid military signature")?)?;
            if self.d.memory.executable_bytes(rva, expected.len())? != expected {
                return Err("Military state signature does not match the executable.".into());
            }
        }
        self.typed(self.country, "CCountry")?;
        if self.integer(self.country + 8, 0, 8191)? != self.tag as i64 {
            return Err("Country identity changed while reading military state.".into());
        }
        Ok(layout)
    }
    pub(super) fn experience(&mut self) -> Result<Value> {
        let layout = self.military_layout()?;
        let offset = layout["experience_offset"]
            .as_u64()
            .ok_or("Invalid experience layout")?;
        let status = self.object_pointer(self.country + offset, "CCountryExperienceStatus")?;
        if self.pointer(status + 8)? != self.country {
            return Err("Experience belongs to another country.".into());
        }
        let mut result = json!({"status":"observed"});
        for (key, offset) in [("army", 0x10), ("navy", 0x28), ("air", 0x40)] {
            let raw = i64::from_le_bytes(self.track(status + offset, 8)?.try_into().unwrap());
            if !(0..=10_000_000 * 32768i64).contains(&raw) {
                return Err("Military experience is outside its qualified range.".into());
            }
            result[key] = json!(raw as f64 / 32768.);
        }
        Ok(result)
    }
    pub(super) fn diplomacy(&mut self) -> Result<Value> {
        let layout = self.military_layout()?;
        let offset = layout["diplomacy_offset"]
            .as_u64()
            .ok_or("Invalid diplomacy layout")?;
        let status = self.object_pointer(self.country + offset, "CDiplomacyStatus")?;
        if self.pointer(status + 0x2d0)? != self.country {
            return Err("Diplomacy belongs to another country.".into());
        }
        let mut at_war = !self.array(status + 0x98, 4096)?.is_empty();
        if !at_war {
            for relation in self.array(status + 0x20, 4096)? {
                if self.integer(relation + 8, 0, 65535)? == 0x380a {
                    self.typed(relation, "CWarRelation")?;
                    at_war = true
                }
            }
        }
        let mut result = json!({"status":"observed","at_war":at_war});
        if self.attributes["foreign_interactions_layout"].is_object() {
            result["interactions"] = match self.foreign_relations(status) {
                Ok(value) => value,
                Err(reason) => json!({"status":"unavailable","reason":reason}),
            };
        }
        Ok(result)
    }
    fn function_signatures(&mut self, section: &str, error: &str) -> Result<()> {
        // Fixture overrides are admitted only on the explicit callback test
        // transport. A real process always uses the compiled immutable profile.
        let signatures = if self.d.memory.fixture_transport()
            && self.attributes["function_signatures"].is_array()
        {
            self.attributes["function_signatures"].clone()
        } else {
            functions()[section].clone()
        };
        for row in signatures.as_array().ok_or("Invalid function profile")? {
            let rva = row[0].as_u64().ok_or("Invalid function RVA")?;
            let size = row[1].as_u64().ok_or("Invalid function size")? as usize;
            if size > 65536 {
                return Err("Function profile exceeds read limit".into());
            }
            let digest = format!(
                "{:x}",
                Sha256::digest(self.d.memory.executable_bytes(rva, size)?)
            );
            if Some(digest.as_str()) != row[2].as_str() {
                return Err(error.into());
            }
        }
        Ok(())
    }
    fn country_mapping(
        &mut self,
        identity_error: &str,
        mapping_error: &str,
    ) -> Result<(Vec<i64>, Vec<u64>)> {
        self.typed(self.world, "CCurrentGameState")?;
        self.typed(self.country, "CCountry")?;
        let rva = self.attributes["world_rva"]
            .as_u64()
            .ok_or("Missing world RVA")?;
        if self.pointer(self.d.base + rva)? != self.world
            || self.integer(self.world + 0x520, 0, 8191)? != self.tag as i64
            || self.integer(self.country + 8, 0, 8191)? != self.tag as i64
        {
            return Err(identity_error.into());
        }
        let mapping = self
            .d
            .array(self.world + 0x340, 4, 8192)?
            .into_iter()
            .map(|v| v as i64)
            .collect::<Vec<_>>();
        self.track(self.world + 0x340, 16)?;
        if !mapping.is_empty() {
            let data = self.pointer(self.world + 0x340)?;
            self.track(data, mapping.len() * 4)?;
        }
        let countries = self.array(self.world + 0x310, 8192)?;
        if mapping
            .get(self.tag as usize)
            .is_none_or(|v| *v < 0 || countries.get(*v as usize) != Some(&self.country))
        {
            return Err(mapping_error.into());
        }
        Ok((mapping, countries))
    }
    fn inline(&mut self, address: u64, stride: u64, maximum: i32) -> Result<Vec<u64>> {
        let raw = self.track(address, 16)?;
        let data = u64::from_le_bytes(raw[..8].try_into().unwrap());
        let capacity = i32::from_le_bytes(raw[8..12].try_into().unwrap());
        let count = i32::from_le_bytes(raw[12..].try_into().unwrap());
        if count < 0 || count > capacity || capacity > maximum || count > 0 && data == 0 {
            return Err("Invalid doctrine membership container.".into());
        }
        (0..count)
            .map(|i| {
                data.checked_add(i as u64 * stride)
                    .ok_or_else(|| "Invalid doctrine membership address".into())
            })
            .collect()
    }
    fn selected_doctrine(
        &mut self,
        definition: u64,
        kind: &str,
        identities: &mut HashSet<String>,
        rows: &mut Vec<Value>,
    ) -> Result<()> {
        self.typed(definition, &format!("{kind}@NDoctrines"))?;
        if self.track(self.d.base + 0x368f570, 1)? != [1] {
            return Err("Doctrine atom table is not initialized.".into());
        }
        let maximum = self.integer(self.d.base + 0x368f574, 0, 1_000_000)?;
        let token = self.integer(definition + 8, 1, maximum)?;
        let atoms = self.pointer(self.d.base + 0x368f5a0)?;
        let key = self.d.string(atoms + token as u64 * 32)?;
        if !identities.insert(key.clone()) {
            return Err("Duplicate selected doctrine identity.".into());
        }
        rows.push(json!({"id":key}));
        Ok(())
    }
    pub(super) fn doctrines(&mut self) -> Result<Value> {
        if self.attributes["sha256"] != functions()["sha256"] {
            return Err("Doctrine selections are not qualified for this executable.".into());
        }
        self.function_signatures(
            "doctrines",
            "Doctrine signature does not match the executable.",
        )?;
        let (mapping, _) = self.country_mapping(
            "Doctrine session identity changed.",
            "Doctrine country mapping is invalid.",
        )?;
        let system = self.object_pointer(self.world + 0x400, "CDoctrineSystem@NDoctrines")?;
        let statuses = self.inline(system + 8, 0xa0, 8192)?;
        let status = *statuses
            .get(mapping[self.tag as usize] as usize)
            .ok_or("Doctrine country record is missing.")?;
        self.typed(status, "CCountryDoctrineStatus@NDoctrines")?;
        if self.integer(status + 8, 0, 8191)? != self.tag as i64 {
            return Err("Doctrine record belongs to another country.".into());
        }
        let mut rows = vec![];
        let mut identities = HashSet::new();
        let mut templates = HashSet::new();
        for folder in self.inline(status + 0x10, 0x50, 64)? {
            self.typed(folder, "CFolderStatus@NDoctrines")?;
            let template = self.object_pointer(folder + 8, "CFolderTemplate@NDoctrines")?;
            if !templates.insert(template) {
                return Err("Duplicate doctrine folder.".into());
            }
            let grand = self.pointer(folder + 0x10)?;
            if grand != 0 {
                self.selected_doctrine(grand, "CGrandDoctrineTemplate", &mut identities, &mut rows)?
            }
            for track in self.inline(folder + 0x18, 0x60, 256)? {
                self.typed(track, "CTrackStatus@NDoctrines")?;
                let sub = self.pointer(track + 8)?;
                if sub != 0 {
                    if grand == 0 {
                        return Err("Selected subdoctrine has no selected grand doctrine.".into());
                    }
                    self.selected_doctrine(
                        sub,
                        "CSubDoctrineTemplate",
                        &mut identities,
                        &mut rows,
                    )?;
                }
            }
        }
        Ok(json!({"status":"observed","items":rows}))
    }
    fn metric64(&mut self, address: u64, limit: i64) -> Result<i128> {
        let value = i64::from_le_bytes(self.track(address, 8)?.try_into().unwrap());
        if value.unsigned_abs() > limit as u64 {
            return Err("Country metric exceeds its qualified arithmetic range.".into());
        }
        Ok(value as i128)
    }
    fn history_sum(&mut self) -> Result<i128> {
        let history = self.object_pointer(self.country + 0xfc8, "CLoopHistory")?;
        let ring = self.object_pointer(history + 0x10, "CLoopHistoryContainer")?;
        if self.pointer(ring + 0x38)? != history {
            return Err("Country metric history has another owner.".into());
        }
        let rows = self.array(ring + 8, 8192)?;
        let raw = self.track(ring + 0x20, 8)?;
        let capacity = i32::from_le_bytes(raw[..4].try_into().unwrap());
        let head = i32::from_le_bytes(raw[4..].try_into().unwrap());
        let wrapped = self.track(ring + 0x28, 1)?[0];
        if capacity as usize != rows.len()
            || wrapped > 1
            || if capacity > 0 {
                head < 0 || head >= capacity
            } else {
                head != 0 || wrapped != 0
            }
        {
            return Err("Country metric history ring is invalid.".into());
        }
        let mut total = 0i128;
        for index in 0..if wrapped == 1 { capacity } else { head } {
            let row = rows[if wrapped == 1 {
                ((head + index) % capacity) as usize
            } else {
                index as usize
            }];
            if self.pointer(row + 0x18)? != history {
                return Err("Country metric history row has another owner.".into());
            }
            let raw = self.track(row, 16)?;
            let data = u64::from_le_bytes(raw[..8].try_into().unwrap());
            let cap = i32::from_le_bytes(raw[8..12].try_into().unwrap());
            let n = i32::from_le_bytes(raw[12..].try_into().unwrap());
            if data == 0 || n < 2 || n > cap || cap > 512 {
                return Err("Country metric history row is incomplete.".into());
            }
            total = (total + self.metric64(data + 8, 10_000_000_000)?)
                .clamp(-0x53e2d620fb40, 0x53e2d620fb40);
            if total.abs() > 10_000_000_000 {
                return Err("Country metric history exceeds its qualified range.".into());
            }
        }
        Ok(total)
    }
    fn metric_modifier(&mut self, index: u32) -> Result<i128> {
        let raw = self.track(self.country + 0x5b8, 16)?;
        let data = u64::from_le_bytes(raw[..8].try_into().unwrap());
        let capacity = i32::from_le_bytes(raw[8..12].try_into().unwrap());
        let count = i32::from_le_bytes(raw[12..].try_into().unwrap());
        if count < 0 || count > capacity || capacity > 4096 || count > 0 && data == 0 {
            return Err("Country metric modifier container is invalid.".into());
        }
        let raw = if count > 0 {
            self.track(data, count as usize * 16)?
        } else {
            vec![]
        };
        let mut last = None;
        let mut found = None;
        for row in raw.as_chunks::<16>().0 {
            let id = u32::from_le_bytes(row[..4].try_into().unwrap());
            if last.is_some_and(|v| id <= v) {
                return Err("Country metric modifiers are not unique and ordered.".into());
            }
            last = Some(id);
            if id == index {
                found = Some(i64::from_le_bytes(row[8..].try_into().unwrap()) as i128)
            }
        }
        let Some(value) = found else { return Ok(0) };
        let definitions = self.pointer(self.d.base + 0x33dd1d0)?;
        let flags = u32::from_le_bytes(
            self.track(definitions + index as u64 * 0x78 + 0x60, 4)?
                .try_into()
                .unwrap(),
        );
        let context = u32::from_le_bytes(
            self.track(definitions + index as u64 * 0x78 + 0x68, 4)?
                .try_into()
                .unwrap(),
        );
        if context != 0 {
            return Err("Country metric modifier requires an unqualified native context.".into());
        }
        if value.abs() > 10_000_000_000 {
            return Err("Country metric modifier is outside its qualified range.".into());
        }
        Ok(if flags & 0x10 != 0 {
            value.clamp(0, 100000)
        } else {
            value
        })
    }
    fn war_addition(&mut self, mapping: &[i64], countries: &[u64]) -> Result<i128> {
        let diplomacy = self.object_pointer(self.country + 0xf88, "CDiplomacyStatus")?;
        if self.pointer(diplomacy + 0x2d0)? != self.country {
            return Err("Country metric diplomacy has another owner.".into());
        }
        let mut at_war = false;
        let mut defensive = false;
        let mut offensive = false;
        for relation in self.array(diplomacy + 0x20, 4096)? {
            if self.integer(relation + 8, 0, 65535)? != 0x380a {
                continue;
            }
            self.typed(relation, "CWarRelation")?;
            let own = self.integer(relation + 0x10, 1, 8191)? as usize;
            let other = self.integer(relation + 0x14, 1, 8191)? as usize;
            let swapped = self.track(relation + 0x158, 1)?[0];
            let valid = |i: usize| {
                mapping
                    .get(i)
                    .is_some_and(|v| *v >= 0 && (*v as usize) < countries.len())
            };
            if swapped > 1
                || !valid(own)
                || !valid(other)
                || ![mapping[own], mapping[other]].contains(&mapping[self.tag as usize])
                || mapping[own] == mapping[other]
            {
                return Err("Country metric war relation identity is invalid.".into());
            }
            let (defender, attacker) = if swapped == 1 {
                (own, other)
            } else {
                (other, own)
            };
            defensive |= mapping[defender] == mapping[self.tag as usize];
            offensive |= mapping[attacker] == mapping[self.tag as usize];
            let right = self.typed(countries[mapping[other] as usize], "CCountry")?;
            let tag = self.integer(right + 8, 1, 8191)? as usize;
            if mapping.get(tag) != Some(&mapping[other]) {
                return Err("Country metric war opponent identity is invalid.".into());
            }
            let opponent = self.object_pointer(right + 0xf88, "CDiplomacyStatus")?;
            if self.pointer(opponent + 0x2d0)? != right {
                return Err("Country metric opponent diplomacy has another owner.".into());
            }
            at_war |= self.integer(opponent + 0x1f0, -2_000_000_000, 2_000_000_000)? <= 0;
        }
        let mut result = 0;
        if at_war && defensive {
            result += self.metric64(self.d.base + 0x33e2c48, 10_000_000)?
        }
        if at_war && offensive {
            result += self.metric64(self.d.base + 0x33e2ce0, 10_000_000)?
        }
        Ok(result)
    }
    pub(super) fn metrics(&mut self) -> Result<Value> {
        if self.attributes["sha256"] != functions()["sha256"]
            || self.attributes["country_metrics_layout"]
                != json!({"version":1,"path":"effective-war-support"})
        {
            return Err("Effective country metrics are not qualified for this executable.".into());
        }
        self.function_signatures(
            "metrics",
            "Country metric signature does not match the executable.",
        )?;
        let (mapping, countries) = self.country_mapping(
            "Country metric session identity changed.",
            "Country metric belongs to another country.",
        )?;
        let mut value = self.metric64(self.country + 0x10d8, 10_000_000_000)?
            + self.war_addition(&mapping, &countries)?;
        let threat = self.object_pointer(self.world + 0x6b0, "CWorldThreat")?;
        let tension = self.metric64(threat + 8, 100000)?;
        if tension < 0 {
            return Err("World tension is outside its qualified range.".into());
        }
        value += tension * self.metric64(self.d.base + 0x33e2d70, 10_000_000)? / 100000;
        value += from_q15(self.metric64(self.country + 0x14c8, 10_000_000_000)?);
        let history = (to_q15(self.history_sum()?)
            * self.metric64(self.d.base + 0x33df5f0, 32768 * 1000)?)
            >> 15;
        let limit = self.metric64(self.d.base + 0x33df678, 10_000_000)?;
        if limit < 0 {
            return Err("War support history limit is invalid.".into());
        }
        value += from_q15(history).clamp(-limit, limit);
        for off in [0x14d0, 0x14d8, 0x14e0] {
            value += from_q15(self.metric64(self.country + off, 10_000_000_000)?)
        }
        value += self.metric_modifier(149)?;
        let upper = self.metric64(self.d.base + 0x33e4528, 10_000_000_000)?;
        let lower = self.metric64(self.d.base + 0x33e45d8, 10_000_000_000)?;
        if (lower, upper) != (0, 100000) {
            return Err("War support bounds are outside the qualified 0..1 scale.".into());
        }
        Ok(json!({"status":"observed","war_support":value.clamp(lower,upper) as f64/100000.}))
    }
}
fn to_q15(value: i128) -> i128 {
    value / 100000 * 32768 + (value % 100000) * 32768 / 100000
}
fn from_q15(value: i128) -> i128 {
    let integer = value >> 15;
    integer * 100000 + ((value - integer * 32768) * 100000 + 16384) / 32768
}
