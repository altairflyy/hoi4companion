//! Directional diplomacy and operative missions; no command execution.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};
struct Countries {
    mapping: Vec<i64>,
    names: BTreeMap<usize, String>,
    aliases: BTreeMap<i64, BTreeSet<String>>,
}
const RELATIONS: [(&str, &str); 18] = [
    ("CWarRelation", "war"), ("CLicenceProductionAccessRelation", "licence_production_access"),
    ("CEmbargoRelation", "embargo"), ("CMilitaryAccessRelation", "military_access"),
    ("COfferMilitaryAccessRelation", "offer_military_access"), ("CImproveRelationsRelation", "improve_relations"),
    ("CAttacheRelation", "attache"), ("CBoostPartyPopularityRelation", "boost_party_popularity"),
    ("CGuaranteeRelation", "guarantee"), ("CNonAggressionPactRelation", "non_aggression_pact"),
    ("CLendLeaseRelation", "lend_lease"), ("CSubjectRelation", "subject"),
    ("CAirBaseAccessRelation", "air_base_access"), ("COfferAirBaseAccessRelation", "offer_air_base_access"),
    ("CDockingRightsRelation", "docking_rights"), ("COfferDockingRightsRelation", "offer_docking_rights"),
    ("CNavalBlockadeRelation", "naval_blockade"),
    ("CMarketAccessRightsRelation@NInternationalMarket", "market_access_rights"),
];
const CLASSES: [&str; 9] = [
    "CNoMission",
    "CBuildIntelNetwork",
    "CQuietIntelNetwork",
    "CCounterIntelligence",
    "CRootOutResistance",
    "CBoostIdeology",
    "CControlTrade",
    "CDiplomaticPressure",
    "CPropaganda",
];
const MISSIONS: [&str; 9] = [
    "no_mission",
    "build_intel_network",
    "quiet_network",
    "counter_intelligence",
    "root_out_resistance",
    "boost_ideology",
    "control_trade",
    "diplomatic_pressure",
    "propaganda",
];
const STATES: [&str; 6] = [
    "captured",
    "cooldown",
    "disbanding",
    "active",
    "operation",
    "killed",
];
impl<M: Memory> Reader<'_, M> {
    fn foreign_qualified(&mut self) -> Result<()> {
        let layout = self.attributes["foreign_interactions_layout"].clone();
        if !layout.is_object() || layout["version"] != 1 {
            return Err("Foreign interactions are not qualified for this executable.".into());
        }
        for signature in layout["signatures"]
            .as_array()
            .ok_or("Invalid foreign signature layout")?
        {
            let text = signature[1].as_str().ok_or("Invalid foreign signature")?;
            if !text.len().is_multiple_of(2) {
                return Err("Invalid foreign signature".into());
            }
            let expected = (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>>>()?;
            if self.d.memory.executable_bytes(
                signature[0]
                    .as_u64()
                    .ok_or("Invalid foreign signature RVA")?,
                expected.len(),
            )? != expected
            {
                return Err("Foreign interaction executable signature changed.".into());
            }
        }
        self.typed(self.country, "CCountry")?;
        if self.integer(self.country + 8, 1, 8191)? != self.tag as i64 {
            return Err("Foreign interaction country identity mismatch.".into());
        }
        Ok(())
    }
    fn foreign_countries(&mut self) -> Result<Countries> {
        let countries = self.array(self.world + 0x310, 8192)?;
        let mapping = self
            .d
            .array(self.world + 0x340, 4, 8192)?
            .into_iter()
            .map(|v| v as i64)
            .collect::<Vec<_>>();
        self.track(self.world + 0x340, 16)?;
        if !mapping.is_empty() {
            let data = self.d.u(self.world + 0x340)?;
            self.track(data, mapping.len() * 4)?;
        }
        let tags = self.pointer(self.world + 0x358)?;
        let count = self.integer(self.world + 0x364, 1, 8192)? as usize;
        if count != mapping.len() {
            return Err("Foreign country registry count mismatch.".into());
        }
        if mapping
            .get(self.tag as usize)
            .is_none_or(|v| *v < 0 || countries.get(*v as usize) != Some(&self.country))
        {
            return Err("Foreign country registry owner mismatch.".into());
        }
        let mut names = BTreeMap::new();
        let mut aliases = BTreeMap::<i64, BTreeSet<String>>::new();
        let mut checked = HashSet::new();
        for (index, slot) in mapping.iter().copied().enumerate().skip(1) {
            if slot == -1 {
                continue;
            }
            let country = *countries
                .get(usize::try_from(slot).map_err(|_| "Invalid foreign country mapping.")?)
                .ok_or("Invalid foreign country mapping.")?;
            if checked.insert(country) {
                self.typed(country, "CCountry")?;
                let tag = self.integer(country + 8, 1, 8191)? as usize;
                if mapping.get(tag) != Some(&slot) {
                    return Err("Foreign country reciprocal identity mismatch.".into());
                }
            }
            let name = self.d.string(tags + index as u64 * 32)?;
            if name.len() != 3
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            {
                return Err("Invalid foreign country tag.".into());
            }
            names.insert(index, name.clone());
            aliases.entry(slot).or_default().insert(name);
        }
        let mut slots_by_name = BTreeMap::<String, BTreeSet<i64>>::new();
        for (index, name) in &names {
            slots_by_name
                .entry(name.clone())
                .or_default()
                .insert(mapping[*index]);
        }
        let ambiguous = slots_by_name
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(k, _)| k)
            .collect::<HashSet<_>>();
        names.retain(|_, v| !ambiguous.contains(v));
        for values in aliases.values_mut() {
            values.retain(|v| !ambiguous.contains(v));
        }
        if !names.contains_key(&(self.tag as usize)) {
            return Err("Played country tag is ambiguous.".into());
        }
        Ok(Countries {
            mapping,
            names,
            aliases,
        })
    }
    pub(super) fn foreign_relations(&mut self, status: u64) -> Result<Value> {
        self.foreign_qualified()?;
        self.typed(status, "CDiplomacyStatus")?;
        if self.pointer(status + 0x2d0)? != self.country {
            return Err("Foreign diplomacy owner mismatch.".into());
        }
        let countries = self.foreign_countries()?;
        let own = countries.mapping[self.tag as usize];
        let mut improved = BTreeSet::new();
        let mut pacts = BTreeSet::new();
        for relation in self.array(status + 0x20, 4096)? {
            let class = self.d.kind(relation)?.1;
            let token = self.integer(relation + 8, 0, 65535)?;
            let expected = match class.as_str() {
                ".?AVCImproveRelationsRelation@@" => 0x2cd7,
                ".?AVCNonAggressionPactRelation@@" => 0x2fbc,
                _ => {
                    if [0x2cd7, 0x2fbc].contains(&token) {
                        return Err("Diplomatic relation token/class mismatch.".into());
                    }
                    continue;
                }
            };
            if token != expected {
                return Err("Diplomatic relation type/token mismatch.".into());
            }
            let raw = self.track(relation + 0x10, 8)?;
            let left = i32::from_le_bytes(raw[..4].try_into().unwrap());
            let right = i32::from_le_bytes(raw[4..].try_into().unwrap());
            let cancelled = self.track(relation + 0x48, 1)?[0];
            if cancelled > 1
                || !countries.names.contains_key(&(left as usize))
                || !countries.names.contains_key(&(right as usize))
            {
                return Err("Diplomatic relation endpoint identity mismatch.".into());
            }
            let left = left as usize;
            let right = right as usize;
            if countries.mapping[left] == countries.mapping[right]
                || ![countries.mapping[left], countries.mapping[right]].contains(&own)
            {
                return Err("Diplomatic relation endpoint identity mismatch.".into());
            }
            if cancelled == 1 || expected == 0x2cd7 && countries.mapping[left] != own {
                continue;
            }
            let other = if countries.mapping[left] == own {
                right
            } else {
                left
            };
            let values = &countries.aliases[&countries.mapping[other]];
            if expected == 0x2cd7 {
                improved.extend(values.iter().cloned())
            } else {
                pacts.extend(values.iter().cloned())
            }
        }
        let relations = match self.relation_records(status, &countries) {
            Ok(value) => value,
            Err(reason) => json!({"status":"unavailable","reason":reason}),
        };
        Ok(json!({"status":"observed","countries":countries.names.values().collect::<BTreeSet<_>>(),"improving_relations":improved,"non_aggression_pacts":pacts,"relations":relations}))
    }
    fn relation_records(&mut self, status: u64, countries: &Countries) -> Result<Value> {
        self.function_signatures("foreign_records", "Common diplomatic relation layout is not qualified")?;
        if !self.attributes["abi_functions"]["foreign_records"].as_array().is_some_and(|s| s.len()==1) {
            return Err("Missing common diplomatic serializer".into());
        }
        let own = countries.mapping[self.tag as usize];
        let mut rows = vec![];
        let mut bases = HashSet::new();
        for relation in self.array(status + 0x20, 4096)? {
            let (vtable, class) = self.d.kind(relation)?;
            let Some((_, kind)) = RELATIONS.iter().find(|(name, _)| class == format!(".?AV{name}@@")) else {
                return Err("Unknown diplomatic relation type: coverage is incomplete".into());
            };
            // War/subject relations override serialization. Their immutable RTTI
            // must still prove a nonvirtual CRelation base at displacement zero.
            if bases.insert(vtable) { self.relation_base(vtable)?; }
            let left = self.integer(relation + 0x10, 1, 8191)? as usize;
            let right = self.integer(relation + 0x14, 1, 8191)? as usize;
            let from = countries.names.get(&left).ok_or("Unknown relation origin")?;
            let to = countries.names.get(&right).ok_or("Unknown relation destination")?;
            if countries.mapping[left] == countries.mapping[right]
                || ![countries.mapping[left], countries.mapping[right]].contains(&own) {
                return Err("Foreign relation does not belong to this country".into());
            }
            let cancelled = self.track(relation + 0x48, 1)?[0];
            if cancelled > 1 { return Err("Invalid diplomatic cancellation flag".into()); }
            if cancelled == 1 { continue; }
            let outgoing = countries.mapping[left] == own;
            let other = if outgoing { right } else { left };
            rows.push(json!({"id":format!("{relation:x}"),"kind":kind,"from_country":from,"to_country":to,
                "direction":if outgoing {"outgoing"} else {"incoming"},"other_countries":countries.aliases[&countries.mapping[other]]}));
        }
        Ok(json!({"status":"observed","source":"native_relation_base_serializer","items":rows,
            "scope":"existing_relations_not_pending_offers_or_acceptance_requirements"}))
    }
    fn relation_base(&mut self, vtable: u64) -> Result<()> {
        let image = self.attributes["image_base"].as_u64().ok_or("Missing executable image base")?;
        let bytes = self.d.memory.executable_bytes(vtable.checked_sub(8).ok_or("Invalid relation vtable")?,8)?;
        let locator = u64::from_le_bytes(bytes.try_into().map_err(|_| "Partial relation RTTI")?)
            .checked_sub(image).ok_or("Invalid relation locator")?;
        let header = self.d.memory.executable_bytes(locator,24)?;
        if header.len()!=24 { return Err("Partial relation locator".into()); }
        let word=|n|u32::from_le_bytes(header[n..n+4].try_into().unwrap());
        if word(0)!=1 || word(4)!=0 || word(20) as u64 != locator {return Err("Invalid relation primary RTTI".into());}
        let hierarchy=self.d.memory.executable_bytes(word(16) as u64,16)?;
        if hierarchy.len()!=16 {return Err("Partial relation hierarchy".into());}
        let count=u32::from_le_bytes(hierarchy[8..12].try_into().unwrap()) as usize;
        let array=u32::from_le_bytes(hierarchy[12..16].try_into().unwrap()) as u64;
        if !(1..=128).contains(&count) {return Err("Invalid relation base count".into());}
        let entries=self.d.memory.executable_bytes(array,count*4)?;
        if entries.len()!=count*4 {return Err("Partial relation base array".into());}
        for entry in entries.as_chunks::<4>().0 {
            let descriptor=u32::from_le_bytes(*entry) as u64;
            let row=self.d.memory.executable_bytes(descriptor,24)?;
            if row.len()!=24 {return Err("Partial relation base descriptor".into());}
            let type_rva=u32::from_le_bytes(row[..4].try_into().unwrap()) as u64;
            let name=self.d.memory.executable_bytes(type_rva+16,128)?;
            let end=name.iter().position(|v|*v==0).ok_or("Unterminated base RTTI name")?;
            if &name[..end]==b".?AVCRelation@@" {
                if row[8..12] != 0i32.to_le_bytes() || row[12..16] != (-1i32).to_le_bytes() {
                    return Err("Diplomatic relation base displacement mismatch".into());
                }
                return Ok(());
            }
        }
        Err("Diplomatic relation has no qualified CRelation base".into())
    }
    fn operative_missions(&mut self, agency: u64) -> Result<Value> {
        let countries = self.foreign_countries()?;
        let mut rows = vec![];
        let mut identities = HashSet::new();
        for agent in self.array(agency + 0xf0, 4096)? {
            self.typed(agent, "COperativeLeader")?;
            let raw = self.track(agent + 8, 8)?;
            let handle = u32::from_le_bytes(raw[..4].try_into().unwrap());
            let sequence = u32::from_le_bytes(raw[4..].try_into().unwrap());
            let id = format!("{handle:x}:{sequence:x}");
            if handle != 0x1269 || sequence == 0 || !identities.insert(id.clone()) {
                return Err("Invalid operative identity.".into());
            }
            let state = self.integer(agent + 0x1080, 0, 5)? as usize;
            let mission = self.typed(agent + 0x1088, "COperativeMission")?;
            if self.pointer(mission + 8)? != agent {
                return Err("Operative mission belongs to another agent.".into());
            }
            let kind = self.integer(mission + 0x18, 0, 8)? as usize;
            let data = self.pointer(mission + 0x10)?;
            if self.d.kind(data)?.1 != format!(".?AV{}@NOperativeMissions@@", CLASSES[kind]) {
                return Err("Operative mission enum/class mismatch.".into());
            }
            let target = if kind == 0 {
                None
            } else {
                if self.pointer(data + 8)? != mission {
                    return Err("Mission data belongs to another mission container.".into());
                }
                let tag = self.integer(data + 0x10, 1, 8191)? as usize;
                Some(
                    countries
                        .names
                        .get(&tag)
                        .ok_or("Operative mission target is unavailable.")?
                        .clone(),
                )
            };
            rows.push(json!({"id":id,"mission_id":MISSIONS[kind],"target_country":target,"state":STATES[state]}));
        }
        Ok(
            json!({"status":"observed","countries":countries.names.values().collect::<BTreeSet<_>>(),"items":rows}),
        )
    }
    pub(super) fn foreign_intelligence(&mut self) -> Result<Value> {
        self.foreign_qualified()?;
        let agency = self.object_pointer(self.country + 0xfc0, "CIntelligenceAgency")?;
        if self.pointer(agency + 0x78)? != self.country {
            return Err("Intelligence agency owner mismatch.".into());
        }
        let flags = self.track(agency + 0xc0, 2)?;
        if flags.iter().any(|b| *b > 1) {
            return Err("Invalid intelligence creation flags.".into());
        }
        let missions = match self.operative_missions(agency) {
            Ok(v) => v,
            Err(reason) => json!({"status":"unavailable","reason":reason}),
        };
        Ok(
            json!({"status":"observed","is_created":flags[0]==1,"in_creation":flags[1]==1,"missions":missions}),
        )
    }
}
