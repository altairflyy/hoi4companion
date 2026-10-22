'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
test('country view renders qualified laws and current event choices, then hides stale values',()=>{
 class Node{constructor(){this.children=[];this.text='';this.classList={toggle(){}};}set textContent(v){this.text=String(v);this.children=[];}get textContent(){return this.text+this.children.map(x=>x.textContent||'').join('');}append(...v){this.children.push(...v);}replaceChildren(...v){this.text='';this.children=v;}addEventListener(){}}
 const html=fs.readFileSync('src/hoi4companion/web/country.html','utf8'),nodes=new Map([...html.matchAll(/id="([^"]+)"/g)].map(m=>[m[1],new Node()]));nodes.get('stateSearch').value='';
 const get=id=>{assert.ok(nodes.has(id),'Missing element '+id);return nodes.get(id);};let render;
 const window={HoiCountryModel:require('../src/hoi4companion/web/country-model.js'),hoiLive:{enabled:true,subscribe(fn){render=fn;}}};
 const document={getElementById:get,createElement:()=>new Node(),body:new Node()};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/country.js','utf8'),{window,document,Intl,Date,Number});
 const current={usable:true,sample:{country:'TST',country_overview:{country:'TST',politics:{status:'observed',ideas:{status:'observed',items:[{id:'test_law',name:'Observed law',group:'economy',is_law:true}]},advisors:{status:'observed',items:[]}},events:{status:'observed',visible:[{event_id:'example.1',name:'Current event',options:[{id:'example.1.a',name:'Choice one'}]}]},decisions:{status:'observed',listed_ids:[]}}}};
 render(current);assert.match(get('politicsIdeaRows').textContent,/Observed law/);assert.match(get('politicsAdvisorsCoverage').textContent,/Nessun incarico attivo/);assert.match(get('visibleEvents').textContent,/Current event.*Choice one/);assert.match(get('decisionsReason').textContent,/^$/);
 render({...current,usable:false});assert.equal(get('politicsIdeaRows').children.length,0);assert.equal(get('visibleEvents').children.length,0);assert.equal(get('eventsStatus').textContent,'Non disponibile');
});
