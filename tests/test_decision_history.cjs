'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
test('decision history: counter contracts reject zero and malformed identities',()=>{
 for(const [key,value,valid]of [['decisions.history.executions_lower_bound:test:country:GER',2,true],['decisions.history.executions_lower_bound:test:country:GER',0,false],['decisions.history.executions_lower_bound:test:country:bad',2,false],['decisions.targeted.history.executed',['test:country:GER'],true],['decisions.targeted.history.executed',['test'],false]]){
  const state=contracts.state({schema_version:1,usable:true,session_id:'s',facts:{[key]:{status:'calculated',source:'campaign_decision_observations',value}}});assert.equal(state.facts[key].status!=='unknown',valid,key);
 }
});
