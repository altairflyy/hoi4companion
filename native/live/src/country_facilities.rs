//! Exact scientist-to-facility assignments, separate from role inventory.
use super::{Reader,Result};
use crate::live::Memory;
use serde_json::{Value,json};
use std::collections::{HashMap,HashSet};
impl<M:Memory> Reader<'_,M> {
    pub(super) fn facility_assignments(&mut self, facilities:&[u64])->Result<Value> {
        self.function_signatures("facility_assignments","Scientist assignments are not qualified")?;
        if !self.attributes["abi_functions"]["facility_assignments"].as_array().is_some_and(|a|a.len()==3) {return Err("Missing scientist assignment serializer evidence".into());}
        let manager=self.object_pointer(self.country+0xff0,"CCountryCharacters")?;
        if self.pointer(manager+8)?!=self.country {return Err("Scientist roster owner mismatch".into());}
        let mut scientists=HashMap::new();
        for row in self.inline(manager+0x10,16,4096)? {
            let flag=self.track(row+11,1)?[0];if flag>1 {return Err("Invalid scientist role flag".into());}if flag==0 {continue;}
            let person=self.object_pointer(row,"CCharacter")?;
            if self.integer(person+0x68,1,8191)?!=self.tag as i64 {return Err("Scientist belongs to another country".into());}
            let scientist=self.object_pointer(person+0xc0,"CScientist")?;
            let assigned=self.track(scientist+0x130,1)?[0];if assigned>1 {return Err("Invalid scientist assignment flag".into());}
            let handle=self.track(person+8,8)?;let token=self.integer(person+0x18,1,1_000_000)?;
            let id=self.token_name(token)?;
            if handle==[0;8] || scientists.insert(handle,(id,assigned==1)).is_some() {return Err("Duplicate scientist identity".into());}
        }
        let mut rows=vec![];let mut assigned_here=HashSet::new();
        for &p in facilities {
            let handle=self.track(p+0x58,8)?;
            let scientist=if handle==[0;8] {Value::Null} else {
                let (id,assigned)=scientists.get(&handle).ok_or("Unknown facility scientist")?;
                if !assigned || !assigned_here.insert(handle) {return Err("Scientist facility reciprocity mismatch".into());}
                json!(id)
            };
            let selected=self.track(p+0x54,1)?[0];if selected>1 {return Err("Invalid facility selection flag".into());}
            rows.push(json!({"instance_id":super::hex(&self.track(p+8,8)?),"building_id":self.d.string(p+0x20)?,"scientist_id":scientist,"busy":selected==1}));
        }
        Ok(json!({"schema_version":1,"status":"observed","source":"native_facility_scientist_assignments","items":rows}))
    }
}
