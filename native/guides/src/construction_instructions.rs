//! Runtime binding of fully understood construction instructions. Keys and text stay intact.
//! A line binds only when every part of it is understood: an optional verb
//! (`Build:`), building lists (abbreviations, plurals, `full`/`max`), `then`
//! phases, places and trailing priority notes. `then` orders the targets for
//! the reader; it is never a completion barrier, every target is required.
//! A place is an exact state name, or a unique prefix ("Mun") among the states
//! the guide's nation owns or cores at game start. An ambiguous or unknown part
//! leaves the whole line an unbound manual step.
use crate::{import_catalog::Index, geography::Geography, construction_binding};
use serde_json::{Value,json};
fn s(v:&Value)->&str { v.as_str().unwrap_or("") }
/// One `then` phase: building words and the places they go to.
pub(crate) struct Phase { buildings:Vec<String>, places:Vec<String> }
pub(crate) fn parse(text:&str)->Option<Vec<Phase>> {
    let re=crate::import_catalog::regex;
    let note=re(r"(?i)^(?:(?:max|maximum|highest|top|lowest)\s+)?(?:prio|priority)$|^future\s+in\s+best$");
    let trailing=re(r"\s*\(([^()]*)\)\s*$");
    let mut body=text.trim();
    while let Some(hit)=trailing.captures(body) {
        if !note.is_match(hit[1].trim()) {return None;}
        body=body[..hit.get(0).unwrap().start()].trim_end();
    }
    let body=re(r"(?i)^(?:build|construct)\b\s*:?\s*").replace(body,"");
    let body=body.trim().trim_end_matches('.').trim();
    let split_in=re(r"(?i)^(.+?),?\s+in\s*:?\s+(.+)$");
    let list=re(r"\s*(?:\+|&|,|\band\b)\s*");
    let items=|text:&str|->Option<Vec<String>>{
        let items=list.split(text).map(|t|t.trim().to_owned()).collect::<Vec<_>>();
        (!items.iter().any(String::is_empty)).then_some(items)
    };
    let mut phases=vec![];
    for part in re(r"(?i)\s*,?\s*\bthen\b\s*").split(body) {
        let (buildings,places)=match split_in.captures(part) {
            Some(hit)=>(items(&hit[1])?,items(&hit[2])?),
            None=>(items(part)?,vec![]),
        };
        phases.push(Phase{buildings,places});
    }
    // A phase without places builds where the next placed phase does
    // ("INFRAs, then MILs in Mun"); a trailing placeless phase is not understood.
    let mut next:Option<Vec<String>>=None;
    for phase in phases.iter_mut().rev() {
        if phase.places.is_empty() {phase.places=next.clone()?;} else {next=Some(phase.places.clone());}
    }
    (!phases.is_empty()).then_some(phases)
}
fn building(index:&Index,token:&str)->Result<Option<Value>,String> {
    let lower=token.trim().to_lowercase();
    let lower=lower.strip_prefix("full ").or_else(||lower.strip_prefix("max ")).or_else(||lower.strip_prefix("maximum ")).unwrap_or(&lower).trim().to_owned();
    let known=match lower.as_str() {
        "infra"|"infras"|"infrastructure"|"infrastructures"=>Some("infrastructure"),
        "mil"|"mils"|"military factory"|"military factories"=>Some("arms_factory"),
        "civ"|"civs"|"civilian factory"|"civilian factories"=>Some("industrial_complex"),
        "dock"|"docks"|"dockyard"|"dockyards"|"naval dockyard"|"naval dockyards"=>Some("dockyard"),
        "land facility"|"land facilities"=>Some("land_facility"),
        "naval facility"|"naval facilities"=>Some("naval_facility"),
        "air facility"|"air facilities"=>Some("air_facility"),
        "air base"|"air bases"|"airbase"|"airbases"=>Some("air_base"),
        "naval base"|"naval bases"=>Some("naval_base"),
        "refinery"|"refineries"=>Some("synthetic_refinery"),
        "radar"|"radars"=>Some("radar_station"),
        _=>None,
    };
    let mut queries=vec![known.map(str::to_owned).unwrap_or(lower.clone())];
    if known.is_none() {
        if let Some(stem)=lower.strip_suffix("ies") {queries.push(format!("{stem}y"));}
        else if let Some(stem)=lower.strip_suffix('s') {queries.push(stem.to_owned());}
    }
    for query in queries {
        let rows=index.entries(json!({"op":"matches","query":query,"kinds":["building"]}))?;
        if rows.len()==1 {return Ok(Some(rows[0].clone()));}
        if rows.len()>1 {return Ok(None);}
    }
    Ok(None)
}
fn words(text:&str)->Vec<String> {
    text.to_lowercase().split(|c:char|!c.is_alphanumeric()).filter(|w|!w.is_empty()).map(str::to_owned).collect()
}
/// States whose name starts with `place` word by word ("N Slo" -> "North
/// Slovenia"), among the start states of `nation`. Several states: none.
fn prefix_state(index:&Index,nation:&str,place:&str)->Option<u64> {
    let query=words(place);
    if nation.is_empty() || query.iter().map(String::len).sum::<usize>()<3 {return None;}
    let mut found=std::collections::BTreeSet::new();
    for row in index.rows().iter().filter(|r|r["kind"]=="state") {
        let national=s(&row["start_owner"])==nation || row["start_cores"].as_array().is_some_and(|c|c.iter().any(|t|s(t)==nation));
        if !national {continue;}
        let names=[&row["name"],&row["localized_name"]].into_iter().chain(row["aliases"].as_array().into_iter().flatten());
        if names.map(|n|words(s(n))).any(|name|name.len()>=query.len() && query.iter().zip(&name).all(|(q,n)|n.starts_with(q.as_str()))) {
            if let Some(id)=s(&row["id"]).parse::<u64>().ok() {found.insert(id);}
        }
    }
    (found.len()==1).then(|| *found.first().unwrap())
}
fn place(index:&Index,geography:&Geography,nation:&str,name:&str,provincial:bool)->Option<Value> {
    // Digits, percentages and footnote marks qualify the place: not understood.
    if name.chars().any(|c|c.is_ascii_digit()||"%*?/".contains(c)) {return None;}
    let exact=geography.resolve(&json!({"state_name":name}),provincial);
    if exact["state_id"].is_number() {return Some(exact);}
    if exact["reason"]!="location_unresolved" {return None;}
    let id=prefix_state(index,nation,name)?;
    Some(geography.resolve(&json!({"state_id":id}),provincial)).filter(|l|l["state_id"].is_number())
}
pub fn bind(guide:&Value,index:&Index)->Result<Value,String>{
    let mut guide=guide.clone();
    let geography=Geography::new(index.rows());
    let nation=s(&guide["applies_to"]["country"]).to_ascii_uppercase();
    for step in guide["tracks"].as_object_mut().ok_or("Missing tracks")?.values_mut().flat_map(|v|v.as_array_mut().into_iter().flatten()) {
        if step["action"]["kind"]!="manual" || step.get("completion").is_some() || step["action"]["params"]["category"]!="checkpoints" {continue;}
        let text=s(&step["title"]).to_owned();
        let Some(phases)=parse(&text) else {continue};
        let mut targets=vec![];
        let mut valid=true;
        'phases: for (order,phase) in phases.iter().enumerate() {
            let mut buildings=vec![];
            for token in &phase.buildings {
                let Some(row)=building(index,token)? else {valid=false;break 'phases};
                buildings.push(row);
            }
            for name in &phase.places {
                for building in &buildings {
                    let provincial=building["location_scope"]=="province";
                    let Some(location)=place(index,&geography,&nation,name,provincial) else {valid=false;break 'phases};
                    let mut target=json!({"category":"construction","building_id":building["id"],"building_scope":building["location_scope"],"state_id":location["state_id"],"state_name":location["state_name"],"quantity_mode":"maximum"});
                    if provincial {target["province_id"]=location["province_id"].clone();}
                    target["building_name"]=building.get("localized_name").filter(|v|!s(v).is_empty()).unwrap_or(&building["name"]).clone();
                    // Reading order only; completion still requires every target.
                    if phases.len()>1 {target["order"]=json!(order+1);}
                    // A catalog ceiling suffices only for non-shared fixed caps.
                    // Shared industrial slots require live effective capacity.
                    if building["shares_slots"]==false && building["max_level"].as_u64().is_some_and(|n|n>0) {
                        target["target_level"]=building["max_level"].clone();
                    }
                    targets.push(target);
                    if targets.len()>64 {valid=false;break 'phases;}
                }
            }
        }
        if !valid || targets.is_empty() {continue;}
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
 {"kind":"building","id":"industrial_complex","name":"Civilian Factory","location_scope":"state","shares_slots":true,"max_level":20},
 {"kind":"state","id":"219","name":"Moscow","geography":{"province_ids":[6380],"cities":[{"name":"Moscow","province_id":6380}]}},
 {"kind":"state","id":"46","name":"Muntenia","start_owner":"ROM","start_cores":["ROM"],"geography":{"province_ids":[9617]}},
 {"kind":"state","id":"47","name":"Munster","start_owner":"ENG","geography":{"province_ids":[100]}},
 {"kind":"state","id":"80","name":"Moldova","start_owner":"ROM","geography":{"province_ids":[200]}},
 {"kind":"state","id":"81","name":"Moldavia","start_cores":["ROM"],"geography":{"province_ids":[201]}},
 {"kind":"state","id":"82","name":"North Slovenia","start_owner":"YUG","geography":{"province_ids":[300]}}
 ]})).unwrap()}
 fn guide(text:&str)->Value {json!({"id":"build-test","dependency_policy":"explicit","tracks":{"operations":[{"key":"build","title":text,"action":{"kind":"manual","params":{"category":"checkpoints"}}}]}})}
 fn national(text:&str,tag:&str)->Value {let mut g=guide(text);g["applies_to"]=json!({"country":tag});g}
 fn targets(g:&Value)->Vec<(String,u64,Option<u64>)> {
  g["tracks"]["operations"][0]["action"]["params"]["construction_targets"].as_array().into_iter().flatten()
   .map(|t|(s(&t["building_id"]).to_owned(),t["state_id"].as_u64().unwrap(),t["order"].as_u64())).collect()
 }
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
  // No built level to observe: queued now, or the player's confirmation.
  assert_eq!(row["completion"]["args"][1]["args"][0],json!({"op":"check","key":row["key"]}));
  assert_eq!(row["completion"]["args"][1]["args"][1]["args"][1]["key"],"construction.queued");
  assert_eq!(row["completion"]["args"][1]["args"][1]["args"][1]["value"],"state:219:arms_factory");
  // Factories finished and no longer queued: the confirmation completes the
  // step, but never in place of an infrastructure level the game contradicts.
  let state=|infra|json!({"schema_version":1,"usable":true,"facts":{"country.states.items.219.infrastructure":{"status":"observed","source":"fixture","value":infra},
   "construction.queued":{"status":"calculated","source":"fixture","value":[]},"construction.queued_known":{"status":"calculated","source":"fixture","value":[]}}});
  let checked=|on|json!({"schema_version":1,"guide_id":"build-test","run_id":"r","checks":{"build":on}});
  let evaluate=|s:&Value,p:&Value|crate::guide_runtime::evaluate(&g,s,p).unwrap()["steps"][0]["status"].clone();
  assert_ne!(evaluate(&state(5),&checked(false)),"completed");
  assert_eq!(evaluate(&state(5),&checked(true)),"completed");
  assert_ne!(evaluate(&state(4),&checked(true)),"completed");
  // A queue line the reader cannot locate leaves the full queue unknown; the
  // lines it located still count as queued, their absence proves nothing.
  let partial=|known:Value|json!({"schema_version":1,"usable":true,"facts":{"country.states.items.219.infrastructure":{"status":"observed","source":"fixture","value":4},
   "construction.queued":{"status":"unknown","source":"","value":null},"construction.queued_known":{"status":"calculated","source":"fixture","value":known}}});
  assert_eq!(evaluate(&partial(json!(["state:219:infrastructure"])),&checked(true)),"completed");
  assert_ne!(evaluate(&partial(json!([])),&checked(true)),"completed");
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
 #[test] fn verbless_then_line_resolves_a_unique_national_prefix_in_order() {
  let g=bind(&national("INFRAs, then MILs in Mun (future in best)","ROM"),&index()).unwrap();
  assert_eq!(targets(&g),vec![("infrastructure".into(),46,Some(1)),("arms_factory".into(),46,Some(2))]);
  // `then` is reading order only: the queued MIL alone does not complete the step.
  let fact=|v|json!({"status":"observed","source":"fixture","value":v});
  let queue=|values:Value|json!({"schema_version":1,"usable":true,"facts":{"country":fact(json!("ROM")),"construction.queued":fact(values)}});
  let evaluate=|s:&Value|crate::guide_runtime::evaluate(&g,s,&Value::Null).unwrap()["steps"][0]["status"].clone();
  assert_ne!(evaluate(&queue(json!(["state:46:arms_factory"]))),"completed");
  assert_eq!(evaluate(&queue(json!(["state:46:infrastructure","state:46:arms_factory"]))),"completed");
 }
 #[test] fn plural_lists_with_several_places_and_word_prefixes_bind() {
  let g=bind(&national("BUILD: CIVs in: Mos, Mun","ROM"),&index()).unwrap();
  assert!(targets(&g).is_empty(),"Mos matches Moldova and Moldavia: ambiguous");
  let g=bind(&national("Build INFRA, then MIL, in: Muntenia","ROM"),&index()).unwrap();
  assert_eq!(targets(&g).len(),2);
  let g=bind(&national("MILs in N Slo (PRIO)","YUG"),&index()).unwrap();
  assert_eq!(targets(&g),vec![("arms_factory".into(),82,None)]);
 }
 #[test] fn ambiguous_foreign_or_partly_understood_lines_stay_unbound() {
  // No German state starts with "Mun"; without a nation prefixes are never tried.
  for (text,tag) in [("INFRAs, then MILs in Mun","GER"),("INFRAs, then MILs in Mun","")] {
   let g=national(text,tag);
   assert_eq!(bind(&g,&index()).unwrap(),g,"{text} / {tag}");
  }
  for text in ["MILs in 100%, then 80%","INFRAs, then MILs","Build 2 REF","4 CIVs in Muntenia","MILs in Mun (if 80%+)","INFRAs & MILs in Ven (then MILs in best)","MILs in best","MILs in Muntenia*"] {
   let g=national(text,"ROM");
   assert_eq!(bind(&g,&index()).unwrap(),g,"{text}");
  }
  // The prefix search is limited to the guide's nation: ENG resolves Munster.
  assert_eq!(targets(&bind(&national("MILs in Mun","ENG"),&index()).unwrap()),vec![("arms_factory".into(),47,None)]);
 }
}
