use super::*;
fn fixture()->(Fixture,Value,u64,u64,u64) {
    let mut m=Fixture::new();m.object(COUNTRY,"CCountry");m.i(COUNTRY+8,1);
    let db=m.alloc();m.object(db,"CIdeaDatabase");m.u(BASE+0x8000,db);
    let defs=m.alloc();m.u(BASE+0x8008,defs);
    let manager=m.alloc();m.object(manager,"CPoliticalStatus");m.u(COUNTRY+0xf90,manager);m.u(manager+0xc8,COUNTRY);
    let category=m.alloc();m.object(category,"CIdeaCategory");m.i(category+0x78,0);m.i(category+0x80,1);
    let group=m.alloc();m.object(group,"CIdeaGroupType");m.string(group+0x38,"trade_laws");m.u(group+0x60,category);m.i(group+0x68,2);m.put(group+0x10,&[1]);m.i(group+0x5c,1);
    let old=m.alloc();m.object(old,"CIdea");m.string(old+0x18,"old");m.u(old+0xaa0,group);m.u(old+0xab0,1_000_000);m.i(old+0xac0,1);
    let slot=m.alloc();m.u(slot,old);m.array(manager+0x80,&[slot]);
    let mut ideas=vec![];for (id,level,cost) in [("intermediate",2,10_000_000),("target",3,15_000_000)] {
        let idea=m.alloc();m.object(idea,"CIdea");m.string(idea+0x18,id);m.u(idea+0xaa0,group);m.i(idea+0xac0,level);m.u(idea+0xaa8,cost);m.u(idea+0xab0,0);ideas.push(idea);
    }m.array(group+0x18,&ideas);m.array(db+0xd8,&[0,group]);
    let mods=m.alloc();m.u(COUNTRY+0x5b8,mods);m.i(COUNTRY+0x5c0,1);m.i(COUNTRY+0x5c4,1);m.i(mods,2);m.put(mods+8,&(-20_000i64).to_le_bytes());
    let a=json!({"function_signatures":[],"abi_functions":{"political_costs":vec![json!([1,1,"fixture"]);9]},"ideas_database_rva":0x8000,"modifier_definitions_rva":0x8008,"political_cost_ids":[{"id":"target","group":"trade_laws"}]});
    (m,a,defs,manager,ideas[1])
}
#[test]fn native_political_cost_adds_dynamic_factor_removal_and_intermediate_levels() {
    let(mut m,a,_,_,target)=fixture();let c=m.request("politics",a.clone()).unwrap()["costs"].clone();
    assert_eq!(c["status"],"observed");assert_eq!(c["rows"][0]["base"],150.);assert_eq!(c["rows"][0]["factor"],0.8);
    assert_eq!(c["rows"][0]["effective"],210.,"120 target + 10 removal + 80 intermediate");
    m.i(target+0xac0,2);assert_eq!(m.request("politics",a).unwrap()["costs"]["rows"][0]["effective"],130.);
}
#[test]fn native_political_cost_rejects_owner_currency_conditional_modifiers_and_missing_evidence() {
    let(mut m,a,defs,manager,_)=fixture();m.i(defs+2*0x78+0x68,1);assert_eq!(m.request("politics",a.clone()).unwrap()["costs"]["status"],"unavailable");
    m.i(defs+2*0x78+0x68,0);m.u(manager+0xc8,WORLD);assert_eq!(m.request("politics",a.clone()).unwrap()["costs"]["status"],"unavailable");
    let(mut m,mut a,_,_,_)=fixture();a["abi_functions"]["political_costs"]=json!([]);assert_eq!(m.request("politics",a).unwrap()["costs"]["status"],"unavailable");
    let(mut m,mut a,_,_,_)=fixture();a["political_cost_ids"][0]["id"]=json!("missing");let costs=m.request("politics",a).unwrap()["costs"].clone();assert_eq!(costs["rows"][0]["status"],"unknown");assert_eq!(costs["rows"][0]["reason"],"Native purchase target is not qualified");assert_ne!(costs["rows"][0]["reason"],costs["coverage"]["advisors"]["reason"],"A missing law must not inherit the optional advisor failure");
}
#[test]fn native_political_cost_does_not_fabricate_zero_for_unpurchasable_or_xp_targets() {
    let(mut m,a,_,_,target)=fixture();m.put(target+0xaa8,&(-100_000i64).to_le_bytes());
    let value=m.request("politics",a.clone()).unwrap();assert_eq!(value["costs"]["rows"][0]["status"],"unknown");assert!(value["costs"]["rows"][0].get("effective").is_none());
    let group=u64::from_le_bytes((0..8).map(|i|m.bytes[&(target+0xaa0+i)]).collect::<Vec<_>>().try_into().unwrap());
    let category=u64::from_le_bytes((0..8).map(|i|m.bytes[&(group+0x60+i)]).collect::<Vec<_>>().try_into().unwrap());
    m.i(category+0x78,2);assert_eq!(m.request("politics",a).unwrap()["costs"]["rows"][0]["status"],"unknown");
}
fn advisor(m:&mut Fixture,a:&mut Value)->u64 {
    a["abi_functions"]["advisors_costs"]=json!(vec![json!([1,1,"fixture"]);3]);a["political_cost_ids"]=json!([{"id":"test_advisor","group":"high_command"}]);
    let manager=m.alloc();m.object(manager,"CCountryCharacters");m.u(manager+8,COUNTRY);m.u(COUNTRY+0xff0,manager);
    let c=m.alloc();m.object(c,"CCharacter");m.i(c+0x68,1);let entries=m.alloc();m.u(manager+0x10,entries);m.i(manager+0x18,1);m.i(manager+0x1c,1);m.u(entries,c);
    let head=m.alloc();let node=m.alloc();m.u(c+0xb0,head);m.u(c+0xb8,1);m.put(head+0x19,&[1]);m.u(head+8,node);m.u(node,head);m.u(node+8,head);m.u(node+0x10,head);m.put(node+0x19,&[0]);
    let role=m.alloc();m.object(role,"CAdvisor");m.u(node+0x48,role);m.u(role+0x10,c);let template=m.alloc();m.object(template,"CAdvisorTemplate");m.u(role+0x18,template);
    m.string(role+0x30,"test_advisor");m.string(role+0x80,"high_command");m.i(role+0x28,4);m.i(role+0x278,2);m.u(role+0xa8,20_000_000);role
}
#[test]fn native_advisor_cost_uses_reciprocal_roster_and_two_sequential_modifiers() {
    let(mut m,mut a,defs,_,_)=fixture();let role=advisor(&mut m,&mut a);
    let mods=m.alloc();m.u(COUNTRY+0x5b8,mods);m.i(COUNTRY+0x5c0,2);m.i(COUNTRY+0x5c4,2);m.i(mods,2);m.put(mods+8,&(-20_000i64).to_le_bytes());m.i(mods+16,337);m.u(mods+24,50_000);
    // The fixture allocator normally reserves 16 KiB, so allocate a separate
    // zeroed modifier descriptor at the admitted index used by the formula.
    m.i(defs+337*0x78+0x60,0);m.i(defs+337*0x78+0x68,0);
    let v=m.request("politics",a.clone()).unwrap();let row=&v["costs"]["rows"][0];assert_eq!(row["id"],"test_advisor");assert_eq!(row["base"],200.);assert_eq!(row["effective"],240.,"200 * .8 * 1.5");
    m.i(role+0x28,5);assert_eq!(m.request("politics",a.clone()).unwrap()["costs"]["rows"][0]["status"],"unknown");
    m.u(role+0x10,WORLD);assert_eq!(m.request("politics",a).unwrap()["costs"]["rows"][0]["status"],"unknown","Unqualified advisor relations remain an explicit unknown");
}
#[test]fn native_decision_constant_cost_keeps_true_zero_and_rejects_custom_or_dynamic_prices() {
    let(mut m,mut a,_,_,_)=fixture();a["abi_functions"]["decision_costs"]=json!(vec![json!([1,1,"fixture"]);5]);a["political_cost_ids"]=json!([{"id":"test_decision","group":"decision"}]);
    let db=m.alloc();m.object(db,"CDecisionDatabase");m.u(BASE+0x8010,db);a["decisions_database_rva"]=json!(0x8010);a["decision_cost_cheat_rva"]=json!(0x8018);m.put(BASE+0x8018,&[0]);
    let d=m.alloc();m.object(d,"CDecision");m.string(d+0x120,"test_decision");m.array(db+0x28,&[0,d]);m.put(d+0x519,&[1]);m.u(d+0x550,5_000_000);
    assert_eq!(m.request("politics",a.clone()).unwrap()["costs"]["rows"][0]["effective"],50.);
    m.u(d+0x550,0);assert_eq!(m.request("politics",a.clone()).unwrap()["costs"]["rows"][0]["effective"],0.);
    m.put(d+0x519,&[0]);assert_eq!(m.request("politics",a.clone()).unwrap()["costs"]["rows"][0]["status"],"unknown");
    m.put(d+0x519,&[1]);m.i(d+0x654,2);assert_eq!(m.request("politics",a.clone()).unwrap()["costs"]["rows"][0]["status"],"unknown");
    m.put(BASE+0x8018,&[1]);assert_eq!(m.request("politics",a).unwrap()["costs"]["rows"][0]["status"],"unknown");
}
#[test]fn missing_optional_cost_family_preserves_qualified_law_and_explains_partial_coverage() {
    let(mut m,mut a,_,_,_)=fixture();a["political_cost_ids"].as_array_mut().unwrap().push(json!({"id":"unknown_advisor","group":"high_command"}));
    let v=m.request("politics",a).unwrap();let c=&v["costs"];assert_eq!(c["status"],"partial");assert_eq!(c["rows"][0]["effective"],210.);assert_eq!(c["rows"][1]["status"],"unknown");assert!(c["rows"][1]["reason"].is_string());assert_eq!(c["coverage"]["advisors"]["status"],"unknown");
}
