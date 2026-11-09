'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const observed=value=>({status:'observed',source:'fixture',value});
const line=(id,equipment,requested)=>({id,equipment,name:equipment,category:'military',requested_factories:requested,assigned_factories:0,active_factories:0,damaged_factories:0});
const guide={schema_version:1,id:'remaining',title:'Remaining factories',applies_to:{country:'TST'},tracks:{operations:[{key:'inf',action:{kind:'manual',params:{category:'production',quantity_mode:'remaining',production_category:'military',production_selector:{equipment_ids:['inf'],variant_names:[],category:'military'}}},completion:{op:'fact',key:'guide.production.inf.remaining_allocated',test:'eq',value:true},started_when:{op:'fact',key:'guide.production.inf.started',test:'eq',value:true}}]}};
const state=(lines,total)=>({schema_version:1,usable:true,facts:{country:observed('TST'),'production.lines':observed(lines),...(total===undefined?{}:{'country.production.effective_factories.military':observed(total)})}});

test('ALL follows remaining capacity after other requested lines, then new factories',()=>{
 const lines=[line('1','sup',3),line('2','inf',7)];
 let row=runtime.evaluate(guide,state(lines,10)).steps[0];assert.equal(row.status,'completed');assert.equal(row.production.target,7);
 row=runtime.evaluate(guide,state(lines,11)).steps[0];assert.equal(row.status,'active');assert.equal(row.production.target,8);
 lines[1].requested_factories=150;assert.equal(runtime.evaluate(guide,state(lines,11)).steps[0].status,'completed');
});

test('ALL requires a unique last military line, capacity evidence, and positive request',()=>{
 const cases=[state([],0),state([line('1','inf',0)],0),state([line('1','inf',150),line('2','sup',1)],10),
 state([line('1','inf',150),line('2','inf',1)],10),state([line('1','inf',150)]),state([line('1','inf',150)],-1)];
 for(const current of cases)assert.notEqual(runtime.evaluate(guide,current).steps[0].status,'completed');
});
