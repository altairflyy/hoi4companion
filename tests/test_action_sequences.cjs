'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const contracts=require('../src/hoi4companion/web/contracts.js');
const summary=require('../src/hoi4companion/web/operational-summary.js');
const cases=require('./fixtures/action-sequences.json');
for(const example of cases)test('action sequence: '+example.name,()=>{
 const before=JSON.stringify(example),result=runtime.evaluate(example.guide,example.state);
 assert.deepEqual(result.steps.map(s=>s.status),example.statuses);
 assert.equal(result.action_sequence.political,example.political);
 assert.equal(JSON.stringify(example),before);
});
test('union of start and completion dependencies is validated and old edges still mean completion',()=>{
 const example=structuredClone(cases[1]),g=example.guide;
 g.tracks.operations[1].depends_on=['project'];delete g.tracks.operations[1].depends_on_started;
 assert.equal(runtime.evaluate(g,example.state).steps[1].status,'waiting');
 g.tracks.operations[0].depends_on_started=['expert'];assert.throws(()=>contracts.guide(g),/Cyclic/);
 for(const cost of [true,-1,Infinity,2**54]){const invalid=structuredClone(cases[0].guide);invalid.tracks.operations[1].political_power_cost=cost;assert.throws(()=>contracts.guide(invalid));}
});
test('summary keeps expensive next appointment and does not promote a cheaper successor',()=>{
 const example=structuredClone(cases[3]);example.guide.tracks.operations[2].depends_on=[];
 example.guide.tracks.operations[2].priority=100;
 const result=runtime.evaluate(example.guide,example.state),view=summary.project(example.guide,result,example.state);
 assert.equal(result.action_sequence.political,'expert');
 const expert=view.items.find(i=>i.step_key==='expert');assert.ok(expert);assert.match(expert.message,/70.*150/);
 assert.equal(view.items.some(i=>i.step_key==='monarchist'),false);
});
