'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const m=require('../src/hoi4companion/web/importer-model.js');
const guide={schema_version:1,id:'opening',title:'Opening',tracks:{research:[{key:'tools',action:{kind:'research',entity:{kind:'technology',id:'basic_machine_tools'}}}]}};

test('authoring documents are delegated intact to the server rather than interpreted as runtime guides',()=>{
 const value={format:'hoi4-guide-authoring',authoring_schema_version:1,sources:[{id:'s'}],guide:{id:'new',steps:[{type:'build'}]}};
 const result=m.readDraft(JSON.stringify(value));
 assert.equal(result.authoring,true);assert.deepEqual(result.document,value);
 assert.equal(result.guide.steps[0].type,'build');
 for(const version of [2,true,'1'])assert.throws(()=>m.readDraft(JSON.stringify({...value,authoring_schema_version:version})));
});
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

test('explicit conversion to a reminder removes obsolete manual tracking only on the editable copy',()=>{
 const value={schema_version:1,id:'x',tracks:{operations:[{key:'method',action:{kind:'manual',params:{instructions:['Rush victory points']}},when:{op:'check',key:'branch'},completion:{op:'check',key:'old'},started_when:{op:'check',key:'old-start'}}]}};
 const edited=m.editStep(value,'method',{tracking:'reminder'}),step=edited.tracks.operations[0];
 assert.equal(step.tracking,'reminder');assert.equal(step.completion,undefined);assert.equal(step.started_when,undefined);assert.deepEqual(step.when,value.tracks.operations[0].when);assert.ok(value.tracks.operations[0].completion);
 assert.equal(m.normalise(edited).tracks.operations[0].tracking,'reminder');
 assert.throws(()=>m.editStep(guide,'tools',{tracking:'reminder'}));
});

test('construction editor changes structured location and target without changing source or evidence',()=>{
 const value={schema_version:1,id:'build',tracks:{operations:[{key:'infra',note:'Original uncertainty',action:{kind:'manual',params:{category:'construction',building_id:'infrastructure',state_id:91,state_name:'Lwow',quantity:1,quantity_mode:'additional',instructions:['Build in Lwow']}},evidence:[{source_id:'s',quote:'Original source'}],started_when:{op:'fact',key:'construction.queued',test:'contains',value:'state:91:infrastructure'},completion:{op:'fact',key:'guide.construction.infra.added',test:'gte',value:1}}]}};
 const patch={state:{kind:'state',id:'762',name:'Katowice'},building:{kind:'building',id:'arms_factory',name:'Military Factory'},quantity:2,quantity_mode:'additional',instruction:'Build two factories in Katowice'};
 const before=JSON.stringify(value),next=m.editConstruction(value,'infra',patch),step=next.tracks.operations[0];
 assert.equal(step.action.params.state_id,762);assert.equal(step.action.params.state_name,'Katowice');assert.equal(step.action.params.building_id,'arms_factory');assert.equal(step.action.params.quantity,2);
 assert.equal(step.started_when,undefined);assert.equal(step.completion,undefined);assert.deepEqual(step.evidence,value.tracks.operations[0].evidence);assert.equal(step.note,'Original uncertainty');assert.equal(JSON.stringify(value),before);
 for(const bad of [{state:null},{quantity:0},{quantity:1.5},{state:{kind:'state',id:'NaN'}},{building:null},{instruction:''},{quantity_mode:'guess'}])assert.throws(()=>m.editConstruction(value,'infra',{...patch,...bad}));
 const absolute=structuredClone(value),row=absolute.tracks.operations[0];Object.assign(row.action.params,{target_level:5,baseline_level:3});row.completion={op:'fact',key:'country.states.items.91.infrastructure',test:'gte',value:5};
 const edited=m.editConstruction(absolute,'infra',{...patch,quantity_mode:'target_total',quantity:4}).tracks.operations[0];assert.equal(edited.completion,undefined);assert.equal(edited.action.params.target_level,undefined);assert.equal(edited.action.params.baseline_level,undefined);
 row.completion={op:'all',args:[row.completion,{op:'check',key:'custom'}]};const custom=JSON.stringify(absolute);assert.throws(()=>m.editConstruction(absolute,'infra',patch),/criteri personalizzati/);assert.equal(JSON.stringify(absolute),custom);
});

test('coverage edits preserve original evidence and reject invalid or empty dispositions',()=>{
 const src=structuredClone(guide);src.source_inventory={schema_version:1,items:[{id:'i',source_id:'s',quote:'source',step_keys:[]}]};
 const before=structuredClone(src);
 assert.throws(()=>m.editCoverage(src,'i',['missing']));assert.throws(()=>m.editCoverage(src,'i',[]));
 assert.throws(()=>m.editCoverage(src,'i',['tools'],'exclude'));assert.throws(()=>m.editCoverage(src,'i',['tools','tools']));
 const excluded=m.editCoverage(src,'i',[],'Overlap');assert.equal(excluded.source_inventory.items[0].excluded_reason,'Overlap');
 const linked=m.editCoverage(excluded,'i',['tools']);assert.equal(linked.source_inventory.items[0].excluded_reason,undefined);
 assert.equal(linked.source_inventory.items[0].quote,'source');assert.deepEqual(src,before);
});

test('source links reject executable schemes and preserve zero and absolute video times',()=>{
 for(const url of ['javascript:alert(1)','data:text/html,x','file:///secret','https://example.com\n'])assert.equal(m.sourceHref({url},1),null);
 assert.equal(m.sourceHref({url:'https://www.youtube.com/watch?v=abc&t=99s'},0),'https://www.youtube.com/watch?v=abc&t=0s');
 assert.equal(m.sourceHref({url:'https://example.com/article#section'},104),'https://example.com/article#section');
});
