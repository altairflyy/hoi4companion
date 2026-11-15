'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const fixture=require('./presentation-fixtures.cjs');
test('the presentation uses the runtime-bound technology rather than a cached source id',()=>{
 const model=require('../src/hoi4companion/web/guide-model.js');
 const {guide,state,progress:p,evaluation}=fixture('research: bound relative target');
 const result=model.evaluate(guide,state,{progress:p,evaluation});
 assert.equal(result.research.rows[0].target,'c');
 assert.equal(result.research.rows[0].action.entity.id,'c');
 const direct=model.evaluate(guide,state,{...p,evaluation});
 assert.equal(direct.research.rows[0].target,'c');
 assert.equal(direct.research.rows[0].runtime_status,'ready');
});
