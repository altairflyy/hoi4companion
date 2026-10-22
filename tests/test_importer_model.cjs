'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const m=require('../src/hoi4companion/web/importer-model.js');
const guide={schema_version:1,id:'opening',title:'Opening',tracks:{research:[{key:'tools',action:{kind:'research',entity:{kind:'technology',id:'basic_machine_tools'}}}]}};
test('imports plain guides and versioned editable drafts without mixing progress',()=>{
 assert.equal(m.readDraft(JSON.stringify(guide)).guide.id,'opening');
 const packet=m.readDraft(JSON.stringify({import_schema_version:1,guide,sources:[{id:'source'}]}));
 assert.equal(packet.sources.length,1);assert.equal(packet.guide.tracks.research[0].key,'tools');
 for(const value of [[],{import_schema_version:2,guide},{usable:true,sample:{}},null])assert.throws(()=>m.readDraft(JSON.stringify(value)));
});
test('preview edits preserve schema conditions params evidence and step ordering',()=>{
 const value=structuredClone(guide);value.tracks.research[0].evidence=[{source_id:'s',basis:'explicit',quote:'tools'}];
 value.tracks.operations=[{key:'build',action:{kind:'manual',params:{quantity:2}},completion:{op:'check',key:'built'}}];
 const before=JSON.stringify(value);
 const edited=m.editStep(value,'tools',{entity:{kind:'technology',id:'improved_machine_tools'},title:'Improved'});
 assert.equal(edited.tracks.research[0].action.entity.id,'improved_machine_tools');
 assert.deepEqual(edited.tracks.operations,value.tracks.operations);assert.deepEqual(edited.tracks.research[0].evidence,value.tracks.research[0].evidence);
 assert.equal(JSON.stringify(value),before);
});
test('final exports require current valid preview and human review',()=>{
 assert.equal(m.canPublish({valid:true},true,false),true);
 assert.equal(m.canPublish({valid:true},false,false),false);
 assert.equal(m.canPublish({valid:false},true,false),false);
 assert.equal(m.canPublish({valid:true},true,true),false);
 assert.equal(m.canPublish(null,true,false),false);
});
test('normalisation and dependencies reuse the same JavaScript schema contract',()=>{
 assert.equal(m.normalise(guide).tracks.research[0].target,'basic_machine_tools');
 const cycle=structuredClone(guide);cycle.tracks.research[0].depends_on=['tools'];
 assert.equal(m.readDraft(JSON.stringify(cycle)).guide.id,'opening'); // invalid drafts remain editable
 assert.throws(()=>m.normalise(cycle));
});
