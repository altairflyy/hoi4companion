'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const presentation=require('../src/hoi4companion/web/guide-presentation.js');
const contracts=require('../src/hoi4companion/web/contracts.js');
const guide=params=>({id:'generic',tracks:{operations:[{key:'order',action:{kind:'manual',params}}]}});
test('requested factories include pending slots; stale evidence cannot complete the action',()=>{
 const g=guide({category:'production',requested_factories:10,production_selector:{category:null,equipment_ids:['rifle'],variant_names:[]}});
 Object.assign(g.tracks.operations[0],{started_when:{op:'fact',key:'guide.production.order.started',test:'eq',value:true},completion:{op:'fact',key:'guide.production.order.requested',test:'eq',value:10}});
 const state={usable:true,sample:{country:'TST',country_overview:{country:'TST',production:{status:'observed',lines:[{id:'1',equipment:'rifle',name:'Rifle',category:'military',requested_factories:10,assigned_factories:3,active_factories:3,damaged_factories:0}]}}}};
 assert.equal(runtime.evaluate(g,state).steps[0].status,'completed');
 state.sample.country_overview.production.lines[0].requested_factories=9;
 assert.equal(runtime.evaluate(g,state).steps[0].status,'active');
 state.usable=false;assert.notEqual(runtime.evaluate(g,state).steps[0].status,'completed');
});
test('project starts and finishes from qualified distinct native states, never just a facility',()=>{
 const g=guide({category:'projects'});Object.assign(g.tracks.operations[0],{started_when:{op:'fact',key:'projects.active',test:'contains',value:'p'},completion:{op:'fact',key:'projects.completed',test:'contains',value:'p'}});
 const state={usable:true,sample:{country:'TST',country_overview:{country:'TST',projects:{status:'observed',items:[{id:'p',active:true,completed:false}]}}}};
 assert.equal(runtime.evaluate(g,state).steps[0].status,'active');
 Object.assign(state.sample.country_overview.projects.items[0],{active:false,completed:true});
 assert.equal(runtime.evaluate(g,state).steps[0].status,'completed');
 delete state.sample.country_overview.projects.items[0].completed;
 assert.notEqual(runtime.evaluate(g,state).steps[0].status,'completed');
 state.sample.country_overview.country='OTH';assert.equal(contracts.fact(contracts.state(state),'projects.active').status,'unknown');
});
test('observed game dates order operations independently from video time and do not complete them',()=>{
 const g=guide({game_date:'1937.1.1',source_seconds:10});
 g.tracks.operations.push({key:'earlier',action:{kind:'manual',params:{game_date:'1936.1.1',source_seconds:20}}});
 const view=presentation.evaluate(g);assert.deepEqual(view.operations.map(x=>x.key),['earlier','order']);assert.equal(view.operations.some(x=>x.runtime_status==='completed'),false);
 for(const value of ['1937.2.29','1936.13.1','0.1.1','01:30',null])assert.throws(()=>contracts.guide(guide({game_date:value})));
});
test('visual evidence selects a useful frame when the action has no explicit source time',()=>{
 const g=guide({category:'army'});g.sources=[{id:'a',type:'youtube',url:'https://www.youtube.com/watch?v=Ht4e89B02Wo'}];g.tracks.operations[0].evidence=[{source_id:'a',basis:'explicit',timestamp_seconds:30},{source_id:'a',basis:'visual',timestamp_seconds:42}];
 assert.equal(presentation.evaluate(g).operations[0].source_seconds,42);
});
