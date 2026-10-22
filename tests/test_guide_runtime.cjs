'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const cases=require('./fixtures/guide-runtime.json');
for(const example of cases)test('runtime: '+example.name,()=>{
 const before=JSON.stringify(example),value=runtime.evaluate(example.guide,example.state,example.progress);
 assert.deepEqual(value.steps.map(x=>x.status),example.expected.statuses);
 if(example.expected.keys)assert.deepEqual(value.steps.map(x=>x.key),example.expected.keys);
 if(example.expected.mode)assert.equal(value.mode,example.expected.mode);
 if(example.expected.progress_status)assert.equal(value.progress_status,example.expected.progress_status);
 if(example.expected.parallel)for(const [lane,keys]of Object.entries(example.expected.parallel))assert.deepEqual(value.parallel[lane].steps,keys);
 for(const [field,prop]of [['selection_values','value'],['selection_sources','source']])if(example.expected[field])assert.deepEqual(value.steps.map(row=>row.selection[prop]),example.expected[field]);
 assert.equal(JSON.stringify(example),before,'the runtime must never mutate definitions, observations or progression');
 assert.deepEqual(runtime.evaluate(example.guide,example.state,example.progress),value);
});
test('invalid progress is rejected and large dependency chains are iterative',()=>{
 const example=cases[0];
 for(const change of [{schema_version:true},{schema_version:2},{checks:null},{checks:{x:1}},{started:null},{started:[]},{started:{x:1}},{run_id:' '},{session_id:''}])assert.throws(()=>runtime.evaluate(example.guide,example.state,{...example.progress,...change}));
 const rows=Array.from({length:2000},(_,i)=>({key:String(i),action:{kind:'select_focus',entity:{kind:'focus',id:String(i)}},depends_on:i?[String(i-1)]:[]}));
 const state=structuredClone(example.state);state.facts['focus.completed'].value=rows.slice(0,-1).map(x=>x.key);
 const guide={id:'deep',tracks:{focus:rows}};
 assert.equal(runtime.evaluate(guide,state).steps.at(-1).status,'ready');
 rows[0].depends_on=['1999'];assert.throws(()=>runtime.evaluate(guide,state),/cicliche/);
});

test('runs with the same step keys are isolated and completion rolls back',()=>{
 const sample=cases.find(x=>x.name==='manual checkpoint is explicit evidence');
 const previous=runtime.evaluate(sample.guide,sample.state,sample.progress);
 const newRun={...sample.progress,run_id:'new',checks:{}};
 assert.equal(previous.steps[0].status,'completed');
 assert.equal(runtime.evaluate(sample.guide,sample.state,newRun).steps[0].status,'unverifiable');
 const complete=cases.find(x=>x.name==='dependency completion unlocks other tracks');
 assert.equal(runtime.evaluate(complete.guide,complete.state,complete.progress).steps[0].status,'completed');
 const earlier=cases[0];assert.equal(runtime.evaluate(earlier.guide,earlier.state,earlier.progress).steps[0].status,'ready');
});
