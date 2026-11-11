use super::*;
use crate::country::Reader;
use crate::live::Decoder;
use std::collections::BTreeMap;
const BASE:u64=0x140000000;
const COUNTRY:u64=0x200000;
struct Fixture { bytes:BTreeMap<u64,u8>, classes:BTreeMap<u64,String>, next:u64 }
impl Fixture {
    fn new()->Self {Self{bytes:BTreeMap::new(),classes:BTreeMap::new(),next:0x300000}}
    fn put(&mut self,p:u64,b:&[u8]) {for(i,b)in b.iter().enumerate(){self.bytes.insert(p+i as u64,*b);}}
    fn u(&mut self,p:u64,v:u64){self.put(p,&v.to_le_bytes());}
    fn i(&mut self,p:u64,v:i32){self.put(p,&v.to_le_bytes());}
    fn alloc(&mut self)->u64{let p=self.next;self.next+=0x4000;p}
    fn object(&mut self,p:u64,kind:&str){let rva=0x1000+self.classes.len() as u64*0x100;let col=0x1000000+self.classes.len()as u64*0x1000;self.u(BASE+rva-8,BASE+col);self.i(BASE+col,1);self.i(BASE+col+12,(col+0x100)as i32);self.i(BASE+col+20,col as i32);self.classes.insert(rva,format!(".?AV{kind}@@"));self.u(p,BASE+rva);}
    fn string(&mut self,p:u64,s:&str){let data=if s.len()<=15{p}else{let d=self.alloc();self.u(p,d);d};self.put(data,s.as_bytes());self.put(data+s.len()as u64,&[0]);self.u(p+16,s.len()as u64);self.u(p+24,s.len().max(15)as u64);}
    fn read_assignments(&mut self)->Result<Value>{let a=json!({"token_table_rva":0x9000,"commander_reference_registry_rva":0x8000,"function_signatures":[],"abi_functions":{"commander_assignments":vec![json!([1,1,"fixture"]);11]}});Reader{d:Decoder{memory:self,base:BASE},world:0x210000,country:COUNTRY,tag:1,watch:BTreeMap::new(),attributes:a,event_cache:None}.commander_assignments()}
}
impl Memory for Fixture {
    fn read(&mut self,p:u64,n:usize)->Result<Vec<u8>>{Ok((0..n).map(|i|*self.bytes.get(&(p+i as u64)).unwrap_or(&0)).collect())}
    fn class_name(&mut self,rva:u64)->Result<String>{self.classes.get(&rva).cloned().ok_or("Unknown RTTI".into())}
    fn fixture_transport(&self)->bool{true}
}
fn fixture(role:i32)->(Fixture,u64,u64,u64,u64){
    let mut m=Fixture::new();m.object(COUNTRY,"CCountry");m.i(COUNTRY+8,1);
    let names=m.alloc();m.u(BASE+0x9030,names);m.put(BASE+0x9000,&[1]);m.i(BASE+0x9004,20000);
    for(t,s)in [(1,"TST_commander"),(0x34e2,"field_marshal"),(0x34c7,"corps_commander"),(0x30b7,"navy_leader")]{m.string(names+t*32,s);}
    let manager=m.alloc();m.object(manager,"CCountryCharacters");m.u(manager+8,COUNTRY);m.u(COUNTRY+0xff0,manager);
    let row=m.alloc();m.u(manager+0x10,row);m.i(manager+0x18,1);m.i(manager+0x1c,1);m.put(row+10,&[1]);
    let person=m.alloc();m.object(person,"CCharacter");m.i(person+0x68,1);m.i(person+0x18,1);m.u(row,person);
    let leader=m.alloc();m.object(leader,if role==2{"CNavyLeader"}else{"CArmyLeader"});m.u(person+0xa8,leader);m.i(leader+0x120,1);m.i(leader+0xe7c,role);m.u(leader+8,0x100001269);
    let target=m.alloc();m.object(target,match role{0=>"CArmyGroup",1=>"COrdersGroup",_=>"CFleet"});
    let typ=if role==2{0x3d}else{0x35};let handle=0x200000000u64|typ;m.u(target+8,handle);m.u(leader+if role==2{0xf60}else{0x1048},handle);
    if role==2{m.u(target+0xa8,COUNTRY);m.u(target+0xb0,0x100001269);m.string(target+0xe0,"Fleet");}else{m.i(target+0x3c,1);m.u(target+0x88,leader);m.string(target+0x160,"Army");}
    let registry=m.alloc();let data=m.alloc();m.u(BASE+0x8000+typ*8,registry);m.u(registry+8,data);m.i(registry+0x14,7);m.put(registry+0x18,&[1]);
    let hash=(typ<<32|2).wrapping_mul(0x49631a45);let bucket=(hash^(hash>>32))&7;m.put(data+bucket*24+4,&[1]);m.u(data+bucket*24+8,handle);m.u(data+bucket*24+16,target);
    (m,leader,target,person,manager)
}
#[test]
fn exact_commander_assignments_decode_all_three_roles(){for(role,name,kind)in[(0,"field_marshal","army_group"),(1,"corps_commander","army"),(2,"navy_leader","fleet")]{let(mut m,_,_,_,_)=fixture(role);let v=m.read_assignments().unwrap();assert_eq!(v["items"][0]["commander_id"],"TST_commander");assert_eq!(v["items"][0]["role"],name);assert_eq!(v["items"][0]["target_kind"],kind);}}
#[test]
fn commander_roster_and_pending_target_do_not_prove_assignment(){let(mut m,l,_,_,manager)=fixture(1);let current=u64::from_le_bytes(m.read(l+0x1048,8).unwrap().try_into().unwrap());m.u(l+0x1048,0);m.u(l+0x1050,current);assert_eq!(m.read_assignments().unwrap()["items"],json!([]));m.i(manager+0x1c,0);assert_eq!(m.read_assignments().unwrap()["items"],json!([]));m.i(manager+0x1c,2);assert!(m.read_assignments().is_err());}
#[test]
fn commander_assignment_rejects_owner_identity_reciprocity_and_role_mismatches(){
    for mutation in 0..7 {let(mut m,l,t,p,_)=fixture(1);match mutation{0=>m.i(p+0x68,2),1=>m.i(l+0x120,2),2=>m.i(t+0x3c,2),3=>m.u(t+8,0x300000035),4=>m.u(t+0x88,COUNTRY),5=>m.i(l+0xe7c,0),_=>m.object(t,"CFleet")};assert!(m.read_assignments().is_err(),"mutation {mutation}");}
    let(mut m,_,t,_,_)=fixture(2);m.u(t+0xa8,COUNTRY+1);assert!(m.read_assignments().is_err());
}
