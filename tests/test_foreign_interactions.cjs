'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const c=require('../src/hoi4companion/web/contracts.js');
function wire(){return {usable:true,sample:{session_id:'session',country:'ROM',country_overview:{country:'ROM',
 diplomacy:{status:'observed',at_war:false,interactions:{status:'observed',countries:['ROM','ITA'],improving_relations:['ITA'],non_aggression_pacts:[]}},
 intelligence:{status:'observed',is_created:true,in_creation:false,missions:{status:'observed',countries:['ROM','ITA'],items:[{id:'agent',mission_id:'diplomatic_pressure',target_country:'ITA',state:'active'}]}}}}};}
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
