'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const model=require('../src/hoi4companion/web/guide-model.js');
const cases=require('./fixtures/guide-schema.json');
for(const example of cases)test('guide schema: '+example.name,()=>{
 const before=JSON.stringify(example.guide);
 if(example.valid){const result=contracts.guide(example.guide);assert.deepEqual(result,example.normalized);assert.deepEqual(contracts.guide(result),result);}
 else assert.throws(()=>contracts.guide(example.guide));
 assert.equal(JSON.stringify(example.guide),before,'normalization must not mutate source definitions');
});
test('large dependency chains and cycles are handled without recursion',()=>{
 const rows=Array.from({length:2000},(_,i)=>({key:String(i),kind:'gate',depends_on:i?[String(i-1)]:[]}));
 const guide={tracks:{a:rows.filter((_,i)=>i%2===0),b:rows.filter((_,i)=>i%2===1)}};
 assert.equal(contracts.guide(guide).tracks.a.length,1000);
 rows[0].depends_on=['1999'];assert.throws(()=>contracts.guide(guide),/cicliche/);
});
test('evidence and completion declarations survive evaluation without becoming observations',()=>{
 const guide={sources:[{id:'source',type:'manual'}],tracks:{research:[{key:'tech',kind:'technology',target:'t',completion:{op:'check',key:'done'},evidence:[{source_id:'source',basis:'explicit'}]}]}};
 const result=model.evaluate(guide,{usable:true,sample:{country:'GER',researched_technologies:[],research_slots:[]}},{checks:{done:true}});
 const step=result.research.rows[0];
 assert.deepEqual(step.completion_condition,guide.tracks.research[0].completion);
 assert.equal(step.completion.value,false);
 assert.equal(step.status,'pending');
 assert.equal(step.availability.status,'unknown');
 assert.deepEqual(step.evidence,guide.tracks.research[0].evidence);
});
