'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');

const activeGuideKey='hoi4-active-guide';
const storage=values=>({getItem:key=>values.get(key)||null,setItem:(key,value)=>values.set(key,value)});
async function openGuidePage({persistent=new Map(),session=new Map(),search='',blocked=false,missing=new Set()}={}){
 const requests=[],window={},catalog=['first','second'].map(id=>({id,title:id}));
 const context={window,URL,URLSearchParams,localStorage:blocked?{getItem(){throw Error('blocked');},setItem(){throw Error('blocked');}}:storage(persistent),sessionStorage:storage(session),location:{href:'http://localhost/walkthrough'+search,search},HoiGuidePresentation:{evaluate:()=>({})},fetch:async url=>{
  requests.push(url);
  if(url==='/api/guides')return {ok:true,json:async()=>({guides:catalog})};
  const id=new URL(url,'http://localhost').searchParams.get('id')||'first';
  if(missing.has(id))return {ok:false,status:404,json:async()=>({error:'Missing guide'})};
  return {ok:true,json:async()=>url.startsWith('/api/guide/progress')?{guide_id:id,checks:{},revision:0}:{id,tracks:{operations:[]}}};
 }};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
 return {api:window.hoiGuide,requests,persistent,session};
}

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
 const window={};const context={window,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{href:'http://localhost/walkthrough',search:''},HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js'),fetch:async(url,options)=>{
  if(options){if(reject)return {ok:false,status:400,json:async()=>({error:'Changed run'})};const body=JSON.parse(options.body);saved={...saved,...body,revision:saved.revision+1};}
  return {ok:true,json:async()=>url==='/api/guides'?{guides:[definition]}:url.startsWith('/api/guide/progress')?saved:definition};
 }};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
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
 const context={window,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{href:'http://localhost/walkthrough',search:''},HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js'),fetch:async url=>{
  requests.push(url);return {ok:true,json:async()=>url==='/api/guides'?{guides:[definition]}:url.startsWith('/api/guide/progress')?saved:definition};
 }};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
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
