use super::*;
fn pool(m:&mut Fixture,p:u64,kind:&str,key:u64,n:i64,stride:u64){
 m.object(p,kind);let rows=m.alloc();m.u(p+8,rows);m.i(p+16,1);m.i(p+20,1);m.u(rows,key);m.u(rows+stride-8,n as u64);
}
fn logistics()->(Fixture,Value,u64,u64,u64){
 let mut m=Fixture::new();m.object(COUNTRY,"CCountry");m.i(COUNTRY+8,1);
 let e=m.alloc();m.object(e,"CEquipmentType");m.string(e+0x18,"infantry_equipment");m.u(e+0x538,e);m.put(e+0x10,&[1]);m.put(e+0x5b5,&[1]);m.u(e+0x5a0,4);
 let db=m.alloc();m.object(db,"CEquipmentDatabase");m.u(BASE+0x8000,db);m.array(db+0x80,&[e]);m.i(BASE+0x9000,7);
 let p=m.alloc();m.object(p,"CProductionStatus");m.u(COUNTRY+0xf68,p);m.u(p+0x290,COUNTRY);
 let v=m.alloc();m.object(v,"CEquipmentVariant");m.u(v+0x3f0,e);m.string(v+0x28,"Rifle");m.object(p+0x200,"CEquipmentVariantPool");let stock=m.alloc();m.u(p+0x220,stock);m.i(p+0x228,1);m.i(p+0x22c,1);m.u(stock,v);m.u(stock+8,1_000_000);
 let line=m.alloc();m.object(line,"CMilitaryProductionLine");m.u(line+0x88,v);m.u(line+0x28,200_000);m.u(line+0x30,300_000);m.array(p+0x58,&[line]);
 let log=m.alloc();m.object(log,"CLogisticsStatus");m.u(COUNTRY+0xf98,log);m.u(log+0x20,COUNTRY);m.array(log+0x28,&[]);
 let mut hs=vec![];let mut first=0;
 for event in 0..19 {let h=m.alloc();m.object(h,"CLoopHistory");let q=m.alloc();m.object(q,"CLoopHistoryContainer");m.u(h+0x10,q);let mut rows=vec![];for _ in 0..7 {let row=m.alloc();m.array(row,&[if event==1 {100_000}else if event==15 {1_000_000}else{0}]);rows.push(row)}m.array(q+8,&rows);m.i(q+0x20,7);m.i(q+0x24,if event==15{6}else{7});m.u(q+0x30,rows[6]);if event==1{first=q}hs.push(h)}m.array(log+8,&hs);
 let reinforce=m.alloc();m.object(reinforce,"CReinforcementStatus");m.u(COUNTRY+0xf78,reinforce);m.u(reinforce+0x38,COUNTRY);
 let group=m.alloc();m.object(group,"CArmyReinforcementRequests");m.u(group+0x40,reinforce);let army=m.alloc();m.object(army,"CArmy");m.u(group+0x20,army);m.i(army+0x1d8,1);let req=m.alloc();m.object(req,"CReinforcementRequest");pool(&mut m,req+0x38,"CEquipmentVariantPool",e,500_000,24);pool(&mut m,req+0x78,"CEquipmentArcheTypePool",e,2_000_000,16);m.array(group+0x28,&[req]);m.array(reinforce+0x20,&[group]);
 let g=m.alloc();m.object(g,"CCountryOccupationStatus");m.u(COUNTRY+0xfd0,g);m.u(g+0x20,COUNTRY);m.array(g+0x28,&[]);
 let deploy=m.alloc();m.object(deploy,"CDeploymentStatus");m.u(deploy+0x118,COUNTRY);m.u(COUNTRY+0xf70,deploy);m.array(deploy+0x60,&[]);
 let ll=m.alloc();m.object(ll,"CCountryResources");m.i(ll+0x20,1);m.u(COUNTRY+0x11f8,ll);m.array(ll+0x758,&[]);
 let ops=m.alloc();m.object(ops,"CCountryOperationManager");m.u(COUNTRY+0x15a8,ops);m.u(ops+8,COUNTRY);m.array(ops+0x10,&[]);
 let map=m.alloc();m.u(WORLD+0x340,map);m.i(map+4,0);let sm=m.alloc();m.u(WORLD+0x3d8,sm);let cs=m.alloc();m.u(sm+0x108,cs);let sc=m.alloc();m.u(cs,sc);
 let a=json!({"layout_id":"hoi4-x64-abi-v1","function_signatures":[],"equipment_database_rva":0x8000,"logistics_days_rva":0x9000,"abi_functions":{"logistics":(0..28).map(|n|json!([n+1,1,"fixture"])).collect::<Vec<_>>()}});
 (m,a,line,first,req)
}
#[test]fn native_logistics_keeps_gross_stock_and_signed_shortage_and_daily_rate(){let(mut m,a,_,_,_)=logistics();let v=m.request("equipment",a.clone()).unwrap();let r=&v["logistics"]["items"][0];assert_eq!(v["items"][0]["stockpile"],10.);assert_eq!(r["gross_stockpile"],10.);assert_eq!(r["needed"],15.);assert_eq!(r["logistics_stockpile"],10.);assert_eq!(r["available"],-5.);assert_eq!(r["balance_per_day"],0.5);assert_eq!(m.request("equipment",a).unwrap(),v);}
#[test]fn native_logistics_negative_daily_balance_and_wrapped_history(){let(mut m,a,l,q,_)=logistics();m.u(l+0x30,0);m.i(q+0x24,2);m.put(q+0x28,&[1]);let v=m.request("equipment",a).unwrap();assert_eq!(v["logistics"]["items"][0]["balance_per_day"],-1.);}
#[test]fn native_logistics_invalid_interval_and_partial_history_do_not_fabricate_rates(){let(mut m,a,_,q,_)=logistics();m.i(q+0x24,8);let v=m.request("equipment",a.clone()).unwrap();assert!(v["logistics"]["items"][0]["balance_per_day"].is_null());assert_eq!(v["logistics"]["items"][0]["available"],-5.);m.i(BASE+0x9000,0);assert_eq!(m.request("equipment",a).unwrap()["logistics"]["status"],"unavailable");}
#[test]fn native_logistics_rejects_foreign_owners_unknown_types_and_partial_need(){let(mut m,a,_,_,req)=logistics();let p=m.bytes.iter().filter(|(k,_)|**k>=COUNTRY+0xf98&&**k<COUNTRY+0xfa0).map(|(_,v)|*v).collect::<Vec<_>>();let log=u64::from_le_bytes(p.try_into().unwrap());m.u(log+0x20,COUNTRY+1);assert_eq!(m.request("equipment",a.clone()).unwrap()["logistics"]["status"],"unavailable");m.u(log+0x20,COUNTRY);m.object(req+0x78,"UnknownPool");let v=m.request("equipment",a).unwrap();assert!(v["logistics"]["items"][0]["needed"].is_null());assert_eq!(v["logistics"]["items"][0]["balance_per_day"],0.5);}
#[test]fn native_logistics_unqualified_signatures_keep_legacy_gross_quantities(){let(mut m,mut a,_,_,_)=logistics();a["abi_functions"]["logistics"]=json!([]);let v=m.request("equipment",a).unwrap();assert_eq!(v["logistics"]["status"],"unavailable");assert_eq!(v["items"][0]["stockpile"],10.);}
#[test]fn native_logistics_rejects_foreign_group_backlinks_and_unknown_pool_keys(){
 let(mut m,a,_,_,req)=logistics();let rein=u64::from_le_bytes((0..8).map(|i|m.bytes[&(COUNTRY+0xf78+i)]).collect::<Vec<_>>().try_into().unwrap());let data=u64::from_le_bytes((0..8).map(|i|m.bytes[&(rein+0x20+i)]).collect::<Vec<_>>().try_into().unwrap());let group=u64::from_le_bytes((0..8).map(|i|m.bytes[&(data+i)]).collect::<Vec<_>>().try_into().unwrap());m.u(group+0x40,rein+1);assert!(m.request("equipment",a.clone()).unwrap()["logistics"]["items"][0]["needed"].is_null());m.u(group+0x40,rein);
 let unknown=m.alloc();m.object(unknown,"CEquipmentType");m.u(unknown+0x538,unknown);let rows=u64::from_le_bytes((0..8).map(|i|m.bytes[&(req+0x80+i)]).collect::<Vec<_>>().try_into().unwrap());m.u(rows,unknown);assert!(m.request("equipment",a).unwrap()["logistics"]["items"][0]["needed"].is_null());
}
#[test]fn native_logistics_uses_the_qualified_interval_not_elapsed_wall_clock(){let(mut m,a,_,_,_)=logistics();m.i(BASE+0x9000,1);let v=m.request("equipment",a).unwrap();assert_eq!(v["logistics"]["interval_days"],1);assert_eq!(v["logistics"]["items"][0]["balance_per_day"],9.5);}


#[test]fn native_logistics_undefined_native_production_overflow_is_unknown(){let(mut m,a,l,_,_)=logistics();m.u(l+0x30,92_233_720_200_000);assert!(m.request("equipment",a).unwrap()["logistics"]["items"][0]["balance_per_day"].is_null());}




fn lu(m:&Fixture,p:u64)->u64{u64::from_le_bytes((0..8).map(|i|*m.bytes.get(&(p+i)).unwrap_or(&0)).collect::<Vec<_>>().try_into().unwrap())}
#[test]fn native_logistics_uses_qualified_production_owner_offset(){
 let(mut m,mut a,_,_,_)=logistics();let production=lu(&m,COUNTRY+0xf68);a["production_owner_offset"]=json!(0x2a8);m.u(production+0x2a8,COUNTRY);m.u(production+0x290,COUNTRY+1);
 let v=m.request("equipment",a).unwrap();assert_eq!(v["logistics"]["status"],"observed");assert_eq!(v["logistics"]["items"][0]["available"],-5.);
}
#[test]fn native_logistics_does_not_fall_back_from_foreign_qualified_production_owner(){
 let(mut m,mut a,_,_,_)=logistics();let production=lu(&m,COUNTRY+0xf68);a["production_owner_offset"]=json!(0x2a8);m.u(production+0x2a8,COUNTRY+1);m.u(production+0x290,COUNTRY);
 assert!(m.request("equipment",a.clone()).is_err());
 let error=Reader{d:Decoder{memory:&mut m,base:BASE},world:WORLD,country:COUNTRY,tag:1,watch:BTreeMap::new(),attributes:a,event_cache:None}.logistics().unwrap_err();
 assert_eq!(error,"Production logistics belongs to another country");
}
fn supply_fixture(m:&mut Fixture,a:&mut Value)->(u64,u64,u64){
 m.object(WORLD,"CCurrentGameState");m.u(BASE+0xa000,WORLD);a["world_rva"]=json!(0xa000);m.i(WORLD+0x520,1);m.array(WORLD+0x310,&[COUNTRY]);let map=m.alloc();m.u(WORLD+0x340,map);m.i(WORLD+0x348,2);m.i(WORLD+0x34c,2);m.i(map,-1);m.i(map+4,0);
 let manager=lu(m,WORLD+0x3d8);let array=lu(m,manager+0x108);let supply=lu(m,array);m.object(supply,"CCountrySupplySystem");m.u(supply+0x78,COUNTRY);let rail=m.alloc();m.u(supply+0x80,rail);
 let db=lu(m,BASE+0x8000);let e=lu(m,lu(m,db+0x80));(e,supply,rail)
}
fn logistics_stock(m:&mut Fixture,n:u64){
 let production=lu(m,COUNTRY+0xf68);let raw=lu(m,production+0x220);m.u(raw+8,n);
 let status=lu(m,COUNTRY+0xf98);let history=lu(m,lu(m,status+8)+15*8);let container=lu(m,history+0x10);let row=lu(m,container+0x30);let data=lu(m,row);m.u(data,n);
}
fn no_request_need(m:&mut Fixture,req:u64){let produced=lu(m,req+0x40);m.u(produced+16,0);let need=lu(m,req+0x80);m.u(need+8,0);}
#[test]fn native_logistics_train_stock_excludes_current_native_reservation(){
 let(mut m,mut a,_,_,req)=logistics();let(e,supply,rail)=supply_fixture(&mut m,&mut a);m.u(e+0x5a0,1<<31);m.i(supply+0x110,189);m.i(rail+0x140,44);logistics_stock(&mut m,18_900_000);no_request_need(&mut m,req);
 let v=m.request("equipment",a.clone()).unwrap();let r=&v["logistics"]["items"][0];assert_eq!(r["status"],"observed");assert_eq!(r["gross_stockpile"],189.);assert_eq!(r["logistics_stockpile"],145.);assert_eq!(r["needed"],0.);assert_eq!(r["available"],145.);
 m.u(supply+0x78,COUNTRY+1);let v=m.request("equipment",a).unwrap();assert!(v["logistics"]["items"][0]["available"].is_null());assert_eq!(v["logistics"]["items"][0]["gross_stockpile"],189.);
}
#[test]fn native_logistics_motorized_stock_and_demand_use_distinct_signed_corrections(){
 for(requested,assigned,stock,need,net)in[(80,20,40.,60.,-20.),(20,80,160.,-60.,220.)]{let(mut m,mut a,_,_,req)=logistics();let(e,supply,_)=supply_fixture(&mut m,&mut a);m.u(e+0x5a0,8);m.put(e+0x44b,&[1]);m.i(supply+0xc8,requested);m.i(supply+0x138,assigned);logistics_stock(&mut m,10_000_000);no_request_need(&mut m,req);let v=m.request("equipment",a).unwrap();let r=&v["logistics"]["items"][0];assert_eq!(r["status"],"observed");assert_eq!(r["gross_stockpile"],100.);assert_eq!(r["logistics_stockpile"],stock);assert_eq!(r["needed"],need);assert_eq!(r["available"],net);}
}
