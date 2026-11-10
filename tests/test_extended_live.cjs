'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
for(const row of require('./fixtures/live-extended-state.json'))test('extended live contract: '+row.name,()=>{
 const before=JSON.stringify(row),value=contracts.state(row.input);
 for(const [key,fields]of Object.entries(row.expected))for(const [field,expected]of Object.entries(fields))assert.deepEqual(contracts.fact(value,key)[field],expected,key+'.'+field);
 assert.equal(JSON.stringify(row),before);
});
for(const row of require('./fixtures/construction-extended.json'))test('extended building baseline: '+row.name,()=>{
 const step=runtime.evaluate(row.guide,row.state,row.progress).steps[0];assert.equal(step.status,row.expected);assert.equal(step.completion.value,row.completed);
});
