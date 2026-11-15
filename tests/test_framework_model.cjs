'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const planning=require('../src/hoi4companion/web/guide-model.js');
const wireExamples=require('./fixtures/framework-state.json');
const fixture=require('./presentation-fixtures.cjs');
function present(name){const {guide,state,progress,evaluation}=fixture(name);return planning.evaluate(guide,state,{progress,evaluation});}
const observed=v=>({status:'observed',value:v,source:'process_memory'});
test('conditions propagate unknown, including negation; zero and false are facts',()=>{
 const state={schema_version:1,usable:true,facts:{zero:observed(0),no:observed(false)}};
 assert.equal(contracts.condition({op:'fact',key:'zero',test:'eq',value:0},state,{}),true);
 assert.equal(contracts.condition({op:'not',arg:{op:'fact',key:'missing',test:'eq',value:true}},state,{}),null);
 assert.equal(contracts.condition({op:'any',args:[{op:'check',key:'yes'},{op:'fact',key:'missing',test:'eq',value:true}]},state,{yes:true}),true);
 assert.equal(contracts.condition({op:'check',key:'no'},state,{no:'yes'}),null);
});
test('multiple declarative priorities work for another country without special cases',()=>{
 const result=present('framework: declarative priorities');
 assert.equal(result.focus.next.key,'c');
 assert.equal(result.focus.next.provenance,'calculated');
 assert.equal(result.focus.rows[0].availability.status,'unknown');
 assert.equal(present('framework: declarative priorities offline').focus.rows[0].status,'planned');
});
test('missing native fields cannot assert completion and malformed versions fail closed',()=>{
 const s=contracts.state({usable:true,sample:{country:'AUS'}});
 assert.equal(s.facts['focus.completed'].status,'unknown');
 assert.equal(contracts.state({schema_version:99,usable:true,facts:{country:observed('AUS')}}).usable,false);
});
test('country-neutral guides support empty and additional tracks without crashing',()=>{
 const r=planning.evaluate({schema_version:1,id:'any',applies_to:{},tracks:{construction:[{key:'build',kind:'free',action:{kind:'free'}}]}},null);
 assert.equal(r.focus.next,null);assert.equal(r.tracks.construction.rows[0].status,'free');
});
test('Rust and JavaScript consume the same versioned wire examples',()=>{
 for(const example of wireExamples){const result=contracts.state(example.input);for(const [key,expected]of Object.entries(example.expected))assert.deepEqual(Object.fromEntries(Object.keys(expected).map(field=>[field,result.facts[key][field]])),expected,example.name||key);for(const key of example.absent||[])assert.equal(Object.hasOwn(result.facts,key),false,example.name||key);}
});
test('research uses the same priorities and unknown membership stays unknown',()=>{
 const result=present('framework: research priorities');
 assert.equal(result.research.next.target,'b');
 assert.equal(result.research.rows[0].completion.status,'unknown');
 assert.equal(result.research.free_slots,null);
});
test('entity scope prevents false completion in another focus tree',()=>{
 const result=present('framework: entity scope');
 assert.equal(result.focus.rows[0].status,'waiting');
 assert.equal(result.focus.rows[0].completion.status,'unknown');
});
