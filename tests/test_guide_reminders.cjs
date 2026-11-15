'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const reminder=(extra={})=>({key:'method',action:{kind:'manual'},tracking:'reminder',...extra});
const focus=()=>({key:'focus',action:{kind:'select_focus',entity:{kind:'focus',id:'focus'}}});
const guide=(...rows)=>({id:'reminders',tracks:{focus:rows}});

test('reminders cannot satisfy completion dependencies',()=>{
 assert.throws(()=>contracts.guide(guide(reminder(),{...focus(),depends_on:['method']})),/reminder/);
});
test('tracking rejects unknown values, native reminders and discarded observable criteria',()=>{
 for(const change of [{tracking:'unknown'},{tracking:null},{completion:{op:'check',key:'done'}},{started_when:{op:'check',key:'started'}},{action:{kind:'free'}},{action:focus().action}])assert.throws(()=>contracts.guide(guide(reminder(change))));
});
