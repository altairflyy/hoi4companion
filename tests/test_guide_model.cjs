'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const model=require('../src/hoi4companion/web/guide-model.js');
const guide={country:'POL',tree_id:'polish_focus',tracks:{focus:[
 {key:'f1',kind:'focus',target:'A'},{key:'wait',kind:'gate'},
 {key:'f2',kind:'focus',target:'B'},{key:'filler',kind:'focus',target:'C',optional_before:'U'},
 {key:'unity',kind:'focus',target:'U',priority_after:'B',available_gate:'unity_available'},
 {key:'repeat',kind:'focus',target:'C'}],research:[
 {key:'r1',kind:'technology',target:'t1'},{key:'discount',kind:'technology',target:'t2',gate:'discount_ready'},
 {key:'project',kind:'project',target:'project1'},{key:'r3',kind:'technology',target:'t3'},
 {key:'r4',kind:'technology',target:'t4'}]}};
function live(extra={}){return {usable:true,sample:{country:'POL',focus_tree_id:'polish_focus',completed_focuses:[],current_focus:null,focus_ids:['A','B','C','U'],technology_ids:['t1','t2','t3','t4'],researched_technologies:[],research_slots:[{slot:1,technology_id:null},{slot:2,technology_id:null}],...extra}};}
test('offline plan and wrong country never consume stale completions',()=>{
 const offline=model.evaluate(guide,{usable:false,sample:live({completed_focuses:['A']}).sample},{});
 assert.equal(offline.mode,'preview');assert.equal(offline.focus.next.key,'f1');assert.equal(offline.focus.rows[0].status,'planned');
 const wrong=model.evaluate(guide,live({country:'ROM',completed_focuses:['A']}),{});
 assert.equal(wrong.mode,'wrong_country');assert.equal(wrong.focus.next.key,'f1');
});
test('an active focus is not completed and manual country gates block the next focus',()=>{
 let r=model.evaluate(guide,live({current_focus:'A'}),{});assert.equal(r.focus.active[0].key,'f1');assert.equal(r.focus.next.key,'wait');
 r=model.evaluate(guide,live({completed_focuses:['A']}),{});assert.equal(r.focus.next.key,'wait');assert.equal(r.focus.rows[2].status,'pending');
 r=model.evaluate(guide,live({completed_focuses:['A']}),{checks:{wait:true}});assert.equal(r.focus.next.key,'f2');
});
test('technology priorities fill parallel slots, with manual projects kept independent',()=>{
 const r=model.evaluate(guide,live({researched_technologies:['t1'],research_slots:[{slot:1,technology_id:'t2'},{slot:2,technology_id:null}]}),{});
 assert.equal(r.research.active[0].key,'discount');assert.equal(r.research.next.key,'r3');assert.equal(r.research.projects[0].key,'project');assert.equal(r.research.free_slots,1);
 const wait=model.evaluate(guide,live({researched_technologies:['t1']}),{});assert.equal(wait.research.next.status,'waiting');assert.equal(wait.research.next.key,'discount');
});
test('Slavic Unity only takes priority after the checkpoint and confirmed availability',()=>{
 let r=model.evaluate(guide,live({completed_focuses:['A','B']}),{checks:{wait:true,unity_available:true}});
 assert.equal(r.focus.next.key,'unity');assert.equal(r.focus.rows.find(x=>x.key==='filler').status,'skipped');
 r=model.evaluate(guide,live({completed_focuses:['A','B','U']}),{checks:{wait:true}});
 assert.equal(r.focus.next.key,'repeat');
 r=model.evaluate(guide,live({completed_focuses:['A','B','C','U']}),{checks:{wait:true}});
 assert.equal(r.focus.next,null);assert.equal(r.focus.rows.find(x=>x.key==='repeat').status,'completed');
});
test('manual research confirmation cannot replace observed technology completion',()=>{
 const r=model.evaluate(guide,live(),{checks:{r1:true,discount_ready:true}});
 assert.equal(r.research.rows[0].status,'pending');assert.equal(r.research.next.key,'r1');
});
test('native tree mismatch suspends guide state even for the correct tag',()=>{
 const r=model.evaluate(guide,live({focus_tree_id:'other',completed_focuses:['A']}),{});
 assert.equal(r.mode,'wrong_tree');assert.equal(r.focus.rows[0].status,'planned');
});
test('fresh events can roll progress back after loading an earlier save',()=>{
 assert.equal(model.evaluate(guide,live({researched_technologies:['t1','t2']}),{}).research.next.key,'r3');
 assert.equal(model.evaluate(guide,live(),{}).research.next.key,'r1');
});

test('explicit project dependencies preserve negative manual checks and separate readiness',()=>{
 const definition={id:'other',tracks:{research:[{key:'p',kind:'project',target:'p',depends_on:[]}]}};
 const result=model.evaluate(definition,live(),{checks:{p:false}});
 assert.equal(result.research.projects[0].runtime_status,'ready');
 assert.equal(result.research.projects[0].completion.value,false);
 assert.equal(result.research.next,null);
});
