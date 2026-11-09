'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
test('walkthrough renders only operational instructions, preserves conditions, runs and undo',async()=>{
 class Node{
  constructor(){this.children=[];this.dataset={};this.hidden=false;this.text='';this.style={};}
  set textContent(value){this.text=String(value);this.children=[];}
  get textContent(){return this.text+this.children.map(x=>x.textContent||'').join('');}
  append(...nodes){this.children.push(...nodes);}
  replaceChildren(...nodes){this.text='';this.children=nodes;}
  setAttribute(key,value){this[key]=value;}getAttribute(key){return this[key];}addEventListener(type,fn){this['on'+type]=fn;}
  pause(){this.paused=true;}load(){}removeAttribute(key){delete this[key];}showModal(){this.open=true;}close(){this.open=false;if(this.onclose)this.onclose();}
 }
 const nodes=new Map(),get=id=>{if(!nodes.has(id))nodes.set(id,new Node());return nodes.get(id);};get('hideDone').checked=true;
 const guide={schema_version:1,id:'two',title:'Generic',phases:[{id:'phase',title:'Phase',when:{op:'check',key:'phase_open'}}],tracks:{focus:[{key:'f',kind:'focus',target:'F',name:'NATIVE FOCUS'}],research:[{key:'r',kind:'technology',target:'R',name:'NATIVE TECH'}],operations:[{key:'build',kind:'gate',name:'Construction',phase:'phase',completion:{op:'check',key:'done'},depends_on:[],action:{kind:'manual',params:{category:'construction',timing:'1936',instructions:['Build first']}}}]}};
 guide.sources=[{id:'doc',type:'file',title:'Original.docx',text:'Original notes <script>not executable</script>\n# Initial Setup\nPrepare the starting army.'}];
 guide.tracks.operations[0].evidence=[{source_id:'doc',basis:'explicit',quote:'See Initial Setup'}];
 guide.tracks.operations[0].action.params.reference_images=[{id:'a'.repeat(64),name:'OFF'}];
 guide.tracks.operations[0].action.params.design_reference={name:'T-I1',image_id:'b'.repeat(64),modules:{gun:'cannon'},upgrades:{engine:0}};
 let subscriber;const requests=[];let mediaReport={references:[]},alternateGuide=null;let run={schema_version:1,guide_id:'two',run_id:'first',session_id:null,checks:{},started:{},revision:0};const history={first:run};
 const window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}},addEventListener(type,fn){this['on'+type]=fn;}};
 const document={getElementById:get,createElement:()=>new Node(),createTextNode:text=>({textContent:text}),querySelectorAll:()=>[],body:{classList:{add(){}}}};
 const fetch=async(url,options)=>{if(url==='/api/guide/active')return {ok:true,json:async()=>options?{guide_id:JSON.parse(options.body).guide_id,revision:1}:{}};requests.push(url);if(url==='/api/guide?id=other'&&alternateGuide)return {ok:true,json:async()=>alternateGuide};if(url.startsWith('/api/guide/media'))return {ok:true,json:async()=>mediaReport};if(url.startsWith('/api/guide/frame'))return {ok:true,blob:async()=>new Blob(['image'])};if(url.startsWith('/api/guide/progress')&&options){const body=JSON.parse(options.body);run=body.run_id?history[body.run_id]:(history.second={...run,run_id:'second',checks:{},started:{}});if(body.checks)run.checks=body.checks;if(body.started)run.started=body.started;run.revision++;}
  return {ok:true,json:async()=>url.startsWith('/api/guide/progress')?run:url==='/api/guides'?{guides:[{id:'two',title:'Generic'}]}:guide};};
 const context={window,document,fetch,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{pathname:'/walkthrough',search:'?guide=two',href:'http://localhost/walkthrough?guide=two'},HoiContracts:require('../src/hoi4companion/web/contracts.js'),HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js'),HoiOperationalSummary:require('../src/hoi4companion/web/operational-summary.js')};
 context.HoiActionPlan=require('../src/hoi4companion/web/action-plan.js');
 context.HoiGuideReferences=require('../src/hoi4companion/web/guide-references.js');
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide.js','utf8'),context);await new Promise(setImmediate);
 assert.equal(get('error').textContent,'');assert.equal(get('guideTitle').textContent,'Generic');assert.equal(get('sourceLink').hidden,true);
 assert.match(get('sourceDocuments').textContent,/Original\.docx/);assert.match(get('sourceDocuments').textContent,/<script>not executable<\/script>/);
 assert.equal(get('sourceDocuments').children[0].children[2].textContent,guide.sources[0].text);
 assert.match(get('operationCards').textContent,/See Initial Setup/);
 const referenceButton=get('operationCards').children.flatMap(function walk(n){return [n,...(n.children||[]).flatMap(walk)];}).find(n=>n.className==='guide-reference-link');
 assert.ok(referenceButton);assert.equal(referenceButton.textContent,'Initial Setup');assert.equal(referenceButton['aria-haspopup'],'dialog');
 assert.ok(get('operationCards').children.flatMap(function walk(n){return [n,...(n.children||[]).flatMap(walk)];}).some(n=>n.className==='guide-reference-related'));
 assert.match(get('hudDiagnosticMessage').textContent,/non verificabile/);
 assert.ok(requests.includes('/api/guide/progress?id=two'));
 assert.ok(get('operationCards').textContent.includes('Construction'));assert.ok(get('operationCards').textContent.includes('Quando: 1936'));
 assert.match(get('operationOrderNotice').textContent,/senza un riferimento temporale/);
 assert.match(get('operationCards').textContent,/Collocazione cronologica non verificata/);
 guide.tracks.operations[0].action.params.category_hint='Regional development';
 subscriber({usable:false});
 assert.match(get('operationCards').textContent,/Fabbriche e infrastrutture · Regional development/);
 assert.equal(get('operationCards').textContent.includes('NATIVE'),false);
 assert.equal(get('nextOperation').textContent.includes('NATIVE'),false);
 assert.equal(get('summaryCards').textContent.includes('NATIVE'),false);assert.match(get('summaryNotice').textContent,/non disponibile/);
 const descendants=node=>[node,...(node.children||[]).flatMap(descendants)];
 const stableCard=descendants(get('operationCards')).find(node=>node.id==='operation-build');
 subscriber({usable:false,metrics:{transport_only:1}});
 assert.equal(descendants(get('operationCards')).find(node=>node.id==='operation-build'),stableCard);
 const referenceImages=descendants(get('operationCards')).filter(node=>node.src?.startsWith('/api/reference-library/image?id='));
 assert.equal(referenceImages.length,2);assert.deepEqual(referenceImages.map(node=>node.alt),['OFF','T-I1']);assert.ok(referenceImages.every(node=>node.loading==='lazy'));
 const controls=descendants(get('operationCards')).filter(node=>node['data-check']);
 assert.deepEqual(controls.map(node=>node['data-check']).sort(),['done','phase_open']);
 subscriber({usable:true,sample:{country:'GER'},guide_revision:1,guide_id:'one',guide_states:{one:{done:true}}});
 assert.equal(get('operationProgress').textContent,'0/1');
 subscriber({usable:true,sample:{country:'GER'},guide_revision:2,guide_states:{two:{done:true}}});
 assert.equal(get('operationProgress').textContent,'1/1');assert.equal(get('operationCards').children.length,1); // empty-state text
 get('hideDone').checked=false;get('hideDone').onchange();
 const checkbox=descendants(get('operationCards')).find(node=>node['data-check']==='done');checkbox.checked=false;checkbox.onchange();await new Promise(setImmediate);
 assert.equal(run.checks.done,false);assert.equal(get('operationProgress').textContent,'0/1');
 const start=()=>descendants(get('operationCards')).find(node=>node['data-start']==='build');
 assert.equal(start().textContent,'Ho impartito l’ordine');
 start().onclick();await new Promise(setImmediate);
 assert.equal(run.started.build,true);assert.equal(get('operationProgress').textContent,'0/1');assert.equal(get('operationActive').textContent,'1 in corso');
 assert.match(get('operationCards').textContent,/Avviata · in corso \(manuale\)/);assert.match(get('operationCards').textContent,/completamento ancora da verificare/);
 assert.equal(get('nextOperation').textContent.includes('Construction'),false);
 const finish=descendants(get('operationCards')).find(node=>node['data-check']==='done');finish.checked=true;finish.onchange();await new Promise(setImmediate);
 assert.equal(get('operationProgress').textContent,'1/1');assert.equal(get('operationActive').textContent,'');
 const reopen=descendants(get('operationCards')).find(node=>node['data-check']==='done');reopen.checked=false;reopen.onchange();await new Promise(setImmediate);
 assert.equal(get('operationActive').textContent,'1 in corso');
 get('reset').onclick();await new Promise(setImmediate);assert.equal(run.run_id,'second');assert.equal(get('resetNotice').hidden,false);
 assert.equal(get('operationActive').textContent,'');
 const undo=get('resetNotice').children.find(node=>typeof node.onclick==='function');await undo.onclick();await new Promise(setImmediate);
 assert.equal(run.run_id,'first');assert.equal(run.checks.done,false);assert.equal(get('resetNotice').hidden,true);
 assert.equal(get('operationActive').textContent,'1 in corso');start().onclick();await new Promise(setImmediate);assert.equal(get('operationActive').textContent,'');assert.equal(run.checks.done,false);
 // A declared live criterion replaces the button, including while the game tick
 // is unchanged. A cancelled queue can never act as completion evidence.
 guide.tracks.operations[0].started_when={op:'fact',key:'construction.queued',test:'contains',value:'state:22:arms_factory'};
 const line={id:'line',building_id:'arms_factory',name:'Military factory',state_id:22,state_name:'Region',province_id:null,amount:3,active_factories:0,built_level:0,repair_levels:0,conversion:false,location_status:'observed'};
 const live=lines=>({usable:true,sample:{country:'GER',native_tick:10,country_overview:{country:'GER',construction:{status:'observed',lines}}}});
 subscriber(live([line]));
 subscriber({...live([line]),overlay:{status:'observed',guide_id:'two',message:'Apri Industria',items:[{code:'different_folder',message:'Ricerca nella scheda Industria',panel:'research',folder:'industry_folder'}]}});
 assert.equal(get('hudDiagnosticMessage').textContent,'Apri Industria');
 assert.ok(descendants(get('hudDiagnosticItems')).some(n=>n.href?.includes('/technologies?folder=industry_folder')));
 subscriber({...live([line]),overlay:{status:'observed',guide_id:'other',message:'OLD_SECRET',items:[]}});
 assert.equal(get('hudDiagnosticMessage').textContent.includes('OLD_SECRET'),false);
 subscriber(live([line]));
 assert.match(get('summaryCards').textContent,/Costruzione avviata/);
 assert.equal(get('operationActive').textContent,'1 in corso');assert.equal(get('operationProgress').textContent,'0/1');
 assert.equal(start(),undefined);assert.match(get('operationCards').textContent,/In corso · avvio rilevato live/);assert.match(get('operationCards').textContent,/3 in coda · 0 fabbriche attive/);
 subscriber(live([{...line,amount:4}]));assert.match(get('operationCards').textContent,/4 in coda/);
 subscriber(live([]));assert.equal(get('operationActive').textContent,'');assert.equal(get('operationProgress').textContent,'0/1');assert.match(get('operationCards').textContent,/Nessun ordine rilevato/);
 subscriber({...live([line]),usable:false});assert.equal(get('operationActive').textContent,'');assert.match(get('operationCards').textContent,/Avvio non verificabile/);assert.equal(get('operationCards').textContent.includes('ORDINI LETTI LIVE'),false);
 assert.equal(get('summaryCards').textContent,'');assert.match(get('summaryNotice').textContent,/non disponibile/);
 guide.tracks.operations.push({key:'method',tracking:'reminder',title:'Follow supply hubs',action:{kind:'manual',params:{category:'army',instructions:['Keep railways connected']}}});
 subscriber(live([]));
 const reminder=descendants(get('operationCards')).find(node=>node.dataset?.step==='method');
 assert.match(reminder.textContent,/Promemoria/);assert.equal(descendants(reminder).some(node=>node['data-check']||node['data-start']),false);
 assert.equal(get('operationProgress').textContent,'0/1');assert.equal(get('nextOperation').textContent.includes('Follow supply hubs'),false);
 guide.tracks.operations.push({key:'event',depends_on:[],title:'Choose event answer',started_when:{op:'fact',key:'events.visible_options',test:'contains',value:'example.1:example.1.a'},action:{kind:'manual',params:{event_id:'example.1',event_option_id:'example.1.a',event_option_name:'Recommended answer'}}});
 const eventLive=live([]);eventLive.sample.country_overview.events={status:'observed',visible:[{instance_id:'1',event_id:'example.1',option_ids:['example.1.a']}]};
 subscriber(eventLive);assert.match(get('operationCards').textContent,/EVENTO APERTO.*Scelta consigliata: Recommended answer · disponibile nella finestra/);
 assert.match(get('nextOperation').textContent,/Recommended answer/);assert.equal(get('operationProgress').textContent,'0/2');
 assert.ok(descendants(get('nextOperation')).some(node=>node.id==='next-operation-event'));
 guide.source={url:'https://www.youtube.com/watch?v=abcdefghijk'};
 guide.tracks.operations[0].action.params.source_seconds=118;subscriber(eventLive);
 let frame=descendants(get('operationCards')).find(n=>n.className==='operation-frame');frame.open=true;await frame.ontoggle();
 assert.equal(requests.filter(path=>path.startsWith('/api/guide/frame')).length,1);
 subscriber(eventLive);frame=descendants(get('operationCards')).find(n=>n.className==='operation-frame');assert.equal(frame.open,true);await frame.ontoggle();
 assert.equal(requests.filter(path=>path.startsWith('/api/guide/frame')).length,1);
 // Inventory visual references change the frame and link, not the instruction's chronology.
 delete guide.tracks.operations[0].action.params.source_seconds;
 guide.sources=[{id:'video',type:'youtube',url:'https://www.youtube.com/watch?v=abcdefghijk'}];
 guide.tracks.operations[0].evidence=[{source_id:'video',basis:'explicit',timestamp_seconds:489}];
 guide.source_inventory={schema_version:1,items:[{id:'visual',source_id:'video',basis:'visual',timestamp_seconds:511,step_keys:['build']}]};
 mediaReport={default_reference:'r0',references:[{id:'r0',basis:'visual',source_title:'Video',url:guide.source.url,seconds:511},{id:'r1',basis:'explicit',source_title:'Video',url:guide.source.url,seconds:489}]};
 subscriber(eventLive);
 assert.match(get('operationCards').textContent,/Mostra fotogramma della guida · 8:31/);
 assert.ok(descendants(get('operationCards')).some(n=>n.className==='operation-source'&&n.href?.includes('t=511')));
 frame=descendants(get('operationCards')).find(n=>n.className==='operation-frame');frame.open=true;await frame.ontoggle();
 assert.equal(requests.filter(path=>path.startsWith('/api/guide/frame')).length,2);
 subscriber(eventLive);frame=descendants(get('operationCards')).find(n=>n.className==='operation-frame');assert.equal(frame.open,true);await frame.ontoggle();
 assert.equal(requests.filter(path=>path.startsWith('/api/guide/frame')).length,2);
 assert.match(frame.textContent,/osservazione visiva/i);assert.match(frame.textContent,/parlato/i);
 const explicitFrame=descendants(frame).find(n=>n.className==='operation-show-frame'&&n.dataset.reference==='r1');
 await explicitFrame.onclick();assert.ok(requests.some(path=>path.includes('/api/guide/frame?')&&path.includes('ref=r1')));
 assert.match(frame.textContent,/non verificat/i);
 const clip=descendants(frame).find(n=>n.className==='operation-watch-clip'&&n.dataset.reference==='r1');
 assert.equal(get('sourceMediaVideo').src,undefined);clip.onclick();
 assert.equal(get('sourceMediaDialog').open,true);assert.match(get('sourceMediaVideo').src,/\/api\/guide\/clip\?.*ref=r1/);
 assert.equal(get('sourceMediaVideo').preload,'metadata');assert.notEqual(get('sourceMediaVideo').autoplay,true);
 assert.match(get('sourceMediaDescription').textContent,/8:09/);
 subscriber(eventLive);assert.equal(get('sourceMediaDialog').open,true);assert.match(get('sourceMediaVideo').src,/ref=r1/);
 get('sourceMediaVideo').onerror();assert.match(get('sourceMediaStatus').textContent,/disponibile/i);
 assert.match(get('sourceMediaOriginal').href,/t=489/);get('sourceMediaClose').onclick();
 assert.equal(get('sourceMediaDialog').open,false);assert.equal(get('sourceMediaVideo').src,undefined);assert.equal(get('sourceMediaVideo').paused,true);
 // Different source videos are listed separately; no arbitrary default frame is fetched.
 guide.sources.push({id:'video2',type:'youtube',url:'https://www.youtube.com/watch?v=zzzzzzzzzzz'});
 guide.source_inventory.items.push({id:'second',source_id:'video2',basis:'visual',timestamp_seconds:42,step_keys:['build']});
 mediaReport={default_reference:null,truncated:true,references:[{id:'r0',basis:'visual',url:guide.sources[0].url,seconds:511},{id:'r1',basis:'visual',url:guide.sources[1].url,seconds:42}]};
 subscriber(eventLive);frame=descendants(get('operationCards')).find(n=>n.className==='operation-frame');frame.open=true;
 const beforeFrames=requests.filter(path=>path.startsWith('/api/guide/frame')).length;await frame.ontoggle();
 assert.equal(requests.filter(path=>path.startsWith('/api/guide/frame')).length,beforeFrames);
 assert.match(frame.textContent,/riferimenti|source references/i);
 assert.ok(descendants(frame).some(n=>n.href?.includes('zzzzzzzzzzz')&&n.href.includes('t=42')));
 guide.sources.pop();guide.source_inventory.items.pop();
 // Operational dependencies may refer to native tracks, not only operations.
 guide.tracks.focus[0].name='Prerequisito nazionale localizzato';guide.tracks.focus[0].href='/?tree=t&focus=F';
 guide.tracks.operations[0].depends_on=['f'];delete guide.tracks.operations[0].phase;subscriber(eventLive);
 const dependent=descendants(get('operationCards')).find(n=>n.dataset?.step==='build');
 assert.match(dependent.textContent,/Prerequisito nazionale localizzato/);
 assert.ok(descendants(dependent).some(n=>n.href?.includes('focus=F')));
 // Long prerequisite lists keep all links accessible without creating hundreds
 // of off-screen nodes until the user opens the remaining details.
 for(let n=0;n<5;n++)guide.tracks.focus.push({key:'f'+n,kind:'focus',target:'F'+n,name:'Prerequisite '+n,href:'/?tree=t&focus=F'+n});
 guide.tracks.operations[0].depends_on=['f','f0','f1','f2','f3','f4'];subscriber(eventLive);
 let cardWithMore=descendants(get('operationCards')).find(node=>node.dataset?.step==='build');
 let more=descendants(cardWithMore).find(node=>node.className==='operation-dependency-more');
 assert.ok(more);assert.match(more.textContent,/Altri 3 prerequisiti/);
 assert.equal(descendants(cardWithMore).filter(node=>node.href?.includes('focus=')).length,3);
 more.open=true;more.ontoggle();assert.equal(descendants(cardWithMore).filter(node=>node.href?.includes('focus=')).length,6);
 subscriber(eventLive);cardWithMore=descendants(get('operationCards')).find(node=>node.dataset?.step==='build');
 more=descendants(cardWithMore).find(node=>node.className==='operation-dependency-more');assert.equal(more.open,true);
 assert.equal(descendants(cardWithMore).filter(node=>node.href?.includes('focus=')).length,6);
 // A cheaper later appointment must not replace the first one or ask for a
 // manual order when live membership already supplies its completion.
 guide.phases=[];guide.tracks={operations:[
  {key:'expert',title:'Next infantry expert',depends_on:[],political_power_cost:150,action:{kind:'manual',params:{category:'characters',advisor_id:'expert',role:'high_command'}},completion:{op:'fact',key:'politics.advisors',test:'contains',value:'expert'}},
  {key:'royalist',title:'Later royalist',depends_on:[],priority:100,political_power_cost:50,action:{kind:'manual',params:{category:'characters',advisor_id:'royalist',role:'political_advisor'}},completion:{op:'fact',key:'politics.advisors',test:'contains',value:'royalist'}}]};
 subscriber({usable:true,sample:{country:'GER',country_overview:{country:'GER',politics:{status:'observed',political_power:70,advisors:{status:'observed',items:[]},ideas:{status:'observed',items:[]}}}},guide_revision:10});
 await new Promise(setImmediate);await new Promise(setImmediate);
 assert.match(get('nextOperation').textContent,/Next infantry expert/);
 assert.match(get('operationCards').textContent,/70.*150/);
 assert.equal(descendants(get('operationCards')).some(n=>n['data-start']==='expert'),false);
 // Switching guide closes the old source and cannot play the previous guide's clip.
 guide.tracks.operations[0].action.params.source_seconds=511;mediaReport={default_reference:'r0',references:[{id:'r0',basis:'visual',url:guide.source.url,seconds:511}]};
 subscriber({usable:false});frame=descendants(get('operationCards')).find(n=>n.className==='operation-frame');frame.open=true;await frame.ontoggle();
 descendants(frame).find(n=>n.className==='operation-watch-clip').onclick();assert.equal(get('sourceMediaDialog').open,true);
 alternateGuide=JSON.parse(JSON.stringify(guide));alternateGuide.id='other';alternateGuide.title='Other guide';await window.hoiGuide.select('other');
 assert.equal(get('sourceMediaDialog').open,false);assert.equal(get('sourceMediaVideo').src,undefined);assert.equal(get('guideTitle').textContent,'Other guide');
 // A replaced allocation stays inspectable as Superseded, without a false
 // completed badge or a manual-start control, and hides with past instructions.
 alternateGuide.phases=[];
 const allocation=(key,n,order)=>({key,title:'Allocation '+key,action:{kind:'manual',params:{category:'production',requested_factories:n,production_selector:{equipment_ids:['sup'],variant_names:[],category:'military'},production_revision:{group:'setup',order}}},completion:{op:'fact',key:'guide.production.'+key+'.requested',test:'eq',value:n}});
 alternateGuide.tracks={operations:[allocation('old-allocation',1,0),allocation('new-allocation',7,1)]};
 const productionLive={schema_version:1,usable:true,guide_revision:15,facts:{country:{status:'observed',source:'fixture',value:'TST'},'production.lines':{status:'observed',source:'fixture',value:[{id:'1',equipment:'sup',name:'Support',category:'military',requested_factories:7,assigned_factories:7,active_factories:7,damaged_factories:0}]}}};
 get('hideDone').checked=false;subscriber(productionLive);await new Promise(setImmediate);await new Promise(setImmediate);
 const oldAllocation=descendants(get('operationCards')).find(n=>n.id==='operation-old-allocation');
 assert.ok(oldAllocation);assert.match(oldAllocation.textContent,/Sostituito/);assert.doesNotMatch(oldAllocation.textContent,/Completato/);
 assert.equal(descendants(oldAllocation).some(n=>n['data-start']==='old-allocation'),false);
 get('hideDone').checked=true;get('hideDone').onchange();
 assert.equal(descendants(get('operationCards')).some(n=>n.id==='operation-old-allocation'),false);
 // Named equipment is checked from its observed configuration. The card shows
 // exact slot differences and clears a previous success when coverage is lost.
 const designStep=allocation('design-check',7,0);delete designStep.action.params.production_revision;
 designStep.action.params.design_reference={name:'Guide design',modules:{main_armament_slot:'small_cannon'},upgrades:{engine:2}};
 designStep.action.params.production_selector.variant_names=['Guide design'];
 alternateGuide.tracks={operations:[designStep]};get('hideDone').checked=false;
 const designLive=JSON.parse(JSON.stringify(productionLive)),designLine=designLive.facts['production.lines'].value[0];
 designLine.name='A renamed production vehicle';designLine.design={status:'observed',modules:{main_armament_slot:'machine_gun'},upgrades:{engine:1}};
 subscriber(designLive);
 const designCard=()=>descendants(get('operationCards')).find(n=>n.id==='operation-design-check');
 assert.match(designCard().textContent,/La configurazione del progetto è diversa/);
 const differences=descendants(designCard()).find(n=>n.className==='design-differences');
 assert.ok(differences);assert.match(differences.textContent,/A renamed production vehicle · Differenze nella configurazione/);
 assert.match(differences.textContent,/Richiesto main_armament_slot: small_cannon · rilevato: machine_gun/);
 assert.match(differences.textContent,/Richiesto engine: 2 · rilevato: 1/);
 assert.doesNotMatch(designCard().textContent,/Completat[ao]|La configurazione del progetto corrisponde/);
 designLine.design={status:'observed',modules:{main_armament_slot:'small_cannon'},upgrades:{engine:2}};subscriber(designLive);
 assert.match(designCard().textContent,/La configurazione del progetto corrisponde/);assert.match(designCard().textContent,/Completat[ao]/);
 designLine.design={status:'unavailable'};subscriber(designLive);
 assert.match(designCard().textContent,/Configurazione del progetto non verificabile/);
 assert.doesNotMatch(designCard().textContent,/Completat[ao]|La configurazione del progetto corrisponde|main_armament_slot: small_cannon · rilevato/);
 assert.equal(descendants(designCard()).some(n=>n.className==='design-differences'),false);
 // The collapsed plan defers card work; expanding applies the latest state.
 get('operationsPlan').open=false;
 designLine.design={status:'observed',modules:{main_armament_slot:'small_cannon'},upgrades:{engine:2}};
 subscriber(designLive);
 assert.match(designCard().textContent,/Configurazione del progetto non verificabile/);
 get('operationsPlan').open=true;get('operationsPlan').ontoggle();
 assert.match(designCard().textContent,/La configurazione del progetto corrisponde/);
 get('operationsPlan').open=false;context.location.hash='#operation-design-check';
 assert.equal(typeof window.onhashchange,'function');window.onhashchange();
 assert.equal(get('operationsPlan').open,true);
});

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}
