require('../src/hoi4companion/web/i18n.js').setLanguage('italian');
'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const summary=require('../src/hoi4companion/web/operational-summary.js');
const fixture=require('./presentation-fixtures.cjs');
const fact=value=>({status:'observed',value,source:'fixture'});
const state=extra=>({schema_version:1,usable:true,facts:{country:fact('TST'),'focus.tree':fact('tree'),
 'focus.active':fact([]),'focus.completed':fact([]),'research.active':fact([]),'research.completed':fact([]),
 'research.slots':fact([{technology_id:null}]),...extra}});
const definition=steps=>({schema_version:1,id:'summary',applies_to:{country:'TST',tree_id:'tree'},tracks:{operations:steps}});
const operation=(key,params,extra={})=>({key,title:'Istruzione '+key,depends_on:[],action:{kind:'manual',params},...extra});
function project(name,options){const f=fixture(name);return summary.project(f.guide,f.evaluation,f.state,options);}

test('production milestone blockers name the prerequisite instead of a manual completion request',()=>{
 const item=project('summary: milestone blocker').items.find(r=>r.step_key==='allocation');
 assert.equal(item.reason,'milestone_not_completed');assert.match(item.message,/Moderate Lawyer/);
 assert.deepEqual(item.blocked_by,['lawyer']);assert.equal(item.links[0].href,'#operation-lawyer');
});

test('requested factories reached with fewer assigned creates attention without undoing runtime completion',()=>{
 const {guide,state:live,evaluation:evaluated}=fixture('summary: requested exceeds assigned'),before=JSON.stringify({guide,live,evaluated});
 const result=summary.project(guide,evaluated,live),item=result.items.find(r=>r.step_key==='production');
 assert.equal(item.tone,'attention');assert.match(item.message,/10 richieste.*3 assegnate/);
 assert.equal(item.runtime_status,'completed');assert.equal(evaluated.steps[0].status,'completed');
 assert.equal(item.reason,'requested_exceeds_assigned');assert.match(item.guide_goal,/10/);
 assert.equal(item.evidence.find(e=>e.label==='Fabbriche assegnate').status,'calculated');
 assert.equal(JSON.stringify({guide,live,evaluated}),before);
 assert.deepEqual(summary.project(guide,evaluated,live),result);
});

test('queued construction stays in progress and disappears on cancellation without claiming completion',()=>{
 let item=project('summary: construction queued').items.find(r=>r.step_key==='build');
 assert.equal(item.tone,'active');assert.match(item.message,/avviat.*completamento.*non/i);
 assert.equal(item.runtime_status,'active');assert.match(item.guide_goal,/7/);
 item=project('summary: construction cancelled').items.find(r=>r.step_key==='build');
 assert.notEqual(item.runtime_status,'completed');assert.notEqual(item.tone,'active');
 assert.equal(project('summary: construction complete').items.some(r=>r.step_key==='build'),false);
});

test('research dependency explains the named prerequisite without creating a future research list',()=>{
 const result=project('summary: research dependency'),item=result.items.find(r=>r.category==='research');
 assert.equal(item.reason,'dependency_not_completed');assert.match(item.message,/SECRET NAME A/);
 assert.equal(JSON.stringify(result).includes('SECRET NAME B'),false);
 assert.match(item.links[0].href,/tech=a/);assert.equal(item.links[0].label,'SECRET NAME A');
 assert.deepEqual(item.blocked_by,['previous']);
});

test('political power is unknown only when explicitly referenced and never assumed from a law cost',()=>{
 const item=project('summary: political power unobserved').items.find(r=>r.step_key==='law');
 assert.match(item.title,/Potere politico non verificabile/);assert.equal(item.tone,'unknown');
 assert.equal(item.evidence.find(e=>e.key==='country.politics.political_power').value,null);
 const zero=project('summary: political power zero').items.find(r=>r.step_key==='law');assert.notEqual(zero.title,item.title);
 assert.equal(zero.evidence.find(e=>e.key==='country.politics.political_power').value,0);
 assert.equal(JSON.stringify(project('summary: law without political power condition')).includes('150'),false);
});

test('stale states, wrong country and mismatched runtime never expose old current-game metrics',()=>{
 const {guide,state:live,evaluation}=fixture('summary: construction for stale checks'),other=fixture('summary: construction for another country');
 for(const [r,s]of [[evaluation,{...live,usable:false}],[{...evaluation,guide_id:'another'},live],[other.evaluation,other.state]]){
  const result=summary.project(guide,r,s);assert.deepEqual(result.items,[]);assert.ok(result.notice);
 }
});

test('long plans are bounded, reminders and not-applicable steps do not become alerts',()=>{
 const result=project('summary: long plan',{limit:4});
 assert.equal(result.items.length,4);assert.equal(result.hidden_count,26);
 assert.equal(result.items.some(r=>['method','skip'].includes(r.step_key)),false);
});

test('manual reported starts are labelled manual and unknown completion is never described as live success',()=>{
 const item=project('summary: manual start').items[0];
 assert.match(item.message,/segnalat.*manualmente/i);assert.equal(item.runtime_status,'active');
 assert.equal(item.evidence.some(e=>e.status==='observed'),false);
});

test('production layouts expose individual product comparisons without summing different goals',()=>{
 const guide=definition([operation('layout',{category:'production',production_targets:[
  {equipment_name:'Rifles',requested_factories:10},{equipment_name:'Artillery',requested_factories:5} ]})]);
 const evaluated={schema_version:1,guide_id:guide.id,mode:'live',country:'TST',progress_status:'none',parallel:{},tracks:{},steps:[
  {key:'layout',status:'active',reason:'selection_true',blocked_by:[],selection:{value:true,source:'declared_start'},completion:{value:false},
   production:{layout:[{target:10,requested:10,assigned:3,basis:'requested',reason:''},{target:5,requested:2,assigned:2,basis:'requested',reason:''}]}}]};
 const result=summary.project(guide,evaluated,state());
 assert.equal(result.items[0].comparisons.length,2);assert.equal(result.items[0].comparisons[0].target,10);
 assert.equal(result.items[0].comparisons[1].target,5);
});

test('busy native capacity is explained without inventing an unmet condition',()=>{
 const result=project('summary: busy research capacity');
 assert.match(result.items[0].message,/slot rilevati sono occupati/);
 assert.equal(result.items[0].runtime_status,'ready');
 assert.match(result.items[0].message,/1 su 1/);
});

test('unknown named dependency remains unverifiable and cannot imply a missing completed action',()=>{
 const {guide,state,evaluation:evaluated}=fixture('summary: named research dependency');
 evaluated.steps[0].status='unverifiable';evaluated.steps[1].reason='dependency_unknown';evaluated.steps[1].blocked_by=['a'];
 evaluated.tracks.research.next='b';evaluated.parallel.research.steps=[];
 const before=JSON.stringify(evaluated),item=summary.project(guide,evaluated,state).items[0];
 assert.match(item.message,/non verificabile.*Macchine di prova/);assert.equal(JSON.stringify(evaluated),before);
});

test('an externally supplied hostile href cannot turn a summary link into executable content',()=>{
 assert.equal(project('summary: hostile href').items[0].href,'/technologies');
});
