'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const model=require('../src/hoi4companion/web/guide-model.js');
const fixture=require('./presentation-fixtures.cjs');
// Each scenario is evaluated by Rust; these tests cover how the model presents that evaluation.
function present(name){const {guide,state,progress,evaluation}=fixture(name);return model.evaluate(guide,state,{progress,evaluation});}

test('guides never compute local recommendations while the shared evaluation is missing',()=>{
 const {guide,state}=fixture('model: admitted evaluation');
 const value=model.evaluate(guide,state);
 assert.equal(value.focus.next,null);assert.equal(value.research.next,null);assert.deepEqual(value.research.suggestions,[]);
 assert.ok(value.runtime.steps.every(row=>row.status==='unverifiable'));
});

test('presentation consumes an admitted Rust result and rejects another guide or run',()=>{
 const {guide,state,progress,evaluation}=fixture('model: admitted evaluation');
 const presented=model.evaluate(guide,state,{progress,evaluation});
 assert.equal(presented.focus.rows[0].runtime_status,'completed');
 assert.throws(()=>model.evaluate(guide,state,{progress,evaluation:{...evaluation,guide_id:'other'}}),/evaluation/i);
 assert.throws(()=>model.evaluate(guide,state,{progress:{...progress,run_id:'new'},evaluation:{...evaluation,run_id:'old'}}),/evaluation/i);
});
test('offline plan and wrong country never consume stale completions',()=>{
 const offline=present('model: offline plan');
 assert.equal(offline.mode,'preview');assert.equal(offline.focus.next.key,'f1');assert.equal(offline.focus.rows[0].status,'planned');
 const wrong=present('model: wrong country');
 assert.equal(wrong.mode,'wrong_country');assert.equal(wrong.focus.next.key,'f1');
});
test('an active focus is not completed and manual country gates block the next focus',()=>{
 let r=present('model: active focus');assert.equal(r.focus.active[0].key,'f1');assert.equal(r.focus.next.key,'wait');
 r=present('model: gate unchecked');assert.equal(r.focus.next.key,'wait');assert.equal(r.focus.rows[2].status,'pending');
 r=present('model: gate checked');assert.equal(r.focus.next.key,'f2');
});
test('technology priorities fill parallel slots, with manual projects kept independent',()=>{
 const r=present('model: parallel research slots');
 assert.equal(r.research.active[0].key,'discount');assert.equal(r.research.next.key,'r3');assert.equal(r.research.projects[0].key,'project');assert.equal(r.research.free_slots,1);
 const wait=present('model: research gate waiting');assert.equal(wait.research.next.status,'waiting');assert.equal(wait.research.next.key,'discount');
});
test('Slavic Unity only takes priority after the checkpoint and confirmed availability',()=>{
 let r=present('model: unity available');
 assert.equal(r.focus.next.key,'unity');assert.equal(r.focus.rows.find(x=>x.key==='filler').status,'skipped');
 r=present('model: unity completed');
 assert.equal(r.focus.next.key,'repeat');
 r=present('model: focus tree finished');
 assert.equal(r.focus.next,null);assert.equal(r.focus.rows.find(x=>x.key==='repeat').status,'completed');
});
test('manual research confirmation cannot replace observed technology completion',()=>{
 const r=present('model: manual research confirmation');
 assert.equal(r.research.rows[0].status,'pending');assert.equal(r.research.next.key,'r1');
});
test('native tree mismatch suspends guide state even for the correct tag',()=>{
 const r=present('model: wrong tree');
 assert.equal(r.mode,'wrong_tree');assert.equal(r.focus.rows[0].status,'planned');
});
test('fresh events can roll progress back after loading an earlier save',()=>{
 assert.equal(present('model: earlier save after later research').research.next.key,'r3');
 assert.equal(present('model: earlier save').research.next.key,'r1');
});

test('explicit project dependencies preserve negative manual checks and separate readiness',()=>{
 const result=present('model: negative project check');
 assert.equal(result.research.projects[0].runtime_status,'ready');
 assert.equal(result.research.projects[0].completion.value,false);
 assert.equal(result.research.next,null);
});
test('explicit policy keeps independent projects visible and respects start dependencies',()=>{
 assert.equal(present('model: explicit independent project').research.projects[0].key,'p');
 assert.equal(present('model: project waits for start').research.projects.length,0);
 assert.equal(present('model: project after start').research.projects[0].key,'p');
});
