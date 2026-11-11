'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),r=require('../src/hoi4companion/web/guide-runtime.js');
for(const f of require('./fixtures/operational-readiness.json'))test(f.name,()=>{const row=r.evaluate(f.guide,f.state,f.progress).steps[0].action_readiness;assert.equal(row.value,f.readiness);assert.equal(row.requirements_declared,true);});
