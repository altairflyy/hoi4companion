'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');

const activeGuideKey='hoi4-active-guide';
const storage=values=>({getItem:key=>values.get(key)||null,setItem:(key,value)=>values.set(key,value)});
async function openGuidePage({persistent=new Map(),session=new Map(),search='',blocked=false,missing=new Set(),shared=null,visibility='visible'}={}){
 let subscriber,selection=shared,selectionRevision=0;const selections=[];
 const requests=[],window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}},catalog=['first','second'].map(id=>({id,title:id}));
 const context={window,document:{visibilityState:visibility},URL,URLSearchParams,localStorage:blocked?{getItem(){throw Error('blocked');},setItem(){throw Error('blocked');}}:storage(persistent),sessionStorage:storage(session),location:{href:'http://localhost/walkthrough'+search,search},HoiGuidePresentation:{evaluate:()=>({})},fetch:async(url,options)=>{
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
