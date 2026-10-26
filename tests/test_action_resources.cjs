'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const contracts=require('../src/hoi4companion/web/contracts.js');
const point='projects.points.specialization_land';
const guide={id:'points',tracks:{operations:[{key:'project',resource_costs:{[point]:1},
 action:{kind:'special_project',entity:{kind:'project',id:'flame'}},
 completion:{op:'fact',key:'projects.completed',test:'contains',value:'flame'}}]}};
const state=amount=>({schema_version:1,usable:true,session_id:'session',facts:Object.fromEntries(
 Object.entries({country:'GER','projects.completed':[],[point]:amount}).map(([key,value])=>[key,{status:'observed',source:'fixture',value}]))});
test('project resources distinguish insufficient, exact threshold and unknown evidence',()=>{
 for(const [amount,available,status] of [[0,false,'waiting'],[1,true,'ready'],[2,true,'ready'],[null,null,'ready'],[true,null,'ready']]){
  const row=runtime.evaluate(guide,state(amount)).steps[0];
  assert.equal(row.action_readiness.resources_available,available);assert.equal(row.status,status);
 }
});
test('unknown project prices do not become zero cost and malformed prices are rejected',()=>{
 const missing=structuredClone(guide);delete missing.tracks.operations[0].resource_costs;
 assert.equal(runtime.evaluate(missing,state(10)).steps[0].action_readiness.resources_available,null);
 for(const cost of [true,-1,Infinity,'1']){const invalid=structuredClone(guide);invalid.tracks.operations[0].resource_costs[point]=cost;assert.throws(()=>contracts.guide(invalid));}
});
test('resource balances are scoped to the observed country and stale samples lose their values',()=>{
 const raw={usable:true,sample:{country:'GER',country_overview:{country:'GER',
  projects:{status:'observed',items:[],breakthroughs:{status:'observed',items:[{id:'specialization_land',available:1}]}},
  politics:{power:{status:'observed',available:150.25}}}}};
 assert.equal(contracts.fact(contracts.state(raw),point).value,1);
 assert.equal(contracts.fact(contracts.state(raw),'country.politics.political_power').value,150.25);
 raw.sample.country_overview.country='POL';assert.equal(contracts.fact(contracts.state(raw),point).value,null);
 raw.sample.country_overview.country='GER';raw.usable=false;assert.equal(contracts.fact(contracts.state(raw),point).value,null);
});
