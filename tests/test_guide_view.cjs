'use strict';
// Runs the actual view/controller against an in-memory DOM; not visual browser QA.
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
test('generic guide without video metadata renders and consumes only its own event checklist',async()=>{
 class Node{
  constructor(){this.children=[];this.dataset={};this.hidden=false;this.text='';}
  set textContent(value){this.text=String(value);this.children=[];}
  get textContent(){return this.text+this.children.map(x=>x.textContent||'').join('');}
  append(...nodes){this.children.push(...nodes);}
  replaceChildren(...nodes){this.text='';this.children=nodes;}
  setAttribute(key,value){this[key]=value;} addEventListener(type,fn){this['on'+type]=fn;}
 }
 const nodes=new Map(),get=id=>{if(!nodes.has(id))nodes.set(id,new Node());return nodes.get(id);};
 const guide={schema_version:1,id:'two',title:'Generic',phases:[{id:'phase',title:'Phase',when:{op:'check',key:'phase_open'}}],tracks:{focus:[{key:'gate',kind:'gate',name:'Confirm'}],research:[{key:'tech',kind:'technology',target:'t',name:'Technology',phase:'phase',completion:{op:'check',key:'tech_done'}}]}};
 let subscriber;const requests=[];let run={schema_version:1,guide_id:'two',run_id:'first',session_id:null,checks:{},revision:0};const history={first:run};
 const window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}};
 const document={getElementById:get,createElement:()=>new Node(),createTextNode:text=>({textContent:text}),querySelector:()=>get('hudLink')};
 const fetch=async (url,options)=>{requests.push(url);if(url.startsWith('/api/guide/progress')&&options){const body=JSON.parse(options.body);run=body.run_id?history[body.run_id]:(history.second={...run,run_id:'second',checks:{}});if(body.checks)run.checks=body.checks;}
  return {ok:true,json:async()=>url.startsWith('/api/guide/progress')?run:url.startsWith('/api/guide/checks')?{checks:{},revision:0}:url==='/api/guides'?{guides:[{id:'two',title:'Generic'}]}:guide};};
 const context={window,document,fetch,URL,URLSearchParams,location:{pathname:'/walkthrough',search:'?guide=two',href:'http://localhost/walkthrough?guide=two'},HoiContracts:require('../src/hoi4companion/web/contracts.js'),HoiGuideModel:require('../src/hoi4companion/web/guide-model.js')};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide.js','utf8'),context);
 await new Promise(setImmediate);
 assert.equal(get('error').textContent,'');assert.equal(get('guideTitle').textContent,'Generic');
 assert.equal(get('sourceLink').hidden,true);assert.ok(requests.includes('/api/guide/checks?id=two'));
 assert.equal(get('researchNote').textContent.includes('undefined'),false);
 assert.ok(get('researchSteps').textContent.includes('Non verificabile'));
 const descendants=node=>[node,...node.children.flatMap(descendants)];
 const controls=descendants(get('researchSteps')).filter(node=>node['data-check']);
 assert.deepEqual(controls.map(node=>node['data-check']).sort(),['phase_open','tech_done']);
 const sample={country:'GER',completed_focuses:[],researched_technologies:[]};
 subscriber({usable:true,sample,guide_revision:1,guide_id:'one',guide_checks:{gate:true},guide_states:{one:{gate:true}}});
 assert.equal(get('focusNext').textContent,'Confirm');
 assert.equal(get('parallel').textContent,'Slot liberi non osservati');
 subscriber({usable:true,sample,guide_revision:2,guide_states:{two:{gate:true}}});
 assert.equal(get('focusNext').textContent,'Priorità della guida completate');
 const checkbox=descendants(get('researchSteps')).find(node=>node['data-check']==='tech_done');
 checkbox.checked=false;checkbox.onchange();await new Promise(setImmediate);
 assert.equal(run.checks.tech_done,false,'negative confirmations survive the HTTP composition');
 assert.equal(run.checks.gate,true);
 get('reset').onclick();await new Promise(setImmediate);
 assert.equal(get('focusNext').textContent,'Confirm');
 assert.equal(get('resetNotice').hidden,false);
 const undo=get('resetNotice').children.find(node=>typeof node.onclick==='function');
 undo.onclick();await new Promise(setImmediate);
 assert.equal(get('resetNotice').hidden,true);
 assert.equal(get('focusNext').textContent,'Priorità della guida completate');
 assert.ok(requests.includes('/api/guide/progress?id=two'));
});
