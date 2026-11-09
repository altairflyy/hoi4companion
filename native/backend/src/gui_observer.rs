//! Read-only GUI observations. Discovery candidates are never evidence: every
//! selected ID, owner, transform and visibility bit is witnessed again.
use crate::{
    hud_geometry::{anchored, intersect, rows},
    live::Memory,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
type Result<T> = std::result::Result<T, String>;
pub(crate) struct Sample<'a, M> {
    pub(crate) memory: &'a mut M,
    pub(crate) base: u64,
    pub(crate) profile: &'a Value,
    witnesses: BTreeMap<(u64, usize), Vec<u8>>,
    visibility: BTreeMap<u64, u8>,
    pub(crate) used: usize,
}
impl<'a, M: Memory> Sample<'a, M> {
    pub(crate) fn new(memory: &'a mut M, base: u64, profile: &'a Value) -> Self {
        Self {
            memory,
            base,
            profile,
            witnesses: BTreeMap::new(),
            visibility: BTreeMap::new(),
            used: 0,
        }
    }
    pub(crate) fn offset(&self, key: &str, default: u64) -> u64 {
        self.profile["gui_layout"][key].as_u64().unwrap_or(default)
    }
    pub(crate) fn offsets(&self, key: &str, default: &[u64]) -> Vec<u64> {
        self.profile["gui_layout"][key]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_else(|| default.to_vec())
    }
    pub(crate) fn read(&mut self, a: u64, n: usize) -> Result<Vec<u8>> {
        self.used = self.used.checked_add(n).ok_or("GUI read limit exceeded")?;
        if self.used > 4 * 1024 * 1024
            || !(0x10000..0x7fff_ffff_ffff).contains(&a)
            || n > 65536
            || a.checked_add(n as u64)
                .is_none_or(|end| end > 0x7fff_ffff_ffff)
        {
            return Err("GUI read limit exceeded".into());
        }
        let bytes = self.memory.discovery_read(a, n)?;
        if bytes.len() != n {
            return Err("Lettura parziale".into());
        }
        if let Some(old) = self.witnesses.get(&(a, n)) {
            if old != &bytes {
                return Err("Stato cambiato durante la lettura".into());
            }
        } else {
            self.witnesses.insert((a, n), bytes.clone());
        }
        Ok(bytes)
    }
    pub(crate) fn u(&mut self, a: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read(a, 8)?.try_into().unwrap()))
    }
    pub(crate) fn i(&mut self, a: u64) -> Result<i32> {
        Ok(i32::from_le_bytes(self.read(a, 4)?.try_into().unwrap()))
    }
    pub(crate) fn f(&mut self, a: u64) -> Result<f64> {
        Ok(f32::from_le_bytes(self.read(a, 4)?.try_into().unwrap()) as f64)
    }
    pub(crate) fn shown(&mut self, a: u64) -> Result<bool> {
        let full = self.witnesses.contains_key(&(a, 1));
        let flag = self.read(a, 1)?[0] & 8;
        if !full {
            self.witnesses.remove(&(a, 1));
        }
        if self
            .visibility
            .insert(a, flag)
            .is_some_and(|old| old != flag)
        {
            return Err("Stato cambiato durante la lettura".into());
        }
        Ok(flag != 0)
    }
    pub(crate) fn verify(&mut self) -> Result<()> {
        // Coalesce nearby witnesses without creating dependencies on unused
        // widget flags; each original byte range and visibility mask is checked.
        let mut ranges: Vec<(u64, u64)> = vec![];
        for &(a, n) in self.witnesses.keys() {
            if n > 0 {
                ranges.push((a, a + n as u64));
            }
        }
        for &a in self.visibility.keys() {
            ranges.push((a, a + 1));
        }
        ranges.sort_unstable();
        let mut blocks: Vec<(u64, u64)> = vec![];
        for (a, end) in ranges {
            if let Some(last) = blocks.last_mut()
                && a <= last.1 + 32
                && end - last.0 <= 4096
            {
                last.1 = last.1.max(end);
            } else {
                blocks.push((a, end));
            }
        }
        for (start, end) in blocks {
            let bytes = self.memory.discovery_read(start, (end - start) as usize)?;
            if bytes.len() != (end - start) as usize {
                return Err("Lettura parziale".into());
            }
            for (&(a, n), expected) in self.witnesses.range((start, 0)..(end, 0)) {
                let off = (a - start) as usize;
                if bytes.get(off..off + n) != Some(expected.as_slice()) {
                    return Err("Stato cambiato durante la lettura".into());
                }
            }
            for (&a, &flag) in self.visibility.range(start..end) {
                if bytes[(a - start) as usize] & 8 != flag {
                    return Err("Stato cambiato durante la lettura".into());
                }
            }
        }
        Ok(())
    }
    pub(crate) fn rtti(&mut self, vtable: u64) -> Result<(String, u32)> {
        let rva = vtable
            .checked_sub(self.base)
            .ok_or("RTTI GUI non riconosciuto")?;
        if self.memory.fixture_transport() {
            if let Some(row) = self.profile["fixture_rtti"].get(vtable.to_string()) {
                return Ok((
                    row[0].as_str().ok_or("Invalid fixture RTTI")?.into(),
                    row[1].as_u64().ok_or("Invalid fixture RTTI")? as u32,
                ));
            }
            return Ok((self.memory.class_name(rva)?, 0));
        }
        let ptr = self
            .memory
            .executable_bytes(rva.checked_sub(8).ok_or("RTTI GUI non riconosciuto")?, 8)?;
        let image = self.profile["image_base"]
            .as_u64()
            .ok_or("Missing GUI image base")?;
        let locator = u64::from_le_bytes(ptr.try_into().map_err(|_| "Invalid RTTI locator")?)
            .checked_sub(image)
            .ok_or("RTTI GUI non riconosciuto")?;
        let header = self.memory.executable_bytes(locator, 24)?;
        if header.len() != 24 {
            return Err("RTTI GUI non riconosciuto".into());
        }
        let word = |off| u32::from_le_bytes(header[off..off + 4].try_into().unwrap());
        if word(0) != 1 || word(20) as u64 != locator {
            return Err("RTTI GUI non riconosciuto".into());
        }
        let bytes = self.memory.executable_bytes(word(12) as u64 + 16, 128)?;
        let raw = &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())];
        if !raw.is_ascii() {
            return Err("RTTI GUI non ASCII".into());
        }
        Ok((String::from_utf8(raw.to_vec()).unwrap(), word(4)))
    }
    pub(crate) fn typed(&mut self, a: u64, name: &str) -> Result<u64> {
        let vt = self.u(a)?;
        if self.rtti(vt)? != (format!(".?AV{name}@@"), 0) {
            return Err(format!("Tipo GUI inatteso: {name}"));
        }
        Ok(a)
    }
    pub(crate) fn ptr(&mut self, a: u64, name: &str) -> Result<u64> {
        let v = self.u(a)?;
        self.typed(v, name)
    }
    pub(crate) fn array(&mut self, a: u64, max: i32) -> Result<Vec<u64>> {
        let header = self.read(a, 16)?;
        let pointer = u64::from_le_bytes(header[..8].try_into().unwrap());
        let capacity = i32::from_le_bytes(header[8..12].try_into().unwrap());
        let count = i32::from_le_bytes(header[12..].try_into().unwrap());
        if count < 0 || count > capacity || capacity > max || count > 0 && pointer == 0 {
            return Err("Contenitore nativo non valido".into());
        }
        if count == 0 {
            return Ok(vec![]);
        }
        Ok(self
            .read(pointer, count as usize * 8)?
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| u64::from_le_bytes(*b))
            .collect())
    }
    pub(crate) fn string(&mut self, a: u64) -> Result<String> {
        let header = self.read(a, 32)?;
        let length = u64::from_le_bytes(header[16..24].try_into().unwrap());
        let cap = u64::from_le_bytes(header[24..].try_into().unwrap());
        if length == 0 || length > 160 || length > cap || !(15..=65535).contains(&cap) {
            return Err("Identificatore nativo non valido".into());
        }
        let pointer = if cap == 15 {
            a
        } else {
            u64::from_le_bytes(header[..8].try_into().unwrap())
        };
        let bytes = self.read(pointer, length as usize + 1)?;
        if bytes.last() != Some(&0)
            || !bytes[..bytes.len() - 1]
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(c))
        {
            return Err("Identificatore nativo non valido".into());
        }
        Ok(String::from_utf8(bytes[..bytes.len() - 1].to_vec()).unwrap())
    }
    pub(crate) fn matrix(&mut self, a: u64) -> Result<[f64; 16]> {
        let bytes = self.read(a + 0x90, 64)?;
        Ok(std::array::from_fn(|i| {
            f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()) as f64
        }))
    }
    pub(crate) fn clip(&mut self, c: u64) -> Result<[f64; 4]> {
        let b = self.read(c + self.offset("container_clip", 0x1594), 16)?;
        let n: [f64; 4] = std::array::from_fn(|i| {
            i32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as f64
        });
        Ok([n[0], n[1], n[2] - n[0], n[3] - n[1]])
    }
    pub(crate) fn container(&mut self, c: u64, root: u64, owner: u64) -> Result<bool> {
        self.typed(c, "CContainerWindow")?;
        if self.u(c + 0x98)? != root || self.u(c + 0x80)? != owner {
            return Err("Proprietario GUI incoerente".into());
        }
        Ok(self.read(c + 0xA4, 2)?[1] & 8 != 0)
    }
    fn primary_base(&mut self, vt: u64, required: &str) -> Result<bool> {
        if self.memory.fixture_transport() {
            return Ok(rows(&self.profile["fixture_bases"][vt.to_string()])
                .iter()
                .any(|v| v == required));
        }
        let image = self.profile["image_base"]
            .as_u64()
            .ok_or("Missing GUI image base")?;
        let raw = self.memory.executable_bytes(
            vt.checked_sub(self.base + 8).ok_or("Invalid RTTI vtable")?,
            8,
        )?;
        let loc = u64::from_le_bytes(raw.try_into().map_err(|_| "Invalid RTTI locator")?)
            .checked_sub(image)
            .ok_or("Invalid RTTI locator")?;
        let h = self.memory.executable_bytes(loc, 24)?;
        let word = |b: &[u8], i| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
        if h.len() != 24 || word(&h, 0) != 1 || word(&h, 4) != 0 || word(&h, 20) as u64 != loc {
            return Ok(false);
        }
        let ch = self.memory.executable_bytes(word(&h, 16) as u64, 16)?;
        if ch.len() != 16 {
            return Err("Invalid RTTI hierarchy".into());
        }
        let count = word(&ch, 8);
        if !(1..=128).contains(&count) {
            return Ok(false);
        }
        let entries = self
            .memory
            .executable_bytes(word(&ch, 12) as u64, count as usize * 4)?;
        if entries.len() != count as usize * 4 {
            return Err("Invalid RTTI bases".into());
        }
        for row in entries.as_chunks::<4>().0 {
            let d = self.memory.executable_bytes(word(row, 0) as u64, 28)?;
            if d.len() != 28 {
                return Err("Invalid RTTI base".into());
            }
            let name = self.memory.executable_bytes(word(&d, 0) as u64 + 16, 128)?;
            let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
            if name[..end] == *format!(".?AV{required}@@").as_bytes() {
                return Ok(word(&d, 8) == 0 && word(&d, 12) == u32::MAX && word(&d, 16) == 0);
            }
        }
        Ok(false)
    }
    pub(crate) fn sprite(&mut self, a: u64, base: &str) -> Result<u64> {
        let vt = self.u(a)?;
        let kind = self.rtti(vt)?;
        if kind == (format!(".?AV{base}@@"), 0) {
            return Ok(a);
        }
        let animated = if base == "CSprite" {
            "CFrameAnimatedSprite"
        } else {
            "CFrameAnimatedSpriteType"
        };
        if kind == (format!(".?AV{animated}@@"), 0) && self.primary_base(vt, base)? {
            return Ok(a);
        }
        Err("Tipo sprite GUI non qualificato".into())
    }
    pub(crate) fn bounds(
        &mut self,
        sprite: u64,
        parent: u64,
        matrix: [f64; 16],
        clip: [f64; 4],
    ) -> Result<Value> {
        self.sprite(sprite, "CSprite")?;
        if self.u(sprite + 0x118)? != parent {
            return Err("Ancoraggio sprite estraneo al widget".into());
        }
        let tp = self.u(sprite + 0x180)?;
        self.sprite(tp, "CSpriteType")?;
        let mut columns = self.i(tp + 0x90)?;
        let mut rows = self.i(tp + 0x94)?;
        let width = self.i(tp + 0xA4)?;
        let height = self.i(tp + 0xA8)?;
        if self.i(tp + 0x10C)? == 1 {
            std::mem::swap(&mut columns, &mut rows);
        }
        if !(1..=1024).contains(&columns) || !(1..=1024).contains(&rows) {
            return Err("Fotogrammi sprite non validi".into());
        }
        let position = [self.f(sprite + 0x128)?, self.f(sprite + 0x12C)?];
        let scale = self.f(sprite + 0x124)?;
        anchored(
            matrix,
            position,
            [(width / columns) as f64, (height / rows) as f64],
            scale,
            clip,
        )
    }
    fn country(&mut self) -> Result<(String, u64, String, u64, u64)> {
        let world = self.ptr(
            self.base
                + self.profile["world_rva"]
                    .as_u64()
                    .ok_or("Missing GUI world profile")?,
            "CCurrentGameState",
        )?;
        let native = self.i(world + 0x520)?;
        let count = self.i(world + 0x364)?;
        if native <= 0 || native >= count || count > 8192 {
            return Err("Identità nazione non qualificata".into());
        }
        let tags = self.u(world + 0x358)?;
        let tag = self.string(tags + native as u64 * 32)?;
        if tag.len() != 3
            || !tag
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Err("TAG nazione non valido".into());
        }
        let map = self.read(world + 0x340, 16)?;
        let data = u64::from_le_bytes(map[..8].try_into().unwrap());
        let cap = i32::from_le_bytes(map[8..12].try_into().unwrap());
        let amount = i32::from_le_bytes(map[12..].try_into().unwrap());
        if !(native < amount && amount <= cap && cap <= 8192) {
            return Err("Mappa nazione non valida".into());
        }
        let index = self.i(data + native as u64 * 4)?;
        let countries = self.array(world + 0x310, 8192)?;
        let ptr = *countries
            .get(usize::try_from(index).map_err(|_| "Mappa nazione non valida")?)
            .ok_or("Mappa nazione non valida")?;
        let country = self.typed(ptr, "CCountry")?;
        if self.i(country + 8)? != native {
            return Err("Identità nazione incoerente".into());
        }
        let tree = self.ptr(country + 0x1370, "CNationalFocusTree")?;
        let technology = self.ptr(country + 0xF60, "CTechnologyStatus")?;
        Ok((tag, tree, self.string(tree + 8)?, technology, country))
    }
    fn focus_bounds(&mut self, c: u64, root: u64, observer: u64, clip: [f64; 4]) -> Result<Value> {
        let parent = self.ptr(
            c + self.offset("container_inner", 0x15F8),
            "C2dPositionObject",
        )?;
        let matrix = self.matrix(parent)?;
        let mut parts = BTreeMap::new();
        for button in self.array(c + self.offset("container_buttons", 0x1120), 64)? {
            self.typed(button, "CButtonStandard")?;
            let def = self.ptr(button + 0x60, "CButtonType")?;
            let name = self.string(def + 0x28)?;
            if name != "bg" && name != "symbol" {
                continue;
            }
            if self.u(button + 0x68)? != root || self.u(button + 0x50)? != observer {
                return Err("Pulsante focus di un altro proprietario".into());
            }
            let sprite = self.u(button + self.offset("button_sprite", 0xF0))?;
            self.sprite(sprite, "CSprite")?;
            if self.u(button + self.offset("button_anchor", 0x1B0))? != parent
                || self.u(sprite + 0x118)? != parent
            {
                return Err("Ancoraggio pulsante estraneo al focus".into());
            }
            parts.insert(name, self.bounds(sprite, parent, matrix, clip)?);
        }
        if parts.len() != 2 || !parts.contains_key("bg") || !parts.contains_key("symbol") {
            return Err("Contorno focus senza simbolo e sfondo qualificati".into());
        }
        let a = &parts["bg"];
        let b = &parts["symbol"];
        if (a["scale"].as_f64().unwrap() - b["scale"].as_f64().unwrap()).abs() > 0.0001 {
            return Err("Scale focus incoerenti".into());
        }
        let x = a["rect"][0]
            .as_f64()
            .unwrap()
            .min(b["rect"][0].as_f64().unwrap());
        let y = a["rect"][1]
            .as_f64()
            .unwrap()
            .min(b["rect"][1].as_f64().unwrap());
        let right = parts
            .values()
            .map(|p| p["rect"][0].as_f64().unwrap() + p["rect"][2].as_f64().unwrap())
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = parts
            .values()
            .map(|p| p["rect"][1].as_f64().unwrap() + p["rect"][3].as_f64().unwrap())
            .fold(f64::NEG_INFINITY, f64::max);
        let rect = [x, y, right - x, bottom - y];
        let clipped = intersect(rect, clip);
        Ok(
            json!({"rect":rect,"clip":clipped,"visible":clipped[2]>0. && clipped[3]>0.,"scale":a["scale"],"parts":{"bg":a["rect"],"symbol":b["rect"]}}),
        )
    }
    fn research_bounds(
        &mut self,
        c: u64,
        root: u64,
        observer: u64,
        clip: [f64; 4],
    ) -> Result<Value> {
        let outer = self.ptr(
            c + self.offset("container_outer", 0x15F0),
            "C2dPositionObject",
        )?;
        let outer_matrix = self.matrix(outer)?;
        let mut backgrounds = vec![];
        for button in self.array(c + self.offset("container_drags", 0x10D8), 64)? {
            self.typed(button, "CButtonDrag")?;
            let def = self.ptr(button + 0x60, "CButtonType")?;
            let name = self.string(def + 0x28)?;
            if name != "Background" && name != "bg" {
                continue;
            }
            if self.u(button + 0x68)? != root || self.u(button + 0x50)? != observer {
                return Err("Sfondo ricerca di un altro proprietario".into());
            }
            if self.u(button + self.offset("drag_anchor", 0x110))? != outer {
                return Err("Ancoraggio sfondo ricerca non qualificato".into());
            }
            let sprite = self.u(button + self.offset("drag_sprite", 0x118))?;
            backgrounds.push(self.bounds(sprite, outer, outer_matrix, clip)?);
        }
        if backgrounds.len() != 1 {
            return Err("Sfondo tecnologia mancante o ambiguo".into());
        }
        let parent = self.ptr(
            c + self.offset("container_inner", 0x15F8),
            "C2dPositionObject",
        )?;
        let matrix = self.matrix(parent)?;
        let mut icons = vec![];
        for icon in self.array(c + self.offset("container_icons", 0x1150), 64)? {
            self.typed(icon, "CIcon")?;
            let def = self.ptr(icon + 0x60, "CIconType")?;
            if self.string(def + 0x28)? != "Icon" {
                continue;
            }
            if self.u(icon + 0x68)? != root || self.u(icon + 0x50)? != observer {
                return Err("Icona ricerca di un altro proprietario".into());
            }
            let sprite = self.u(icon + self.offset("icon_sprite", 0xE8))?;
            self.sprite(sprite, "CSprite")?;
            if self.u(icon + self.offset("icon_anchor", 0xF0))? != parent
                || self.u(sprite + 0x118)? != parent
            {
                return Err("Ancoraggio icona estraneo alla ricerca".into());
            }
            icons.push(self.bounds(sprite, parent, matrix, clip)?);
        }
        if icons.len() != 1 {
            return Err("Icona tecnologia mancante o ambigua".into());
        }
        let mut result = backgrounds.remove(0);
        result["parts"] = json!({"background":result["rect"],"icon":icons[0]["rect"]});
        Ok(result)
    }
    fn tabs(
        &mut self,
        c: u64,
        root: u64,
        observer: u64,
        clip: [f64; 4],
        folders: &Value,
    ) -> Result<Vec<Value>> {
        self.container(c, root, observer)?;
        let parent = self.ptr(
            c + self.offset("container_inner", 0x15F8),
            "C2dPositionObject",
        )?;
        let matrix = self.matrix(parent)?;
        let mut out = vec![];
        for button in self.array(c + self.offset("container_buttons", 0x1120), 64)? {
            self.typed(button, "CButtonStandard")?;
            let def = self.ptr(button + 0x60, "CButtonType")?;
            let name = self.string(def + 0x28)?;
            let Some(folder) = name
                .strip_suffix("_tab")
                .filter(|key| folders.get(key).is_some())
            else {
                continue;
            };
            if self.u(button + 0x68)? != root || self.u(button + 0x50)? != observer {
                return Err("Scheda ricerca di un altro proprietario".into());
            }
            let sprite = self.u(button + self.offset("button_sprite", 0xF0))?;
            self.sprite(sprite, "CSprite")?;
            if self.u(button + self.offset("button_anchor", 0x1B0))? != parent
                || self.u(sprite + 0x118)? != parent
            {
                return Err("Ancoraggio scheda ricerca non qualificato".into());
            }
            let mut bounds = self.bounds(sprite, parent, matrix, clip)?;
            bounds["panel"] = json!("research_tab");
            bounds["identity"] = json!(folder);
            bounds["visible"] = json!(self.shown(button + 0x75)? && bounds["visible"] == true);
            out.push(bounds);
        }
        Ok(out)
    }
    fn folder(&mut self, c: u64, folders: &Value) -> Result<String> {
        let mut anchors = HashMap::new();
        for (name, c) in folders.as_object().ok_or("Missing research folders")? {
            let p = self.u(c.as_u64().ok_or("Invalid folder pointer")?
                + self.offset("container_inner", 0x15F8))?;
            anchors.insert(p, name.clone());
        }
        let mut current = self.u(c + self.offset("container_outer", 0x15F0))?;
        let mut seen = HashSet::new();
        for _ in 0..16 {
            if current == 0 || !seen.insert(current) {
                break;
            }
            self.typed(current, "C2dPositionObject")?;
            if let Some(name) = anchors.get(&current) {
                return Ok(name.clone());
            }
            current = self.u(current + 0x118)?;
        }
        Err("Cartella della tecnologia non qualificata".into())
    }
}

impl<M: Memory> Memory for Sample<'_, M> {
    fn fixture_transport(&self) -> bool {
        self.memory.fixture_transport()
    }
    fn read(&mut self, a: u64, n: usize) -> Result<Vec<u8>> {
        Sample::read(self, a, n)
    }
    fn discovery_read(&mut self, a: u64, n: usize) -> Result<Vec<u8>> {
        Sample::read(self, a, n)
    }
    fn class_name(&mut self, rva: u64) -> Result<String> {
        let (name, offset) = self.rtti(self.base + rva)?;
        if offset != 0 {
            return Err("RTTI non riconosciuto".into());
        }
        Ok(name)
    }
    fn executable_bytes(&mut self, rva: u64, n: usize) -> Result<Vec<u8>> {
        self.memory.executable_bytes(rva, n)
    }
}

pub(crate) fn snapshot(memory: &mut impl Memory, args: &Value, bindings: &Value) -> Result<Value> {
    let profile = &args["profile"];
    let base = args["base"].as_u64().ok_or("Missing GUI module base")?;
    let mut s = Sample::new(memory, base, profile);
    let root = s.ptr(base + s.offset("root_rva", 0x34531E0), "CGameGui")?;
    if bindings
        .get("root")
        .is_some_and(|r| r.as_u64() != Some(root))
    {
        return Err("Contesto GUI cambiato durante la ricerca".into());
    }
    let (country, tree, scope, technology, country_pointer) = s.country()?;
    let mut panels = json!({});
    let mut nodes = vec![];
    let mut tabs = vec![];
    let mut resolution: Option<[i32; 2]> = None;
    for (panel, name, target_field) in [
        ("focus", "CNationalFocusView", "focus_ids"),
        ("research", "CCountryTechTreeView", "technology_ids"),
    ] {
        let owner = s.typed(
            bindings["views"][name].as_u64().ok_or("Missing GUI view")?,
            name,
        )?;
        let c = s.u(owner + s.offset("view_container", 0x578))?;
        let visible = s.container(c, root, owner + 40)?;
        let position = s.ptr(c + s.offset("container_outer", 0x15F0), "C2dPositionObject")?;
        let dims = [s.i(position + 0x130)?, s.i(position + 0x134)?];
        if dims.iter().any(|n| !(64..=32768).contains(n)) {
            return Err("Risoluzione GUI non valida".into());
        }
        if resolution.is_some_and(|r| r != dims) {
            return Err("Viste con risoluzioni incoerenti".into());
        }
        resolution = Some(dims);
        let mut clip = intersect(s.clip(c)?, [0., 0., dims[0] as f64, dims[1] as f64]);
        panels[panel] = json!({"visible":visible,"clip":clip});
        if panel == "research" {
            let def = s.ptr(c + 0x90, "CContainerWindowType")?;
            if s.string(def + 0x28)? != "countrytechtreeview" {
                return Err("Contesto albero ricerca cambiato".into());
            }
            let tabs_container = bindings["tabs_container"].as_u64().filter(|a| *a > 0);
            if visible && tabs_container.is_none() {
                return Err("Schede native di ricerca non qualificate".into());
            }
            if visible && let Some(t) = tabs_container {
                tabs = s.tabs(t, root, owner + 40, clip, &bindings["folders"])?;
            }
            let mut selected = vec![];
            for (name, address) in bindings["folders"]
                .as_object()
                .ok_or("Missing research folders")?
            {
                let fc = address.as_u64().ok_or("Invalid folder pointer")?;
                if s.container(fc, root, owner + 40)? {
                    let def = s.ptr(fc + 0x90, "CContainerWindowType")?;
                    if s.string(def + 0x28)? != *name {
                        return Err("Identità cartella ricerca cambiata".into());
                    }
                    selected.push((name, fc));
                }
            }
            if visible && selected.len() != 1 {
                return Err("Cartella ricerca visibile mancante o ambigua".into());
            }
            panels[panel]["folder"] = if selected.len() == 1 {
                json!(selected[0].0)
            } else {
                Value::Null
            };
            if visible {
                clip = intersect(clip, s.clip(selected[0].1)?);
                panels[panel]["clip"] = json!(clip);
            }
        }
        if !visible {
            continue;
        }
        if panel == "focus" {
            for off in s.offsets("focus_tree_fields", &[0x20E8, 0x20F0]) {
                if s.u(owner + off)? != tree {
                    return Err("Vista focus di un altro albero".into());
                }
            }
            let optional = s.u(owner + s.offset("focus_country_optional", 0x20F8))?;
            if optional != 0 && optional != country_pointer {
                return Err("Vista focus di un’altra nazione".into());
            }
            for off in s.offsets("focus_country_fields", &[0x2100, 0x2108]) {
                if s.u(owner + off)? != country_pointer {
                    return Err("Vista focus di un’altra nazione".into());
                }
            }
        }
        let mut targets: Vec<_> = rows(&args[target_field])
            .iter()
            .filter_map(Value::as_str)
            .collect();
        targets.sort_unstable();
        targets.dedup();
        for key in targets {
            for item in rows(&bindings[panel][key]) {
                let item = s.typed(
                    item.as_u64().ok_or("Invalid GUI candidate")?,
                    if panel == "focus" {
                        "CNationalFocusTreeItem"
                    } else {
                        "CTechnologyTreeTechItem"
                    },
                )?;
                let c = s.u(item + 0x10)?;
                let mut shown = s.container(c, root, item + 24)?;
                let mut item_folder = None;
                let mut item_clip = clip;
                if panel == "focus" {
                    let def = s.u(item + 0x20)?;
                    let vt = s.u(def)?;
                    let kind = s.rtti(vt)?;
                    let joint = kind == (".?AVCJointNationalFocus@@".into(), 0)
                        && vt.checked_sub(base) == profile["joint_focus_vtable"].as_u64();
                    if kind != (".?AVCNationalFocus@@".into(), 0) && !joint {
                        return Err("Tipo focus GUI non qualificato".into());
                    }
                    if s.u(def + 0x160)? != if joint { 0 } else { tree }
                        || s.string(def + 0x18)? != key
                        || joint && !s.array(tree + 0x30, 4096)?.contains(&def)
                    {
                        return Err("Focus GUI estraneo alla nazione o ID cambiato".into());
                    }
                } else {
                    let model = s.ptr(item + 0xC8, "CTechnology")?;
                    if s.u(model + 0x158)? != technology {
                        return Err("Ricerca GUI di un’altra nazione".into());
                    }
                    let def = s.ptr(model + 0x160, "CTechnologyTemplate")?;
                    if s.string(def + 0x10)? != key {
                        return Err("ID ricerca GUI cambiato".into());
                    }
                    let tv = s.ptr(item + 0x30, "CCountryTechTreeView")?;
                    if tv != owner {
                        return Err("Tecnologia di un altro albero visualizzato".into());
                    }
                    let tc = s.u(tv + s.offset("view_container", 0x578))?;
                    shown &= s.container(tc, root, tv + 40)?;
                    item_clip = intersect(clip, s.clip(tc)?);
                    let folder = s.folder(c, &bindings["folders"])?;
                    shown &= panels["research"]["folder"] == folder;
                    item_folder = Some(folder);
                }
                let mut bounds = if panel == "focus" {
                    s.focus_bounds(c, root, item + 24, clip)?
                } else {
                    s.research_bounds(c, root, item + 24, item_clip)?
                };
                bounds["visible"] = json!(shown && bounds["visible"] == true);
                bounds["panel"] = json!(panel);
                bounds["identity"] = json!(key);
                bounds["scope"] = if panel == "focus" {
                    json!(scope)
                } else {
                    Value::Null
                };
                bounds["folder"] = json!(item_folder);
                nodes.push(bounds);
            }
        }
    }
    let mut active: Vec<_> = ["focus", "research"]
        .into_iter()
        .filter(|p| panels[p]["visible"] == true)
        .collect();
    if active.len() > 1 {
        return Err("Viste sovrapposte: geometria non esclusiva".into());
    }
    let resolution = resolution.ok_or("Missing GUI resolution")?;
    // Current profiles deliberately withhold unqualified popup choice offsets.
    // Every known visible popup suppresses geometry instead of guessing.
    if profile["gui_layout"].is_object() {
        for event in rows(&bindings["events"]) {
            let e = event.as_u64().ok_or("Invalid event candidate")?;
            let c = s.u(e + s.offset("view_container", 0x578))?;
            if s.container(c, root, e + 40)? {
                return Err(
                    "Evento in primo piano: geometria risposte non ancora qualificata".into(),
                );
            }
        }
    }
    let event_data = if profile["gui_layout"].is_object() {
        json!({"status":"unavailable","event_windows":[],"nodes":[],"visible_ids":[]})
    } else {
        crate::gui_actions::events(
            &mut s,
            root,
            &bindings["events"],
            &args["event_ids"],
            resolution,
        )?
    };
    let event_visible = !rows(&event_data["event_windows"]).is_empty();
    panels["event"] = json!({"visible":event_visible,"clip":[0,0,resolution[0],resolution[1]],"status":event_data["status"]});
    if event_visible {
        active = vec!["event"];
    }
    nodes.extend(rows(&event_data["nodes"]).iter().cloned());
    let action_data =
        if args["action_ui"] == true && active.is_empty() && !profile["gui_layout"].is_object() {
            crate::gui_actions::controls(
                &mut s,
                root,
                &bindings.get("action_roots").cloned().unwrap_or(json!({})),
                resolution,
                country_pointer,
            )
        } else {
            json!({})
        };
    let production_data = if args["production_ui"] == true
        && active.is_empty()
        && profile["gui_layout"].is_object()
    {
        crate::gui_production::read(&mut s, root, bindings, resolution, country_pointer)?
    } else {
        json!({})
    };
    if production_data["visible"] == true {
        active = vec!["production"];
    }
    s.verify()?;
    Ok(
        json!({"available":true,"source":"native_gui","pid":args["pid"],"process_created":args["created"],"country":country,"scope":scope,"resolution":resolution,"active_panel":active.first(),"panels":panels,"nodes":nodes,"tabs":tabs,"events":{"status":panels["event"]["status"],"visible_ids":event_data["visible_ids"]},"event_windows":event_data["event_windows"],"action_ui":action_data,"production_ui":production_data,"diagnostics":{"bytes":s.used}}),
    )
}

pub(crate) fn fixture(args: &Value) -> Result<Value> {
    struct Sparse {
        bytes: HashMap<u64, u8>,
        classes: Value,
    }
    impl Memory for Sparse {
        fn fixture_transport(&self) -> bool {
            true
        }
        fn read(&mut self, a: u64, n: usize) -> Result<Vec<u8>> {
            if n > 65536 {
                return Err("Fixture read exceeds limit".into());
            }
            Ok((0..n)
                .map(|i| *self.bytes.get(&(a + i as u64)).unwrap_or(&0))
                .collect())
        }
        fn class_name(&mut self, rva: u64) -> Result<String> {
            self.classes[rva.to_string()]
                .as_str()
                .map(str::to_owned)
                .ok_or("RTTI non riconosciuto".into())
        }
    }
    let data = args["memory"]
        .as_object()
        .ok_or("Missing GUI fixture bytes")?;
    if data.len() > 2 * 1024 * 1024 {
        return Err("GUI fixture exceeds limit".into());
    }
    let mut memory = Sparse {
        bytes: HashMap::new(),
        classes: args["classes"].clone(),
    };
    for (a, b) in data {
        memory.bytes.insert(
            a.parse().map_err(|_| "Invalid fixture address")?,
            b.as_u64()
                .and_then(|v| u8::try_from(v).ok())
                .ok_or("Invalid fixture byte")?,
        );
    }
    if let Some(section) = args["section"].as_str() {
        let mut s = Sample::new(
            &mut memory,
            args["base"].as_u64().ok_or("Missing fixture base")?,
            &args["profile"],
        );
        let root = args["root"].as_u64().ok_or("Missing fixture GUI root")?;
        let resolution = [
            args["resolution"][0]
                .as_i64()
                .ok_or("Missing fixture width")? as i32,
            args["resolution"][1]
                .as_i64()
                .ok_or("Missing fixture height")? as i32,
        ];
        let value = match section {
            "events" => crate::gui_actions::events(
                &mut s,
                root,
                &args["bindings"]["events"],
                &args["event_ids"],
                resolution,
            )?,
            "actions" => crate::gui_actions::controls(
                &mut s,
                root,
                &args["roots"],
                resolution,
                args["country"].as_u64().ok_or("Missing fixture country")?,
            ),
            "production" => crate::gui_production::read(
                &mut s,
                root,
                &args["bindings"],
                resolution,
                args["country"].as_u64().ok_or("Missing fixture country")?,
            )?,
            _ => return Err("Unknown GUI fixture section".into()),
        };
        s.verify()?;
        return Ok(value);
    }
    if args["discover"] == true {
        return crate::gui_registry::Registry::default().observe(&mut memory, args);
    }
    snapshot(&mut memory, args, &args["bindings"])
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Changing {
        values: Vec<u8>,
        reads: usize,
    }
    impl Memory for Changing {
        fn read(&mut self, _: u64, n: usize) -> Result<Vec<u8>> {
            let value = self.values[self.reads.min(self.values.len() - 1)];
            self.reads += 1;
            Ok(vec![value; n])
        }
        fn class_name(&mut self, _: u64) -> Result<String> {
            Err("No RTTI in flag fixture".into())
        }
    }
    #[test]
    fn visibility_ignores_animation_bits_but_keeps_real_visibility_changes() {
        for (values, valid) in [(vec![8, 24], true), (vec![8, 0], false)] {
            let mut memory = Changing { values, reads: 0 };
            let profile = json!({});
            let mut s = Sample::new(&mut memory, 0x140000000, &profile);
            assert!(s.shown(0x10000).unwrap());
            assert_eq!(s.verify().is_ok(), valid);
        }
    }
    #[test]
    fn an_explicit_full_byte_witness_is_not_weakened_to_a_visibility_mask() {
        let mut memory = Changing {
            values: vec![8, 8, 24],
            reads: 0,
        };
        let profile = json!({});
        let mut s = Sample::new(&mut memory, 0x140000000, &profile);
        assert_eq!(s.read(0x10000, 1).unwrap(), [8]);
        assert!(s.shown(0x10000).unwrap());
        assert!(s.verify().is_err());
    }
}
