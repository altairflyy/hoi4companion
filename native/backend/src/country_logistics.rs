//! Native logistics formulas, evaluated from witnessed objects. No wall-clock
//! samples, stockpile deltas or inferred history slopes are used here.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value,json};
use std::collections::{BTreeMap,HashSet};
const SCALE:i64=100_000;
// The native fixed-point operators saturate at +/- 0x53e2d620fb40.
const LIMIT:i64=92_233_720_200_000;
fn add(a:i64,b:i64)->i64 {a.saturating_add(b).clamp(-LIMIT,LIMIT)}
fn flags(mut x:u64)->u64 {
    if x & 0x80004003c1 != 0 {
        if x&0x40!=0 {x&=!0x181} else if x&0x80!=0 {x&=!0x101} else if x&0x100!=0 {x&=!1}
    } x
}
fn amount(n:i64)->Value {json!(n as f64/SCALE as f64)}
#[derive(Clone)]
struct Equipment {p:u64,id:String,column:usize,land:bool,air:bool,train:bool,motorized:bool,raw_flags:u64}
impl<M:Memory> Reader<'_,M> {
    fn logistics_fixed(&mut self,p:u64)->Result<i64> {
        let n=i64::from_le_bytes(self.track(p,8)?.try_into().unwrap());
        if !(-LIMIT..=LIMIT).contains(&n) {return Err("Native logistics quantity outside bounds".into())} Ok(n)
    }
    fn logistics_pool(&mut self,p:u64,archetype:bool)->Result<BTreeMap<u64,i64>> {
        self.typed(p,if archetype {"CEquipmentArcheTypePool"} else {"CEquipmentVariantPool"})?;
        let mut out=BTreeMap::new();
        for row in self.inline(p+8,if archetype {16}else{24},4096)? {
            let key=self.object_pointer(row,"CEquipmentType")?;
            if self.pointer(key+0x538)?!=key {return Err("Logistics pool key is not an archetype".into())}
            let n=self.logistics_fixed(row+if archetype {8}else{16})?;
            if out.insert(key,n).is_some() {return Err("Duplicate logistics pool archetype".into())}
        } Ok(out)
    }
    fn logistics_difference(&mut self,need:u64,produced:u64)->Result<BTreeMap<u64,i64>> {
        let mut out=self.logistics_pool(need,true)?;
        for (p,n) in self.logistics_pool(produced,false)? {let v=out.entry(p).or_default();*v=add(*v,-n)}
        Ok(out)
    }
    fn logistics_requests(&mut self,header:u64,parent:u64,garrison:bool)->Result<BTreeMap<u64,i64>> {
        let mut out=BTreeMap::new();let mut seen=HashSet::new();
        for group in self.array(header,4096)? {
            self.typed(group,if garrison {"CGarrisonReinforcementRequests"}else{"CArmyReinforcementRequests"})?;
            if self.pointer(group+0x40)?!=parent {return Err("Logistics group belongs to another country manager".into())}
            let owner=self.pointer(group+0x20)?;
            if garrison {
                self.typed(owner,"CStateGarrisonData")?;
                if owner+8!=group {return Err("Garrison group owner mismatch".into())}
                self.object_pointer(group+0x48,"CState")?;
            }else{
                self.typed(owner,"CArmy")?;
                if self.integer(owner+0x1d8,1,8191)?!=self.tag as i64 {return Err("Reinforcement army belongs to another country".into())}
            }
            // Country getters return the complete group array; serializers and
            // field-bearing aggregator qualify the child header and both pools.
            for request in self.array(group+0x28,16384)? {
                self.typed(request,"CReinforcementRequest")?;
                if !seen.insert(request) {return Err("Duplicate logistics request".into())}
                for (p,n) in self.logistics_difference(request+0x78,request+0x38)? {let v=out.entry(p).or_default();*v=add(*v,n)}
            }
        } Ok(out)
    }
    fn logistics_history(&mut self,status:u64,event:usize,days:usize,columns:usize)->Result<Vec<i64>> {
        let histories=self.array(status+8,19)?;
        if histories.len()!=19 || event>=19 {return Err("Incomplete logistics event registry".into())}
        let history=self.typed(histories[event],"CLoopHistory")?;
        let queue=self.object_pointer(history+0x10,"CLoopHistoryContainer")?;
        let rows=self.array(queue+8,4096)?;
        let size=self.integer(queue+0x20,1,4096)? as usize;
        let cursor=self.integer(queue+0x24,0,size as i64)? as usize;
        let wrapped=self.track(queue+0x28,1)?[0];
        if rows.len()!=size || wrapped>1 || days>size {return Err("Invalid native daily history window".into())}
        let count=if wrapped==0 {cursor.min(days)} else {days};
        let mut sums=vec![0;columns];
        for back in 0..count {
            let index=(cursor+size-1-back)%size;
            // CLoopHistoryEntry is a plain fixed quantity array, not an RTTI object.
            let values=self.inline(rows[index],8,4096)?;
            if values.len()!=columns {return Err("Logistics history columns differ from equipment catalog".into())}
            for (column,at) in values.into_iter().enumerate() {sums[column]=add(sums[column],self.logistics_fixed(at)?)}
        } Ok(sums)
    }
    pub(super) fn logistics(&mut self)->Result<Value> {
        self.function_signatures("logistics","Native national logistics formulas are not qualified")?;
        if !self.attributes["abi_functions"]["logistics"].as_array().is_some_and(|v|v.len()==28) {return Err("Incomplete native logistics formula evidence".into())}
        self.typed(self.country,"CCountry")?;
        if self.integer(self.country+8,1,8191)?!=self.tag as i64 {return Err("Logistics country identity changed".into())}
        let db_rva=self.attributes["equipment_database_rva"].as_u64().ok_or("Missing qualified equipment database")?;
        let days_rva=self.attributes["logistics_days_rva"].as_u64().ok_or("Missing qualified logistics interval")?;
        let db=self.object_pointer(self.d.base+db_rva,"CEquipmentDatabase")?;
        let definitions=self.array(db+0x80,4096)?;
        let mut eq=vec![];let mut keys=HashSet::new();
        for (column,p) in definitions.iter().copied().enumerate() {
            self.typed(p,"CEquipmentType")?;
            if self.pointer(p+0x538)?!=p || self.track(p+0x5b5,1)?!=[1] || self.integer(p+0x598,0,4095)?!=column as i64 {return Err("Invalid native equipment archetype catalog".into())}
            let id=self.d.string(p+0x18)?;
            if id.is_empty()||!keys.insert(id.clone()) {return Err("Duplicate native equipment identity".into())}
            let raw_flags=self.pointer(p+0x5a0)?;
            let f=flags(raw_flags|self.pointer(p+0x5a8)?);
            let motorized=self.track(p+0x44b,1)?[0];if motorized>1 {return Err("Invalid native motorized classification".into())}
            eq.push(Equipment{p,id,column,land:f&0x408000003c!=0,air:f&0x1f0037fc00!=0,train:f&(1<<31)!=0,motorized:motorized!=0,raw_flags});
        }
        if eq.is_empty() {return Err("Empty native equipment archetype registry".into())}
        let days=self.integer(self.d.base+days_rva,1,30)? as usize;
        let production=self.object_pointer(self.country+0xf68,"CProductionStatus")?;
        let production_owner_offset=self.attributes["production_owner_offset"].as_u64().unwrap_or(0x290);
        if self.pointer(production+production_owner_offset)?!=self.country {return Err("Production logistics belongs to another country".into())}
        let status=self.object_pointer(self.country+0xf98,"CLogisticsStatus")?;
        if self.pointer(status+0x20)?!=self.country {return Err("Logistics belongs to another country".into())}
        let known:HashSet<_>=eq.iter().map(|x|x.p).collect();
        let mut gross=BTreeMap::new();
        self.typed(production+0x200,"CEquipmentVariantPool")?;
        for row in self.inline(production+0x220,16,4096)? {
            let variant=self.object_pointer(row,"CEquipmentVariant")?;
            let model=self.object_pointer(variant+0x3f0,"CEquipmentType")?;
            let p=self.pointer(model+0x538)?;
            if !known.contains(&p) {return Err("Stockpile contains an unknown equipment archetype".into())}
            let n=self.logistics_fixed(row+8)?;if n<0 {return Err("Negative gross stockpile".into())}
            let v=gross.entry(p).or_default();*v=add(*v,n);
        }
        let mut output=BTreeMap::new();let mut invalid_output=HashSet::new();
        for line in self.array(production+0x58,4096)? {
            let kind=self.d.kind(line)?.1;
            if ![".?AVCMilitaryProductionLine@@",".?AVCNavalProductionLine@@",".?AVCEquipmentProductionLine@@"].contains(&kind.as_str()) {return Err("Unknown native production line family".into())}
            let variant=self.object_pointer(line+0x88,"CEquipmentVariant")?;
            let model=self.object_pointer(variant+0x3f0,"CEquipmentType")?;
            let p=self.pointer(model+0x538)?;
            if !known.contains(&p) {return Err("Production contains an unknown equipment archetype".into())}
            // Native UI sums a seven-day rate, then divides by configured days.
            // Keep its fixed numerator and integer truncation exactly.
            let cost=self.logistics_fixed(line+0x28)?;
            let rate=self.logistics_fixed(line+0x30)?;
            if cost<=0||rate<0 {invalid_output.insert(p);continue}
            let Some(numerator)=rate.checked_mul(7).and_then(|n|n.checked_mul(SCALE)) else {invalid_output.insert(p);continue};
            let n=numerator/cost;
            if n>LIMIT {invalid_output.insert(p);continue}
            let v=output.entry(p).or_default();*v=add(*v,n);
        }
        let mut used=vec![0;eq.len()];let mut history_error=None;
        for event in [1,2,3,4,5,18] {
            match self.logistics_history(status,event,days,eq.len()) {Ok(values)=>for (i,n) in values.into_iter().enumerate(){used[i]=add(used[i],n)},Err(e)=>{history_error=Some(e);break}}
        }
        // Collect each national component once. A missing/unreadable component
        // invalidates need, while independent daily flow remains useful.
        let needs=self.logistics_national_needs(status,&eq);
        let stockpiles=self.logistics_available_stockpiles(status,&eq);
        let mut items=vec![];let mut partial=false;
        for item in eq {
            let stock=*gross.get(&item.p).unwrap_or(&0);
            let need=needs.as_ref().ok().map(|m|*m.get(&item.p).unwrap_or(&0));
            let usable=stockpiles.as_ref().ok().map(|m|*m.get(&item.p).unwrap_or(&0));
            let daily=if history_error.is_none()&&!invalid_output.contains(&item.p) {Some((add(*output.get(&item.p).unwrap_or(&0),-used[item.column]))/days as i64)}else{None};
            let reason=needs.as_ref().err().cloned().or_else(||stockpiles.as_ref().err().cloned()).or_else(||history_error.clone()).or_else(||invalid_output.contains(&item.p).then(||"Native production rate is undefined".into()));
            partial|=reason.is_some();
            if stock==0&&usable==Some(0)&&need==Some(0)&&daily==Some(0){continue}
            items.push(json!({"id":item.id,"aggregation":"equipment_archetype","gross_stockpile":amount(stock),"logistics_stockpile":usable.map(amount),"needed":need.map(amount),"available":usable.zip(need).map(|(stock,n)|amount(add(stock,-n))),"balance_per_day":daily.map(amount),"status":if reason.is_some(){"partial"}else{"observed"},"reason":reason}));
        }
        Ok(json!({"schema_version":1,"status":if partial{"partial"}else{"observed"},"source":"native_logistics_ui_formula","aggregation":"equipment_archetype","interval_days":days,"items":items,"reason":"Fabbisogno e disponibilità per archetipo; flusso giornaliero dalla formula nativa della Logistica e dai suoi registri giornalieri. Scorte lorde per modello e variante restano separate."}))
    }
    fn logistics_supply(&mut self,eq:&[Equipment])->Result<u64> {
        if !eq.iter().any(|e|e.train||e.motorized) {return Ok(0)}
        let (mapping,_)=self.country_mapping("Logistics world identity changed","Logistics country slot mismatch")?;
        let slot=mapping[self.tag as usize] as u64;
        let manager=self.pointer(self.world+0x3d8)?;
        let countries=self.pointer(manager+0x108)?;
        let supply=self.object_pointer(countries+slot*8,"CCountrySupplySystem")?;
        if self.pointer(supply+0x78)?!=self.country {return Err("Logistics supply reservation belongs to another country".into())}
        Ok(supply)
    }
    fn logistics_available_stockpiles(&mut self,status:u64,eq:&[Equipment])->Result<BTreeMap<u64,i64>> {
        let histories=self.array(status+8,19)?;
        if histories.len()!=19 {return Err("Incomplete native stockpile history registry".into())}
        let h=self.typed(histories[15],"CLoopHistory")?;
        let container=self.object_pointer(h+0x10,"CLoopHistoryContainer")?;
        let current=self.pointer(container+0x30)?;
        if !self.array(container+8,4096)?.contains(&current) {return Err("Native current stockpile record is outside its history container".into())}
        let values=self.inline(current,8,4096)?;
        if values.len()!=eq.len() {return Err("Native current stockpile columns differ from equipment catalog".into())}
        let supply=self.logistics_supply(eq)?;
        let mut out=BTreeMap::new();
        for e in eq {
            let mut n=self.logistics_fixed(values[e.column])?;
            // Current UI stock record, never a difference between samples.
            if e.motorized {
                n=n.checked_add((self.integer(supply+0x138,0,900_000_000)?-self.integer(supply+0xc8,0,900_000_000)?)*SCALE).ok_or("Native motorized reservation overflow")?;
            }else if e.train {
                let rail=self.pointer(supply+0x80)?;
                n=n.checked_sub(self.integer(rail+0x140,0,900_000_000)?*SCALE).ok_or("Native rail reservation overflow")?;
            }
            n=n.max(0);
            if n>LIMIT {return Err("Native available stockpile outside fixed bounds".into())}
            out.insert(e.p,n);
        }
        Ok(out)
    }
    fn logistics_national_needs(&mut self,status:u64,eq:&[Equipment])->Result<BTreeMap<u64,i64>> {
        let reinforcement=self.object_pointer(self.country+0xf78,"CReinforcementStatus")?;
        if self.pointer(reinforcement+0x38)?!=self.country{return Err("Reinforcement demand belongs to another country".into())}
        let land=self.logistics_requests(reinforcement+0x20,reinforcement,false)?;
        let garrison=self.object_pointer(self.country+0xfd0,"CCountryOccupationStatus")?;
        if self.pointer(garrison+0x20)?!=self.country {return Err("Garrison demand belongs to another country".into())}
        let garrison=self.logistics_requests(garrison+0x28,garrison,true)?;
        let deployment=self.object_pointer(self.country+0xf70,"CDeploymentStatus")?;
        if self.pointer(deployment+0x118)?!=self.country {return Err("Queued equipment demand belongs to another country".into())}
        let mut queued=BTreeMap::new();
        for unit in self.array(deployment+0x60,16384)? {
            let ext=self.pointer(unit+0xb8)?;
            let template=self.pointer(ext+0x38)?;
            let definition=self.pointer(template+0x20)?;
            let need=self.logistics_pool(definition+0x120,true)?;
            let held=self.logistics_pool(unit+0x20,false)?;
            for (p,n) in need {let v=queued.entry(p).or_default();*v=add(*v,add(n,-held.get(&p).copied().unwrap_or(0)).max(0))}
        }
        let relations=self.object_pointer(self.country+0x11f8,"CCountryResources")?;
        if self.integer(relations+0x20,1,8191)?!=self.tag as i64 {return Err("Lend-lease equipment demand belongs to another country".into())}
        let mut lend=BTreeMap::new();
        for agreement in self.array(relations+0x758,8192)? {
            for (p,n) in self.logistics_difference(agreement+0x198,agreement+0x1d8)? {let v=lend.entry(p).or_default();*v=add(*v,n)}
        }
        let manager=self.object_pointer(self.country+0x15a8,"CCountryOperationManager")?;
        if self.pointer(manager+8)?!=self.country {return Err("Operation demand belongs to another country".into())}
        let mut operation=BTreeMap::new();
        for op in self.array(manager+0x10,4096)? {
            let held=self.logistics_pool(op+0x110,false)?;
            let mut seen=HashSet::new();
            for row in self.inline(op+0x158,32,4096)? {
                let p=self.object_pointer(row+0x10,"CEquipmentType")?;
                if self.pointer(p+0x538)?!=p||self.track(p+0x10,1)?!=[1]||self.track(p+0x5b5,1)?!=[1] {continue}
                if !seen.insert(p) {continue} // Native lookup returns the first requirement.
                let n=add(self.logistics_fixed(row)?,-held.get(&p).copied().unwrap_or(0));
                let v=operation.entry(p).or_default();*v=add(*v,n);
            }
        }
        let mut air=BTreeMap::new();
        for row in self.inline(status+0x28,24,8192)? {
            let p=self.object_pointer(row,"CEquipmentType")?;
            let n=self.integer(row+0x10,0,900_000_000)?*SCALE;
            let v=air.entry(p).or_default();*v=add(*v,n);
        }
        let mut out=BTreeMap::new();
        let known:HashSet<_>=eq.iter().map(|e|e.p).collect();
        if [&land,&garrison,&queued,&lend,&operation,&air].iter().any(|map|map.keys().any(|p|!known.contains(p))) {return Err("National demand contains an unknown equipment archetype".into())}
        // Special train/motorized adjustments are qualified through the national
        // supply manager's country-indexed slot, never another country's row.
        let supply=self.logistics_supply(eq)?;
        for e in eq {
            let mut n=0;
            if e.land {
                n=add(*queued.get(&e.p).unwrap_or(&0),*land.get(&e.p).unwrap_or(&0));
                if e.train {let rail=self.pointer(supply+0x80)?;n=add(n,(self.integer(rail+0x140,0,900_000_000)?-self.integer(supply+0x110,0,900_000_000)?)*SCALE).max(0)}
                else if e.motorized&&e.raw_flags==8 {n=add(n,(self.integer(supply+0xc8,0,900_000_000)?-self.integer(supply+0x138,0,900_000_000)?)*SCALE)}
                n=add(n,*garrison.get(&e.p).unwrap_or(&0));
            }
            if e.air {n=add(n,*air.get(&e.p).unwrap_or(&0))}
            n=add(n,*operation.get(&e.p).unwrap_or(&0));
            n=add(n,*lend.get(&e.p).unwrap_or(&0));
            out.insert(e.p,n);
        }
        Ok(out)
    }
}




