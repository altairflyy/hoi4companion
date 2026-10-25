'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const model=require('../src/hoi4companion/web/importer-model.js');
test('province queue membership requires an observed province and never infers completion',()=>{
 const line={id:'x',building_id:'bunker',location_status:'observed',state_id:64,province_id:6521,
  amount:1,active_factories:0,built_level:0,repair_levels:0,conversion:false};
 const value={usable:true,sample:{country:'GER',country_overview:{country:'GER',construction:{status:'observed',lines:[line]}}}};
 assert.deepEqual(contracts.state(value).facts['construction.queued'].value,['building:bunker','province:6521:bunker','state:64:bunker']);
 line.province_id=null;
 assert.deepEqual(contracts.state(value).facts['construction.queued'].value,['building:bunker','state:64:bunker']);
 line.location_status='unknown';
 assert.equal(contracts.state(value).facts['construction.queued'].status,'unknown');
});
test('construction correction rebuilds province bindings and clears obsolete province targets',()=>{
 const guide={schema_version:1,id:'geo',tracks:{operations:[{key:'fort',action:{kind:'manual',params:{category:'construction',
  building_id:'bunker',state_id:64,province_id:6521,quantity:1,quantity_mode:'additional'}},
  started_when:{op:'fact',key:'construction.queued',test:'contains',value:'province:6521:bunker'}}]}};
 const patch={state:{kind:'state',id:'77',name:'Other'},building:{kind:'building',id:'bunker',name:'Fort'},
  quantity:1,quantity_mode:'additional',instruction:'Build fort',province_id:123};
 const before=JSON.stringify(guide),row=model.editConstruction(guide,'fort',patch).tracks.operations[0];
 assert.equal(row.action.params.province_id,123);assert.equal(row.started_when,undefined);
 assert.equal(JSON.stringify(guide),before);
 assert.equal(model.editConstruction(guide,'fort',{...patch,province_id:null}).tracks.operations[0].action.params.province_id,undefined);
 for(const province_id of [true,-1,0,1.5,'123'])assert.throws(()=>model.editConstruction(guide,'fort',{...patch,province_id}));
});
