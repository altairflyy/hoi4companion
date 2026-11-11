//! Qualified land recruitment lines. Repeat orders are not deployed divisions.
use super::{Reader, Result};
use crate::live::Memory;
use serde_json::{Value,json};
use std::collections::HashSet;
impl<M:Memory> Reader<'_,M> {
    pub(super) fn training(&mut self)->Result<Value> {
        self.function_signatures("training", "Land recruitment layout is not qualified")?;
        if !self.attributes["abi_functions"]["training"].as_array().is_some_and(|v|v.len()==4) {return Err("Missing land recruitment serializer evidence".into());}
        self.typed(self.country,"CCountry")?;
        if self.integer(self.country+8,1,8191)?!=self.tag as i64 {return Err("Recruitment country identity changed".into());}
        let manager=self.object_pointer(self.country+0xf70,"CDeploymentStatus")?;
        if self.pointer(manager+0x118)?!=self.country {return Err("Recruitment manager belongs to another country".into());}
        let mut rows=vec![];let mut units=HashSet::new();let mut manpower=0i64;
        for conveyor in self.array(manager+0x48,4096)? {
            let kind=self.d.kind(conveyor)?.1;
            if [".?AVCNavalDeployment@@",".?AVCShipRefitDeployment@@",".?AVCRocketDeployment@@"].contains(&kind.as_str()) { continue; }
            // Only known non-land families can be excluded from this subtotal.
            self.typed(conveyor,"CMilitaryDeploymentConveyor")?;
            if self.integer(conveyor+0x18,0,65535)?!=0x34b2 {return Err("Unexpected land deployment family".into());}
            let template=self.object_pointer(conveyor+0x20,"CReferencedDivisionTemplate")?;
            let template_id=super::hex(&self.track(template+8,8)?);
            let requested=self.integer(conveyor+0x48,-1,1_000_000)?;
            let closed=self.track(conveyor+0x94,1)?[0];if closed>1 {return Err("Invalid recruitment closed flag".into());}
            let mut items=vec![];
            for line in self.array(conveyor+0x78,8192)? {
                self.typed(line,"CMilitaryDeploymentLine")?;
                if self.pointer(line+0x38)?!=conveyor {return Err("Recruitment line belongs to another conveyor".into());}
                let deployed_template=self.object_pointer(line+0x10,"CReferencedDivisionTemplate")?;
                if deployed_template!=template {return Err("Recruitment template mismatch".into());}
                let unit=self.object_pointer(line+0x30,"CMilitaryDeployment")?;
                let identity=super::hex(&self.track(unit+0x20,8)?);
                if !units.insert(unit) {return Err("Duplicate military deployment instance".into());}
                let current=self.typed(unit+0x70,"CArmyManpower")?;
                let mut men=0i64;
                for row in self.inline(current+8,8,1024)? {
                    self.integer(row,1,1_000_000)?;
                    men+=self.integer(row+4,0,2_000_000_000)?;
                }
                manpower=manpower.checked_add(men).filter(|v|*v<=2_000_000_000).ok_or("Recruitment manpower outside bounds")?;
                items.push(json!({"instance_id":identity,"native_line_amount":self.integer(line+0x28,-1,1_000_000)?,"manpower":men}));
            }
            rows.push(json!({"template_id":template_id,"template_name":self.name(template+0x20)?,"requested":if requested<0 {Value::Null}else{json!(requested)},"repeat":requested<0,"closed":closed==1,"parallel_lines":items.len(),"items":items}));
        }
        Ok(json!({"schema_version":1,"status":"observed","source":"native_land_deployment_lines","scope":"land_queue_only","land_manpower":manpower,"rows":rows}))
    }
}
