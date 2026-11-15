'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const definition=()=>({id:'spirit',tracks:{operations:[{key:'spirit',action:{kind:'manual',params:{category:'politics',idea_id:'loyalty',idea_group:'academy_spirit',cost_currency:'army_experience'}},resource_costs:{'country.experience.army':20},completion:{op:'fact',key:'politics.ideas',test:'contains',value:'loyalty'}}]}});

test('malformed or incompatible experience metadata is rejected',()=>{
 for(const value of ['PP',null,[],{},1]){const g=definition();g.tracks.operations[0].action.params.cost_currency=value;assert.throws(()=>contracts.guide(g));}
 const naval=definition();naval.tracks.operations[0].action.params.idea_group='navy_spirit';assert.throws(()=>contracts.guide(naval));
 const missing=definition();delete missing.tracks.operations[0].resource_costs;assert.throws(()=>contracts.guide(missing));
 const advisor=definition();delete advisor.tracks.operations[0].action.params.idea_id;advisor.tracks.operations[0].action.params.advisor_id='chief';assert.throws(()=>contracts.guide(advisor));
});
