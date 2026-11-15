'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const model=require('../src/hoi4companion/web/importer-model.js');
const fixture=require('./presentation-fixtures.cjs');
const cases=require('./fixtures/fortifications.json');
test('native province controllers stay internal to the fortification adapter',()=>{
 const section={status:'observed',items:[{id:10,bunker:1},{id:11,bunker:1}],borders_status:'observed',borders:{YUG:[10,11]},controllers:{10:'HUN',20:'YUG'}};
 const wire={usable:true,sample:{country:'HUN',country_overview:{country:'HUN',fortifications:section}}};
 assert.equal(contracts.state(wire).facts['country.fortifications.controllers.10'],undefined);
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
 const {guide,state,evaluation}=fixture('construction: border fortification');
 const result=view.evaluate(guide,state,{},evaluation);
 assert.equal(result.operations[0].fortifications.total_provinces,2);
});
