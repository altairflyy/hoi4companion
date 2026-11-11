//! Read-only idea purchase costs. Reproduces the admitted native fixed-point
//! formula, including selected-idea removal and intermediate law levels.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const SCALE: i64 = 100_000;
fn modified(base: i64, factor: i64) -> Result<i64> {
    let value = base.checked_mul(factor).ok_or("Political cost overflow")? / SCALE;
    if !(0..=100_000_000_000).contains(&value) {return Err("Political cost outside bounds".into());}
    Ok(value)
}
impl<M: Memory> Reader<'_, M> {
    fn cost_fixed(&mut self, at:u64)->Result<i64> {
        let n=i64::from_le_bytes(self.track(at,8)?.try_into().unwrap());
        if !(-100_000_000_000..=100_000_000_000).contains(&n) {return Err("Political cost outside bounds".into());}
        Ok(n)
    }
    fn cost_modifier(&mut self, index:i64, values:&BTreeMap<i64,i64>, definitions:u64)->Result<i64> {
        // Missing key is zero only after the complete sorted native modifier
        // container has been witnessed, matching GetModifier's missing branch.
        let Some(&value)=values.get(&index) else {return Ok(0)};
        let entry=definitions+index as u64*0x78;
        if self.integer(entry+0x68,0,i32::MAX as i64)?!=0 {
            return Err("Conditional native cost modifier is not qualified".into());
        }
        let flags=self.integer(entry+0x60,0,i32::MAX as i64)?;
        Ok(if flags&0x10!=0 {value.clamp(0,SCALE)} else {value})
    }
    pub(super) fn political_costs(&mut self)->Result<Value> {
        self.function_signatures("political_costs","Native political cost formula is not qualified")?;
        if !self.attributes["abi_functions"]["political_costs"].as_array().is_some_and(|a|a.len()==9) {
            return Err("Incomplete native political cost evidence".into());
        }
        self.typed(self.country,"CCountry")?;
        if self.integer(self.country+8,1,8191)?!=self.tag as i64 {return Err("Political cost country changed".into());}
        let targets=self.attributes["political_cost_ids"].as_array().ok_or("Political cost targets missing")?.clone();
        if targets.len()>4096 {return Err("Political cost target limit exceeded".into());}
        let mut wanted=BTreeMap::<String,BTreeSet<String>>::new();
        for row in targets {
            let id=row["id"].as_str().ok_or("Invalid political cost target")?;
            let group=row["group"].as_str().ok_or("Invalid political cost group")?;
            if [id,group].iter().any(|s|s.is_empty()||s.len()>160||!s.bytes().all(|c|c.is_ascii_alphanumeric()||b"_.-".contains(&c))) {return Err("Invalid political cost identity".into());}
            wanted.entry(group.into()).or_default().insert(id.into());
        }
        let db_rva=self.attributes["ideas_database_rva"].as_u64().ok_or("Idea database unavailable")?;
        let db=self.object_pointer(self.d.base+db_rva,"CIdeaDatabase")?;
        let defs_rva=self.attributes["modifier_definitions_rva"].as_u64().ok_or("Modifier definitions unavailable")?;
        let definitions=self.pointer(self.d.base+defs_rva)?;
        if definitions<65536 {return Err("Modifier definitions unavailable".into());}
        let mut values=BTreeMap::new();let mut last=None;
        for at in self.inline(self.country+0x5b8,16,16384)? {
            let index=self.integer(at,0,16383)?;
            if last.is_some_and(|v|v>=index) {return Err("Native modifier order is not qualified".into());}
            values.insert(index,self.cost_fixed(at+8)?);last=Some(index);
        }
        let manager=self.object_pointer(self.country+0xf90,"CPoliticalStatus")?;
        if self.pointer(manager+0xc8)?!=self.country {return Err("Political cost owner mismatch".into());}
        let selected=self.array(manager+0x80,4096)?;
        let mut rows=vec![];let mut groups=BTreeSet::new();
        for group in self.array(db+0xd8,4096)?.into_iter().skip(1) {
            self.typed(group,"CIdeaGroupType")?;
            let key=self.d.string(group+0x38)?;
            if !groups.insert(key.clone()) {return Err("Duplicate native idea group".into());}
            let Some(ids)=wanted.get(&key) else {continue};
            let category=self.object_pointer(group+0x60,"CIdeaCategory")?;
            // Category enum 0 is PP; 1 is national spirit, 2/3/4 are XP.
            if self.integer(category+0x78,0,4)?!=0 {continue;}
            let group_index=self.integer(group+0x68,0,16383)?;
            let category_index=self.integer(category+0x80,0,16383)?;
            let factor=SCALE+self.cost_modifier(group_index,&values,definitions)?
                +self.cost_modifier(category_index,&values,definitions)?;
            let exclusive=self.track(group+0x10,1)?[0];
            if exclusive>1 {return Err("Invalid idea group exclusivity".into());}
            let mut old_removal=0;let mut old_level=-1;let mut old_id=Value::Null;
            if exclusive==1 {
                let index=self.integer(group+0x5c,1,4096)? as usize;
                let slot=*selected.get(index-1).ok_or("Political cost group outside selected slots")?;
                let old=self.pointer(slot)?;
                let kind=self.d.kind(old)?.1;
                if kind==".?AVCIdea@@" {
                    if self.pointer(old+0xaa0)?!=group {return Err("Selected cost idea group mismatch".into());}
                    old_id=json!(self.d.string(old+0x18)?);
                    old_removal=self.cost_fixed(old+0xab0)?.max(0);
                    old_level=self.integer(old+0xac0,-1,1000)?;
                } else if kind!=".?AV?$TNullObject@VCIdea@@V1@@@" {return Err("Selected political cost idea has unknown type".into());}
            }
            let mut native=vec![];
            for idea in self.array(group+0x18,16384)? {
                self.typed(idea,"CIdea")?;
                if self.pointer(idea+0xaa0)?!=group {return Err("Political cost definition group mismatch".into());}
                native.push((self.d.string(idea+0x18)?,self.cost_fixed(idea+0xaa8)?,self.cost_fixed(idea+0xab0)?.max(0),self.integer(idea+0xac0,-1,1000)?));
            }
            let mut seen=BTreeSet::new();
            for (id,base,_,level) in &native {
                if !ids.contains(id) {continue;}
                if !seen.insert(id) {return Err("Duplicate political cost target".into());}
                let calculation=(||->Result<i64>{
                    if *base<0 {return Err("Native idea cannot be purchased".into());}
                    let mut total=modified(*base,factor)?+old_removal;
                    if exclusive==1 && !old_id.is_null() {
                        // The native loop takes the first definition at each
                        // intermediate rank, in the registry's observed order.
                        for rank in (old_level.min(*level)+1)..old_level.max(*level) {
                            if let Some((_,base,removal,_))=native.iter().find(|(_,_,_,l)|*l==rank) {
                                total=total.checked_add(modified(*base,factor)?+*removal).ok_or("Political cost overflow")?;
                            }
                        }
                    }
                    if !(0..=100_000_000_000).contains(&total) {return Err("Political cost outside bounds".into());}Ok(total)
                })();
                match calculation {
                    Ok(effective)=>rows.push(json!({"id":id,"group":key,"currency":"political_power","base":*base as f64/SCALE as f64,"effective":effective as f64/SCALE as f64,"factor":factor as f64/SCALE as f64,"selected_id":old_id,"status":"calculated"})),
                    Err(reason)=>rows.push(json!({"id":id,"group":key,"status":"unknown","reason":reason})),
                }
            }
        }
        // Advisor qualification is optional: an unavailable role serializer
        // cannot invalidate already qualified law costs.
        let mut coverage=json!({});
        match self.advisor_cost_rows(&wanted,&values,definitions) {
          Ok(advisors)=>{
            coverage["advisors"]=json!({"status":"qualified"});
            for row in advisors {
                if let Some(existing)=rows.iter_mut().find(|r|r["group"]==row["group"]&&r["id"]==row["id"]) {
                    *existing=json!({"id":row["id"],"group":row["group"],"status":"unknown","reason":"Ambiguous native purchase family"});
                } else {rows.push(row);}
            }
          },
          Err(reason)=>coverage["advisors"]=json!({"status":"unknown","reason":reason}),
        }
        match self.decision_cost_rows(&wanted) {
            Ok(decisions)=>{rows.extend(decisions);coverage["decisions"]=json!({"status":"qualified"});},
            Err(reason)=>coverage["decisions"]=json!({"status":"unknown","reason":reason}),
        }
        for (group,ids) in &wanted {
            for id in ids {
                if !rows.iter().any(|r|r["group"]==*group&&r["id"]==*id) {
                    // A group alone does not identify a modern advisor: laws
                    // and old ideas use this namespace too. Preserve the
                    // optional advisor failure in coverage rather than attach
                    // it to an unrelated missing law.
                    let reason=if group=="decision" {coverage["decisions"]["reason"].as_str()}else{None};
                    rows.push(json!({"id":id,"group":group,"status":"unknown","reason":reason.unwrap_or("Native purchase target is not qualified")}));
                }
            }
        }
        let partial=rows.iter().any(|r|r["status"]!="calculated");
        Ok(json!({"schema_version":1,"status":if partial{"partial"}else{"observed"},"source":"native_political_purchase_formula","scope":"qualified_native_purchase_targets","coverage":coverage,"rows":rows}))
    }
    fn decision_cost_rows(&mut self,wanted:&BTreeMap<String,BTreeSet<String>>)->Result<Vec<Value>> {
        let Some(ids)=wanted.get("decision") else {return Ok(vec![])};
        self.function_signatures("decision_costs","Native decision price is not qualified")?;
        if !self.attributes["abi_functions"]["decision_costs"].as_array().is_some_and(|a|a.len()==5) {return Err("Incomplete decision price evidence".into());}
        let db_rva=self.attributes["decisions_database_rva"].as_u64().ok_or("Decision cost registry unavailable")?;
        let db=self.object_pointer(self.d.base+db_rva,"CDecisionDatabase")?;
        let cheat=self.attributes["decision_cost_cheat_rva"].as_u64().ok_or("Decision cost mode unavailable")?;
        if self.track(self.d.base+cheat,1)?[0]!=0 {return Err("Decision cost cheat mode is not qualified".into());}
        let mut rows=vec![];let mut seen=BTreeSet::new();
        for decision in self.array(db+0x28,16384)?.into_iter().skip(1) {
            self.typed(decision,"CDecision")?;let id=self.d.string(decision+0x120)?;
            if !ids.contains(&id) {continue;}
            if !seen.insert(id.clone()) {return Err("Ambiguous decision cost identity".into());}
            let calculation=(||->Result<(i64,i64)>{
                if self.integer(decision+0x654,0,16)?!=0 {return Err("Decision uses custom costs".into());}
                let flags=self.track(decision+0xb00,2)?;
                if flags.iter().any(|n|*n>1) {return Err("Invalid decision cost flags".into());}
                if self.track(decision+0x519,1)?[0]!=1 {return Err("Scripted decision cost is not a qualified constant".into());}
                let cost=self.cost_fixed(decision+0x550)?;
                if cost<0 {return Err("Negative decision cost is not qualified".into());}Ok((cost,if flags==[1,0]{0}else{cost}))
            })();
            match calculation {
                Ok((base,cost))=>rows.push(json!({"id":id,"group":"decision","currency":"political_power","base":base as f64/SCALE as f64,"effective":cost as f64/SCALE as f64,"factor":if base>0{cost as f64/base as f64}else{1.},"status":"calculated"})),
                Err(reason)=>rows.push(json!({"id":id,"group":"decision","status":"unknown","reason":reason})),
            }
        }
        Ok(rows)
    }
    fn advisor_cost_rows(&mut self,wanted:&BTreeMap<String,BTreeSet<String>>,values:&BTreeMap<i64,i64>,definitions:u64)->Result<Vec<Value>> {
        self.function_signatures("advisors_costs","Native advisor price formula is not qualified")?;
        if !self.attributes["abi_functions"]["advisors_costs"].as_array().is_some_and(|a|a.len()==3) {return Err("Incomplete advisor cost evidence".into());}
        let manager=self.object_pointer(self.country+0xff0,"CCountryCharacters")?;
        if self.pointer(manager+8)?!=self.country {return Err("Advisor costs belong to another country".into());}
        let mut rows=vec![];let mut identities=BTreeSet::new();let mut characters=BTreeSet::new();
        for entry in self.inline(manager+0x10,16,4096)? {
            let character=self.object_pointer(entry,"CCharacter")?;
            if self.integer(character+0x68,1,8191)?!=self.tag as i64||!characters.insert(character) {return Err("Advisor cost character owner mismatch".into());}
            let head=self.pointer(character+0xb0)?;let count=self.pointer(character+0xb8)?;
            if count>64||head<65536||self.track(head+0x19,1)?[0]!=1 {return Err("Invalid advisor role tree".into());}
            let mut pending=vec![(self.pointer(head+8)?,head)];let mut seen=BTreeSet::new();
            while let Some((node,parent))=pending.pop() {
                if node==head {continue;}
                if !seen.insert(node)||seen.len()>count as usize||self.pointer(node+8)?!=parent||self.track(node+0x19,1)?[0]!=0 {return Err("Invalid advisor role tree links".into());}
                pending.push((self.pointer(node)?,node));pending.push((self.pointer(node+0x10)?,node));
                let role=self.object_pointer(node+0x48,"CAdvisor")?;
                if self.pointer(role+0x10)?!=character {return Err("Advisor cost reciprocal character mismatch".into());}
                self.object_pointer(role+0x18,"CAdvisorTemplate")?;
                let id=self.d.string(role+0x30)?;let group=self.d.string(role+0x80)?;
                if !wanted.get(&group).is_some_and(|ids|ids.contains(&id)) {continue;}
                if !identities.insert((group.clone(),id.clone())) {return Err("Ambiguous advisor price identity".into());}
                let calculation=(||->Result<(i64,i64,i64)>{
                    let base=self.cost_fixed(role+0xa8)?;
                    if base<0 {return Err("Native advisor cannot be purchased".into());}
                    let index=self.integer(role+0x278,0,16383)?;
                    let first=SCALE+self.cost_modifier(index,values,definitions)?;
                    let military=match self.integer(role+0x28,0,16)? {4=>Some(337),8=>Some(29),16=>Some(20),0=>None,_=>return Err("Advisor ledger enum is not qualified".into())};
                    let second=SCALE+match military {Some(index)=>self.cost_modifier(index,values,definitions)?,None=>0};
                    Ok((base,modified(modified(base,first)?,second)?,first.checked_mul(second).ok_or("Advisor price factor overflow")?/SCALE))
                })();
                match calculation {
                    Ok((base,effective,factor))=>rows.push(json!({"id":id,"group":group,"currency":"political_power","base":base as f64/SCALE as f64,"effective":effective as f64/SCALE as f64,"factor":factor as f64/SCALE as f64,"status":"calculated"})),
                    Err(reason)=>rows.push(json!({"id":id,"group":group,"status":"unknown","reason":reason})),
                }
            }
            if seen.len()!=count as usize {return Err("Incomplete advisor role tree".into());}
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn native_fixed_point_cost_uses_both_modifiers_and_truncates() {
        assert_eq!(modified(15_000_000,80_000).unwrap(),12_000_000);
        assert_eq!(modified(15_000_000,125_000).unwrap(),18_750_000);
        assert_eq!(modified(100_001,33_333).unwrap(),33_333);
        assert!(modified(i64::MAX,100_000).is_err());
        assert!(modified(1_000_000,-100_000).is_err());
    }
}
