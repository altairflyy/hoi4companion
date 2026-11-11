//! Native targeted decision instances. Availability is not execution, and the
//! targeted enum deliberately differs from the ordinary timed decision enum.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value, json};
use std::collections::HashSet;

impl<M: Memory> Reader<'_, M> {
    pub(super) fn targeted_decisions(&mut self, manager: u64) -> Result<Value> {
        self.function_signatures("targeted_decisions", "Targeted decision records are not qualified")?;
        if !self.attributes["abi_functions"]["targeted_decisions"].as_array().is_some_and(|a| a.len() == 3) {
            return Err("Missing targeted decision serializer evidence".into());
        }
        let mut records = vec![];
        let mut instances = HashSet::new();
        let mut state_registry: Option<(u64, Vec<u64>)> = None;
        for (offset, timed) in [(0xe8, false), (0x100, true)] {
            for instance in self.array(manager + offset, 4096)? {
                self.typed(instance, "CTargetedDecision")?;
                if !instances.insert(instance) { return Err("Duplicate targeted decision instance".into()); }
                let definition = self.pointer(instance + 0x10)?;
                let id = self.decision_id(definition)?;
                let target = instance + 0x18;
                if self.d.kind(target)?.1 != ".?AUSTargetedDecisionTarget@@" {
                    return Err("Invalid targeted decision target RTTI".into());
                }
                self.track(target, 8)?;
                let country = self.integer(target + 8, 0, 8191)?;
                let state = self.integer(target + 12, 0, 4095)?;
                if (country > 0) == (state > 0) { return Err("Ambiguous or missing decision target".into()); }
                let target = if country > 0 {
                    let (mapping, countries) = self.country_mapping("Decision country changed", "Invalid decision country mapping")?;
                    let slot = *mapping.get(country as usize).ok_or("Unknown decision country target")?;
                    let pointer = *countries.get(usize::try_from(slot).map_err(|_| "Unknown decision country target")?).ok_or("Unknown decision country target")?;
                    self.typed(pointer, "CCountry")?;
                    let canonical = self.integer(pointer + 8, 1, 8191)?;
                    if mapping.get(canonical as usize) != Some(&slot) { return Err("Decision target reciprocal identity mismatch".into()); }
                    let names = self.pointer(self.world + 0x358)?;
                    let count = self.integer(self.world + 0x364, 1, 8192)?;
                    if country >= count { return Err("Decision country target outside registry".into()); }
                    let name = self.d.string(names + country as u64 * 32)?;
                    if name.len() != 3 || !name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) { return Err("Invalid decision target tag".into()); }
                    json!({"kind":"country","id":name})
                } else {
                    if state_registry.is_none() {
                        self.track(self.world + 0x2c8, 16)?;
                        let data = self.pointer(self.world + 0x2c8)?;
                        state_registry = Some((data, self.d.array(self.world + 0x2c8, 8, 4096)?));
                    }
                    let (data, states) = state_registry.as_ref().unwrap();
                    let pointer = *states.get(state as usize).ok_or("Unknown decision state target")?;
                    self.track(*data + state as u64 * 8, 8)?;
                    self.typed(pointer, "CState")?;
                    if self.integer(pointer + 0x58, 1, 4095)? != state { return Err("Decision state target identity mismatch".into()); }
                    json!({"kind":"state","id":state})
                };
                let native = self.integer(instance + 0x28, 0, 4)? as usize;
                let ignored = self.track(instance + 0x2c, 1)?[0];
                if ignored > 1 { return Err("Invalid targeted decision ignore flag".into()); }
                let days = self.integer(instance + 0x30, -1_000_000, 1_000_000)?;
                let phase = [if timed {"active"} else {"available"},"completed","cooldown","failed","aborted"][native];
                records.push(json!({"id":id,"instance_id":format!("{instance:x}"),"record_kind":if timed {"targeted_timed"} else {"targeted"},
                    "target":target,"phase":phase,"native_state":(["available","completed","re_enable_cooldown","failed","aborted"][native]),"ignored":ignored==1,"days":days}));
            }
        }
        Ok(json!({"schema_version":1,"status":"observed","source":"native_targeted_decision_records","scope":"current_native_records","records":records}))
    }
}
