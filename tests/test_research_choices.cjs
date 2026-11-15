'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const presentation=require('../src/hoi4companion/web/guide-presentation.js');
const contracts=require('../src/hoi4companion/web/contracts.js');
const fixture=require('./presentation-fixtures.cjs');
function view(name){const {guide,state,progress,evaluation}=fixture(name);return presentation.evaluate(guide,state,progress?{progress}:{},evaluation);}
test('open branch choice has two purple options',()=>{
 assert.deepEqual(view('research: open branch choice').lanes.research.markers.map(m=>[m.id,m.tone]),[['c1','choice'],['d1','choice']]);
});
test('stored selection marks the same branch as next',()=>{
 assert.deepEqual(view('research: stored branch choice').lanes.research.markers.map(m=>[m.id,m.tone]),[['d2','next']]);
});
test('malformed alternative branches are rejected by contracts',()=>{
 const {guide}=fixture('research: open branch choice');
 for(const change of [c=>c.branches.dispersed=['c1'],c=>c.level=0,c=>c.level=4,c=>c.group='',c=>c.branches={one:['c1']}]){
  const g=structuredClone(guide);change(g.tracks.research[0].research_choice);
  assert.throws(()=>contracts.guide(g));
 }
});
test('optional continuous focus is purple without occupying the mandatory focus slot',()=>{
 const result=view('focus: optional continuous focus');
 assert.deepEqual(result.lanes.focus.markers.map(m=>[m.id,m.tone]),[['federation','next'],['suppress','choice']]);
 assert.deepEqual(result.model.runtime.parallel.focus,{capacity:1,steps:['next'],optional_steps:['optional']});
});
