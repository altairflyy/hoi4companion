'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const cases=require('./fixtures/guide-runtime.json');
test('continuous focuses use qualified continuous selection and never native completion',()=>{
 const guide={id:'continuous',dependency_policy:'explicit',tracks:{focus:[
  {key:'suppress',action:{kind:'select_focus',entity:{kind:'focus',id:'continuous_restrict_freedom'},params:{focus_mode:'continuous'}}},
  {key:'after_start',action:{kind:'select_focus',entity:{kind:'focus',id:'next'}},depends_on_started:['suppress']},
  {key:'after_end',action:{kind:'select_focus',entity:{kind:'focus',id:'last'}},depends_on:['suppress']} ]}};
 const observed=value=>({status:'observed',source:'fixture',value});
 const state={schema_version:1,usable:true,facts:{'focus.active':observed([]),'focus.completed':observed([]),'focus.continuous':observed(['continuous_restrict_freedom'])}};
 let result=runtime.evaluate(guide,state);
 assert.deepEqual(result.steps.map(r=>r.status),['active','ready','waiting']);
 assert.equal(result.steps[0].completion.value,false);
 delete state.facts['focus.continuous'];state.facts['focus.active']=observed(['continuous_restrict_freedom']);
 assert.equal(runtime.evaluate(guide,state).steps[0].selection.value,null);
 guide.tracks.focus[0].action.params.focus_mode='invalid';assert.throws(()=>runtime.evaluate(guide,state),/focus mode/);
});
test('explicit policy removes only implicit barriers, leaving unknowns and declared dependencies',()=>{
 const state=structuredClone(cases[0].state),focus=(key,extra={})=>({key,action:{kind:'select_focus',entity:{kind:'focus',id:key}},...extra});
 const guide={id:'generic',tracks:{focus:[{key:'gate',action:{kind:'manual'}},focus('independent'),focus('dependent',{depends_on:['gate']}),focus('conditional',{when:{op:'check',key:'unknown'}})]}};
 const before=JSON.stringify(guide),legacy=runtime.evaluate(guide,state),explicit=runtime.evaluate({...guide,dependency_policy:'explicit'},state);
 assert.equal(legacy.steps[1].reason,'dependency_unknown');assert.equal(explicit.steps[1].status,'ready');
 assert.equal(explicit.steps[2].reason,'dependency_unknown');assert.equal(explicit.steps[3].reason,'condition_unknown');assert.equal(JSON.stringify(guide),before);
 for(const bad of [null,[],{},1,'bad'])assert.throws(()=>runtime.evaluate({...guide,dependency_policy:bad},state));
});
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
 rows[0].depends_on=['1999'];assert.throws(()=>runtime.evaluate(guide,state),/Cyclic/);
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
