'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),c=require('../src/hoi4companion/web/contracts.js');
const sample=()=>({usable:true,sample:{country:'TST',country_overview:{country:'TST',politics:{status:'observed',ideas:{status:'observed',items:[{id:'war_economy',group:'economy',category:'law',is_law:true}]},advisors:{status:'observed',items:[{id:'TST_advisor',character_id:'TST_person',slot:1}]}},events:{status:'observed',visible:[{instance_id:'1',event_id:'example.1',option_ids:['example.1.a']}]}}}});
test('qualified politics and visible event options retain known empty vs missing',()=>{
 const v=sample(),f=c.state(v).facts;assert.deepEqual(f['politics.ideas'].value,['war_economy']);assert.deepEqual(f['politics.advisors'].value,['TST_advisor']);assert.deepEqual(f['events.visible_options'].value,['example.1:example.1.a']);
 delete v.sample.country_overview.events.visible[0].option_ids;assert.deepEqual(c.state(v).facts['events.visible'].value,['example.1']);assert.equal(c.state(v).facts['events.visible_options'].status,'unknown');
 v.sample.country_overview.politics.advisors.items=[];assert.deepEqual(c.state(v).facts['politics.advisors'].value,[]);
});
test('stale/foreign/malformed/subsection unavailable politics never qualify',()=>{
 for(const mutate of [v=>v.usable=false,v=>v.sample.country_overview.country='OTH',v=>v.sample.country_overview.politics.ideas.status='unavailable',v=>v.sample.country_overview.politics.ideas.items.push({id:'war_economy'})]){const v=sample();mutate(v);assert.equal(c.state(v).facts['politics.ideas'].status,'unknown');}
 for(const key of ['politics.ideas','politics.advisors','events.visible','events.visible_options'])assert.equal(c.state({schema_version:1,usable:true,facts:{[key]:{status:'observed',value:[null],source:'fixture'}}}).facts[key].status,'unknown');
});
