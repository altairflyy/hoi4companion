'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const cases=require('./fixtures/production-progress.json');
for(const example of cases)test('production: '+example.name,()=>{
 const before=JSON.stringify(example),row=runtime.evaluate(example.guide,example.state,example.progress).steps[0];
 assert.equal(row.status,example.expected);assert.equal(row.production.assigned,example.assigned);
 assert.equal(JSON.stringify(example),before);
});
