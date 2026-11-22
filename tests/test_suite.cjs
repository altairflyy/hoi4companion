'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const model=require('../src/hoi4companion/web/suite-model.js');
const campaignStatus=require('../src/hoi4companion/web/campaign-status.js');
function suiteHarness(sidebarButtons=[],emptyActive=false,projectionBody=null,libraryGuides=[]){
 const elements=new Map(),events={},calls=[];let listener;
 const element=id=>{if(!elements.has(id))elements.set(id,{id,dataset:{},value:'',checked:false,hidden:id==='suiteError',src:'about:blank',contentWindow:{dispatchEvent(){},location:{origin:'http://localhost'}},addEventListener(name,fn){events[id+':'+name]=fn;},children:[],replaceChildren(){this.children=[];},append(...c){this.children.push(...c);},setAttribute(){},removeAttribute(){}});return elements.get(id);};
 const document={visibilityState:'visible',focused:true,hasFocus(){return this.focused;},getElementById:element,createElement:tag=>({tag,className:'',textContent:'',dataset:{},children:[],append(...c){this.children.push(...c);},setAttribute(){},addEventListener(){}}),querySelectorAll:selector=>selector==='.suite-sidebar [data-view]'?sidebarButtons:[],addEventListener(name,fn){events[name]=fn;}};
 const window={hoiLive:{subscribe(fn){listener=fn;}},addEventListener(name,fn){events[name]=fn;}};
 const location={origin:'http://localhost',hash:'#overview'};
 const fetch=async path=>{calls.push(path);return {ok:!(emptyActive&&path==='/api/guide/active'),json:async()=>path==='/api/guides'?{guides:libraryGuides}:path==='/api/guide/active'?(emptyActive?{error:'No guide available'}:{guide_id:'a'}):(projectionBody||{schema_version:1,engine:'rust-guide-core',guide_id:'a',guide:{tracks:{}},evaluation:{guide_id:'a'},progress:{guide_id:'a'},state:{usable:false}})};};
 const source=fs.readFileSync(require.resolve('../src/hoi4companion/web/suite.js'),'utf8').replace(/\}\)\(\);\s*$/, 'window.__suiteSettled=()=>loading;})();');
 vm.runInNewContext(source,{window,document,location,fetch,HoiSuiteModel:model,HoiCampaignStatus:campaignStatus,HoiI18n:{t:(s,...args)=>s.replace(/\{(\d+)\}/g,(_,i)=>args[i]),language:'english'},HoiOperationalSummary:{project:()=>({items:[]})},Intl,URL,Event});
 return {document,location,events,calls,element,settled:()=>window.__suiteSettled(),send:value=>listener(value),flush:()=>new Promise(resolve=>setImmediate(resolve))};
}
test('an empty guide collection does not show an error',async()=>{
 const h=suiteHarness([],true);await h.settled();
 assert.equal(h.element('suiteError').hidden,true);
 assert.equal(h.element('activeGuideTitle').textContent,'Choose a guide to begin');
});
test('home has one primary action with a destination for each guide state',async()=>{
 const html=fs.readFileSync(require.resolve('../src/hoi4companion/web/suite.html'),'utf8');
 const overview=html.slice(html.indexOf('id="overviewView"'),html.indexOf('id="libraryView"'));
 assert.equal((overview.match(/class="primary"/g)||[]).length,1);
 assert.ok(overview.indexOf('id="nowRoute"')<overview.indexOf('class="campaign-metrics"'));
 const states=[
  [false,[], 'Continue the plan','walkthrough'],
  [true,[{id:'a'}], 'Choose a guide','library'],
  [true,[], 'Import a ZIP','importer']
 ];
 for(const [noActive,library,label,destination] of states){
  const h=suiteHarness([],noActive,null,library);await h.settled();await h.flush();
  const button=h.element('overviewAction');
  assert.equal(button.hidden,false);
  assert.equal(button.textContent,label);
  assert.equal(button.dataset.view,destination);
  assert.equal(h.element('suiteError').hidden,true);
 }
});
test('sidebar marks settings and diagnostics as current when navigating',()=>{
 const button=view=>({dataset:{view},current:false,setAttribute(name,value){if(name==='aria-current')this.current=value;},removeAttribute(name){if(name==='aria-current')this.current=false;}});
 const buttons=['overview','settings','analysis'].map(button),h=suiteHarness(buttons);
 assert.equal(buttons[0].current,'page');
 h.location.hash='#settings';h.events.hashchange();assert.deepEqual(buttons.map(b=>b.current),[false,'page',false]);
 h.location.hash='#analysis';h.events.hashchange();assert.deepEqual(buttons.map(b=>b.current),[false,false,'page']);
});
test('embedded collection links retain suite context and keep their own destinations',()=>{
 const h=suiteHarness(),frame=h.element('workspaceFrame');
 const links={addEventListener(name,fn){this[name]=fn;}};
 frame.contentDocument={documentElement:{classList:{add(){}}},head:{append(){}},createElement:h.document.createElement,
  querySelector:selector=>selector==='.collection-links'?links:null,getElementById:()=>null};
 const go=(key,search,title)=>{
  h.location.hash='#'+key;h.events.hashchange();
  const url=new URL(frame.src);assert.equal(url.pathname,'/importer');assert.equal(url.search,search);assert.equal(url.hash,'');
  assert.equal(h.element('viewTitle').textContent,title);h.events['workspaceFrame:load']();
  assert.equal(h.element('frameStatus').textContent,'');
 };
 const follow=(key,modifiers={})=>{
  let prevented=false;
  links.click({target:{closest:()=>({dataset:{suiteView:key}})},button:0,preventDefault(){prevented=true;},...modifiers});
  return prevented;
 };
 go('templates','?view=references&suite=1','Templates and references');
 assert.equal(follow('importer'),true);assert.equal(h.location.hash,'importer');
 go('importer','?suite=1','Import ZIPs');
 assert.equal(follow('templates',{ctrlKey:true}),false);assert.equal(h.location.hash,'#importer');
 assert.equal(follow('templates'),true);assert.equal(h.location.hash,'templates');
 go('templates','?view=references&suite=1','Templates and references');
 assert.equal(follow('saved-guides'),true);assert.equal(h.location.hash,'saved-guides');
 go('saved-guides','?view=library&suite=1','Saved guides');
});
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
test('library groups guides by nation, flags duplicates by definition hash and keeps non-national entries apart (UX R15)',()=>{
 const guides=[
  {id:'h2',title:'HUNGARY Broken EU.docx',applies_to:{country:'HUN'},definition_hash:'x'},
  {id:'h1',title:'HUNGARY Broken EU.docx',applies_to:{country:'HUN'},definition_hash:'y'},
  {id:'r1',title:'ROMANIA Eternal.docx',applies_to:{country:'ROM'},definition_hash:'r'},
  {id:'r0',title:'ROMANIA Eternal copy.docx',applies_to:{country:'ROM'},definition_hash:'r'},
  {id:'v',title:'VIDEO LINK.docx',applies_to:{},definition_hash:'v'},
  {id:'n',title:'No hash.docx',applies_to:{country:'AUS'}},{id:'n2',title:'No hash 2.docx',applies_to:{country:'AUS'}}];
 const groups=model.libraryGroups(guides,'ROM',true);
 assert.deepEqual(groups.nations.map(g=>g.country),['ROM','AUS','HUN'],'observed nation first, then by tag');
 assert.deepEqual(groups.other.map(r=>r.guide.id),['v']);
 const rom=groups.nations[0].guides;
 assert.equal(rom.find(r=>r.guide.id==='r0').duplicateOf.id,'r1','the later guide in title order is the duplicate');
 assert.equal(rom.find(r=>r.guide.id==='r1').duplicateOf,null);
 assert.equal(rom.some(r=>r.sameTitle),false,'a duplicate is not also a different version');
 const hun=groups.nations[2].guides;assert.equal(hun.length,2,'nothing is hidden');
 assert.ok(hun.every(r=>r.sameTitle&&!r.duplicateOf),'same title with different content is a version, not a duplicate');
 assert.ok(groups.nations[1].guides.every(r=>!r.duplicateOf&&!r.sameTitle),'a missing hash never makes a duplicate');
 assert.equal(model.libraryGroups(guides,'ROM',false).nations.map(g=>g.country).join(),'ROM','country scope still applies');
 assert.equal(model.libraryGroups(guides,'ROM',false).other.length,1,'non-national entries stay listed');
 assert.equal(model.libraryGroups(guides,null,true,'eternal copy').nations[0].guides[0].duplicateOf?.id,'r1','flags are computed on the whole library, not the filtered view');
});
test('library view draws nation sections, a duplicate note and an Other documents section',async()=>{
 const guides=[{id:'r0',title:'ROMANIA A.docx',applies_to:{country:'ROM'},definition_hash:'r'},{id:'r1',title:'ROMANIA B.docx',applies_to:{country:'ROM'},definition_hash:'r'},{id:'v',title:'VIDEO LINK.docx',definition_hash:'v'}];
 const h=suiteHarness([],false,null,guides);h.location.hash='#library';h.events.hashchange();await h.flush();await h.flush();
 const sections=h.element('guideCards').children;assert.equal(sections.length,2);
 const text=node=>[node.textContent,...(node.children||[]).map(text)].join(' ');
 assert.match(text(sections[0].children[0]),/ROM/);assert.match(text(sections[1].children[0]),/Other documents/);
 const cards=sections[0].children[1].children;assert.equal(cards.length,2);
 assert.equal(cards[1].dataset.duplicate,'true');assert.match(text(cards[1]),/Duplicate: same content as/);
 assert.equal(cards[0].dataset.duplicate,'false');
 assert.match(text(sections[1].children[1].children[0]),/Not a national guide/);
});
test('suite rejects prior guide/campaign/session projections before displaying operational guidance',()=>{
 const packet={usable:true,active_guide_id:'a',guide_runs:{a:{run_id:'r',country:'AAA',campaign_id:'campaign'}},sample:{country:'AAA',session_id:'session',campaign_identity:{status:'observed',id:'campaign'}}};
 const projection={engine:'rust-guide-core',guide_id:'a',state:{usable:true,session_id:'session'},progress:{run_id:'r',campaign_id:'campaign'}};
 assert.equal(model.currentProjection(packet,projection),true);
 for(const change of [p=>p.guide_id='b',p=>p.progress.campaign_id='old',p=>p.progress.run_id='old',p=>p.state.session_id='old',p=>p.state.usable=false]){const copy=structuredClone(projection);change(copy);assert.equal(model.currentProjection(packet,copy),false);}
 packet.sample.campaign_identity.status='unknown';assert.equal(model.currentProjection(packet,projection),false);
});
test('suite presents the backend campaign codes in the home and HUD panel',async()=>{
 const sample={country:'POL',session_id:'session',focus_tree_id:'tree',campaign_identity:{status:'observed',id:'campaign'}};
 const run={run_id:'r',country:'POL',campaign_id:'campaign',guide_id:'a'};
 const live={usable:true,sample,active_guide_id:'a',guide_runs:{a:run},guide_server_id:'server'};
 const liveProjection={schema_version:1,engine:'rust-guide-core',server_id:'server',guide_id:'a',guide:{tracks:{}},
  state:{usable:true,sample},progress:run,evaluation:{guide_id:'a',mode:'live',run_id:'r'}};
 for(const [packet,campaign,message,destination,label] of [
  [null,null,/Checking the game/,'walkthrough','Continue the plan'],
  [{usable:false,reader_status:'game_absent'},null,/HOI4 is not running/,'walkthrough','Continue the plan'],
  [{usable:false,reader_status:'build_unsupported'},null,/not supported for reading/,'walkthrough','Continue the plan'],
  [{usable:false,reader_status:'campaign_unverified'},null,/Wait for the campaign/,'walkthrough','Continue the plan'],
  [{usable:false,status:'stale',sample},null,/Checking the game/,'walkthrough','Continue the plan'],
  [live,{code:'campaign_unverified'},/Wait for the campaign/,'walkthrough','Continue the plan'],
  [live,{code:'wrong_country',country:'POL',guide_country:'GER'},/POL, guide for GER/,'library','Choose another guide'],
  [live,{code:'wrong_tree'},/different focus tree/,'library','Choose another guide'],
  [live,{code:'wrong_campaign'},/does not match the saved checklist/,'library','Choose another guide'],
  [live,{code:'linked',country:'POL',date:19360101},/Game linked: POL, 1936-01-01/,'walkthrough','Continue the plan'],
 ]){
  const h=suiteHarness([],false,packet?.usable?{...liveProjection,campaign_status:campaign}:previewProjection());
  if(packet)h.send({...packet,overlay:{message:'HUD visible'}});
  await h.settled();await h.flush();
  assert.match(h.element('gameStatus').textContent,message);
  assert.equal(h.element('hudStatus').textContent,h.element('gameStatus').textContent);
  assert.equal(h.element('overviewAction').dataset.view,destination);
  assert.equal(h.element('overviewAction').textContent,label);
 }
 const h=suiteHarness([],false,{...liveProjection,campaign_status:{code:'linked',country:'POL'}});
 h.send(live);await h.settled();await h.flush();
 assert.match(h.element('hudStatus').textContent,/Game linked: POL/);
 h.send({...live,sample:{...sample,session_id:'new-session'}});
 assert.match(h.element('hudStatus').textContent,/Checking the game/);
 await h.flush();
 assert.match(h.element('hudStatus').textContent,/Checking the game/);
 h.send({...live,usable:false,status:'stale'});
 assert.match(h.element('hudStatus').textContent,/Checking the game/);
 assert.equal(model.matchingLiveProjection({...live,sample:{...sample,focus_tree_id:'new'}},liveProjection),false);
 assert.equal(model.matchingLiveProjection({...live,guide_server_id:'restarted'},liveProjection),false);
});
test('HUD panel explains verified highlights and shows the step card shortcut in both languages',()=>{
 const html=fs.readFileSync(require.resolve('../src/hoi4companion/web/suite.html'),'utf8');
 const panel=html.match(/<section class="session-panel">([\s\S]*?)<\/section>/)[1];
 const {messages}=require('../src/hoi4companion/translations.json');
 const highlights='The HUD highlights verified guide targets in a linked game.';
 const shortcut='During the game, press Ctrl+Alt+H to show or hide the step card.';
 for(const key of [highlights,shortcut]){
  assert.ok(panel.includes(`data-copy="${key}">${key}<`));
  assert.ok(messages[key]);
 }
 assert.match(messages[shortcut],/Ctrl\+Alt\+H/);
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
// UX R11: the home previews the first steps from the Rust preview evaluation only.
const cases=require('./fixtures/presentation-cases.json'),evaluations=require('./fixtures/presentation-evaluations.json');
function previewProjection(id='a'){const guide=structuredClone(cases.find(c=>c.name==='model: offline plan').guide);guide.id=id;
 const evaluation={...structuredClone(evaluations['model: offline plan']),guide_id:id};
 return {schema_version:1,engine:'rust-guide-core',guide_id:id,guide,evaluation,progress:{guide_id:id,run_id:evaluation.run_id},state:{usable:false}};}
const keys=groups=>groups.map(g=>[g.track,g.items.map(i=>i.key+':'+i.status)]);
test('home preview lists the first open steps of each track from the Rust preview evaluation',()=>{
 const p=previewProjection();
 assert.deepEqual(keys(model.preview(p,'a')),[['focus',['f1:unverifiable','wait:unverifiable','f2:unverifiable']],['research',['r1:unverifiable','discount:unverifiable','project:unverifiable']]]);
 // Rust statuses decide what is left out; nothing is recomputed in JS.
 p.evaluation.steps.find(r=>r.key==='f1').status='completed';p.evaluation.steps.find(r=>r.key==='wait').status='not_applicable';p.evaluation.steps.find(r=>r.key==='r1').status='ready';
 assert.deepEqual(keys(model.preview(p,'a'))[0][1],['f2:unverifiable','filler:unverifiable','unity:unverifiable']);
 assert.equal(model.preview(p,'a')[1].items[0].status,'ready');
 p.evaluation.tracks.focus.next=null;assert.deepEqual(model.preview(p,'a').map(g=>g.track),['research']);
});
test('home preview rejects live, foreign or non-Rust evaluations',()=>{
 for(const change of [p=>p.evaluation.mode='live',p=>p.evaluation.mode='wrong_country',p=>p.engine='js',p=>p.guide_id='b',p=>p.evaluation.guide_id='b',p=>delete p.evaluation.steps]){const p=previewProjection();change(p);assert.equal(model.preview(p,'a'),null);}
 assert.equal(model.preview(previewProjection(),''),null);assert.equal(model.preview(null,'a'),null);
});
test('Now displays only the steps nominated by Rust for the matching run',()=>{
 const p=previewProjection();
 assert.deepEqual(model.nextSteps(p,'a','preview').map(i=>i.key),['f1','r1']);
 p.evaluation.tracks.focus.next=null;
 p.evaluation.tracks.research.next='r4';
 assert.deepEqual(model.nextSteps(p,'a','preview').map(i=>i.key),['r4']);
 p.evaluation.steps.find(r=>r.key==='r4').status='completed';
 assert.deepEqual(model.nextSteps(p,'a','preview'),[]);
 for(const change of [p=>p.progress.run_id='previous',p=>p.guide.id='other',p=>p.evaluation.mode='live']){
  const invalid=previewProjection();change(invalid);
  assert.deepEqual(model.nextSteps(invalid,'a','preview'),[]);
 }
});
test('suite home without the game shows the Rust preview and drops it once the game is readable',async()=>{
 const h=suiteHarness([],false,previewProjection());await h.settled();await h.flush();
 assert.ok(h.calls.some(p=>p.startsWith('/api/guide/projection?id=a&live=0')));assert.ok(!h.calls.some(p=>p.includes('live=1')));
 assert.equal(h.element('routeStatus').textContent,'Preview without the game');
 const items=h.element('nowItems').children;assert.equal(items.length,2);
 const text=item=>item.children[1].children.map(c=>c.textContent);
 assert.deepEqual(text(items[0]).slice(0,2),['National focuses · To check in game','f1']);assert.equal(items[1].href,'/technologies');
 assert.equal(h.element('routeItems').children.length,4);
 assert.equal(h.element('nowNotice').textContent,'Preview: no progress is counted until the game is linked.');
 h.send({usable:true,active_guide_id:'a',sample:{country:'POL'}});await h.settled();await h.flush();
 assert.ok(h.calls.some(p=>p.includes('live=1')));
 assert.equal(h.element('routeStatus').textContent,'Waiting for current game data');assert.equal(h.element('routeItems').children.length,0);
 assert.equal(h.element('nowRoute').hidden,true);assert.equal(h.element('nowItems').children.length,0);
});
