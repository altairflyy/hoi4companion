'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const summary=require('../src/hoi4companion/web/operational-summary.js');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const fact=value=>({status:'observed',value,source:'fixture'});
const state=extra=>({schema_version:1,usable:true,facts:{country:fact('TST'),'focus.tree':fact('tree'),
 'focus.active':fact([]),'focus.completed':fact([]),'research.active':fact([]),'research.completed':fact([]),
 'research.slots':fact([{technology_id:null}]),...extra}});
const definition=steps=>({schema_version:1,id:'summary',applies_to:{country:'TST',tree_id:'tree'},tracks:{operations:steps}});
const operation=(key,params,extra={})=>({key,title:'Istruzione '+key,depends_on:[],action:{kind:'manual',params},...extra});
function project(guide,live,progress=null,options){return summary.project(guide,runtime.evaluate(guide,live,progress),live,options);}

test('requested factories reached with fewer assigned creates attention without undoing runtime completion',()=>{
 const guide=definition([operation('production',{category:'production',equipment_id:'rifle',requested_factories:10,
  production_selector:{equipment_ids:['rifle'],variant_names:[],category:'military'}},
  {completion:{op:'fact',key:'guide.production.production.requested',test:'eq',value:10}})]);
 const live=state({'production.lines':fact([{id:'line',equipment:'rifle',name:'Rifle',category:'military',
  requested_factories:10,assigned_factories:3,active_factories:3,damaged_factories:0}])});
 const evaluated=runtime.evaluate(guide,live),before=JSON.stringify({guide,live,evaluated});
 const result=summary.project(guide,evaluated,live),item=result.items.find(r=>r.step_key==='production');
 assert.equal(item.tone,'attention');assert.match(item.message,/10 richieste.*3 assegnate/);
 assert.equal(item.runtime_status,'completed');assert.equal(evaluated.steps[0].status,'completed');
 assert.equal(item.reason,'requested_exceeds_assigned');assert.match(item.guide_goal,/10/);
 assert.equal(item.evidence.find(e=>e.label==='Fabbriche assegnate').status,'calculated');
 assert.equal(JSON.stringify({guide,live,evaluated}),before);
 assert.deepEqual(summary.project(guide,evaluated,live),result);
});

test('queued construction stays in progress and disappears on cancellation without claiming completion',()=>{
 const guide=definition([operation('build',{category:'construction',building_id:'arms_factory',state_id:64,quantity:2,quantity_mode:'target_total'},
  {started_when:{op:'fact',key:'construction.queued',test:'contains',value:'state:64:arms_factory'},
   completion:{op:'fact',key:'country.states.items.64.military_factories',test:'gte',value:7}})]);
 const live=state({'construction.queued':fact(['state:64:arms_factory']),'country.states.items.64.military_factories':fact(5)});
 let item=project(guide,live).items.find(r=>r.step_key==='build');
 assert.equal(item.tone,'active');assert.match(item.message,/avviat.*completamento.*non/i);
 assert.equal(item.runtime_status,'active');assert.match(item.guide_goal,/7/);
 live.facts['construction.queued']=fact([]);item=project(guide,live).items.find(r=>r.step_key==='build');
 assert.notEqual(item.runtime_status,'completed');assert.notEqual(item.tone,'active');
 live.facts['country.states.items.64.military_factories']=fact(7);
 assert.equal(project(guide,live).items.some(r=>r.step_key==='build'),false);
});

test('research dependency explains the named prerequisite without creating a future research list',()=>{
 const guide=definition([]);guide.tracks.research=[
  {key:'previous',title:'SECRET NAME A',href:'/technologies?folder=industry&tech=a',action:{kind:'research',entity:{kind:'technology',id:'a'}}},
  {key:'next',title:'SECRET NAME B',href:'/technologies?folder=industry&tech=b',depends_on:['previous'],action:{kind:'research',entity:{kind:'technology',id:'b'}}}];
 const live=state({'research.active':fact(['a']),'research.slots':fact([{technology_id:'a'},{technology_id:null}])});
 const result=project(guide,live),item=result.items.find(r=>r.category==='research');
 assert.equal(item.reason,'dependency_not_completed');assert.match(item.message,/SECRET NAME A/);
 assert.equal(JSON.stringify(result).includes('SECRET NAME B'),false);
 assert.match(item.links[0].href,/tech=a/);assert.equal(item.links[0].label,'SECRET NAME A');
 assert.deepEqual(item.blocked_by,['previous']);
});

test('political power is unknown only when explicitly referenced and never assumed from a law cost',()=>{
 const guide=definition([operation('law',{category:'politics',law_id:'example_law'},
  {when:{op:'fact',key:'country.politics.political_power',test:'gte',value:150}})]);
 const live=state(),item=project(guide,live).items.find(r=>r.step_key==='law');
 assert.match(item.title,/Potere politico non verificabile/);assert.equal(item.tone,'unknown');
 assert.equal(item.evidence.find(e=>e.key==='country.politics.political_power').value,null);
 live.facts['country.politics.political_power']=fact(0);
 const zero=project(guide,live).items.find(r=>r.step_key==='law');assert.notEqual(zero.title,item.title);
 assert.equal(zero.evidence.find(e=>e.key==='country.politics.political_power').value,0);
 delete guide.tracks.operations[0].when;
 assert.equal(JSON.stringify(project(guide,state())).includes('150'),false);
});

test('stale states, wrong country and mismatched runtime never expose old current-game metrics',()=>{
 const guide=definition([operation('build',{category:'construction'})]),live=state();
 const evaluation=runtime.evaluate(guide,live);
 for(const [r,s]of [[evaluation,{...live,usable:false}],[{...evaluation,guide_id:'another'},live],
  [runtime.evaluate(guide,state({country:fact('OTH')})),state({country:fact('OTH')})]]){
  const result=summary.project(guide,r,s);assert.deepEqual(result.items,[]);assert.ok(result.notice);
 }
});

test('long plans are bounded, reminders and not-applicable steps do not become alerts',()=>{
 const guide=definition(Array.from({length:30},(_,i)=>operation('step'+i,{category:'army'})));
 guide.tracks.operations.push(operation('method',{category:'army'},{tracking:'reminder'}));
 guide.tracks.operations.push(operation('skip',{category:'army'},{skip_when:{op:'fact',key:'country',test:'eq',value:'TST'}}));
 const result=project(guide,state(),null,{limit:4});
 assert.equal(result.items.length,4);assert.equal(result.hidden_count,26);
 assert.equal(result.items.some(r=>['method','skip'].includes(r.step_key)),false);
});

test('manual reported starts are labelled manual and unknown completion is never described as live success',()=>{
 const guide=definition([operation('manual',{category:'army'})]);
 const progress={schema_version:1,guide_id:guide.id,run_id:'r',checks:{},started:{manual:true}};
 const result=project(guide,state(),progress),item=result.items[0];
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
 const guide=definition([]);guide.tracks.research=[{key:'next',action:{kind:'research',entity:{kind:'technology',id:'b'}}}];
 const result=project(guide,state({'research.active':fact(['a']),'research.slots':fact([{technology_id:'a'}])}));
 assert.match(result.items[0].message,/slot rilevati sono occupati/);
 assert.equal(result.items[0].runtime_status,'ready');
 assert.match(result.items[0].message,/1 su 1/);
});

test('unknown named dependency remains unverifiable and cannot imply a missing completed action',()=>{
 const guide=definition([]);guide.tracks.research=[
  {key:'a',name:'Macchine di prova',action:{kind:'research',entity:{kind:'technology',id:'a'}}},
  {key:'b',depends_on:['a'],action:{kind:'research',entity:{kind:'technology',id:'b'}}}];
 const evaluated=runtime.evaluate(guide,state());
 evaluated.steps[0].status='unverifiable';evaluated.steps[1].reason='dependency_unknown';evaluated.steps[1].blocked_by=['a'];
 evaluated.tracks.research.next='b';evaluated.parallel.research.steps=[];
 const before=JSON.stringify(evaluated),item=summary.project(guide,evaluated,state()).items[0];
 assert.match(item.message,/non verificabile.*Macchine di prova/);assert.equal(JSON.stringify(evaluated),before);
});

test('an externally supplied hostile href cannot turn a summary link into executable content',()=>{
 const guide=definition([]);guide.tracks.research=[{key:'next',href:'javascript:alert(1)',action:{kind:'research',entity:{kind:'technology',id:'b'}}}];
 assert.equal(project(guide,state()).items[0].href,'/technologies');
});
