'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const model=require('../src/hoi4companion/web/suite-model.js');
function suiteHarness(){
 const elements=new Map(),events={},calls=[];let listener;
 const element=id=>{if(!elements.has(id))elements.set(id,{id,dataset:{},value:'',checked:false,hidden:false,src:'about:blank',contentWindow:{dispatchEvent(){}},addEventListener(){},replaceChildren(){},append(){},setAttribute(){},removeAttribute(){}});return elements.get(id);};
 const document={visibilityState:'visible',focused:true,hasFocus(){return this.focused;},getElementById:element,querySelectorAll:()=>[],addEventListener(name,fn){events[name]=fn;}};
 const window={hoiLive:{subscribe(fn){listener=fn;}},addEventListener(name,fn){events[name]=fn;}};
 const location={origin:'http://localhost',hash:'#overview'};
 const fetch=async path=>{calls.push(path);return {ok:true,json:async()=>path==='/api/guides'?{guides:[]}:path==='/api/guide/active'?{guide_id:'a'}:{schema_version:1,engine:'rust-guide-core',guide_id:'a',guide:{tracks:{}},evaluation:{guide_id:'a'},progress:{guide_id:'a'},state:{usable:false}}};};
 vm.runInNewContext(fs.readFileSync(require.resolve('../src/hoi4companion/web/suite.js'),'utf8'),{window,document,location,fetch,HoiSuiteModel:model,HoiI18n:{t:s=>s,language:'english'},Intl,URL,Event});
 return {document,location,events,calls,send:value=>listener(value),flush:()=>new Promise(resolve=>setImmediate(resolve))};
}
test('suite does not compete with HUD for projections in embedded views or while hidden',async()=>{
 const h=suiteHarness();await h.flush();h.calls.length=0;
 const packet={usable:true,active_guide_id:'a',sample:{country:'AAA',native_tick:1}};
 h.location.hash='#walkthrough';h.events.hashchange();h.send(packet);await h.flush();
 assert.equal(h.calls.filter(p=>p.includes('/api/guide/projection')).length,0);
 h.location.hash='#overview';h.document.visibilityState='hidden';h.events.hashchange();h.send({...packet,sample:{...packet.sample,native_tick:2}});await h.flush();
 assert.equal(h.calls.filter(p=>p.includes('/api/guide/projection')).length,0);
});
test('suite refreshes real game changes without reprojecting reader ticks',async()=>{
 const h=suiteHarness();await h.flush();
 const packet={usable:true,active_guide_id:'a',sample:{country:'AAA',native_tick:1,political_power:10}};
 h.send(packet);await h.flush();h.calls.length=0;
 h.send({...packet,sample:{...packet.sample,native_tick:2,metrics:{duration_ms:20}}});await h.flush();
 assert.equal(h.calls.length,0);
 h.send({...packet,sample:{...packet.sample,political_power:11}});await h.flush();
 assert.equal(h.calls.filter(p=>p.includes('/api/guide/projection')).length,1);
 h.location.hash='#walkthrough';h.events.hashchange();h.calls.length=0;
 h.send({...packet,sample:{...packet.sample,political_power:12}});await h.flush();assert.equal(h.calls.length,0);
 h.location.hash='#overview';h.events.hashchange();await h.flush();
 assert.equal(h.calls.filter(p=>p.includes('/api/guide/projection')).length,1);
 h.calls.length=0;h.document.focused=false;h.events.blur();h.send({...packet,sample:{...packet.sample,political_power:13}});await h.flush();assert.equal(h.calls.length,0);
 h.document.focused=true;h.events.focus();await h.flush();assert.equal(h.calls.filter(p=>p.includes('/api/guide/projection')).length,1);
});
test('suite metrics preserve observed zero, signed PP and unknown/stale/foreign sections',()=>{
 const packet={usable:true,sample:{country:'AAA',country_overview:{country:'AAA',divisions:{status:'observed',total:0},politics:{status:'observed',power:{status:'observed',available:-2}},production:{status:'observed',effective_factories:{military:0}}}}};
 assert.deepEqual(model.metrics(packet),{current:true,country:'AAA',divisions:0,politicalPower:-2,factories:0});
 packet.sample.country_overview.politics.power.status='unknown';assert.equal(model.metrics(packet).politicalPower,null);
 packet.sample.country_overview.country='BBB';assert.equal(model.metrics(packet).divisions,null);assert.equal(model.metrics(packet).factories,null);
 packet.usable=false;assert.deepEqual(model.metrics(packet),{current:false,country:null,divisions:null,politicalPower:null,factories:null});
});
test('suite library scopes recommendations without discarding guides or bypassing DLCs',()=>{
 const guides=[{id:'b',title:'Other.docx',applies_to:{country:'BBB'}},{id:'a',title:'Guide_Name.docx',applies_to:{country:'AAA'}},{id:'c',title:'Unavailable',applies_to:{country:'AAA'},compatibility:{blocked:true}}];
 assert.deepEqual(model.library(guides,'AAA',false).map(g=>g.id),['a','c']);
 assert.equal(model.library(guides,'AAA',true).length,3);assert.equal(guides[0].id,'b');
 assert.equal(model.library(guides,'AAA',false,'name')[0].title,'Guide_Name.docx');
 assert.equal(model.selectable(guides[0],'AAA'),false);assert.equal(model.selectable(guides[2],'AAA'),false);
 assert.equal(model.title('Guide_Name.docx'),'Guide Name');
});
test('suite rejects prior guide/campaign/session projections before displaying operational guidance',()=>{
 const packet={usable:true,active_guide_id:'a',guide_runs:{a:{run_id:'r',country:'AAA',campaign_id:'campaign'}},sample:{country:'AAA',session_id:'session',campaign_identity:{status:'observed',id:'campaign'}}};
 const projection={engine:'rust-guide-core',guide_id:'a',state:{usable:true,session_id:'session'},progress:{run_id:'r',campaign_id:'campaign'}};
 assert.equal(model.currentProjection(packet,projection),true);
 for(const change of [p=>p.guide_id='b',p=>p.progress.campaign_id='old',p=>p.progress.run_id='old',p=>p.state.session_id='old',p=>p.state.usable=false]){const copy=structuredClone(projection);change(copy);assert.equal(model.currentProjection(packet,copy),false);}
 packet.sample.campaign_identity.status='unknown';assert.equal(model.currentProjection(packet,projection),false);
});
test('embedded views share the stream and release subscriptions without owning an EventSource',()=>{
 let disposed=0,subscribed=0;const handlers={},source={enabled:true,value:{usable:false},sample:()=>null,subscribe(fn){subscribed++;fn(this.value);return ()=>disposed++;}};
 const window={location:{origin:'http://localhost'},parent:{location:{origin:'http://localhost'},HoiSuiteLive:source},addEventListener(name,fn){handlers[name]=fn;}};
 vm.runInNewContext(fs.readFileSync(require.resolve('../src/hoi4companion/web/live-state.js'),'utf8'),{window,document:{},EventSource(){throw Error('A child must not acquire its own stream')}});
 let value;window.hoiLive.subscribe(v=>value=v);assert.equal(value,source.value);assert.equal(window.hoiLive.enabled,true);handlers.pagehide();assert.equal(disposed,1);
 handlers.pageshow({persisted:true});assert.equal(subscribed,2);
 window.parent.HoiSuiteFrameActive=false;assert.equal(window.hoiLive.enabled,false);assert.equal(window.hoiLive.value,null);
 window.parent.HoiSuiteFrameActive=true;handlers.suiteviewchange();assert.equal(value,source.value);
});
