'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const importer=require('../src/hoi4companion/web/importer-model.js');
const fixtures=require('./fixtures/focus-sequence-contract.json');
for(const item of fixtures)test('focus metadata parity: '+item.name,()=>{
 if(item.valid)assert.doesNotThrow(()=>contracts.guide(item.guide));
 else assert.throws(()=>contracts.guide(item.guide));
});
test('manual importer correction locks its ID and retains the original position/query',()=>{
 const guide=structuredClone(fixtures.find(x=>x.name==='source provenance').guide),before=structuredClone(guide);
 const result=importer.editStep(guide,'one',{entity:{kind:'focus',id:'B'}});
 assert.deepEqual(guide,before);
 assert.deepEqual(result.tracks.focus[0].focus_resolution,{...before.tracks.focus[0].focus_resolution,
  entity:{kind:'focus',id:'B'},locked:true,classification:'direct'});
 assert.equal(result.tracks.focus[0].focus_resolution.position,1);
});
