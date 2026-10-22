'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const cases=require('./fixtures/construction-progress.json');
for(const example of cases)test('construction baseline: '+example.name,()=>{
 const before=JSON.stringify(example),row=runtime.evaluate(example.guide,example.state,example.progress).steps[0];
 assert.equal(row.status,example.expected);assert.equal(row.completion.value,example.completed);
 assert.equal(JSON.stringify(example),before);
});
test('malformed construction baselines are rejected',()=>{
 const c=cases[0];
 for(const baselines of [null,[],{factory:{value:true}},{factory:{value:-1}}])assert.throws(()=>runtime.evaluate(c.guide,c.state,{...c.progress,baselines}));
});

test('the walkthrough exposes the measured target and uses automatic conditions',()=>{
 const view=require('../src/hoi4companion/web/guide-presentation.js'),example=cases[0];
 const row=view.evaluate(example.guide,example.state,example.progress).operations[0];
 assert.deepEqual(row.construction,{baseline:5,current:5,target:7});
 assert.equal(row.start_basis,'live');assert.equal(row.completion_condition.op,'fact');
});
