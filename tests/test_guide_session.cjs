'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');

const activeGuideKey='hoi4-active-guide';
const storage=values=>({getItem:key=>values.get(key)||null,setItem:(key,value)=>values.set(key,value)});
const nativeDefinition=id=>({id,tracks:{operations:[]},runtime_engine:'rust-guide-core',runtime_definition_revision:id==='first'?'a'.repeat(64):'b'.repeat(64)});
const nativeProgress=id=>({schema_version:1,guide_id:id,run_id:'run-'+id,checks:{},revision:0,server_id:'core'});
const nativePacket=id=>({schema_version:1,engine:'rust-guide-core',server_id:'core',guide_id:id,definition_revision:nativeDefinition(id).runtime_definition_revision,
 state:{schema_version:1,usable:false,facts:{},from_shared:id},progress:nativeProgress(id),evaluation:{schema_version:1,guide_id:id,run_id:'run-'+id,steps:[]}});
async function openGuidePage({persistent=new Map(),session=new Map(),search='',blocked=false,missing=new Set(),shared=null,visibility='visible',fetchOverride=null,presentationEvaluate=()=>({})}={}){
 let subscriber,selection=shared,selectionRevision=0;const selections=[];
 const requests=[],window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}},catalog=['first','second'].map(id=>({id,title:id}));
 const context={window,document:{visibilityState:visibility},URL,URLSearchParams,localStorage:blocked?{getItem(){throw Error('blocked');},setItem(){throw Error('blocked');}}:storage(persistent),sessionStorage:storage(session),location:{href:'http://localhost/walkthrough'+search,search},HoiGuidePresentation:{evaluate:presentationEvaluate},fetch:async(url,options)=>{
  if(fetchOverride){const override=await fetchOverride(url,options);if(override)return override;}
  if(url==='/api/guide/active'){
   if(options){selection=JSON.parse(options.body).guide_id;selectionRevision++;selections.push(selection);}
   return {ok:true,json:async()=>selection?{guide_id:selection,revision:selectionRevision}:{}};
  }
  requests.push(url);
  if(url==='/api/guides')return {ok:true,json:async()=>({guides:catalog})};
  const id=new URL(url,'http://localhost').searchParams.get('id')||'first';
  if(missing.has(id))return {ok:false,status:404,json:async()=>({error:'Missing guide'})};
  return {ok:true,json:async()=>url.startsWith('/api/guide/progress')?{guide_id:id,checks:{},revision:0}:{id,tracks:{operations:[]}}};
 }};
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
 return {api:window.hoiGuide,requests,persistent,session,selections,emit:subscriber};
}

test('server active guide wins over old browser preferences and explicit selection updates the shared guide',async()=>{
 const page=await openGuidePage({shared:'second',persistent:new Map([[activeGuideKey,'first']])});
 assert.equal(page.api.value.guide.id,'second');
 await page.api.select('first');assert.equal(page.selections.at(-1),'first');
 const explicit=await openGuidePage({shared:'first',search:'?guide=second'});
 assert.equal(explicit.api.value.guide.id,'second');assert.equal(explicit.selections.at(-1),'second');
});

test('Rust pages consume a coherent shared packet and discard a late packet after selecting another guide',async()=>{
 let release,first=true;
 const page=await openGuidePage({shared:'first',fetchOverride:async(url)=>{
  if(url==='/api/guides'||url==='/api/guide/active')return;
  const id=new URL(url,'http://localhost').searchParams.get('id')||'first';
  if(url.startsWith('/api/guide/projection')){
   if(id==='first'&&first){first=false;return new Promise(resolve=>{release=resolve;});}
   return {ok:true,json:async()=>nativePacket(id)};
  }
  return {ok:true,json:async()=>url.startsWith('/api/guide/progress')?nativeProgress(id):nativeDefinition(id)};
 }});
 assert.equal(typeof release,'function');const selecting=page.api.select('second');await new Promise(setImmediate);
 release({ok:true,json:async()=>nativePacket('first')});await selecting;await new Promise(setImmediate);
 assert.equal(page.api.value.guide.id,'second');assert.equal(page.api.value.state.from_shared,'second');assert.equal(page.api.value.progress.guide_id,'second');
});

test('a malformed shared packet is rejected instead of falling back to client rules',async()=>{
 const page=await openGuidePage({shared:'first',fetchOverride:async(url)=>{
  if(url==='/api/guides'||url==='/api/guide/active')return;
  if(url.startsWith('/api/guide/projection'))return {ok:true,json:async()=>nativePacket('second')};
  return {ok:true,json:async()=>url.startsWith('/api/guide/progress')?nativeProgress('first'):nativeDefinition('first')};
 }});
 await new Promise(setImmediate);assert.match(page.api.value.error,/shared.*evaluation/i);
 assert.notEqual(page.api.value.state?.from_shared,'second');
});

test('creating a new run never publishes the old run evaluation with new progress',async()=>{
 let run='run-first',packets=0;
 const page=await openGuidePage({shared:'first',presentationEvaluate:(guide,state,progress,evaluation)=>{
  if(evaluation)assert.equal(evaluation.run_id,progress.run_id,'mixed run evaluation');
  return {sharedRun:evaluation?.run_id};
 },fetchOverride:async(url,options)=>{
  if(url==='/api/guides'||url==='/api/guide/active')return;
  if(url.startsWith('/api/guide/projection')){packets++;const p=nativePacket('first');p.progress.run_id=run;p.evaluation.run_id=run;return {ok:true,json:async()=>p};}
  if(url.startsWith('/api/guide/progress')){if(options)run='new-run';return {ok:true,json:async()=>({...nativeProgress('first'),run_id:run})};}
  return {ok:true,json:async()=>nativeDefinition('first')};
 }});
 assert.equal(page.api.value.view.sharedRun,'run-first');
 await page.api.newRun();
 assert.equal(page.api.value.progress.run_id,'new-run');assert.equal(page.api.value.view.sharedRun,'new-run');
 assert.equal(packets,2);assert.equal(page.api.value.error,'');
});

test('the server can withhold a saved run while the live campaign is unverified',async()=>{
 const page=await openGuidePage({shared:'first',fetchOverride:async(url)=>{
  if(url==='/api/guides'||url==='/api/guide/active')return;
  if(url.startsWith('/api/guide/projection')){const p=nativePacket('first');p.evaluation.run_id=null;return {ok:true,json:async()=>p};}
  return {ok:true,json:async()=>url.startsWith('/api/guide/progress')?nativeProgress('first'):nativeDefinition('first')};
 }});
 assert.equal(page.api.value.error,'');assert.equal(page.api.value.state.from_shared,'first');
 assert.equal(page.api.value.progress.run_id,'run-first');
});

test('late recovery from a failed write cannot replace progress after selecting another guide',async()=>{
 let rejected=false,release;
 const page=await openGuidePage({shared:'first',fetchOverride:async(url,options)=>{
  if(url.startsWith('/api/guide/progress')&&url.includes('id=first')){
   if(options){rejected=true;return {ok:false,status:409,json:async()=>({error:'Changed run'})};}
   if(rejected)return new Promise(resolve=>{release=resolve;});
  }
 }});
 const writing=page.api.confirm('build',true),failed=assert.rejects(writing,/Changed run/);
 await new Promise(setImmediate);assert.equal(typeof release,'function');
 await page.api.select('second');
 release({ok:true,json:async()=>({guide_id:'first',checks:{old:true},revision:99})});
 await failed;
 assert.equal(page.api.value.guide.id,'second');assert.equal(page.api.value.progress.guide_id,'second');
 assert.equal(page.api.value.progress.checks.old,undefined);
});

test('server selection events update the page without a POST feedback loop and reject old revisions',async()=>{
 const page=await openGuidePage({shared:'first'}),before=page.selections.length;
 page.emit({active_guide_id:'second',active_guide_revision:3,guide_server_id:'server'});await new Promise(setImmediate);
 assert.equal(page.api.value.guide.id,'second');assert.equal(page.selections.length,before);
 page.emit({active_guide_id:'first',active_guide_revision:2,guide_server_id:'server'});await new Promise(setImmediate);
 assert.equal(page.api.value.guide.id,'second');
});

test('a hidden page cannot override the shared guide merely because an older URL is loaded',async()=>{
 const page=await openGuidePage({shared:'first',search:'?guide=second',visibility:'hidden'});
 assert.equal(page.selections.length,0);
});

test('a successful guide selection survives a fresh tab and browser session',async()=>{
 const persistent=new Map(),page=await openGuidePage({persistent});
 await page.api.select('second');assert.equal(persistent.get(activeGuideKey),'second');
 const reopened=await openGuidePage({persistent});
 assert.equal(reopened.api.value.guide.id,'second');
 assert.ok(!reopened.requests.includes('/api/guide'));
});

test('explicit guide links take priority and update the remembered selection',async()=>{
 const persistent=new Map([[activeGuideKey,'second']]);
 const page=await openGuidePage({persistent,search:'?guide=first'});
 assert.equal(page.api.value.guide.id,'first');assert.equal(persistent.get(activeGuideKey),'first');
});

test('the last selection across tabs wins over an older tab-local preference',async()=>{
 const page=await openGuidePage({persistent:new Map([[activeGuideKey,'second']]),session:new Map([[activeGuideKey,'first']])});
 assert.equal(page.api.value.guide.id,'second');
});

test('old session preferences migrate, and unavailable browser storage remains usable',async()=>{
 const migrated=await openGuidePage({session:new Map([[activeGuideKey,'second']])});
 assert.equal(migrated.api.value.guide.id,'second');assert.equal(migrated.persistent.get(activeGuideKey),'second');
 const blocked=await openGuidePage({blocked:true,session:new Map([[activeGuideKey,'second']])});
 assert.equal(blocked.api.value.guide.id,'second');assert.equal(blocked.api.value.error,'');
});

test('deleted remembered guides fall back without requesting a missing guide',async()=>{
 const page=await openGuidePage({persistent:new Map([[activeGuideKey,'deleted']]),missing:new Set(['deleted'])});
 assert.equal(page.api.value.guide.id,'first');assert.equal(page.persistent.get(activeGuideKey),'first');
 assert.ok(!page.requests.some(url=>url.includes('deleted')));
});

test('an explicitly missing guide reports an error without overwriting the previous choice',async()=>{
 const persistent=new Map([[activeGuideKey,'second']]);
 const page=await openGuidePage({persistent,search:'?guide=deleted',missing:new Set(['deleted'])});
 assert.match(page.api.value.error,/Missing guide/);assert.equal(persistent.get(activeGuideKey),'second');
});
test('editing split legacy confirmations preserves sibling activities and rolls back rejected writes',async()=>{
 const definition={id:'g',title:'Generic',tracks:{operations:['build','train'].map(key=>({key,action:{kind:'manual',params:{legacy_key:'old'}}}))},milestones:[{key:'old',title:'Old'}]};
 let saved={schema_version:1,guide_id:'g',run_id:'run',session_id:null,checks:{old:true},revision:0},reject=false;
 const window={};const context={window,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{href:'http://localhost/walkthrough',search:''},HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js'),fetch:async(url,options)=>{if(url==='/api/guide/active')return {ok:true,json:async()=>options?{guide_id:JSON.parse(options.body).guide_id,revision:1}:{}};
  if(options){if(reject)return {ok:false,status:400,json:async()=>({error:'Changed run'})};const body=JSON.parse(options.body);saved={...saved,...body,revision:saved.revision+1};}
  return {ok:true,json:async()=>url==='/api/guides'?{guides:[definition]}:url.startsWith('/api/guide/progress')?saved:definition};
 }};
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
 await window.hoiGuide.confirm('build',false);
 assert.deepEqual(JSON.parse(JSON.stringify(saved.checks)),{old:false,build:false,train:true});
 await window.hoiGuide.start('build',true);
 assert.deepEqual(JSON.parse(JSON.stringify(saved.started)),{build:true});
 assert.equal(saved.checks.build,false);
 assert.equal(window.hoiGuide.value.view.operations.find(r=>r.key==='build').runtime_status,'active');
 reject=true;await assert.rejects(window.hoiGuide.confirm('train',false),/Changed run/);
 assert.equal(window.hoiGuide.value.progress.checks.train,true);
 assert.equal(window.hoiGuide.value.pending,0);assert.match(window.hoiGuide.value.error,/Conferma non salvata/);
 await assert.rejects(window.hoiGuide.start('build',false),/Changed run/);
 assert.equal(window.hoiGuide.value.progress.started.build,true);
});

test('server baseline events reach the shared presentation without a manual start or extra request',async()=>{
 const example=require('./fixtures/construction-progress.json')[0],definition=example.guide;
 let subscriber;const requests=[],window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}};
 const saved={...example.progress,baselines:{},revision:0};
 const context={window,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{href:'http://localhost/walkthrough',search:''},HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js'),fetch:async(url,options)=>{if(url==='/api/guide/active')return {ok:true,json:async()=>options?{guide_id:JSON.parse(options.body).guide_id,revision:1}:{}};
  requests.push(url);return {ok:true,json:async()=>url==='/api/guides'?{guides:[definition]}:url.startsWith('/api/guide/progress')?saved:definition};
 }};
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
 const before=requests.length;
 subscriber({...example.state,guide_revision:1,guide_runs:{build:{schema_version:1,guide_id:'build',run_id:'r',session_id:'session'}},guide_baselines:{build:example.progress.baselines}});
 assert.equal(window.hoiGuide.value.view.operations[0].construction.target,7);
 assert.equal(window.hoiGuide.value.view.operations[0].runtime_status,'ready');
 const queued=structuredClone(example.state);queued.facts['construction.queued'].value=['state:77:arms_factory'];
 subscriber({...queued,guide_revision:1,guide_baselines:{build:example.progress.baselines}});
 assert.equal(window.hoiGuide.value.view.operations[0].runtime_status,'active');
 const complete=structuredClone(example.state);complete.facts['country.states.items.77.military_factories'].value=7;
 subscriber({...complete,guide_revision:1,guide_baselines:{build:example.progress.baselines}});
 assert.equal(window.hoiGuide.value.view.operations[0].runtime_status,'completed');
 assert.equal(requests.length,before);
 subscriber({...complete,guide_revision:2,guide_runs:{build:{schema_version:1,guide_id:'build',run_id:'new',session_id:'session'}},guide_baselines:{build:{}}});
 assert.equal(window.hoiGuide.value.view.operations[0].construction.target,null);
 assert.equal(window.hoiGuide.value.view.operations[0].runtime_status,'unverifiable');
});

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}

test('relative bindings refresh the shared guide and are cleared on a new run',async()=>{
 const definition={schema_version:1,id:'relative',title:'Relative',tracks:{research:[{key:'r',action:{kind:'research',entity:{kind:'technology',id:'a'}},relative_research:{query:'Artillery UP',chain:['a','b']}}]}};
 let saved={schema_version:1,guide_id:'relative',run_id:'old',session_id:'s',checks:{},started:{},revision:0},subscriber;
 const window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}};
 const fact=value=>({status:'observed',source:'fixture',value});
 const live={schema_version:1,usable:true,session_id:'s',facts:{country:fact('POL'),'research.completed':fact(['a']),'research.active':fact([]),'research.slots':fact([{technology_id:null}])}};
 const context={window,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{href:'http://localhost/walkthrough',search:''},HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js'),fetch:async(url)=>{
  let value;if(url==='/api/guide/active')value={};else if(url==='/api/guides')value={guides:[definition]};else if(url.startsWith('/api/guide/progress'))value=saved;else {value=structuredClone(definition);value.tracks.research[0].action.entity.id=saved.research_bindings?.r||'unbound-up:r';}
  return {ok:true,json:async()=>value};
 }};
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
 saved={...saved,revision:1,research_bindings:{r:'b'}};
 subscriber({...live,guide_revision:1,guide_runs:{relative:saved}});await new Promise(setImmediate);
 assert.equal(window.hoiGuide.value.guide.tracks.research[0].action.entity.id,'b');
 assert.equal(window.hoiGuide.value.view.model.research.next.target,'b');
 saved={schema_version:1,guide_id:'relative',run_id:'new',session_id:'s',checks:{},started:{},revision:2};
 subscriber({...live,guide_revision:2,guide_runs:{relative:saved}});await new Promise(setImmediate);
 assert.deepEqual(window.hoiGuide.value.progress.research_bindings,undefined);
 assert.notEqual(window.hoiGuide.value.guide.tracks.research[0].action.entity.id,'b');
});

test('campaign war observations arrive through the shared stream and clear on a new run',async()=>{
 const page=await openGuidePage();
 page.emit({guide_revision:1,guide_runs:{first:{run_id:'old',campaign_conditions:{country:'TST',war_seen:true}}}});
 assert.deepEqual(JSON.parse(JSON.stringify(page.api.value.progress.campaign_conditions)),{country:'TST',war_seen:true});
 page.emit({guide_revision:2,guide_runs:{first:{run_id:'new'}}});
 assert.deepEqual(JSON.parse(JSON.stringify(page.api.value.progress.campaign_conditions)),{});
});
