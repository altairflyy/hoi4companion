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
  setAttribute(){} addEventListener(){}
 }
 const nodes=new Map(),get=id=>{if(!nodes.has(id))nodes.set(id,new Node());return nodes.get(id);};
 const guide={schema_version:1,id:'two',title:'Generic',tracks:{focus:[{key:'gate',kind:'gate',name:'Confirm'}],research:[{key:'tech',kind:'technology',target:'t',name:'Technology'}]}};
 let subscriber;const requests=[];
 const window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}};
 const document={getElementById:get,createElement:()=>new Node(),createTextNode:text=>({textContent:text}),querySelector:()=>get('hudLink')};
 const fetch=async url=>{requests.push(url);return {ok:true,json:async()=>url.startsWith('/api/guide/checks')?{checks:{},revision:0}:url==='/api/guides'?{guides:[{id:'two',title:'Generic'}]}:guide};};
 const context={window,document,fetch,URL,URLSearchParams,location:{pathname:'/walkthrough',search:'?guide=two',href:'http://localhost/walkthrough?guide=two'},HoiContracts:require('../src/hoi4companion/web/contracts.js'),HoiGuideModel:require('../src/hoi4companion/web/guide-model.js')};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide.js','utf8'),context);
 await new Promise(setImmediate);
 assert.equal(get('error').textContent,'');assert.equal(get('guideTitle').textContent,'Generic');
 assert.equal(get('sourceLink').hidden,true);assert.ok(requests.includes('/api/guide/checks?id=two'));
 assert.equal(get('researchNote').textContent.includes('undefined'),false);
 const sample={country:'GER',completed_focuses:[],researched_technologies:[]};
 subscriber({usable:true,sample,guide_revision:1,guide_id:'one',guide_checks:{gate:true},guide_states:{one:{gate:true}}});
 assert.equal(get('focusNext').textContent,'Confirm');
 assert.equal(get('parallel').textContent,'Slot liberi non osservati');
 subscriber({usable:true,sample,guide_revision:2,guide_states:{two:{gate:true}}});
 assert.equal(get('focusNext').textContent,'Priorità della guida completate');
});
