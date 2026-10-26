'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
test('walkthrough renders only operational instructions, preserves conditions, runs and undo',async()=>{
 class Node{
  constructor(){this.children=[];this.dataset={};this.hidden=false;this.text='';}
  set textContent(value){this.text=String(value);this.children=[];}
  get textContent(){return this.text+this.children.map(x=>x.textContent||'').join('');}
  append(...nodes){this.children.push(...nodes);}
  replaceChildren(...nodes){this.text='';this.children=nodes;}
  setAttribute(key,value){this[key]=value;}getAttribute(key){return this[key];}addEventListener(type,fn){this['on'+type]=fn;}
 }
 const nodes=new Map(),get=id=>{if(!nodes.has(id))nodes.set(id,new Node());return nodes.get(id);};get('hideDone').checked=true;
 const guide={schema_version:1,id:'two',title:'Generic',phases:[{id:'phase',title:'Phase',when:{op:'check',key:'phase_open'}}],tracks:{focus:[{key:'f',kind:'focus',target:'F',name:'NATIVE FOCUS'}],research:[{key:'r',kind:'technology',target:'R',name:'NATIVE TECH'}],operations:[{key:'build',kind:'gate',name:'Construction',phase:'phase',completion:{op:'check',key:'done'},depends_on:[],action:{kind:'manual',params:{category:'construction',timing:'1936',instructions:['Build first']}}}]}};
 let subscriber;const requests=[];let run={schema_version:1,guide_id:'two',run_id:'first',session_id:null,checks:{},started:{},revision:0};const history={first:run};
 const window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}};
 const document={getElementById:get,createElement:()=>new Node(),createTextNode:text=>({textContent:text}),querySelectorAll:()=>[],body:{classList:{add(){}}}};
 const fetch=async(url,options)=>{if(url==='/api/guide/active')return {ok:true,json:async()=>options?{guide_id:JSON.parse(options.body).guide_id,revision:1}:{}};requests.push(url);if(url.startsWith('/api/guide/frame'))return {ok:true,blob:async()=>new Blob(['image'])};if(url.startsWith('/api/guide/progress')&&options){const body=JSON.parse(options.body);run=body.run_id?history[body.run_id]:(history.second={...run,run_id:'second',checks:{},started:{}});if(body.checks)run.checks=body.checks;if(body.started)run.started=body.started;run.revision++;}
  return {ok:true,json:async()=>url.startsWith('/api/guide/progress')?run:url==='/api/guides'?{guides:[{id:'two',title:'Generic'}]}:guide};};
 const context={window,document,fetch,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{pathname:'/walkthrough',search:'?guide=two',href:'http://localhost/walkthrough?guide=two'},HoiContracts:require('../src/hoi4companion/web/contracts.js'),HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js'),HoiOperationalSummary:require('../src/hoi4companion/web/operational-summary.js')};
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide.js','utf8'),context);await new Promise(setImmediate);
 assert.equal(get('error').textContent,'');assert.equal(get('guideTitle').textContent,'Generic');assert.equal(get('sourceLink').hidden,true);
 assert.match(get('hudDiagnosticMessage').textContent,/non verificabile/);
 assert.ok(requests.includes('/api/guide/progress?id=two'));
 assert.ok(get('operationCards').textContent.includes('Construction'));assert.ok(get('operationCards').textContent.includes('Quando: 1936'));
 assert.equal(get('operationCards').textContent.includes('NATIVE'),false);
 assert.equal(get('nextOperation').textContent.includes('NATIVE'),false);
 assert.equal(get('summaryCards').textContent.includes('NATIVE'),false);assert.match(get('summaryNotice').textContent,/non disponibile/);
 const descendants=node=>[node,...(node.children||[]).flatMap(descendants)];
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
 assert.match(get('nextOperation').textContent,/Choose event answer/);assert.equal(get('operationProgress').textContent,'0/2');
 guide.tracks.operations[0].action.params.source_seconds=118;subscriber(eventLive);
 let frame=descendants(get('operationCards')).find(n=>n.className==='operation-frame');frame.open=true;await frame.ontoggle();
 assert.equal(requests.filter(path=>path.startsWith('/api/guide/frame')).length,1);
 subscriber(eventLive);frame=descendants(get('operationCards')).find(n=>n.className==='operation-frame');assert.equal(frame.open,true);await frame.ontoggle();
 assert.equal(requests.filter(path=>path.startsWith('/api/guide/frame')).length,1);
 // Operational dependencies may refer to native tracks, not only operations.
 guide.tracks.focus[0].name='Prerequisito nazionale localizzato';guide.tracks.focus[0].href='/?tree=t&focus=F';
 guide.tracks.operations[0].depends_on=['f'];delete guide.tracks.operations[0].phase;subscriber(eventLive);
 const dependent=descendants(get('operationCards')).find(n=>n.dataset?.step==='build');
 assert.match(dependent.textContent,/Prerequisito nazionale localizzato/);
 assert.ok(descendants(dependent).some(n=>n.href?.includes('focus=F')));
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
});

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}
