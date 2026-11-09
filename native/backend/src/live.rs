//! Focus/research decoder over a bounded read-only memory abstraction. Process
//! discovery, executable qualification and final witnessing are host duties.
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
type Result<T> = std::result::Result<T, String>;
pub trait Memory {
    fn discovery_read(&mut self, address: u64, size: usize) -> Result<Vec<u8>> {
        self.read(address, size)
    }
    fn fixture_transport(&self) -> bool {
        false
    }
    fn read(&mut self, address: u64, size: usize) -> Result<Vec<u8>>;
    fn class_name(&mut self, rva: u64) -> Result<String>;
    fn executable_bytes(&mut self, _rva: u64, _size: usize) -> Result<Vec<u8>> {
        Err("Executable bytes unavailable".into())
    }
}
#[derive(Deserialize)]
pub struct Profile {
    pub schema_version: u32,
    pub base: u64,
    pub world_rva: u64,
    pub definition_vtable: u64,
    pub joint_focus_vtable: Option<u64>,
    pub watch: bool,
}
pub(crate) struct Decoder<'a, M> {
    pub(crate) memory: &'a mut M,
    pub(crate) base: u64,
}
impl<M: Memory> Decoder<'_, M> {
    pub(crate) fn read(&mut self, a: u64, n: usize) -> Result<Vec<u8>> {
        if !(0x10000..0x7fff_ffff_ffff).contains(&a)
            || n > 65536
            || a.checked_add(n as u64)
                .is_none_or(|end| end > 0x7fff_ffff_ffff)
        {
            return Err("Indirizzo o dimensione non validi".into());
        }
        let bytes = self.memory.read(a, n)?;
        if bytes.len() != n {
            return Err("Lettura parziale".into());
        }
        Ok(bytes)
    }
    pub(crate) fn u(&mut self, a: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.read(a, 8)?.try_into().expect("read length"),
        ))
    }
    pub(crate) fn i(&mut self, a: u64) -> Result<i32> {
        Ok(i32::from_le_bytes(
            self.read(a, 4)?.try_into().expect("read length"),
        ))
    }
    pub(crate) fn array(&mut self, a: u64, width: usize, maximum: i32) -> Result<Vec<u64>> {
        let head = self.read(a, 16)?;
        let pointer = u64::from_le_bytes(head[..8].try_into().unwrap());
        let capacity = i32::from_le_bytes(head[8..12].try_into().unwrap());
        let count = i32::from_le_bytes(head[12..16].try_into().unwrap());
        if count < 0 || count > capacity || capacity > maximum || count > 0 && pointer == 0 {
            return Err("Contenitore nativo non valido".into());
        }
        if count == 0 {
            return Ok(vec![]);
        }
        let bytes = self.read(pointer, count as usize * width)?;
        bytes
            .chunks_exact(width)
            .map(|v| {
                if width == 8 {
                    Ok(u64::from_le_bytes(v.try_into().unwrap()))
                } else {
                    Ok(i32::from_le_bytes(v.try_into().unwrap()) as i64 as u64)
                }
            })
            .collect()
    }
    pub(crate) fn string(&mut self, a: u64) -> Result<String> {
        let head = self.read(a, 32)?;
        let length = u64::from_le_bytes(head[16..24].try_into().unwrap());
        let capacity = u64::from_le_bytes(head[24..32].try_into().unwrap());
        if length == 0 || length > 160 || length > capacity || !(15..=65535).contains(&capacity) {
            return Err("Identificatore nativo non valido".into());
        }
        let pointer = if capacity == 15 {
            a
        } else {
            u64::from_le_bytes(head[..8].try_into().unwrap())
        };
        let raw = self.read(pointer, length as usize + 1)?;
        if raw.last() != Some(&0) {
            return Err("Identificatore privo di terminatore".into());
        }
        let value = &raw[..raw.len() - 1];
        if !value.is_ascii() {
            return Err("Identificatore non ASCII".into());
        }
        if !value
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(b))
        {
            return Err("Caratteri inattesi in un identificatore".into());
        }
        String::from_utf8(value.to_vec()).map_err(|e| e.to_string())
    }
    pub(crate) fn kind(&mut self, a: u64) -> Result<(u64, String)> {
        let vtable = self
            .u(a)?
            .checked_sub(self.base)
            .ok_or("RTTI non riconosciuto")?;
        Ok((vtable, self.memory.class_name(vtable)?))
    }
    pub(crate) fn object(&mut self, a: u64, name: &str) -> Result<u64> {
        if self.kind(a)?.1 != format!(".?AV{name}@@") {
            Err(format!("Tipo nativo inatteso: {name}"))
        } else {
            Ok(a)
        }
    }
    pub(crate) fn pointer_object(&mut self, a: u64, name: &str) -> Result<u64> {
        let pointer = self.u(a)?;
        self.object(pointer, name)
    }
}
fn unique(values: &[u64]) -> bool {
    values.iter().copied().collect::<HashSet<_>>().len() == values.len()
}

pub fn collect(memory: &mut impl Memory, p: Profile) -> Result<Value> {
    if p.schema_version != 1
        || p.base < 0x10000
        || p.base >= 0x7fff_ffff_ffff
        || p.world_rva > 0xffff_ffff
        || p.definition_vtable > 0xffff_ffff
    {
        return Err("Profilo lettura non valido".into());
    }
    let mut d = Decoder {
        memory,
        base: p.base,
    };
    let world = d.u(p.base + p.world_rva)?;
    if world == 0 {
        return Err("Nessuna partita caricata".into());
    }
    d.object(world, "CCurrentGameState")?;
    let tick = d.i(world + 0x468)?;
    let native_tag = d.i(world + 0x520)?;
    if native_tag <= 0 {
        return Err("Paese giocato non identificato con certezza".into());
    }
    let tag_count = d.i(world + 0x364)?;
    if native_tag >= tag_count || tag_count > 8192 {
        return Err("Indice nazione fuori intervallo".into());
    }
    let tag_base = d.u(world + 0x358)?;
    let tag = d.string(
        tag_base
            .checked_add(native_tag as u64 * 32)
            .ok_or("Indirizzo o dimensione non validi")?,
    )?;
    if tag.len() != 3
        || !tag
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return Err("TAG nazione non valido".into());
    }
    let tags = d.array(world + 0x340, 4, 8192)?;
    let countries = d.array(world + 0x310, 8, 8192)?;
    let country_index = *tags
        .get(native_tag as usize)
        .ok_or("Mappa nazione non valida")?;
    let country = *countries
        .get(country_index as usize)
        .ok_or("Mappa nazione non valida")?;
    d.object(country, "CCountry")?;
    if d.i(country + 8)? != native_tag {
        return Err("Identità nazione incoerente".into());
    }
    let tree = d.pointer_object(country + 0x1370, "CNationalFocusTree")?;
    let tree_id = d.string(tree + 8)?;
    let focus_state = d.pointer_object(country + 0x1380, "CNationalFocusProgress")?;
    if d.u(focus_state + 8)? != country {
        return Err("Stato focus di un’altra nazione".into());
    }
    let focus_defs = d.array(tree + 0x30, 8, 4096)?;
    if focus_defs.is_empty() || !unique(&focus_defs) {
        return Err("Albero dei focus vuoto o duplicato".into());
    }
    let mut focus_ids = HashMap::new();
    for &pointer in &focus_defs {
        let (vtable, kind) = d.kind(pointer)?;
        let joint = kind == ".?AVCJointNationalFocus@@" && p.joint_focus_vtable == Some(vtable);
        if kind != ".?AVCNationalFocus@@" && !joint {
            return Err("Tipo nativo inatteso: CNationalFocus".into());
        }
        if d.u(pointer + 0x160)? != if joint { 0 } else { tree } {
            return Err("Focus non appartenente all’albero".into());
        }
        focus_ids.insert(pointer, d.string(pointer + 0x18)?);
    }
    if focus_ids.values().collect::<HashSet<_>>().len() != focus_ids.len() {
        return Err("Identificatori focus duplicati".into());
    }
    let completed = d.array(focus_state + 0x40, 8, 4096)?;
    let current = d.u(focus_state + 0x10)?;
    let continuous = d.u(focus_state + 0x18)?;
    let continuous_id = if continuous != 0 {
        d.object(continuous, "CContinuousNationalFocus")?;
        Some(d.string(continuous + 0x18)?)
    } else {
        None
    };
    if completed.iter().any(|v| !focus_ids.contains_key(v))
        || current != 0 && (!focus_ids.contains_key(&current) || completed.contains(&current))
    {
        return Err("Stato focus incoerente".into());
    }
    let mut seen = HashSet::new();
    let completed_ids: Vec<_> = completed
        .iter()
        .filter(|p| seen.insert(**p))
        .map(|p| focus_ids[p].clone())
        .collect();
    let tech = d.pointer_object(country + 0xf60, "CTechnologyStatus")?;
    let models = d.array(tech + 0x88, 8, 4096)?;
    let done = d.array(tech + 0x40, 8, 4096)?;
    let slots = d.array(tech + 0xa0, 8, 64)?;
    if models.is_empty() || slots.is_empty() || !unique(&models) || !unique(&slots) {
        return Err("Stato ricerca vuoto o duplicato".into());
    }
    if !unique(&done) || done.iter().any(|p| !models.contains(p)) {
        return Err("Elenco tecnologie completate incoerente".into());
    }
    let mut active = vec![];
    for &pointer in &slots {
        d.object(pointer, "CResearchSlot")?;
        if d.u(pointer + 0x10)? != tech {
            return Err("Slot ricerca di un’altra nazione".into());
        }
        let target = d.u(pointer + 0x18)?;
        if target != 0 && !models.contains(&target) {
            return Err("Slot ricerca non riconosciuto".into());
        }
        active.push(target);
    }
    let mut model_ids = HashMap::new();
    let mut researched = vec![];
    for (index, &pointer) in models.iter().enumerate() {
        d.object(pointer, "CTechnology")?;
        if d.u(pointer + 0x158)? != tech {
            return Err("Tecnologia di un’altra nazione".into());
        }
        let definition = d.u(pointer + 0x160)?;
        if d.u(definition)? != p.base + p.definition_vtable
            || d.i(definition + 0x38)? != index as i32 + 1
        {
            return Err("Definizione tecnologia non riconosciuta".into());
        }
        let id = d.string(definition + 0x10)?;
        model_ids.insert(pointer, id.clone());
        let level = d.i(pointer + 0x174)?;
        let maximum = d.i(definition + 0x3dc)?;
        if !(0..=1_000_000).contains(&level) || !(0..=1_000_000).contains(&maximum) {
            return Err("Livello tecnologia non valido".into());
        }
        let complete = maximum > 0 && level >= maximum;
        if done.contains(&pointer) != complete {
            return Err("Livelli ed elenco completamenti non concordano".into());
        }
        if active.contains(&pointer) && complete {
            return Err("Tecnologia contemporaneamente attiva e completata".into());
        }
        if level > 0 {
            researched.push(id)
        }
    }
    if model_ids.values().collect::<HashSet<_>>().len() != model_ids.len() {
        return Err("Identificatori ricerca duplicati".into());
    }
    let mut ranges = vec![];
    if p.watch {
        ranges = vec![
            (p.base + p.world_rva, 8),
            (world, 8),
            (world + 0x520, 4),
            (world + 0x358, 8),
            (world + 0x364, 4),
            (world + 0x340, 16),
            (world + 0x310, 16),
            (country, 8),
            (country + 8, 4),
            (country + 0x1370, 8),
            (country + 0x1380, 8),
            (country + 0xf60, 8),
            (tree, 8),
            (focus_state, 16),
            (focus_state + 0x10, 16),
            (tech, 8),
        ];
        ranges.extend([
            (
                d.u(world + 0x340)?
                    .checked_add(native_tag as u64 * 4)
                    .ok_or("Mappa nazione non valida")?,
                4,
            ),
            (
                d.u(world + 0x310)?
                    .checked_add(country_index * 8)
                    .ok_or("Mappa nazione non valida")?,
                8,
            ),
        ]);
        for (address, values) in [
            (tree + 0x30, &focus_defs),
            (focus_state + 0x40, &completed),
            (tech + 0x88, &models),
            (tech + 0x40, &done),
            (tech + 0xa0, &slots),
        ] {
            ranges.push((address, 16));
            if !values.is_empty() {
                ranges.push((d.u(address)?, values.len() * 8))
            }
        }
        for &pointer in &slots {
            ranges.extend([(pointer, 8), (pointer + 0x10, 16)])
        }
        for &pointer in &models {
            ranges.extend([(pointer, 8), (pointer + 0x158, 32)])
        }
    }
    for &(address, size) in &ranges {
        d.read(address, size)?;
    }
    let mut focuses: Vec<_> = focus_ids.values().collect();
    focuses.sort();
    let mut technologies: Vec<_> = model_ids.values().collect();
    technologies.sort();
    Ok(
        json!({"world":world,"country_pointer":country,"native_tag":native_tag,"ranges":ranges,"state":{
        "country":tag,"native_tick":tick,"country_identity_validation":"primary_selector_and_country_map","focus_complete":true,"research_complete":true,
        "focus_ids":focuses,"completed_focuses":completed_ids,"native_focus_tree_id":tree_id,"continuous_focus":continuous_id,"current_focus":focus_ids.get(&current),"focus_progress":null,
        "researched_technologies":researched,"technology_ids":technologies,"research_slots":active.iter().enumerate().map(|(index,pointer)|json!({"slot":index+1,"technology_id":model_ids.get(pointer),"progress":null})).collect::<Vec<_>>()}}),
    )
}
