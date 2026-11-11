'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),runtime=require('../src/hoi4companion/web/guide-runtime.js'),contracts=require('../src/hoi4companion/web/contracts.js');
for(const row of require('./fixtures/political-costs.json'))test(row.name,()=>{
 const r=runtime.evaluate(row.guide,row.state,row.progress).steps[0].action_readiness;
 assert.equal(r.value,row.expected.readiness);assert.equal(r.political_power.cost,row.expected.cost);assert.equal(r.political_power.effective_cost,row.expected.effective);
 if(row.expected.readiness===null&&row.expected.effective===null)assert.equal(r.political_power.affordable,null);
});
test('native political costs reject duplicate identities and wrong currencies during projection',()=>{
 const cost={id:'free_trade',group:'trade_laws',currency:'political_power',base:150,effective:120,factor:.8,status:'calculated'};
 const sample={country:'NOR',country_overview:{country:'NOR',politics:{costs:{schema_version:1,scope:'qualified_native_purchase_targets',status:'observed',source:'native_political_purchase_formula',rows:[cost]}}}},raw={usable:true,sample};
 let state=contracts.state(raw);assert.equal(state.facts['politics.costs'].value['trade_laws/free_trade'].effective,120);
 sample.country_overview.politics.costs.rows.push({...cost});assert.equal(contracts.state(raw).facts['politics.costs'].status,'unknown');
 sample.country_overview.politics.costs.rows=[{...cost,currency:'army_experience'}];assert.equal(contracts.state(raw).facts['politics.costs'].status,'unknown');
 sample.country_overview.politics.costs.rows=[{status:'unknown',id:'../bad',group:'trade_laws'},cost];assert.equal(contracts.state(raw).facts['politics.costs'].status,'unknown');
 sample.country_overview.politics.costs.rows=[cost];sample.country_overview.country='ENG';assert.equal(contracts.state(raw).facts['politics.costs'].status,'unknown');
});

test('canonical cost facts require the admitted calculation provenance',()=>{
 const value={'trade_laws/free_trade':{currency:'political_power',base:150,effective:120,factor:.8}};
 for(const [status,source]of [['calculated','catalog_base'],['observed','native_political_purchase_formula']]){
  const packet={schema_version:1,usable:true,facts:{'politics.costs':{status,source,value}}};
  assert.equal(contracts.state(packet).facts['politics.costs'].status,'unknown');
  assert.equal(contracts.fact(packet,'politics.costs').status,'unknown');
 }
});
