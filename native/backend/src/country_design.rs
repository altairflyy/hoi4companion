//! Production and complete design observations. A missing design never erases a
//! qualified production line; the unavailable design carries its own reason.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::OnceLock;
fn profile() -> &'static Value {
    static PROFILE: OnceLock<Value> = OnceLock::new();
    PROFILE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../src/hoi4companion/design-profile-v1.json"
        ))
        .expect("bundled design profile")
    })
}
impl<M: Memory> Reader<'_, M> {
    fn design_token(&mut self, token: i64) -> Result<String> {
        let base = self.d.base;
        if self.track(base + 0x368f570, 1)? != [1] {
            return Err("Equipment design token table is not initialized.".into());
        }
        let maximum = self.integer(base + 0x368f574, 0, 1_000_000)?;
        if !(0..=maximum).contains(&token) {
            return Err("Equipment design identifier is outside the token table.".into());
        }
        let address = self
            .pointer(base + 0x368f5a0)?
            .checked_add(token as u64 * 32)
            .ok_or("Invalid equipment token address")?;
        let value = self.d.string(address)?;
        let header = self.track(address, 32)?;
        let length = u64::from_le_bytes(header[16..24].try_into().unwrap());
        let capacity = u64::from_le_bytes(header[24..32].try_into().unwrap());
        let data = if capacity == 15 {
            address
        } else {
            u64::from_le_bytes(header[..8].try_into().unwrap())
        };
        self.track(data, length as usize + 1)?;
        Ok(value)
    }
    fn design_entries(&mut self, address: u64) -> Result<Vec<[u8; 16]>> {
        let raw = self.track(address, 16)?;
        let data = u64::from_le_bytes(raw[..8].try_into().unwrap());
        let capacity = i32::from_le_bytes(raw[8..12].try_into().unwrap());
        let count = i32::from_le_bytes(raw[12..16].try_into().unwrap());
        if count < 0 || count > capacity || capacity > 256 || count > 0 && data == 0 {
            return Err(
                "Equipment design container is invalid or exceeds the admission bound.".into(),
            );
        }
        if count == 0 {
            Ok(vec![])
        } else {
            Ok(self
                .track(data, count as usize * 16)?
                .as_chunks::<16>()
                .0
                .to_vec())
        }
    }
    pub(super) fn design(&mut self, variant: u64) -> Result<Value> {
        if (self.attributes["sha256"] != profile()["sha256"]
            && self.attributes["layout_id"] != "hoi4-x64-abi-v1")
            || self.attributes["executable_bytes"] != true
        {
            return Err("Equipment design layout is not qualified for this executable.".into());
        }
        let signatures = if self.attributes["layout_id"] == "hoi4-x64-abi-v1" {
            &self.attributes["abi_design_signatures"]
        } else {
            &profile()["signatures"]
        };
        let signatures = signatures
            .as_array()
            .filter(|v| !v.is_empty())
            .ok_or("Equipment design signatures unavailable")?;
        for signature in signatures {
            let rva = signature[0].as_u64().unwrap();
            let text = signature[1].as_str().unwrap();
            let expected = (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            if self.d.memory.executable_bytes(rva, expected.len())? != expected {
                return Err("Equipment design executable signature does not match.".into());
            }
        }
        self.typed(variant, "CEquipmentVariant")?;
        self.object_pointer(variant + 0x3f0, "CEquipmentType")?;
        self.typed(variant + 0x68, "CEquipmentUpgradesInstance")?;
        if self.design_token(0x3069)? != "upgrades" || self.design_token(0x3b6b)? != "modules" {
            return Err("Equipment design field identities do not match.".into());
        }
        let mut modules = serde_json::Map::new();
        let mut slots = HashSet::new();
        for part in self.design_entries(variant + 0xb8)? {
            let slot =
                self.design_token(u32::from_le_bytes(part[..4].try_into().unwrap()) as i64)?;
            if !slots.insert(slot.clone()) {
                return Err("Duplicate equipment design module slot.".into());
            }
            let module = u64::from_le_bytes(part[8..].try_into().unwrap());
            if module == 0 {
                continue;
            }
            self.typed(module, "CEquipmentModule")?;
            if self.track(module + 0x10, 1)? != [1] {
                return Err("Equipment design module definition is not initialized.".into());
            }
            let token = self.integer(module + 8, 0, 1_000_000)?;
            modules.insert(slot, json!(self.design_token(token)?));
        }
        let mut upgrades = serde_json::Map::new();
        for part in self.design_entries(variant + 0x70)? {
            let upgrade = u64::from_le_bytes(part[..8].try_into().unwrap());
            self.typed(upgrade, "CEquipmentUpgrade")?;
            let token = self.integer(upgrade + 8, 0, 1_000_000)?;
            let key = self.design_token(token)?;
            if upgrades.insert(key, json!(part[8])).is_some() {
                return Err("Duplicate equipment design upgrade.".into());
            }
        }
        Ok(json!({"status":"observed","modules":modules,"upgrades":upgrades}))
    }
    pub(super) fn production(&mut self) -> Result<Value> {
        let status = self.object_pointer(self.country + 0xf68, "CProductionStatus")?;
        let owner = self.attributes["production_owner_offset"]
            .as_u64()
            .unwrap_or(0x290);
        if self.pointer(status + owner)? != self.country {
            return Err("Produzione di un’altra nazione".into());
        }
        let mut lines = vec![];
        for (index, line) in self.array(status + 0x58, 4096)?.into_iter().enumerate() {
            let name = self.d.kind(line)?.1;
            if name != ".?AVCMilitaryProductionLine@@" && name != ".?AVCNavalProductionLine@@" {
                return Err("Tipo linea produzione sconosciuto".into());
            }
            if self.pointer(line + 0x20)? != status {
                return Err("Linea produzione di un’altra nazione".into());
            }
            let variant = self.object_pointer(line + 0x88, "CEquipmentVariant")?;
            let equipment = self.object_pointer(variant + 0x3f0, "CEquipmentType")?;
            let key = self.d.string(equipment + 0x18)?;
            let requested = self.integer(line + 0xe8, 0, 10000)?;
            let unassigned = self.integer(line + 0x40, 0, requested)?;
            let active = self.integer(line + 0x18, 0, 10000)?;
            let damaged = self.integer(line + 0x1c, 0, 10000)?;
            if active + damaged != requested - unassigned {
                return Err("Contatori fabbriche assegnate non concordano".into());
            }
            let raw = self.track(line + 0x28, 16)?;
            let cost = i64::from_le_bytes(raw[..8].try_into().unwrap());
            let daily = i64::from_le_bytes(raw[8..].try_into().unwrap());
            if !(1..1_000_000_000_000_000).contains(&cost)
                || !(0..1_000_000_000_000_000).contains(&daily)
            {
                return Err("Costo o produzione giornaliera fuori intervallo".into());
            }
            let mut label = self.name(variant + 0x28)?;
            if label.is_empty() {
                label = key.clone()
            }
            let design = match self.design(variant) {
                Ok(value) => value,
                Err(reason) => json!({"status":"unavailable","reason":reason}),
            };
            lines.push(json!({"id":(index+1).to_string(),"equipment":key,"name":label,"category":if name.contains("Naval"){"naval"}else{"military"},"requested_factories":requested,"assigned_factories":requested-unassigned,"active_factories":active,"damaged_factories":damaged,"output_per_day":daily as f64/cost as f64,"design":design}));
        }
        let offsets = self.attributes["production_totals"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![json!(0x2b8), json!(0x318), json!(0x378)]);
        if offsets.len() != 3 {
            return Err("Invalid production totals layout".into());
        }
        let mut totals = serde_json::Map::new();
        for (key, off) in ["military", "dockyards", "civilian"].iter().zip(offsets) {
            let raw = i64::from_le_bytes(
                self.track(
                    status + off.as_u64().ok_or("Invalid production totals offset")?,
                    8,
                )?
                .try_into()
                .unwrap(),
            );
            if !(0..=1_000_000_000_000).contains(&raw) {
                return Err("Contatore fabbriche fuori intervallo".into());
            }
            totals.insert((*key).into(), json!(raw / 100000));
        }
        Ok(
            json!({"status":"observed","lines":lines,"effective_factories":totals,"reason":"Fabbriche effettivamente assegnate; richieste conservate separatamente. Uscita teorica corrente in unità/giorno."}),
        )
    }
}
