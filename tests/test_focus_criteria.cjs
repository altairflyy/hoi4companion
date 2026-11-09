'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const criteria=require('../src/hoi4companion/web/focus-criteria.js');
const contracts=require('../src/hoi4companion/web/contracts.js');
const presentation=require('../src/hoi4companion/web/guide-presentation.js');
const cases=require('./fixtures/focus-criteria.json');
for(const item of cases)test('focus criteria runtime parity: '+item.name,()=>assert.deepEqual(runtime.evaluate(item.guide,item.state),item.expected));
test('independent alternative is purple and the original focus remains waiting',()=>{
 const {guide,state}=cases[0],p=presentation.evaluate(guide,state);
 assert.equal(p.lanes.focus.next.key,'main');
 assert.deepEqual(p.lanes.focus.markers.map(x=>[x.key,x.tone]),[['independent','alternative'],['main','waiting']]);
});
test('foreign partial stale and invalid native metric values stay unknown',()=>{
 for(const [country,status,value,known]of [['HUN','observed',.87043,true],['GER','observed',.87,false],['HUN','partial',.87,false],['HUN','observed',87,false]]){
  const state=contracts.state({usable:true,sample:{country:'HUN',country_overview:{country,metrics:{status,war_support:value}}}});
  assert.equal(state.facts['country.metrics.war_support'].status,known?'calculated':'unknown');
 }
});
test('malformed focus expressions are rejected instead of evaluated',()=>{
 const r=structuredClone(cases[0].guide.tracks.focus[0].focus_criteria);
 r.available.args[0].value='84';assert.throws(()=>criteria.validate(r));
 r.available={op:'eval',code:'true'};assert.throws(()=>criteria.validate(r));
});
