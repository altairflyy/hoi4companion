//! Qualified country sections over the same bounded, witnessed memory transport.
use crate::live::{Decoder, Memory};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
type Result<T> = std::result::Result<T, String>;
#[path = "country_actions.rs"]
mod actions;
#[path = "country_campaign.rs"]
mod campaign;
#[path = "country_design.rs"]
mod design;
#[path = "country_foreign.rs"]
mod foreign;
#[path = "country_forts.rs"]
mod forts;
#[path = "country_military.rs"]
mod military;
#[path = "country_events.rs"]
mod windows;
#[path = "country_tests.rs"]
#[cfg(test)]
mod tests;
#[derive(Deserialize)]
pub struct Request {
    pub schema_version: u32,
    pub base: u64,
    pub world: u64,
    pub country: u64,
    pub tag: i32,
    pub section: String,
    #[serde(default)]
    pub qualify: bool,
    #[serde(default)]
    pub attributes: Value,
}
struct Reader<'a, M> {
    d: Decoder<'a, M>,
    world: u64,
    country: u64,
    tag: i32,
    watch: BTreeMap<(u64, usize), Vec<u8>>,
    attributes: Value,
    event_cache: Option<Value>,
}
impl<M: Memory> Reader<'_, M> {
    fn track(&mut self, address: u64, size: usize) -> Result<Vec<u8>> {
        let value = self.d.read(address, size)?;
        if size > 0 {
            self.watch.insert((address, size), value.clone());
        }
        Ok(value)
    }
    fn pointer(&mut self, address: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.track(address, 8)?.try_into().unwrap(),
        ))
    }
    fn integer(&mut self, address: u64, minimum: i64, maximum: i64) -> Result<i64> {
        let n = i32::from_le_bytes(self.track(address, 4)?.try_into().unwrap()) as i64;
        if !(minimum..=maximum).contains(&n) {
            return Err("Intero fuori intervallo nel riepilogo paese".into());
        }
        Ok(n)
    }
    fn typed(&mut self, address: u64, name: &str) -> Result<u64> {
        if self.d.kind(address)?.1 != format!(".?AV{name}@@") {
            return Err(format!("Tipo inatteso nel riepilogo: {name}"));
        }
        Ok(address)
    }
    fn object_pointer(&mut self, address: u64, name: &str) -> Result<u64> {
        let p = self.pointer(address)?;
        self.typed(p, name)
    }
    fn array(&mut self, address: u64, maximum: i32) -> Result<Vec<u64>> {
        let values = self.d.array(address, 8, maximum)?;
        self.track(address, 16)?;
        if !values.is_empty() {
            let data = self.d.u(address)?;
            self.track(data, values.len() * 8)?;
        }
        if values.iter().collect::<HashSet<_>>().len() != values.len() {
            return Err("Elenco duplicato nel riepilogo paese".into());
        }
        Ok(values)
    }
    fn name(&mut self, address: u64) -> Result<String> {
        let header = self.d.read(address, 32)?;
        let length = u64::from_le_bytes(header[16..24].try_into().unwrap());
        let capacity = u64::from_le_bytes(header[24..32].try_into().unwrap());
        if length > 512 || capacity < length.max(15) || capacity > 65535 {
            return Err("Nome nativo fuori intervallo".into());
        }
        let pointer = if capacity == 15 {
            address
        } else {
            u64::from_le_bytes(header[..8].try_into().unwrap())
        };
        let bytes = self.d.read(pointer, length as usize + 1)?;
        if bytes.last() != Some(&0) {
            return Err("Nome nativo privo di terminatore".into());
        }
        let bytes = &bytes[..bytes.len() - 1];
        if let Ok(text) = std::str::from_utf8(bytes) {
            return Ok(text.to_owned());
        }
        const CP1252: [char; 32] = [
            '€', '\u{fffd}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{fffd}',
            'Ž', '\u{fffd}', '\u{fffd}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›',
            'œ', '\u{fffd}', 'ž', 'Ÿ',
        ];
        Ok(bytes
            .iter()
            .map(|b| {
                if (0x80..=0x9f).contains(b) {
                    CP1252[(b - 0x80) as usize]
                } else {
                    char::from(*b)
                }
            })
            .collect())
    }
    fn divisions(&mut self) -> Result<Value> {
        let units = self.array(self.country + 0x290, 8192)?;
        let mut types = BTreeMap::<String, u64>::new();
        let mut templates = BTreeMap::<(String, String, String), u64>::new();
        let mut identities = HashSet::new();
        for unit in &units {
            self.typed(*unit, "CArmy")?;
            if self.integer(unit + 0x1d8, 0, 8191)? != self.tag as i64 {
                return Err("Divisione non controllata dalla nazione".into());
            }
            let identity = self.track(unit + 0x18, 8)?;
            if identity[..4] == [0; 4] || identity[4..] == [0; 4] || !identities.insert(identity) {
                return Err("Identità divisione assente o duplicata".into());
            }
            let template = self.object_pointer(unit + 0x3b8, "CReferencedDivisionTemplate")?;
            let primary = self.pointer(template + 0x1a8)?;
            let kind = if primary != 0 {
                self.typed(primary, "CSubUnitDefinition")?;
                self.d.string(primary + 0x18)?
            } else {
                "unknown".into()
            };
            *types.entry(kind.clone()).or_default() += 1;
            let id = hex(&self.d.read(template + 8, 8)?);
            let mut name = self.name(template + 0x20)?;
            if name.is_empty() {
                name = self.name(template + 0x40)?
            }
            *templates.entry((id, name, kind)).or_default() += 1;
        }
        Ok(
            json!({"status":"observed","total":units.len(),"by_type":types.into_iter().map(|(k,v)|json!({"type":k,"name":k,"count":v})).collect::<Vec<_>>(),"by_template":templates.into_iter().map(|((id,name,kind),count)|json!({"id":id,"name":name,"type":kind,"count":count})).collect::<Vec<_>>(),"reason":"Divisioni controllate, comprese le forze di spedizione. Tipo del battaglione principale del modello."}),
        )
    }
    fn manpower(&mut self) -> Result<Value> {
        let mut reserve = self.integer(self.country + 0x334, -2_000_000_000, 2_000_000_000)?;
        for state in self.array(self.country + 0x460, 4096)? {
            self.typed(state, "CState")?;
            reserve += self.integer(state + 0x848, -2_000_000_000, 2_000_000_000)?;
        }
        if !(0..=2_000_000_000).contains(&reserve) {
            return Err("Manpower disponibile fuori intervallo".into());
        }
        Ok(
            json!({"status":"partial","available":reserve,"fielded":self.integer(self.country+0x348,0,2_000_000_000)?,"training":null,"reason":"Disponibili e totale schierato letti dal contatore nativo. Coda addestramento non ancora decodificata."}),
        )
    }
    fn states(&mut self) -> Result<Value> {
        let rva = self.attributes["building_database_rva"]
            .as_u64()
            .unwrap_or(0x332ee28);
        let registry = self.d.u(self.d.base + rva)?;
        self.typed(registry, "CBuildingDatabase")?;
        let mut definitions = BTreeMap::new();
        let scopes = self.attributes["building_scopes"].clone();
        for (ordinal, definition) in self.array(registry + 0x40, 512)?.into_iter().enumerate() {
            if definition == 0 { continue; }
            if self.d.kind(definition)?.1 != ".?AVCBuildingTemplate@@" {
                continue;
            }
            let key = self.d.string(definition + 0x270)?;
            if [
                "infrastructure",
                "industrial_complex",
                "arms_factory",
                "dockyard",
            ]
            .contains(&key.as_str()) || scopes[&key] == "state"
            {
                if definitions.contains_key(&key) {
                    return Err("Definizione edificio duplicata".into());
                }
                let slot = self.integer(definition + 0x2e0, 0, 511)? as usize;
                if slot != ordinal { return Err("Building definition registry mismatch".into()); }
                definitions.insert(key, (definition, slot));
            }
        }
        if ["infrastructure", "industrial_complex", "arms_factory", "dockyard"].iter().any(|k| !definitions.contains_key(*k)) {
            return Err("Definizioni edifici mancanti".into());
        }
        let mut items = vec![];
        let mut identifiers = HashSet::new();
        for state in self.array(self.world + 0x2c8, 4096)? {
            if state == 0 {
                continue;
            }
            self.typed(state, "CState")?;
            let owner = self.integer(state + 0xc8, 0, 8191)?;
            let controller = self.integer(state + 0xcc, 0, 8191)?;
            if owner != self.tag as i64 && controller != self.tag as i64 {
                continue;
            }
            let id = self.integer(state + 0x58, 1, 100000)?;
            if !identifiers.insert(id) {
                return Err("Identità territorio duplicata".into());
            }
            let mapping = self.d.array(state + 0x140, 4, 512)?;
            self.track(state + 0x140, 16)?;
            if !mapping.is_empty() {
                let data = self.d.u(state + 0x140)?;
                self.track(data, mapping.len() * 4)?;
            }
            let buildings = self.array(state + 0x158, 512)?;
            let mut levels = BTreeMap::new();
            let mut indices = HashSet::new();
            for (key, (definition, slot)) in &definitions {
                let index = *mapping.get(*slot).ok_or("Mappa edifici incompleta")? as i64;
                if index == -1 {
                    levels.insert(key, 0);
                    continue;
                }
                let building = *buildings
                    .get(usize::try_from(index).map_err(|_| "Indice edificio non valido")?)
                    .ok_or("Indice edificio non valido")?;
                self.typed(building, "CBuilding")?;
                if !indices.insert(index) || self.pointer(building + 0x1e0)? != *definition {
                    return Err("State building template/index mismatch".into());
                }
                let level = i16::from_le_bytes(self.track(building + 0x40, 2)?.try_into().unwrap());
                if !(0..=1000).contains(&level) {
                    return Err("Livello edificio non valido".into());
                }
                levels.insert(key, level);
            }
            items.push(json!({"id":id,"name":format!("STATE_{id}"),"owned":owner==self.tag as i64,"controlled":controller==self.tag as i64,"civilian_factories":levels[&"industrial_complex".to_string()],"military_factories":levels[&"arms_factory".to_string()],"dockyards":levels[&"dockyard".to_string()],"infrastructure":levels[&"infrastructure".to_string()],"buildings":levels}));
        }
        items.sort_by_key(|r| r["id"].as_i64());
        Ok(
            json!({"status":"observed","items":items,"reason":"Edifici costruiti nei territori posseduti o controllati. Danni, occupazione e bonus modificano le fabbriche effettivamente disponibili."}),
        )
    }
    fn equipment(&mut self) -> Result<Value> {
        let status = self.object_pointer(self.country + 0xf68, "CProductionStatus")?;
        if self.pointer(
            status
                + self.attributes["production_owner_offset"]
                    .as_u64()
                    .unwrap_or(0x290),
        )? != self.country
        {
            return Err("Scorte di un’altra nazione".into());
        }
        let pool = self.typed(status + 0x200, "CEquipmentVariantPool")?;
        let header = self.track(pool + 0x20, 16)?;
        let data = u64::from_le_bytes(header[..8].try_into().unwrap());
        let capacity = i32::from_le_bytes(header[8..12].try_into().unwrap());
        let count = i32::from_le_bytes(header[12..].try_into().unwrap());
        if count < 0 || count > capacity || capacity > 4096 || count > 0 && data == 0 {
            return Err("Elenco scorte non valido".into());
        }
        let raw = if count > 0 {
            self.track(data, count as usize * 16)?
        } else {
            vec![]
        };
        let mut seen = HashSet::new();
        let mut amounts = BTreeMap::<String, u128>::new();
        let mut variants = vec![];
        for part in raw.as_chunks::<16>().0 {
            let variant = u64::from_le_bytes(part[..8].try_into().unwrap());
            let amount = i64::from_le_bytes(part[8..].try_into().unwrap());
            if !(0..=10_000_000_000_000_000).contains(&amount) || !seen.insert(variant) {
                return Err("Variante scorte duplicata o quantità non valida".into());
            }
            self.typed(variant, "CEquipmentVariant")?;
            let equipment = self.object_pointer(variant + 0x3f0, "CEquipmentType")?;
            let key = self.d.string(equipment + 0x18)?;
            *amounts.entry(key.clone()).or_default() += amount as u128;
            let mut name = self.name(variant + 0x28)?;
            if name.is_empty() {
                name = key.clone()
            }
            variants.push(json!({"equipment":key,"name":name,"stockpile":fixed(amount as u128)}));
        }
        Ok(
            json!({"status":"partial","items":amounts.into_iter().map(|(key,amount)|json!({"id":key,"name":key,"stockpile":fixed(amount),"balance_per_day":null,"needed":null})).collect::<Vec<_>>(),"variants":variants,"reason":"Quantità lorde nel magazzino nativo, incluse le varianti estere. Il saldo della Logistica sottrae anche fabbisogni: fabbisogno e saldo netto giornaliero non ancora qualificati."}),
        )
    }
    fn token_name(&mut self, token: i64) -> Result<String> {
        let table = self.attributes["token_table_rva"]
            .as_u64()
            .unwrap_or(0x35e1ab0);
        if self.d.read(self.d.base + table, 1)? != [1] {
            return Err("Cache nomi nativi non inizializzata".into());
        }
        let maximum = self.integer(self.d.base + table + 4, 0, 1_000_000)?;
        if !(0..=maximum).contains(&token) {
            return Err("Indice nome nativo non valido".into());
        }
        let names = self.d.u(self.d.base + table + 0x30)?;
        self.d.string(
            names
                .checked_add(token as u64 * 32)
                .ok_or("Indice nome nativo non valido")?,
        )
    }
    fn resources(&mut self) -> Result<Value> {
        let manager = self.object_pointer(self.country + 0x11f8, "CCountryResources")?;
        if self.integer(manager + 0x20, 0, 8191)? != self.tag as i64 {
            return Err("Risorse di un’altra nazione".into());
        }
        for (token, name) in [
            (0x2fac, "produced"),
            (0x30c2, "imported"),
            (0x30c3, "to_use"),
            (0x30c5, "exported"),
        ] {
            if self.token_name(token)? != name {
                return Err("Identità campi risorse non confermata".into());
            }
        }
        let registry = self.d.u(self.d.base
            + self.attributes["resources_database_rva"]
                .as_u64()
                .unwrap_or(0x332f088))?;
        let mut resources = vec![];
        let mut names = HashSet::new();
        for (index, definition) in self
            .array(registry + 0x28, 128)?
            .into_iter()
            .enumerate()
            .skip(1)
        {
            self.typed(definition, "CStrategicResource")?;
            let token = self.integer(definition + 8, 0, 1_000_000)?;
            if self.integer(definition + 0xd8, 0, 127)? != index as i64 {
                return Err("Indice risorsa non coerente".into());
            }
            let name = self.token_name(token)?;
            if !names.insert(name.clone()) {
                return Err("Risorsa duplicata".into());
            }
            resources.push((name, token));
        }
        let mut groups = BTreeMap::<&str, BTreeMap<i64, f64>>::new();
        for (name, offset) in [
            ("produced", 0x28),
            ("imported", 0x188),
            ("available", 0x2e8),
            ("demand", 0x398),
            ("exported", 0x5b0),
        ] {
            let header = self.track(manager + offset + 8, 16)?;
            let data = u64::from_le_bytes(header[..8].try_into().unwrap());
            let capacity = i32::from_le_bytes(header[8..12].try_into().unwrap());
            let count = i32::from_le_bytes(header[12..].try_into().unwrap());
            if count as usize != resources.len() + 1 || count > capacity || capacity > 128 {
                return Err("Elenco risorse incompleto".into());
            }
            let raw = self.track(data, count as usize * 16)?;
            let mut values = BTreeMap::new();
            for part in raw.as_chunks::<16>().0 {
                let amount = i64::from_le_bytes(part[..8].try_into().unwrap());
                let token = u32::from_le_bytes(part[8..12].try_into().unwrap()) as i64;
                if !(-1_000_000_000_000_000..=1_000_000_000_000_000).contains(&amount)
                    || values.insert(token, amount as f64 / 100000.).is_some()
                {
                    return Err("Quantità o identità risorsa non valida".into());
                }
            }
            if resources
                .iter()
                .any(|(_, token)| !values.contains_key(token))
            {
                return Err("Risorsa mancante nel contenitore".into());
            }
            groups.insert(name, values);
        }
        Ok(
            json!({"status":"observed","items":resources.into_iter().map(|(name,token)|{
            let demand=groups["demand"][&token];
            json!({"id":name,"name":name,"available":groups["available"][&token],"used":if demand==0.{json!(0)}else{json!(-demand)},"balance":groups["available"][&token]+demand,"produced":groups["produced"][&token],"imported":groups["imported"][&token],"exported":groups["exported"][&token]})
        }).collect::<Vec<_>>(),"reason":"Risorse industriali: disponibilità e domanda corrente; saldo confrontato con il pannello Produzione. Petrolio distinto dalle scorte di carburante."}),
        )
    }
    fn location_member(&mut self, header: u64, id: i64, maximum: i32) -> Result<u64> {
        let raw = self.track(header, 16)?;
        let data = u64::from_le_bytes(raw[..8].try_into().unwrap());
        let capacity = i32::from_le_bytes(raw[8..12].try_into().unwrap());
        let count = i32::from_le_bytes(raw[12..].try_into().unwrap());
        if id <= 0 || id >= count as i64 || count > capacity || capacity > maximum || data == 0 {
            return Err("Ubicazione costruzione fuori dal catalogo".into());
        }
        self.pointer(
            data.checked_add(id as u64 * 8)
                .ok_or("Ubicazione costruzione fuori dal catalogo")?,
        )
    }
    fn construction(&mut self) -> Result<Value> {
        let production = self.object_pointer(self.country + 0xf68, "CProductionStatus")?;
        if self.pointer(
            production
                + self.attributes["production_owner_offset"]
                    .as_u64()
                    .unwrap_or(0x290),
        )? != self.country
        {
            return Err("Costruzioni di un’altra nazione".into());
        }
        let entries = self.array(production + 0x70, 2048)?;
        if entries.is_empty() {
            return Ok(
                json!({"status":"observed","lines":[],"reason":"Coda di costruzione vuota osservata."}),
            );
        }
        let registry = self.d.u(self.d.base
            + self.attributes["building_database_rva"]
                .as_u64()
                .unwrap_or(0x332ee28))?;
        self.typed(registry, "CBuildingDatabase")?;
        let definitions = self.array(registry + 0x40, 512)?;
        let mut lines = vec![];
        let mut handles = HashSet::new();
        let mut incomplete = false;
        let layout = self.attributes["construction_layout"].clone();
        for (position, line) in entries.into_iter().enumerate() {
            if self.d.kind(line)?.1 != ".?AVCBuildingProductionLine@@" {
                incomplete = true;
                continue;
            }
            if self.pointer(line + 0x20)? != production {
                return Err("Ordine di costruzione di un’altra nazione".into());
            }
            let handle = self.track(line + 8, 8)?;
            if u32::from_le_bytes(handle[..4].try_into().unwrap()) != 0x3b
                || handle[4..] == [0; 4]
                || !handles.insert(handle.clone())
            {
                return Err("Identità ordine di costruzione assente o duplicata".into());
            }
            let building = self.object_pointer(
                line + layout["building"].as_u64().unwrap_or(0x70),
                "CBuilding",
            )?;
            let status = self.object_pointer(building + 0x1d8, "CBuildingStatus")?;
            let template = self.object_pointer(building + 0x1e0, "CBuildingTemplate")?;
            let ordinal = self.d.i(template + 0x2e0)?;
            if ordinal < 0 || definitions.get(ordinal as usize) != Some(&template) {
                return Err("Definizione costruzione fuori dal catalogo corrente".into());
            }
            let key = self.d.string(template + 0x270)?;
            let level = i16::from_le_bytes(self.track(building + 0x40, 2)?.try_into().unwrap());
            if !(0..=1000).contains(&level) {
                return Err("Livello costruito non valido".into());
            }
            let amount = self.integer(line + 0x48, 0, 1000)?;
            let repairs =
                self.integer(line + layout["repairs"].as_u64().unwrap_or(0x78), 0, 1000)?;
            let conversion =
                self.track(line + layout["conversion"].as_u64().unwrap_or(0x8c), 1)?[0];
            if conversion > 1 && layout["boolean_semantics"] != "nonzero" {
                return Err("Indicatore conversione non valido".into());
            }
            let mut row = json!({"id":hex(&handle),"position":position+1,"building_id":key,"amount":amount,"built_level":level,"active_factories":self.integer(line+0x18,0,10000)?,"repair_levels":repairs,"conversion":conversion!=0,"state_id":null,"province_id":null,"location_status":"unavailable"});
            let flags = self.integer(status + 0x68, 0, 0x7fff_ffff)?;
            let id = self.integer(status + 0x6c, 1, 100000)?;
            let national = self.integer(status + 0x70, 0, 8191)?;
            let (state, own_province) = if flags & 1 != 0 {
                let province = self.location_member(self.world + 0x2b0, id, 20000)?;
                self.typed(province, "CProvince")?;
                if self.integer(province + 0xa4, 1, 100000)? != id || province + 0x190 != status {
                    return Err("Identità provincia costruzione non coerente".into());
                }
                let state = self.object_pointer(province + 0xc0, "CState")?;
                row["province_id"] = json!(id);
                (
                    state,
                    self.integer(province + 0x188, 0, 8191)? == self.tag as i64,
                )
            } else {
                let state = self.location_member(self.world + 0x2c8, id, 4096)?;
                self.typed(state, "CState")?;
                if state + 0x120 != status {
                    return Err("Contenitore regione costruzione non coerente".into());
                }
                (state, true)
            };
            let state_id = self.integer(state + 0x58, 1, 100000)?;
            if self.location_member(self.world + 0x2c8, state_id, 4096)? != state {
                return Err("Identità regione costruzione non coerente".into());
            }
            let owner = self.integer(state + 0xc8, 0, 8191)?;
            let controller = self.integer(state + 0xcc, 0, 8191)?;
            if national == 0
                && own_province
                && owner == self.tag as i64
                && controller == self.tag as i64
            {
                let mapping = self.d.array(status + 0x20, 4, 512)?;
                self.track(status + 0x20, 16)?;
                if !mapping.is_empty() {
                    let data = self.d.u(status + 0x20)?;
                    self.track(data, mapping.len() * 4)?;
                }
                let members = self.array(status + 0x38, 512)?;
                let mapped = mapping.get(ordinal as usize).copied().map(|v| v as i64);
                if mapped.is_none_or(|v| v < 0 || members.get(v as usize) != Some(&building)) {
                    return Err(
                        "Edificio non appartenente alla regione/provincia dichiarata".into(),
                    );
                }
                row["state_id"] = json!(state_id);
                row["location_status"] = json!("observed");
            } else {
                incomplete = true
            }
            lines.push(row);
        }
        Ok(
            json!({"status":if incomplete{"partial"}else{"observed"},"lines":lines,"reason":"Ordini in coda, compresi quelli senza fabbriche attive. Riparazioni e conversioni distinte. La scomparsa di un ordine non prova il completamento."}),
        )
    }
    fn intelligence(&mut self) -> Result<Value> {
        let agency = self.object_pointer(self.country + 0xfc0, "CIntelligenceAgency")?;
        if self.pointer(agency + 0x78)? != self.country {
            return Err("Agenzia di intelligence di un’altra nazione".into());
        }
        let flags = self.track(agency + 0xc0, 2)?;
        if flags.iter().any(|b| *b > 1) {
            return Err("Indicatori creazione agenzia non validi".into());
        }
        Ok(
            json!({"status":"observed","is_created":flags[0]==1,"in_creation":flags[1]==1,"reason":"Creazione avviata e agenzia operativa sono indicatori nativi distinti. Migliorie, operazioni e percentuali non determinate."}),
        )
    }
}
fn fixed(value: u128) -> f64 {
    format!("{}.{:05}", value / 100000, value % 100000)
        .parse()
        .expect("bounded decimal")
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize]);
        out.push(HEX[(b & 15) as usize]);
    }
    String::from_utf8(out).expect("hex ASCII")
}
pub(crate) fn native_name(memory: &mut impl Memory, base: u64, address: u64) -> Result<String> {
    Reader {
        d: Decoder { memory, base },
        world: 0,
        country: 0,
        tag: 0,
        watch: BTreeMap::new(),
        attributes: Value::Null,
        event_cache: None,
    }
    .name(address)
}
pub fn collect(memory: &mut impl Memory, request: Request) -> Result<Value> {
    if request.schema_version != 1
        || (!matches!(request.section.as_str(), "design" | "campaign")
            && !(1..=8191).contains(&request.tag))
    {
        return Err("Sessione paese cambiata".into());
    }
    let mut r = Reader {
        d: Decoder {
            memory,
            base: request.base,
        },
        world: request.world,
        country: request.country,
        tag: request.tag,
        watch: BTreeMap::new(),
        attributes: request.attributes,
        event_cache: None,
    };
    if request.qualify {
        r.typed(r.country, "CCountry")?;
        let world_rva = r.attributes["world_rva"]
            .as_u64()
            .ok_or("World profile unavailable")?;
        if r.d.u(r.d.base + world_rva)? != r.world
            || r.integer(r.world + 0x520, 0, 8191)? != r.tag as i64
        {
            return Err("Sessione paese cambiata".into());
        }
    }
    let value = match request.section.as_str() {
        "divisions" => r.divisions()?,
        "manpower" => r.manpower()?,
        "states" => r.states()?,
        "equipment" => r.equipment()?,
        "resources" => r.resources()?,
        "construction" => r.construction()?,
        "intelligence" => {
            if r.attributes["foreign_interactions_layout"].is_object() {
                r.foreign_intelligence()?
            } else {
                r.intelligence()?
            }
        }
        "foreign_relations" => {
            let status = r.attributes["diplomacy_status"]
                .as_u64()
                .ok_or("Missing diplomacy status")?;
            r.foreign_relations(status)?
        }
        "production" => r.production()?,
        "experience" => r.experience()?,
        "diplomacy" => r.diplomacy()?,
        "metrics" => r.metrics()?,
        "politics" => r.politics()?,
        "projects" => r.projects()?,
        "decisions" => r.decisions()?,
        "events" => r.events()?,
        "campaign" => match r.campaign() {
            Ok(value) => value,
            Err(reason) => json!({"status":"unknown","id":null,"source":"","reason":reason}),
        },
        "fortifications" => r.fortifications()?,
        "doctrines" => match r.doctrines() {
            Ok(value) => value,
            Err(reason) => json!({"status":"unavailable","reason":reason}),
        },
        "design" => {
            let variant = r.attributes["variant"]
                .as_u64()
                .ok_or("Missing equipment variant")?;
            match r.design(variant) {
                Ok(value) => value,
                Err(reason) => json!({"status":"unavailable","reason":reason}),
            }
        }
        _ => return Err("Sezione nativa non implementata".into()),
    };
    let mut result = json!({"value":value,"watch":r.watch.iter().map(|((address,size),bytes)|json!([address,size,hex(bytes)])).collect::<Vec<_>>()});
    if let Some(cache) = r.event_cache {
        result["event_cache"] = cache
    }
    Ok(result)
}
