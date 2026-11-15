'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const fixture=require('./presentation-fixtures.cjs');

test('the walkthrough exposes the measured target and uses automatic conditions',()=>{
 const view=require('../src/hoi4companion/web/guide-presentation.js'),example=fixture('construction: measured target');
 const row=view.evaluate(example.guide,example.state,{progress:example.progress},example.evaluation).operations[0];
 assert.deepEqual(row.construction,{baseline:5,current:5,target:7});
 assert.equal(row.start_basis,'live');assert.equal(row.completion_condition.op,'fact');
});
