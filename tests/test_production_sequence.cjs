'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
const summary=require('../src/hoi4companion/web/operational-summary.js');
const fixture=require('./presentation-fixtures.cjs');
const instruction=(key,n,order,when)=>({key,title:key,when,action:{kind:'manual',params:{category:'production',equipment_id:'sup',requested_factories:n,production_revision:{group:'setup',order},production_selector:{equipment_ids:['sup'],variant_names:[],category:'military'}}},completion:{op:'fact',key:'guide.production.'+key+'.requested',test:'eq',value:n}});
function definitions(){const rows=[instruction('initial',1,0),instruction('current',7,1,{op:'fact',key:'focus.completed',test:'contains',value:'phase'}),instruction('future',30,2,{op:'fact',key:'unknown.condition',test:'eq',value:true})];delete rows[0].when;return {schema_version:1,id:'phases',title:'Phased guide',dependency_policy:'explicit',applies_to:{country:'TST'},tracks:{operations:rows}};}

test('a superseded layout leaves the summary while a phase still waiting for its adoption stays',()=>{
 const {guide,state,evaluation}=fixture('production: phase waiting for adoption'),items=summary.project(guide,evaluation,state).items;
 assert(!items.some(x=>x.step_key==='initial'));assert(items.some(x=>x.step_key==='current'));
});
test('the summary shows only the current production phase and never mutates its inputs',()=>{
 const {guide:g,state:s,evaluation:r}=fixture('production: current phase'),before=JSON.stringify({g,s,r});
 const projected=summary.project(g,r,s);assert.deepEqual(projected.items.map(x=>x.step_key),['current']);
 assert.equal(JSON.stringify({g,s,r}),before);
});
test('revision validation rejects malformed values',()=>{
 for(const revision of [{group:'x',order:true},{group:'',order:0},{group:'x',order:-1},{group:'x',order:1,extra:true}]){
  const bad=definitions();bad.tracks.operations[0].action.params.production_revision=revision;assert.throws(()=>contracts.guide(bad));
 }
});
