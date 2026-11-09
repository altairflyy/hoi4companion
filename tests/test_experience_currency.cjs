'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const definition=()=>({id:'spirit',tracks:{operations:[{key:'spirit',action:{kind:'manual',params:{category:'politics',idea_id:'loyalty',idea_group:'academy_spirit',cost_currency:'army_experience'}},resource_costs:{'country.experience.army':20},completion:{op:'fact',key:'politics.ideas',test:'contains',value:'loyalty'}}]}});
const state=amount=>({schema_version:1,usable:true,facts:Object.fromEntries([['politics.ideas',[]],...(amount===null?[]:[['country.experience.army',amount]])].map(([key,value])=>[key,{status:'observed',source:'fixture',value}]))});

test('experience prices use their declared currency without inventing a PP price',()=>{
 const step=runtime.evaluate(definition(),state(20)).steps[0];
 assert.equal(step.status,'ready');assert.equal(step.action_readiness.resources_available,true);
 assert.equal(step.action_readiness.political_power,undefined);
 assert.equal(runtime.evaluate(definition(),state(10)).steps[0].status,'waiting');
 assert.equal(runtime.evaluate(definition(),state(null)).steps[0].action_readiness.resources_available,null);
});

test('malformed or incompatible experience metadata is rejected',()=>{
 for(const value of ['PP',null,[],{},1]){const g=definition();g.tracks.operations[0].action.params.cost_currency=value;assert.throws(()=>contracts.guide(g));}
 const naval=definition();naval.tracks.operations[0].action.params.idea_group='navy_spirit';assert.throws(()=>contracts.guide(naval));
 const missing=definition();delete missing.tracks.operations[0].resource_costs;assert.throws(()=>contracts.guide(missing));
 const advisor=definition();delete advisor.tracks.operations[0].action.params.idea_id;advisor.tracks.operations[0].action.params.advisor_id='chief';assert.throws(()=>contracts.guide(advisor));
});
