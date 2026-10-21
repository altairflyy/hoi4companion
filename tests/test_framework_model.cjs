'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const planning=require('../src/hoi4companion/web/guide-model.js');
const wireExamples=require('./fixtures/framework-state.json');
const observed=v=>({status:'observed',value:v,source:'process_memory'});
test('conditions propagate unknown, including negation; zero and false are facts',()=>{
 const state={schema_version:1,usable:true,facts:{zero:observed(0),no:observed(false)}};
 assert.equal(contracts.condition({op:'fact',key:'zero',test:'eq',value:0},state,{}),true);
 assert.equal(contracts.condition({op:'not',arg:{op:'fact',key:'missing',test:'eq',value:true}},state,{}),null);
 assert.equal(contracts.condition({op:'any',args:[{op:'check',key:'yes'},{op:'fact',key:'missing',test:'eq',value:true}]},state,{yes:true}),true);
 assert.equal(contracts.condition({op:'check',key:'no'},state,{no:'yes'}),null);
});
test('multiple declarative priorities work for another country without special cases',()=>{
 const guide={schema_version:1,id:'generic',applies_to:{country:'GER',tree_id:'g'},tracks:{focus:[
  {key:'a',kind:'focus',target:'A',action:{kind:'select_focus',entity:{kind:'focus',id:'A'}}},
  {key:'b',kind:'focus',target:'B',action:{kind:'select_focus',entity:{kind:'focus',id:'B'}},priority:10,priority_when:{op:'check',key:'b_ready'}},
  {key:'c',kind:'focus',target:'C',action:{kind:'select_focus',entity:{kind:'focus',id:'C'}},priority:20,priority_when:{op:'check',key:'c_ready'}}],research:[]}};
 const value={usable:true,sample:{country:'GER',focus_tree_id:'g',completed_focuses:[],researched_technologies:[],research_slots:[]}};
 const result=planning.evaluate(guide,value,{checks:{b_ready:true,c_ready:true}});
 assert.equal(result.focus.next.key,'c');
 assert.equal(result.focus.next.provenance,'calculated');
 assert.equal(result.focus.rows[0].availability.status,'unknown');
 assert.equal(planning.evaluate(guide,{...value,usable:false},{}).focus.rows[0].status,'planned');
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
test('Python and JavaScript consume the same versioned wire examples',()=>{
 for(const example of wireExamples){const result=contracts.state(example.input);for(const [key,expected]of Object.entries(example.expected))assert.deepEqual({status:result.facts[key].status,value:result.facts[key].value},expected,key);}
});
test('research uses the same priorities and unknown membership stays unknown',()=>{
 const guide={tracks:{research:[{kind:'technology',target:'a'},{kind:'technology',target:'b',priority:5,priority_when:{op:'check',key:'ready'}}]}};
 const result=planning.evaluate(guide,{usable:true,sample:{country:'GER'}},{checks:{ready:true}});
 assert.equal(result.research.next.target,'b');
 assert.equal(result.research.rows[0].completion.status,'unknown');
 assert.equal(result.research.free_slots,null);
});
test('entity scope prevents false completion in another focus tree',()=>{
 const guide={tracks:{focus:[{action:{kind:'select_focus',entity:{kind:'focus',id:'shared',scope:'a'}}}]}};
 const result=planning.evaluate(guide,{usable:true,sample:{country:'GER',focus_tree_id:'b',completed_focuses:['shared']}});
 assert.equal(result.focus.rows[0].status,'waiting');
 assert.equal(result.focus.rows[0].completion.status,'unknown');
});
