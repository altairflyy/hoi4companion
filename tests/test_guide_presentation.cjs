'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const view=require('../src/hoi4companion/web/guide-presentation.js');
const guide={id:'neutral',applies_to:{country:'GER',tree_id:'g'},tracks:{focus:[
 {key:'f',kind:'focus',target:'F',name:'Focus',href:'/?tree=g&focus=F'}],research:[
 {key:'r',kind:'technology',target:'R',name:'Research',href:'/technologies?folder=industry&tech=R'},
 {key:'p',kind:'project',target:'P',name:'Project'}],operations:[
 {key:'build',action:{kind:'manual',params:{category:'construction',timing:'Start',instructions:['Build a facility']}},depends_on:[],title:'Construction',name:'Construction'}]},milestones:[{key:'build',title:'Construction'},{key:'legacy',title:'Legacy advice'}]};
const sample=(extra={})=>({usable:true,sample:{session_id:'s',country:'GER',focus_tree_id:'g',completed_focuses:[],current_focus:null,researched_technologies:[],research_slots:[{technology_id:null}],...extra}});

test('event recommendations qualify visible choice independently of completion and never infer a click',()=>{
 const input={id:'event-guide',tracks:{operations:[{key:'event',action:{kind:'manual',params:{event_id:'event.1',event_option_id:'event.1.a',event_option_name:'Recommended'}}}]}};
 const current={usable:true,sample:{country:'GER',country_overview:{country:'GER',events:{status:'observed',visible:[{instance_id:'1',event_id:'event.1',option_ids:['event.1.a']}]}}}};
 let row=view.evaluate(input,current).operations[0];assert.equal(row.event_observation.visible,true);assert.equal(row.event_observation.option_available,true);assert.notEqual(row.runtime_status,'completed');
 delete current.sample.country_overview.events.visible[0].option_ids;row=view.evaluate(input,current).operations[0];assert.equal(row.event_observation.visible,true);assert.equal(row.event_observation.option_available,null);
 current.usable=false;assert.equal(view.evaluate(input,current).operations[0].event_observation.visible,null);
});
test('native actions appear only as tree annotations; operations retain checkpoints and projects',()=>{
 const before=JSON.stringify(guide),result=view.evaluate(guide,sample(),{checks:{}});
 assert.equal(result.lanes.focus.markers[0].tone,'next');assert.equal(result.lanes.research.markers[0].tone,'next');
 assert.equal(result.lanes.research.folder,'industry');
 assert.deepEqual(result.operations.map(x=>x.key).sort(),['build','legacy','p']);
 assert.equal(result.operations.filter(x=>x.key==='build').length,1);
 assert.equal(result.operations.find(x=>x.key==='build').category,'construction');
 assert.equal(JSON.stringify(guide),before);
});
test('stale/wrong scope suppress actionable highlights and busy slots mark upcoming only',()=>{
 assert.equal(view.evaluate(guide,{usable:false,sample:sample().sample}).lanes.focus.markers[0].tone,'preview');
 assert.equal(view.evaluate(guide,sample({country:'POL'})).lanes.focus.markers.length,0);
 assert.equal(view.evaluate(guide,sample({research_slots:[{technology_id:'X'}]})).lanes.research.markers[0].tone,'upcoming');
 const active=view.evaluate(guide,sample({current_focus:'F'}));
 assert.equal(active.lanes.focus.markers[0].tone,'active');assert.equal(active.lanes.focus.next,null);
});
test('gates prevent later focus illumination; confirmations and phases remain operational',()=>{
 const input=structuredClone(guide);input.tracks.focus.unshift({key:'wait',kind:'gate',name:'Wait'});
 assert.equal(view.evaluate(input,sample()).lanes.focus.markers.length,0);
 assert.equal(view.evaluate(input,sample()).lanes.focus.blocker.key,'wait');
 assert.equal(view.evaluate(input,sample(),{checks:{wait:true}}).lanes.focus.markers[0].id,'F');
});
test('duplicate target markers are unique and tree scope is attached to annotations',()=>{
 const input=structuredClone(guide);input.tracks.focus.push({key:'repeat',kind:'focus',target:'F'});
 const result=view.evaluate(input,sample());assert.equal(result.lanes.focus.markers.length,1);
 assert.equal(result.lanes.focus.markers[0].scope,'g');
});

test('free choices and old combined milestone aliases do not create duplicate instructions',()=>{
 const input=structuredClone(guide);input.tracks.focus.push({key:'free',kind:'free'});
 input.tracks.operations[0].action.params.legacy_key='legacy';
 const result=view.evaluate(input,sample({completed_focuses:['F']}));
 assert.equal(result.lanes.focus.blocker,null);
 assert.equal(result.operations.some(row=>['free','legacy'].includes(row.key)),false);
});
