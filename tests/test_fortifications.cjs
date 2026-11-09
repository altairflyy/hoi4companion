'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const contracts=require('../src/hoi4companion/web/contracts.js');
const model=require('../src/hoi4companion/web/importer-model.js');
const cases=require('./fixtures/fortifications.json');
for(const item of cases)test('fortification shared contract: '+item.name,()=>{
 const before=JSON.stringify(item),row=runtime.evaluate(item.guide,item.state).steps[0];
 assert.equal(row.status,item.expected);
 assert.equal(row.fortifications.target_level,1);
 assert.equal(row.construction,undefined);
 assert.equal(JSON.stringify(item),before);
});
test('native province adapter requires complete qualified unique levels and border ownership',()=>{
 const section={status:'observed',items:[{id:10,bunker:1},{id:11,bunker:1}],borders_status:'observed',borders:{YUG:[10,11]},controllers:{10:'HUN',20:'YUG'}};
 const wire={usable:true,sample:{country:'HUN',country_overview:{country:'HUN',fortifications:section}}};
 assert.equal(runtime.evaluate(cases[0].guide,wire).steps[0].status,'completed');
 assert.equal(contracts.state(wire).facts['country.fortifications.controllers.10'],undefined);
 section.items.push({id:10,bunker:1});
 assert.notEqual(runtime.evaluate(cases[0].guide,wire).steps[0].status,'completed');
 section.items.pop();section.items[0].bunker=true;
 assert.notEqual(runtime.evaluate(cases[0].guide,wire).steps[0].status,'completed');
});
test('editing a fortification goal clears only generated criteria and old border scope',()=>{
 const g=cases[0].guide,original=JSON.stringify(g);
 const patch={state:{kind:'state',id:'1',name:'Region'},building:{kind:'building',id:'bunker',name:'Land Fort'},
  quantity:3,quantity_mode:'target_total',province_id:10,instruction:'Level 3 in province 10'};
 const edited=model.editConstruction(g,'fort',patch).tracks.operations[0];
 assert.equal(edited.completion,undefined);assert.equal(edited.started_when,undefined);
 assert.equal(edited.action.params.province_selector,undefined);assert.equal(edited.action.params.province_id,10);
 assert.equal(JSON.stringify(g),original);
 const custom=JSON.parse(original);custom.tracks.operations[0].completion={op:'fact',key:'custom.goal',test:'eq',value:true};
 assert.throws(()=>model.editConstruction(custom,'fort',patch));
});
test('border construction instructions identify a per-province level',()=>{
 const plan=require('../src/hoi4companion/web/action-plan.js');
 const row=cases[0].guide.tracks.operations[0];
 const description=plan.describe(row);
 assert.match(description.instructions.join(' '),/level 1 in each province/);
 assert.match(description.instructions.join(' '),/bordering YUG/);
 const view=require('../src/hoi4companion/web/guide-presentation.js');
 const result=view.evaluate(cases[0].guide,cases[0].state);
 assert.equal(result.operations[0].fortifications.total_provinces,2);
});
test('additional fort levels use the same run-relative baseline as other buildings',()=>{
 const g=JSON.parse(JSON.stringify(cases[0].guide)),r=g.tracks.operations[0];
 r.action.params={category:'construction',building_id:'bunker',province_id:10,quantity:2,quantity_mode:'additional'};
 r.completion={op:'fact',key:'guide.construction.fort.added',test:'gte',value:2};
 r.started_when={op:'fact',key:'construction.queued',test:'contains',value:'province:10:bunker'};
 const state=JSON.parse(JSON.stringify(cases[0].state));state.session_id='session';state.facts['country.fortifications.items.10.bunker'].value=5;
 const progress={schema_version:1,guide_id:g.id,run_id:'r',session_id:'session',checks:{},baselines:{fort:{fact_key:'country.fortifications.items.10.bunker',value:3,country:'HUN',quantity:2,source:'fixture'}}};
 const row=runtime.evaluate(g,state,progress).steps[0];assert.equal(row.status,'completed');assert.equal(row.fortifications.target_level,5);
 progress.baselines.fort.country='OTHER';assert.notEqual(runtime.evaluate(g,state,progress).steps[0].status,'completed');
});
