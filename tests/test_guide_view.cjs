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
 const fetch=async(url,options)=>{requests.push(url);if(url.startsWith('/api/guide/progress')&&options){const body=JSON.parse(options.body);run=body.run_id?history[body.run_id]:(history.second={...run,run_id:'second',checks:{},started:{}});if(body.checks)run.checks=body.checks;if(body.started)run.started=body.started;run.revision++;}
  return {ok:true,json:async()=>url.startsWith('/api/guide/progress')?run:url==='/api/guides'?{guides:[{id:'two',title:'Generic'}]}:guide};};
 const context={window,document,fetch,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{pathname:'/walkthrough',search:'?guide=two',href:'http://localhost/walkthrough?guide=two'},HoiContracts:require('../src/hoi4companion/web/contracts.js'),HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js')};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide.js','utf8'),context);await new Promise(setImmediate);
 assert.equal(get('error').textContent,'');assert.equal(get('guideTitle').textContent,'Generic');assert.equal(get('sourceLink').hidden,true);
 assert.ok(requests.includes('/api/guide/progress?id=two'));
 assert.ok(get('operationCards').textContent.includes('Construction'));assert.ok(get('operationCards').textContent.includes('Quando: 1936'));
 assert.equal(get('operationCards').textContent.includes('NATIVE'),false);
 assert.equal(get('nextOperation').textContent.includes('NATIVE'),false);
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
 assert.equal(get('operationActive').textContent,'1 in corso');assert.equal(get('operationProgress').textContent,'0/1');
 assert.equal(start(),undefined);assert.match(get('operationCards').textContent,/Avviata · rilevata live/);assert.match(get('operationCards').textContent,/3 in coda · 0 fabbriche attive/);
 subscriber(live([{...line,amount:4}]));assert.match(get('operationCards').textContent,/4 in coda/);
 subscriber(live([]));assert.equal(get('operationActive').textContent,'');assert.equal(get('operationProgress').textContent,'0/1');assert.match(get('operationCards').textContent,/Nessun ordine rilevato/);
 subscriber({...live([line]),usable:false});assert.equal(get('operationActive').textContent,'');assert.match(get('operationCards').textContent,/Avvio non verificabile/);assert.equal(get('operationCards').textContent.includes('ORDINI LETTI LIVE'),false);
});
