'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const view=require('../src/hoi4companion/web/guide-presentation.js');
const fixture=require('./presentation-fixtures.cjs');
function present(name){const {guide,state,progress,evaluation}=fixture(name);return view.evaluate(guide,state,{progress},evaluation);}
test('guide headings separate file metadata without changing the imported guide',()=>{
 assert.deepEqual(view.heading('HUNGARY (Non-Aligned)_ BROKEN EU Path! — Difficulty_⭐⭐⭐⭐ (NEW & UPDATED FOR 1.19.4).docx'),{title:'HUNGARY (Non-Aligned) BROKEN EU Path!',details:'Difficulty ⭐⭐⭐⭐ (NEW & UPDATED FOR 1.19.4)'});
 assert.deepEqual(view.heading('Industry / Army choices'),{title:'Industry / Army choices',details:''});
 assert.deepEqual(view.heading('Guide.hoi4-guide.json'),{title:'Guide',details:''});
 assert.deepEqual(view.heading(''),{title:'',details:''});
});
const guide={id:'neutral',applies_to:{country:'GER',tree_id:'g'},tracks:{focus:[
 {key:'f',kind:'focus',target:'F',name:'Focus',href:'/?tree=g&focus=F'}],research:[
 {key:'r',kind:'technology',target:'R',name:'Research',href:'/technologies?folder=industry&tech=R'},
 {key:'p',kind:'project',target:'P',name:'Project'}],operations:[
 {key:'build',action:{kind:'manual',params:{category:'construction',timing:'Start',instructions:['Build a facility']}},depends_on:[],title:'Construction',name:'Construction'}]},milestones:[{key:'build',title:'Construction'},{key:'legacy',title:'Legacy advice'}]};
const sample=(extra={})=>({usable:true,sample:{session_id:'s',country:'GER',focus_tree_id:'g',completed_focuses:[],current_focus:null,researched_technologies:[],research_slots:[{technology_id:null}],...extra}});

test('event recommendations qualify visible choice independently of completion and never infer a click',()=>{
 const input={id:'event-guide',tracks:{operations:[{key:'event',action:{kind:'manual',params:{event_id:'event.1',event_option_id:'event.1.a',event_option_name:'Recommended'}}}]}};
 const current={usable:true,sample:{country:'GER',country_overview:{country:'GER',events:{status:'observed',visible:[{instance_id:'1',event_id:'event.1',option_ids:['event.1.a']}]}}}};
 let row=present('presentation: visible event option').operations[0];assert.equal(row.event_observation.visible,true);assert.equal(row.event_observation.option_available,true);assert.notEqual(row.runtime_status,'completed');
 row=present('presentation: visible event without options').operations[0];assert.equal(row.event_observation.visible,true);assert.equal(row.event_observation.option_available,null);
 assert.equal(present('presentation: event stale').operations[0].event_observation.visible,null);
});
test('native actions appear only as tree annotations; operations retain checkpoints and projects',()=>{
 const before=JSON.stringify(guide),result=present('presentation: neutral guide');
 assert.equal(result.lanes.focus.markers[0].tone,'next');assert.equal(result.lanes.research.markers[0].tone,'next');
 assert.equal(result.lanes.research.folder,'industry');
 assert.deepEqual(result.operations.map(x=>x.key).sort(),['build','legacy','p']);
 assert.equal(result.operations.filter(x=>x.key==='build').length,1);
 assert.equal(result.operations.find(x=>x.key==='build').category,'construction');
 assert.equal(JSON.stringify(guide),before);
});
test('stale/wrong scope suppress actionable highlights and busy slots mark upcoming only',()=>{
 assert.equal(present('presentation: neutral guide stale').lanes.focus.markers[0].tone,'preview');
 assert.equal(present('presentation: neutral guide wrong country').lanes.focus.markers.length,0);
 assert.equal(present('presentation: neutral guide busy slots').lanes.research.markers[0].tone,'upcoming');
 const active=present('presentation: neutral guide active focus');
 assert.equal(active.lanes.focus.markers[0].tone,'active');assert.equal(active.lanes.focus.next,null);
});
test('gates prevent later focus illumination; confirmations and phases remain operational',()=>{
 assert.equal(present('presentation: gate unchecked').lanes.focus.markers.length,0);
 assert.equal(present('presentation: gate unchecked').lanes.focus.blocker.key,'wait');
 assert.equal(present('presentation: gate checked').lanes.focus.markers[0].id,'F');
});
test('duplicate target markers are unique and tree scope is attached to annotations',()=>{
 const result=present('presentation: repeated focus target');assert.equal(result.lanes.focus.markers.length,1);
 assert.equal(result.lanes.focus.markers[0].scope,'g');
});

test('free choices and old combined milestone aliases do not create duplicate instructions',()=>{
 const input=structuredClone(guide);input.tracks.focus.push({key:'free',kind:'free'});
 input.tracks.operations[0].action.params.legacy_key='legacy';
 const result=view.evaluate(input,sample({completed_focuses:['F']}));
 assert.equal(result.lanes.focus.blocker,null);
 assert.equal(result.operations.some(row=>['free','legacy'].includes(row.key)),false);
});
