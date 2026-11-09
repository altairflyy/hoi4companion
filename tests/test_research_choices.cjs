'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const presentation=require('../src/hoi4companion/web/guide-presentation.js');
const contracts=require('../src/hoi4companion/web/contracts.js');
const branches={concentrated:['c1','c2','c3'],dispersed:['d1','d2','d3']};
const guide={schema_version:1,id:'choice',title:'Choice',applies_to:{country:'FRA'},tracks:{research:[0,1,2].map(i=>({
 key:'r'+i,action:{kind:'research',entity:{kind:'technology',id:'c1'}},
 research_choice:{group:'industry_layout',query:i?'Concentrated/dispersed UP':'Concentrated/dispersed Industry 1',branches,...(i?{}:{level:1})}
}))}};
const fact=value=>({status:'observed',source:'fixture',value});
const state={schema_version:1,usable:true,session_id:'s',facts:{country:fact('FRA'),'research.completed':fact([]),
 'research.active':fact([]),'research.slots':fact([{technology_id:null},{technology_id:null}])}};
const progress={schema_version:1,guide_id:'choice',run_id:'x',session_id:'s',checks:{},research_bindings:{}};
test('open branch choice has two purple options and occupies one research proposal',()=>{
 const result=runtime.evaluate(guide,state,progress);
 assert.deepEqual(result.parallel.research.steps,['r0']);
 assert.deepEqual(result.steps[0].research_options,['c1','d1']);
 const view=presentation.evaluate(guide,state,{progress});
 assert.deepEqual(view.lanes.research.markers.map(m=>[m.id,m.tone]),[['c1','choice'],['d1','choice']]);
});
test('stored selection uses the same branch; unknown/stale/conflicting facts never choose',()=>{
 const p={...progress,research_bindings:{r0:'d1',r1:'d2'}};
 const s=structuredClone(state);s.facts['research.completed']=fact(['d1']);
 const result=runtime.evaluate(guide,s,p);
 assert.equal(result.steps[0].status,'completed');assert.equal(result.steps[1].status,'ready');
 assert.deepEqual(presentation.evaluate(guide,s,{progress:p}).lanes.research.markers.map(m=>[m.id,m.tone]),[['d2','next']]);
 assert.deepEqual(result.parallel.research.steps,['r1']);
 for(const live of [structuredClone(state),structuredClone(state)]){
  live.facts['research.completed']=fact(['c1','d1']);
  assert.deepEqual(runtime.evaluate(guide,live,progress).parallel.research.steps,[]);
 }
 assert.deepEqual(runtime.evaluate(guide,state,{...progress,session_id:'other'}).parallel.research.steps,[]);
});
test('malformed alternative branches are rejected by both contracts and runtime',()=>{
 for(const change of [c=>c.branches.dispersed=['c1'],c=>c.level=0,c=>c.level=4,c=>c.group='',c=>c.branches={one:['c1']}]){
  const g=structuredClone(guide);change(g.tracks.research[0].research_choice);
  assert.throws(()=>contracts.guide(g));
 }
});
test('optional continuous focus is purple without occupying the mandatory focus slot',()=>{
 const g={id:'optional',title:'Optional',tracks:{focus:[
  {key:'optional',action:{kind:'select_focus',entity:{kind:'focus',id:'suppress'},params:{focus_mode:'continuous',focus_optional:true}}},
  {key:'next',action:{kind:'select_focus',entity:{kind:'focus',id:'federation'}}}
 ]}};
 const s=structuredClone(state);Object.assign(s.facts,{'focus.active':fact([]),'focus.completed':fact([]),'focus.continuous':fact([])});
 const view=presentation.evaluate(g,s);
 assert.deepEqual(view.lanes.focus.markers.map(m=>[m.id,m.tone]),[['federation','next'],['suppress','choice']]);
 assert.deepEqual(view.model.runtime.parallel.focus,{capacity:1,steps:['next'],optional_steps:['optional']});
});
