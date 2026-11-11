'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
const runtime=require('../src/hoi4companion/web/guide-runtime.js'),contracts=require('../src/hoi4companion/web/contracts.js');
const fixture=JSON.parse(fs.readFileSync('tests/fixtures/decision-history.json','utf8'));
for(const row of fixture.cases)test('decision history: '+row.name,()=>{
 const {guide,state,progress}=structuredClone(fixture),h=progress.decision_history;
 if(row.executions!==undefined)h.entries['test:country:GER'].executions_lower_bound=row.executions;
 if(row.tick!==undefined)state.facts['campaign.tick'].value=row.tick;
 if(row.missing_tick)delete state.facts['campaign.tick'];
 if(row.tick_source)state.facts['campaign.tick'].source=row.tick_source;
 if(row.history_country)h.country=row.history_country;if(row.history_session)h.session_id=row.history_session;
 if(row.empty)h.entries={};if(row.guide_id)guide.id=progress.guide_id=row.guide_id;
 const result=runtime.evaluate(guide,state,progress);assert.equal(result.steps[0].status==='completed',row.completed);
 if(row.empty||row.missing_tick||row.tick_source||row.tick!==undefined)assert.equal(result.steps[0].completion.value,null);
});
test('decision history: counter contracts reject zero and malformed identities',()=>{
 for(const [key,value,valid]of [['decisions.history.executions_lower_bound:test:country:GER',2,true],['decisions.history.executions_lower_bound:test:country:GER',0,false],['decisions.history.executions_lower_bound:test:country:bad',2,false],['decisions.targeted.history.executed',['test:country:GER'],true],['decisions.targeted.history.executed',['test'],false]]){
  const state=contracts.state({schema_version:1,usable:true,session_id:'s',facts:{[key]:{status:'calculated',source:'campaign_decision_observations',value}}});assert.equal(state.facts[key].status!=='unknown',valid,key);
 }
});

for(const row of fixture.invalid_histories)test('decision history rejects '+row.name,()=>{
 const {guide,state,progress}=structuredClone(fixture),entry=Object.values(progress.decision_history.entries)[0];
 Object.assign(entry,{id:row.id,target:row.target});progress.decision_history.entries={[row.token]:entry};
 assert.throws(()=>runtime.evaluate(guide,state,progress),/Storico decisioni campagna non valido/);
});
