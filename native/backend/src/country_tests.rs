//! Native-memory fixtures exercise the real decoders, not replacement readers.
use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct Fixture { bytes: BTreeMap<u64,u8>, classes: BTreeMap<u64,String>, next: u64, native_rtti: bool }
const BASE:u64=0x140000000;
const COUNTRY:u64=0x200000;
const WORLD:u64=0x210000;
impl Fixture {
    fn new()->Self { Self {next:0x300000,..Default::default()} }
    fn put(&mut self,a:u64,b:&[u8]) { for (i,v) in b.iter().enumerate(){self.bytes.insert(a+i as u64,*v);} }
    fn u(&mut self,a:u64,n:u64){self.put(a,&n.to_le_bytes());}
    fn i(&mut self,a:u64,n:i32){self.put(a,&n.to_le_bytes());}
    fn alloc(&mut self)->u64{let p=self.next;self.next+=0x4000;p}
    fn object(&mut self,p:u64,name:&str)->u64 {
        let rva=0x1000+self.classes.len() as u64*0x100;
        let meta=0x1000000+self.classes.len() as u64*0x1000;
        self.u(BASE+rva-8,BASE+meta);self.i(BASE+meta,1);self.i(BASE+meta+12,(meta+0x100) as i32);self.i(BASE+meta+16,(meta+0x200) as i32);self.i(BASE+meta+20,meta as i32);
        self.put(BASE+meta+0x110,format!(".?AV{name}@@\0").as_bytes());
        self.i(BASE+meta+0x208,1);self.i(BASE+meta+0x20c,(meta+0x280) as i32);self.i(BASE+meta+0x280,(meta+0x300) as i32);
        self.i(BASE+meta+0x300,(meta+0x400) as i32);self.i(BASE+meta+0x30c,-1);self.put(BASE+meta+0x410,b".?AVCRelation@@\0");
        self.classes.insert(rva,format!(".?AV{name}@@")); self.u(p,BASE+rva);rva
    }
    fn array(&mut self,a:u64,values:&[u64]){
        let p=self.alloc();self.u(a,p);self.i(a+8,values.len() as i32);self.i(a+12,values.len() as i32);
        for (i,v) in values.iter().enumerate(){self.u(p+i as u64*8,*v);}
    }
    fn string(&mut self,a:u64,s:&str){
        let p=if s.len()<=15{a}else{let p=self.alloc();self.u(a,p);p};
        self.put(p,s.as_bytes());self.put(p+s.len() as u64,&[0]);self.u(a+16,s.len() as u64);self.u(a+24,s.len().max(15) as u64);
    }
    fn request(&mut self,section:&str,attributes:Value)->Result<Value>{
        collect(self,Request{schema_version:1,base:BASE,world:WORLD,country:COUNTRY,tag:1,section:section.into(),qualify:false,attributes}).map(|v|v["value"].clone())
    }
}
impl Memory for Fixture {
    fn read(&mut self,a:u64,n:usize)->Result<Vec<u8>>{Ok((0..n).map(|i|*self.bytes.get(&(a+i as u64)).unwrap_or(&0)).collect())}
    fn class_name(&mut self,rva:u64)->Result<String>{self.classes.get(&rva).cloned().ok_or("Unknown fixture RTTI".into())}
    fn fixture_transport(&self)->bool{!self.native_rtti}
    fn executable_bytes(&mut self,rva:u64,n:usize)->Result<Vec<u8>>{self.read(BASE+rva,n)}
}
fn regions()->(Fixture,Value,u64){
    let mut m=Fixture::new();let registry=m.alloc();m.object(registry,"CBuildingDatabase");m.u(BASE+0x8000,registry);
    let state=m.alloc();m.object(state,"CState");m.i(state+0x58,110);m.i(state+0xc8,1);m.i(state+0xcc,1);m.array(WORLD+0x2c8,&[state]);
    let mut definitions=vec![];let mut buildings=vec![];
    for (i,key) in ["infrastructure","arms_factory","industrial_complex","dockyard","radar_station","bunker"].iter().enumerate(){
        let d=m.alloc();m.object(d,"CBuildingTemplate");m.string(d+0x270,key);m.i(d+0x2e0,i as i32);definitions.push(d);
        let b=m.alloc();m.object(b,"CBuilding");m.u(b+0x1e0,d);m.put(b+0x40,&(i as i16+1).to_le_bytes());buildings.push(b);
    }
    m.array(registry+0x40,&definitions);m.array(state+0x158,&buildings);
    let mapping=m.alloc();m.u(state+0x140,mapping);m.i(state+0x148,6);m.i(state+0x14c,6);
    for i in 0..6{m.i(mapping+i*4,i as i32);}
    (m,json!({"building_database_rva":0x8000,"building_scopes":{"radar_station":"state","bunker":"province"}}),buildings[4])
}
#[test]
fn all_catalog_qualified_state_buildings_keep_legacy_counts_and_exclude_provincial_zeros(){
    let(mut m,a,_)=regions();let v=m.request("states",a).unwrap();let r=&v["items"][0];
    assert_eq!(r["buildings"]["radar_station"],5);assert!(r["buildings"].get("bunker").is_none());
    assert_eq!(r["infrastructure"],1);assert_eq!(r["military_factories"],2);assert_eq!(r["civilian_factories"],3);
}
#[test]
fn state_building_wrong_template_is_not_accepted_as_a_level(){
    let(mut m,a,b)=regions();m.u(b+0x1e0,0xdeadbeef);assert!(m.request("states",a).unwrap_err().contains("template"));
}
#[test]
fn controlled_divisions_are_counted_and_wrong_ownership_is_rejected(){
    let mut m=Fixture::new();let unit=m.alloc();m.object(unit,"CArmy");m.i(unit+0x1d8,1);m.u(unit+0x18,0x100000001);
    let template=m.alloc();m.object(template,"CReferencedDivisionTemplate");m.u(unit+0x3b8,template);m.string(template+0x20,"Infantry");
    let sub=m.alloc();m.object(sub,"CSubUnitDefinition");m.string(sub+0x18,"infantry");m.u(template+0x1a8,sub);m.array(COUNTRY+0x290,&[unit]);
    assert_eq!(m.request("divisions",json!({})).unwrap()["total"],1);
    m.i(unit+0x1d8,2);assert!(m.request("divisions",json!({})).is_err());
    m.array(COUNTRY+0x290,&[]);assert_eq!(m.request("divisions",json!({})).unwrap()["total"],0);
}
fn decisions()->(Fixture,Value,u64){
    let mut m=Fixture::new();let manager=m.alloc();m.object(manager,"CDecisionStatus");m.u(COUNTRY+0xfa0,manager);m.u(manager+8,COUNTRY);
    let decision=m.alloc();m.object(decision,"CDecision");m.string(decision+0x120,"taken_decision");m.array(manager+0x40,&[decision]);
    let other=m.alloc();m.object(other,"CDecision");m.string(other+0x120,"timed_decision");let timed=m.alloc();m.object(timed,"CTimedDecision");m.u(timed+0x10,other);m.i(timed+0x18,20);m.i(timed+0x1c,0);m.array(manager+0xa0,&[timed]);
    (m,json!({"layout_id":"hoi4-x64-abi-v1","function_signatures":[],"abi_functions":{"decisions":[[1,1,"fixture"],[2,1,"fixture"],[3,1,"fixture"]]}}),timed)
}
#[test]
fn taken_and_timed_decisions_use_native_state_not_random_counters(){
    let(mut m,a,timed)=decisions();let v=m.request("decisions",a.clone()).unwrap();let h=&v["execution_history"];
    assert_eq!(h["completed_ids"],json!([]));assert_eq!(h["executed_ids"],json!(["taken_decision","timed_decision"]));assert_eq!(h["active_ids"],json!(["timed_decision"]));
    for (state,name,done) in [(1,"completed",true),(2,"failed",false),(3,"aborted",false),(4,"cooldown",false)]{
        m.i(timed+0x1c,state);let v=m.request("decisions",a.clone()).unwrap();let h=&v["execution_history"];
        assert_eq!(h["timed"][0]["state"],name);assert_eq!(h["active_ids"],json!([]));
        assert_eq!(h["completed_ids"].as_array().unwrap().contains(&json!("timed_decision")),done);
    }
}
#[test]
fn missing_decision_serializer_evidence_does_not_publish_execution(){
    let(mut m,mut a,_)=decisions();a["abi_functions"]["decisions"]=Value::Null;
    assert_eq!(m.request("decisions",a).unwrap()["execution_history"]["status"],"unavailable");
}
fn foreign()->(Fixture,Value,u64,u64){
    let mut m=Fixture::new();m.object(COUNTRY,"CCountry");m.i(COUNTRY+8,1);
    let other=m.alloc();m.object(other,"CCountry");m.i(other+8,2);m.array(WORLD+0x310,&[COUNTRY,other]);
    let map=m.alloc();m.u(WORLD+0x340,map);m.i(WORLD+0x348,3);m.i(WORLD+0x34c,3);
    for (i,v) in [-1,0,1].iter().enumerate(){m.i(map+i as u64*4,*v);}
    let tags=m.alloc();m.u(WORLD+0x358,tags);m.i(WORLD+0x364,3);m.string(tags+32,"TST");m.string(tags+64,"OTH");
    let status=m.alloc();m.object(status,"CDiplomacyStatus");m.u(status+0x2d0,COUNTRY);
    let relation=m.alloc();let vt=m.object(relation,"CGuaranteeRelation");m.u(BASE+vt+16,BASE+0x6000);m.i(relation+8,0x28da);m.i(relation+0x10,1);m.i(relation+0x14,2);m.array(status+0x20,&[relation]);
    let displacement=BASE+0x1000000+(vt-0x1000)/0x100*0x1000+0x308;
    (m,json!({"layout_id":"hoi4-x64-abi-v1","image_base":BASE,"diplomacy_status":status,"foreign_interactions_layout":{"version":1,"signatures":[]},"function_signatures":[],"abi_functions":{"foreign_records":[[0x6000,1,"fixture"]]}}),relation,displacement)
}
#[test]
fn diplomacy_records_preserve_direction_and_cancellation_with_base_serializer_checks(){
    let(mut m,a,r,method)=foreign();let v=m.request("foreign_relations",a.clone()).unwrap();
    assert_eq!(v["relations"]["items"][0]["kind"],"guarantee");assert_eq!(v["relations"]["items"][0]["direction"],"outgoing");
    m.i(r+0x10,2);m.i(r+0x14,1);assert_eq!(m.request("foreign_relations",a.clone()).unwrap()["relations"]["items"][0]["direction"],"incoming");
    m.put(r+0x48,&[1]);assert_eq!(m.request("foreign_relations",a.clone()).unwrap()["relations"]["items"],json!([]));
    m.put(r+0x48,&[0]);m.u(method,BASE+0x6001);
    assert_eq!(m.request("foreign_relations",a).unwrap()["relations"]["status"],"unavailable");
}
#[test]
fn unresolved_relation_types_do_not_become_a_verified_empty_list(){
    let(mut m,a,r,_)=foreign();let vt=m.object(r,"CMarketAccessRightsRelation@NInternationalMarket");m.u(BASE+vt+16,BASE+0x6000);
    assert_eq!(m.request("foreign_relations",a.clone()).unwrap()["relations"]["items"][0]["kind"],"market_access_rights");
    m.object(r,"CFutureRelation");
    assert_eq!(m.request("foreign_relations",a).unwrap()["relations"]["status"],"unavailable");
}
#[test]
fn war_state_uses_typed_relations_not_the_non_pointer_98_container(){
    let(mut m,mut a,r,_)=foreign();let status=a["diplomacy_status"].as_u64().unwrap();
    m.u(COUNTRY+0xf88,status);a["military_layout"]=json!({"signatures":[],"diplomacy_offset":0xf88});
    m.array(status+0x98,&[0,0,0]);
    m.object(r,"CWarRelation");m.i(r+8,0x380a);
    assert_eq!(m.request("diplomacy",a.clone()).unwrap()["at_war"],true);
    m.put(r+0x48,&[1]);assert_eq!(m.request("diplomacy",a.clone()).unwrap()["at_war"],false);
    m.put(r+0x48,&[2]);assert!(m.request("diplomacy",a.clone()).is_err());
    m.put(r+0x48,&[0]);m.object(r,"CGuaranteeRelation");assert!(m.request("diplomacy",a).is_err());
}
#[test]
fn project_facility_reciprocity_uses_optional_atom_not_adjacent_project_fields(){
    let mut m=Fixture::new();let status=m.alloc();m.object(status,"CProgramStatus@NProject");m.u(COUNTRY+0xfa8,status);m.i(status+0x10,1);
    let pool=m.alloc();m.object(pool,"CProjectPool@NProject");m.u(status+0x18,pool);m.i(pool+8,1);
    let facility=m.alloc();m.object(facility,"CProgram@NProject");m.i(facility+0x18,1);m.u(facility+8,0x200000053);m.string(facility+0x20,"land_facility");
    m.i(facility+0x50,123);m.put(facility+0x54,&[1]);m.array(status+0x20,&[facility]);
    let project=m.alloc();m.object(project,"CProject@NProject");m.u(pool+0x10,project);m.i(pool+0x18,1);m.i(pool+0x1c,1);
    let template=m.alloc();m.object(template,"CProjectTemplate@NProject");m.i(template+8,123);m.string(template+0x238,"sp_test");m.u(project+0x20,template+0x18);
    let machine=m.alloc();m.object(machine,"CProjectStateMachine@NProject");m.u(project+0x28,machine);m.u(machine+8,machine+0x18);
    m.i(project+0x18,123);m.i(project+0x1c,24);m.u(project+0x60,0x200000053);
    let v=m.request("projects",json!({})).unwrap();assert_eq!(v["items"][0]["active"],true);assert_eq!(v["items"][0]["facility"],"land_facility");
    m.i(facility+0x50,124);assert!(m.request("projects",json!({})).is_err());m.i(facility+0x50,123);
    m.put(facility+0x54,&[0]);assert!(m.request("projects",json!({})).is_err());
    m.put(facility+0x54,&[2]);assert!(m.request("projects",json!({})).is_err());
}
#[test]
fn operative_classes_and_target_identities_are_checked_for_every_mission(){
    let(mut m,a,_,_)=foreign();let agency=m.alloc();m.object(agency,"CIntelligenceAgency");m.u(COUNTRY+0xfc0,agency);m.u(agency+0x78,COUNTRY);m.put(agency+0xc0,&[1,0]);
    let agent=m.alloc();m.object(agent,"COperativeLeader");m.u(agent+8,0x100001269);m.i(agent+0x1080,3);m.array(agency+0xf0,&[agent]);
    let mission=agent+0x1088;m.object(mission,"COperativeMission");m.u(mission+8,agent);let data=m.alloc();m.u(mission+0x10,data);m.u(data+8,mission);m.i(data+0x10,2);
    let names=["no_mission","build_intel_network","quiet_network","counter_intelligence","root_out_resistance","boost_ideology","control_trade","diplomatic_pressure","propaganda"];
    let classes=["CNoMission","CBuildIntelNetwork","CQuietIntelNetwork","CCounterIntelligence","CRootOutResistance","CBoostIdeology","CControlTrade","CDiplomaticPressure","CPropaganda"];
    for (kind,class) in classes.iter().enumerate(){
        m.i(mission+0x18,kind as i32);m.object(data,&format!("{class}@NOperativeMissions"));
        let v=m.request("intelligence",a.clone()).unwrap();let row=&v["missions"]["items"][0];
        assert_eq!(row["mission_id"],names[kind]);assert_eq!(row["state"],"active");
        assert_eq!(row["target_country"],if kind==0 {Value::Null}else{json!("OTH")});
    }
    m.i(data+0x10,3);assert_eq!(m.request("intelligence",a).unwrap()["missions"]["status"],"unavailable");
}
#[test]
fn province_buildings_keep_their_native_location_and_do_not_become_state_levels(){
    let mut m=Fixture::new();let registry=m.alloc();m.object(registry,"CBuildingDatabase");m.u(BASE+0x8000,registry);
    let definition=m.alloc();m.object(definition,"CBuildingTemplate");m.string(definition+0x270,"naval_base");m.i(definition+0x2e0,0);m.array(registry+0x40,&[definition]);
    let state=m.alloc();m.object(state,"CState");m.i(state+0x58,1);m.i(state+0xc8,1);m.i(state+0xcc,1);m.array(WORLD+0x2c8,&[0,state]);
    let province=m.alloc();m.object(province,"CProvince");m.i(province+0xa4,1);m.u(province+0xc0,state);m.i(province+0x188,1);m.array(WORLD+0x2b0,&[0,province]);
    let tags=m.alloc();m.string(tags+32,"TST");m.u(WORLD+0x358,tags);m.i(WORLD+0x364,2);
    let status=province+0x190;m.object(status,"CBuildingStatus");m.i(status+0x68,1);m.i(status+0x6c,1);m.i(status+0x70,0);
    let map=m.alloc();m.u(status+0x20,map);m.i(status+0x28,1);m.i(status+0x2c,1);m.i(map,0);
    let building=m.alloc();m.object(building,"CBuilding");m.u(building+0x1d8,status);m.u(building+0x1e0,definition);m.put(building+0x40,&2i16.to_le_bytes());m.array(status+0x38,&[building]);
    let a=json!({"building_database_rva":0x8000,"building_scopes":{"naval_base":"province"}});
    let v=m.request("fortifications",a.clone()).unwrap();assert_eq!(v["items"][0]["buildings"]["naval_base"],2);assert!(v["items"][0].get("bunker").is_none());
    m.i(status+0x6c,2);assert!(m.request("fortifications",a).is_err());
}
#[test]
fn character_roles_use_native_identity_and_do_not_become_appointments(){
    let mut m=Fixture::new();m.object(COUNTRY,"CCountry");m.i(COUNTRY+8,1);let manager=m.alloc();m.object(manager,"CCountryCharacters");m.u(COUNTRY+0xff0,manager);m.u(manager+8,COUNTRY);
    let character=m.alloc();m.object(character,"CCharacter");m.i(character+0x68,1);m.i(character+0x18,1);m.string(character+0x48,"Display name");
    let data=m.alloc();m.u(manager+0x10,data);m.i(manager+0x18,1);m.i(manager+0x1c,1);m.u(data,character);m.put(data+8,&[0,0,1,0]);
    m.put(BASE+0x9000,&[1]);m.i(BASE+0x9004,1);let names=m.alloc();m.u(BASE+0x9030,names);m.string(names+32,"TST_commander");
    let a=json!({"layout_id":"hoi4-x64-abi-v1","token_table_rva":0x9000,"function_signatures":[],"abi_action_signatures":{"advisors":[]},"abi_functions":{"characters":[[1,1,"x"],[2,1,"x"],[3,1,"x"]]}});
    let v=m.request("politics",a.clone()).unwrap();assert_eq!(v["characters"]["items"][0]["id"],"TST_commander");assert_eq!(v["characters"]["items"][0]["unit_leader"],true);assert_eq!(v["advisors"]["items"],json!([]));
    m.i(character+0x68,2);assert_eq!(m.request("politics",a).unwrap()["characters"]["status"],"unavailable");
}
#[test]
fn event_owner_is_recovered_from_the_actual_secondary_rtti_offset(){
    let mut m=Fixture::new();m.native_rtti=true;
    let root=m.alloc();m.object(root,"CGameGui");m.u(BASE+0x8000,root);
    let owner=m.alloc();m.object(owner,"CEventWindow");
    let gui=m.alloc();m.u(gui+80,owner+40);m.array(root+264,&[gui]);
    let vt=BASE+0xf000;let col=0xf100;let desc=0xf200;
    m.u(owner+40,vt);m.u(vt-8,BASE+col);m.i(BASE+col,1);m.i(BASE+col+4,40);m.i(BASE+col+12,desc as i32);m.i(BASE+col+20,col as i32);m.put(BASE+desc+16,b".?AVCEventWindow@@\0");
    let container=m.alloc();m.object(container,"CContainerWindow");m.u(owner+0x50,container);m.u(container+48+104,root);m.u(container+48+80,owner+56);
    let a=json!({"image_base":BASE,"gui_layout":{"root_rva":0x8000,"registry":[264,288]}});
    assert_eq!(m.request("events",a.clone()).unwrap()["visible"],json!([]));
    m.i(BASE+col+4,600);assert!(m.request("events",a).unwrap_err().contains("non individuata"));
}
