'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const guide={schema_version:1,id:'g',title:'G',applies_to:{country:'POL'},tracks:{research:[
 {key:'r',action:{kind:'research',entity:{kind:'technology',id:'a'}},relative_research:{query:'Artillery UP',chain:['a','b','c']}}
]}};
const fact=value=>({status:'observed',source:'fixture',value});
const state={schema_version:1,usable:true,session_id:'s',facts:{country:fact('POL'),'research.completed':fact(['a','b']),
 'research.active':fact([]),'research.slots':fact([{technology_id:null}])}};
test('relative research requires a campaign binding and retains completed target',()=>{
 const progress={schema_version:1,guide_id:'g',run_id:'x',session_id:'s',checks:{},research_bindings:{r:'b'}};
 assert.equal(runtime.evaluate(guide,state,progress).steps[0].status,'completed');
 assert.equal(runtime.evaluate(guide,state,{...progress,research_bindings:{}}).steps[0].status,'unverifiable');
 assert.equal(runtime.evaluate(guide,state,{...progress,session_id:'other'}).steps[0].status,'unverifiable');
 assert.equal(runtime.evaluate(guide,state,{...progress,research_bindings:{r:'unrelated'}}).steps[0].status,'unverifiable');
});
test('malformed research chains are rejected',()=>{
 const broken=structuredClone(guide);broken.tracks.research[0].relative_research.chain=['a','a'];
 assert.throws(()=>runtime.evaluate(broken,state,null));
});
test('waiting UPs do not hide independent research and future bound levels wait',()=>{
 const g=structuredClone(guide);g.tracks.research.push({...structuredClone(g.tracks.research[0]),key:'later'},{key:'other',action:{kind:'research',entity:{kind:'technology',id:'d'}}});
 const s=structuredClone(state);s.facts['research.completed']=fact(['a']);s.facts['research.active']=fact(['b']);
 const p={schema_version:1,guide_id:'g',run_id:'x',session_id:'s',checks:{},research_bindings:{r:'b'}};
 let result=runtime.evaluate(g,s,p);assert.equal(result.steps[1].status,'waiting');assert.deepEqual(result.parallel.research.steps,['other']);
 result=runtime.evaluate(g,s,{...p,research_bindings:{r:'c'}});assert.equal(result.steps[0].status,'waiting');
});
test('a fully observed finished chain is not applicable; missing evidence stays unknown',()=>{
 const s=structuredClone(state);s.facts['research.completed']=fact(['a','b','c']);
 const p={schema_version:1,guide_id:'g',run_id:'x',session_id:'s',checks:{}};
 assert.equal(runtime.evaluate(guide,s,p).steps[0].status,'not_applicable');
 delete s.facts['research.completed'];assert.equal(runtime.evaluate(guide,s,p).steps[0].status,'unverifiable');
});
test('the presentation uses the runtime-bound technology rather than a cached source id',()=>{
 const model=require('../src/hoi4companion/web/guide-model.js');
 const p={schema_version:1,guide_id:'g',run_id:'x',session_id:'s',checks:{},research_bindings:{r:'c'}};
 const result=model.evaluate(guide,state,{progress:p});
 assert.equal(result.research.rows[0].target,'c');
 assert.equal(result.research.rows[0].action.entity.id,'c');
 const direct=model.evaluate(guide,state,p);
 assert.equal(direct.research.rows[0].target,'c');
 assert.equal(direct.research.rows[0].runtime_status,'ready');
});
