'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const contracts=require('../src/hoi4companion/web/contracts.js');
const reminder=(extra={})=>({key:'method',action:{kind:'manual'},tracking:'reminder',...extra});
const focus=()=>({key:'focus',action:{kind:'select_focus',entity:{kind:'focus',id:'focus'}}});
const guide=(...rows)=>({id:'reminders',tracks:{focus:rows}});
const live=()=>({usable:true,sample:{country:'GER',completed_focuses:[],current_focus:null,researched_technologies:[],research_slots:[]}});

test('reminders are visible instructions without confirmation, completion or implicit barriers',()=>{
 const definition=guide(reminder(),focus()),before=structuredClone(definition);
 const result=runtime.evaluate(definition,live(),{schema_version:1,guide_id:'reminders',run_id:'r',checks:{method:true},started:{method:true}}),row=result.steps[0];
 assert.equal(row.status,'ready');assert.equal(row.reason,'reminder_available');assert.equal(row.completion_basis,'reminder');
 assert.equal(row.tracking,'reminder');assert.equal(row.completion.value,null);assert.equal(row.completion.reason,'not_tracked');assert.equal(row.selection.value,null);
 assert.deepEqual(result.ready,['focus']);assert.equal(result.tracks.focus.next,'focus');assert.deepEqual(result.parallel.focus.steps,['focus']);
 assert.deepEqual(definition,before);
});
test('unknown reminder visibility never blocks an independent recommendation',()=>{
 const result=runtime.evaluate(guide(reminder({when:{op:'fact',key:'missing',test:'eq',value:true}}),focus()),live());
 assert.equal(result.steps[0].reason,'condition_unknown');assert.deepEqual(result.parallel.focus.steps,['focus']);
});
test('reminder may depend on a real action but cannot satisfy completion dependencies',()=>{
 const result=runtime.evaluate(guide(focus(),reminder({depends_on:['focus']})),live());
 assert.equal(result.steps[1].status,'waiting');assert.deepEqual(result.steps[1].blocked_by,['focus']);
 assert.throws(()=>contracts.guide(guide(reminder(),{...focus(),depends_on:['method']})),/promemoria/);
});
test('tracking rejects unknown values, native reminders and discarded observable criteria',()=>{
 for(const change of [{tracking:'unknown'},{tracking:null},{completion:{op:'check',key:'done'}},{started_when:{op:'check',key:'started'}},{action:{kind:'free'}},{action:focus().action}])assert.throws(()=>contracts.guide(guide(reminder(change))));
 assert.equal(runtime.evaluate(guide({key:'manual',tracking:'action',action:{kind:'manual'}}),live()).steps[0].reason,'manual_confirmation_required');
});
