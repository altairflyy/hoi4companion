'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
function countryView(language='italian'){
 class Node{constructor(){this.children=[];this.text='';this.classList={toggle(){}};}set textContent(v){this.text=String(v);this.children=[];}get textContent(){return this.text+this.children.map(x=>x.textContent||'').join('');}append(...v){this.children.push(...v);}replaceChildren(...v){this.text='';this.children=v;}addEventListener(){}}
 const html=fs.readFileSync('src/hoi4companion/web/country.html','utf8'),nodes=new Map([...html.matchAll(/id="([^"]+)"/g)].map(m=>[m[1],new Node()]));nodes.get('stateSearch').value='';
 const get=id=>{assert.ok(nodes.has(id),'Missing element '+id);return nodes.get(id);};let render;
 const window={HoiCountryModel:require('../src/hoi4companion/web/country-model.js'),hoiLive:{enabled:true,subscribe(fn){render=fn;}}};
 const document={getElementById:get,createElement:()=>new Node(),body:new Node()};
 runLocalized(fs.readFileSync('src/hoi4companion/web/country.js','utf8'),{window,document,Intl,Date,Number},undefined,language);
 return {render,get};
}
test('country view renders qualified laws and current event choices, then hides stale values',()=>{
 const {render,get}=countryView();
 const current={usable:true,sample:{country:'TST',country_overview:{country:'TST',politics:{status:'observed',ideas:{status:'observed',items:[{id:'test_law',name:'Observed law',group:'economy',is_law:true}]},advisors:{status:'observed',items:[]}},events:{status:'observed',visible:[{event_id:'example.1',name:'Current event',options:[{id:'example.1.a',name:'Choice one'}]}]},decisions:{status:'observed',listed_ids:[]}}}};
 render(current);assert.match(get('politicsIdeaRows').textContent,/Observed law/);assert.match(get('politicsAdvisorsCoverage').textContent,/Nessun incarico attivo/);assert.match(get('visibleEvents').textContent,/Current event.*Choice one/);assert.match(get('decisionsReason').textContent,/^$/);
 render({...current,usable:false});assert.equal(get('politicsIdeaRows').children.length,0);assert.equal(get('visibleEvents').children.length,0);assert.equal(get('eventsStatus').textContent,'Non disponibile');
});

test('country view displays zero and fractional military experience plus war and peace in both languages, hiding stale values',()=>{
 for(const language of ['english','italian']){
  const {render,get}=countryView(language),current={usable:true,sample:{country:'TST',country_overview:{country:'TST',experience:{status:'observed',army:0,navy:12.5,air:3.75},diplomacy:{status:'observed',at_war:true}}}};
  render(current);
  assert.equal(get('experienceArmy').textContent,'0');assert.equal(get('experienceNavy').textContent,language==='italian'?'12,5':'12.5');
  assert.equal(get('experienceAir').textContent,language==='italian'?'3,75':'3.75');assert.equal(get('diplomacyWar').textContent,language==='italian'?'In guerra':'At war');
  current.sample.country_overview.diplomacy.at_war=false;render(current);
  assert.equal(get('diplomacyWar').textContent,language==='italian'?'In pace':'At peace');
  render({...current,usable:false});
  for(const id of ['experienceArmy','experienceNavy','experienceAir','diplomacyWar'])assert.equal(get(id).textContent,'—');
 }
});

function runLocalized(code,context,options,language='italian'){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage(language);context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}
