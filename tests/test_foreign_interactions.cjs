'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const c=require('../src/hoi4companion/web/contracts.js');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const f=(key,test,value)=>({op:'fact',key,test,value});
function wire(){return {usable:true,sample:{session_id:'session',country:'ROM',country_overview:{country:'ROM',
 diplomacy:{status:'observed',at_war:false,interactions:{status:'observed',countries:['ROM','ITA'],improving_relations:['ITA'],non_aggression_pacts:[]}},
 intelligence:{status:'observed',is_created:true,in_creation:false,missions:{status:'observed',countries:['ROM','ITA'],items:[{id:'agent',mission_id:'diplomatic_pressure',target_country:'ITA',state:'active'}]}}}}};}
function guide(){const current={op:'all',args:[f('diplomacy.countries','contains','ITA'),f('diplomacy.improving_relations','contains','ITA')]};return {schema_version:1,id:'guide',title:'Foreign',dependency_policy:'explicit',applies_to:{country:'ROM'},tracks:{operations:[
 {key:'relations',action:{kind:'manual',params:{operation_id:'improve_relations',target_country:'ITA'}},started_when:current,completion:{op:'any',args:[current,f('guide.campaign.milestones','contains','foreign:recorded')]}},
 {key:'later',action:{kind:'manual',params:{operation_id:'assign_operative_mission',mission_id:'diplomatic_pressure',target_country:'ITA'}},when:f('focus.completed','contains','later_focus'),completion:f('intelligence.active_missions','contains','diplomatic_pressure:ITA')}
 ]}};}
test('qualified spy activity distinguishes active, captured and operating agents',()=>{
 const raw=wire();assert.deepEqual(c.state(raw).facts['intelligence.active_missions'].value,['diplomatic_pressure:ITA']);
 for(const state of ['captured','cooldown','disbanding','operation','killed']){raw.sample.country_overview.intelligence.missions.items[0].state=state;assert.deepEqual(c.state(raw).facts['intelligence.active_missions'].value,[]);}
 raw.sample.country_overview.intelligence.missions.items[0].state='unknown';assert.equal(c.state(raw).facts['intelligence.active_missions'].status,'unknown');
});
test('unknown lists never prove a stopped diplomatic action or a missing mission',()=>{
 const raw=wire();raw.sample.country_overview.diplomacy.interactions.improving_relations=null;
 assert.equal(c.state(raw).facts['diplomacy.improving_relations'].status,'unknown');
 raw.sample.country_overview.intelligence.missions.status='partial';assert.equal(c.state(raw).facts['intelligence.active_missions'].status,'unknown');
 raw.usable=false;assert.equal(c.state(raw).facts['diplomacy.non_aggression_pacts'].status,'unknown');
});
test('mission evidence cannot complete a future conditional action',()=>{
 const raw=wire();raw.sample.completed_focuses=[];
 const r=runtime.evaluate(guide(),raw);assert.equal(r.steps[0].status,'completed');assert.equal(r.steps[1].status,'waiting');
 raw.sample.completed_focuses=['later_focus'];assert.equal(runtime.evaluate(guide(),raw).steps[1].status,'completed');
});
test('session-qualified history survives reassignment but cannot cross sessions',()=>{
 const raw=wire();raw.sample.country_overview.diplomacy.interactions.improving_relations=[];
 const progress={schema_version:1,guide_id:'guide',run_id:'run',session_id:'session',checks:{},started:{},campaign_conditions:{country:'ROM',milestones:{'foreign:recorded':true}}};
 assert.equal(runtime.evaluate(guide(),raw,progress).steps[0].status,'completed');
 raw.sample.session_id='other';assert.notEqual(runtime.evaluate(guide(),raw,progress).steps[0].status,'completed');
 raw.sample.country='ITA';assert.equal(runtime.evaluate(guide(),raw,progress).steps[0].status,'not_applicable');
});
