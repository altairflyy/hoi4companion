'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),c=require('../src/hoi4companion/web/contracts.js'),runtime=require('../src/hoi4companion/web/guide-runtime.js');
const rows=[['field_marshal','marshal','army_group','3500000001000000'],['corps_commander','general','army','3500000002000000'],['navy_leader','admiral','fleet','3d00000003000000']].map(([role,commander_id,target_kind,instance_id])=>({role,commander_id,target_kind,instance_id,name:'Native target'}));
const sample=()=>({usable:true,sample:{country:'TST',country_overview:{country:'TST',politics:{status:'partial',commander_assignments:{schema_version:1,status:'observed',source:'native_reciprocal_commander_assignments',items:structuredClone(rows)}}}}});
test('commander facts retain exact role and native target rather than roster membership',()=>{
 const facts=c.state(sample()).facts;
 assert.deepEqual(facts['politics.commanders.assigned'].value,['corps_commander:general','field_marshal:marshal','navy_leader:admiral']);
 assert.ok(facts['politics.commander_assignments'].value.includes('navy_leader:admiral:fleet:3d00000003000000'));
 const empty=sample();empty.sample.country_overview.politics.commander_assignments.items=[];assert.deepEqual(c.state(empty).facts['politics.commanders.assigned'].value,[]);
 const roster=sample();delete roster.sample.country_overview.politics.commander_assignments;roster.sample.country_overview.politics.characters={status:'observed',source:'native_character_status_roles',items:[{id:'general',country_leader:false,advisor:false,unit_leader:true,scientist:false}]};assert.equal(c.state(roster).facts['politics.commanders.assigned'].status,'unknown');
});
test('commander wire rejects foreign country, role mismatch, duplicate targets and malformed identity',()=>{
 for(const change of [s=>s.usable=false,s=>s.sample.country_overview.country='OTH',s=>s.sample.country_overview.politics.commander_assignments.status='unavailable',s=>s.sample.country_overview.politics.commander_assignments.items[0].target_kind='fleet',s=>s.sample.country_overview.politics.commander_assignments.items[0].instance_id='bad',s=>s.sample.country_overview.politics.commander_assignments.items[0].name='bad\nname',s=>s.sample.country_overview.politics.commander_assignments.items[1].instance_id=rows[0].instance_id]){const s=sample();change(s);assert.equal(c.state(s).facts['politics.commander_assignments'].status,'unknown');}
 for(const value of [['corps_commander:general:army:invalid'],['field_marshal:marshal:fleet:3500000001000000']])assert.equal(c.state({schema_version:1,usable:true,facts:{'politics.commander_assignments':{status:'observed',source:'fixture',value}}}).facts['politics.commander_assignments'].status,'unknown');
});
test('exact target and role control commander completion in the browser runtime',()=>{
 const node={op:'fact',key:'politics.commander_assignments',test:'contains',value:'navy_leader:admiral:fleet:3d00000003000000'};
 const step={key:'assign',requirements:{op:'fact',key:'country',test:'eq',value:'TST'},completion:node,action:{kind:'manual',params:{commander_id:'admiral',role:'navy_leader',political_binding:{completion:node}}}};
 const guide={id:'commanders',dependency_policy:'explicit',tracks:{operations:[step]}};
 const s=c.state(sample());assert.equal(runtime.evaluate(guide,s).steps[0].status,'completed');
 s.facts['politics.commander_assignments'].value=['navy_leader:admiral:fleet:3d00000004000000'];assert.notEqual(runtime.evaluate(guide,s).steps[0].status,'completed');
 s.facts['politics.commander_assignments'].value=[];assert.equal(runtime.evaluate(guide,s).steps[0].action_readiness.value,true);
 delete s.facts['politics.commander_assignments'];assert.ok(runtime.evaluate(guide,s).steps[0].action_readiness.unknown.includes('commander_assignment_unobserved'));
});

