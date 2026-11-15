'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const summary=require('../src/hoi4companion/web/operational-summary.js');
const fixture=require('./presentation-fixtures.cjs');
const cases=require('./fixtures/action-sequences.json');
test('union of start and completion dependencies is validated',()=>{
 const example=structuredClone(cases[1]),g=example.guide;
 g.tracks.operations[1].depends_on=['project'];delete g.tracks.operations[1].depends_on_started;
 g.tracks.operations[0].depends_on_started=['expert'];assert.throws(()=>contracts.guide(g),/Cyclic/);
 for(const cost of [true,-1,Infinity,2**54]){const invalid=structuredClone(cases[0].guide);invalid.tracks.operations[1].political_power_cost=cost;assert.throws(()=>contracts.guide(invalid));}
});
test('summary keeps expensive next appointment and does not promote a cheaper successor',()=>{
 const {guide,state,evaluation}=fixture('politics: expensive next appointment'),view=summary.project(guide,evaluation,state);
 const expert=view.items.find(i=>i.step_key==='expert');assert.ok(expert);assert.match(expert.message,/70.*150/);
 assert.equal(view.items.some(i=>i.step_key==='monarchist'),false);
});
