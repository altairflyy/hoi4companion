//! Runtime binding of fully understood construction instructions. Keys and text stay intact.
use crate::{import_catalog::Index, geography::Geography, construction_binding};
use serde_json::{Value,json};
fn s(v:&Value)->&str { v.as_str().unwrap_or("") }
pub(crate) fn bind(guide:&Value,index:&Index)->Result<Value,String>{
    let mut guide=guide.clone();
    let geography=Geography::new(index.rows());
    let pattern=crate::import_catalog::regex(r"(?i)^\s*(?:build|construct)\s*:?\s+(.+?)\s+in\s*:?\s+(.+?)\s*$");
    let split=crate::import_catalog::regex(r"\s*(?:\+|&)\s*");
    for step in guide["tracks"].as_object_mut().ok_or("Missing tracks")?.values_mut().flat_map(|v|v.as_array_mut().into_iter().flatten()) {
        if step["action"]["kind"]!="manual" || step.get("completion").is_some() || step["action"]["params"]["category"]!="checkpoints" {continue;}
        let text=s(&step["title"]).to_owned();
        let Some(hit)=pattern.captures(&text) else {continue};
        let mut buildings=vec![];
        let mut valid=true;
        for token in split.split(&hit[1]) {
            let query=match token.trim().to_ascii_lowercase().as_str(){
                "infra"=>"infrastructure", "mil"|"mils"|"military factories"=>"arms_factory",
                "civ"|"civs"|"civilian factories"=>"industrial_complex", _=>token.trim()
            };
            let rows=index.entries(json!({"op":"matches","query":query,"kinds":["building"]}))?;
            if rows.len()!=1 {valid=false;break;} buildings.push(rows[0].clone());
        }
        let places=hit[2].split(',').map(str::trim).collect::<Vec<_>>();
        if !valid || buildings.is_empty() || places.is_empty() || buildings.len()*places.len()>64 {continue;}
        let mut targets=vec![];
        for place in places {
            for building in &buildings {
                let provincial=building["location_scope"]=="province";
                let location=geography.resolve(&json!({"state_name":place}),provincial);
                if !location["state_id"].is_number() {valid=false;break;}
                let mut target=json!({"category":"construction","building_id":building["id"],"building_scope":building["location_scope"],"state_id":location["state_id"],"state_name":location["state_name"],"quantity_mode":"maximum"});
                if provincial {target["province_id"]=location["province_id"].clone();}
                target["building_name"]=building.get("localized_name").filter(|v|!s(v).is_empty()).unwrap_or(&building["name"]).clone();
                // A catalog ceiling suffices only for non-shared fixed caps.
                // Shared industrial slots require live effective capacity.
                if building["shares_slots"]==false && building["max_level"].as_u64().is_some_and(|n|n>0) {
                    target["target_level"]=building["max_level"].clone();
                }
                targets.push(target);
            }
        }
        if !valid {continue;}
        step["action"]["params"]["category"]=json!("construction");
        step["action"]["params"]["construction_targets"]=json!(targets);
        step["action"]["params"]["completion_mode"]=json!("queued_or_built");
        if step["tracking"]=="reminder" {step.as_object_mut().unwrap().remove("tracking");}
        *step=construction_binding::bind(step,index,&geography)?["step"].clone();
    }
    Ok(guide)
}

#[cfg(test)]
mod tests {
 use super::*;
 fn index()->Index {Index::new(&json!({"entries":[
 {"kind":"building","id":"infrastructure","name":"Infrastructure","location_scope":"state","shares_slots":false,"max_level":5},
 {"kind":"building","id":"land_facility","name":"Land Facility","location_scope":"province","shares_slots":false,"max_level":1,"specializations":["specialization_land"]},
 {"kind":"building","id":"arms_factory","name":"Military Factory","location_scope":"state","shares_slots":true,"max_level":20},
 {"kind":"state","id":"219","name":"Moscow","geography":{"province_ids":[6380],"cities":[{"name":"Moscow","province_id":6380}]}}
 ]})).unwrap()}
 fn guide(text:&str)->Value {json!({"id":"build-test","dependency_policy":"explicit","tracks":{"operations":[{"key":"build","title":text,"action":{"kind":"manual","params":{"category":"checkpoints"}}}]}})}
 #[test] fn real_compound_binds_to_distinct_state_and_province_targets() {
  let g=bind(&guide("Build: INFRA+Land Facility in Moscow"),&index()).unwrap();
  let row=&g["tracks"]["operations"][0];
  assert_eq!(row["action"]["params"]["construction_targets"][0]["state_id"],219);
  assert_eq!(row["action"]["params"]["construction_targets"][1]["province_id"],6380);
  let fact=|v|json!({"status":"observed","source":"fixture","value":v});
  let mut state=json!({"schema_version":1,"usable":true,"facts":{"country.states.items.219.infrastructure":fact(5),"country.fortifications.items.6380.buildings.land_facility":fact(1)}});
  let evaluate=|s:&Value|crate::guide_runtime::evaluate(&g,s,&Value::Null).unwrap()["steps"][0]["status"].clone();
  assert_eq!(evaluate(&state),"completed");
  state["facts"]["country.fortifications.items.6380.buildings.land_facility"]["value"]=json!(0);
  assert_ne!(evaluate(&state),"completed");
 }
 #[test] fn industrial_maximum_is_not_the_static_shared_slot_cap() {
  let g=bind(&guide("Build: INFRA+MILs in: Moscow"),&index()).unwrap();
  let row=&g["tracks"]["operations"][0];
  assert!(row["action"]["params"]["construction_targets"][1]["target_level"].is_null());
  assert_eq!(row["completion"]["args"][1]["key"],"construction.queued");
  assert_eq!(row["completion"]["args"][1]["value"],"state:219:arms_factory");
  assert_eq!(bind(&guide("Build: INFRA+Land Facility in Unknown Place"),&index()).unwrap(),guide("Build: INFRA+Land Facility in Unknown Place"));
 }
 #[test] fn queue_start_satisfies_every_required_target_by_id() {
  let g=bind(&guide("Build: INFRA+Land Facility in Moscow"),&index()).unwrap();
  let queue=|values:Value|json!({"schema_version":1,"usable":true,"facts":{"construction.queued":{"status":"calculated","source":"fixture","value":values}}});
  let evaluate=|s:&Value|crate::guide_runtime::evaluate(&g,s,&Value::Null).unwrap()["steps"][0]["status"].clone();
  assert_eq!(evaluate(&queue(json!(["state:219:infrastructure","province:6380:land_facility"]))),"completed");
  assert_ne!(evaluate(&queue(json!(["state:219:infrastructure","province:9999:land_facility"]))),"completed");
  assert_ne!(evaluate(&queue(json!([]))),"completed");
 }
}
