'use strict';
// The suite (web/action-plan.js) and the HUD card (native/guides/src/display_category.rs)
// show the same category for a step.
const test=require('node:test'),assert=require('node:assert/strict');
const plan=require('../src/hoi4companion/web/action-plan.js');
const fixture=require('./fixtures/display-category-contract.json');

test('every contract case gets the same display category as in Rust',()=>{
 for(const c of fixture.cases)assert.equal(plan.category(c.step),c.category,c.name);
});
