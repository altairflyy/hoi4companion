'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const model=require('../src/hoi4companion/web/events-model.js');
const data={country:'SOV',events:[
 {id:'sov.1',title:'Purga',description:'Il partito...',triggered_by:[{kind:'focus',id:'SOV_purge',name:'La Grande Purga'}],options:[{id:'a',name:'Fallo'}]},
 {id:'sov.2',title:'Patto',triggered_by:[{kind:'decision',id:'d',name:'Firma'},{kind:'event',id:'sov.1',name:'Purga'}],options:[]},
 {id:'sov.3',title:'Città',triggered_by:[],options:[]}],
 possible:[{id:'any.1',title:'Scontro',triggered_by:[],options:[]}]};
test('events filter by what triggers them, search text and possible events',()=>{
 assert.deepEqual(model.visible(data).map(e=>e.id),['sov.1','sov.2','sov.3']);
 assert.deepEqual(model.visible(data,{origin:'focus'}).map(e=>e.id),['sov.1']);
 assert.deepEqual(model.visible(data,{origin:'event'}).map(e=>e.id),['sov.2'],'a chain event is listed under events');
 assert.deepEqual(model.visible(data,{origin:'none'}).map(e=>e.id),['sov.3']);
 assert.deepEqual(model.visible(data,{query:'grande purga'}).map(e=>e.id),['sov.1'],'the trigger name is searchable');
 assert.deepEqual(model.visible(data,{query:'FALLO'}).map(e=>e.id),['sov.1'],'answers are searchable, any case');
 assert.deepEqual(model.visible(data,{possible:true}).map(e=>e.id),['sov.1','sov.2','sov.3','any.1']);
 assert.equal(model.visible(data,{possible:true}).at(-1).possible,true);
 assert.deepEqual(model.counts(data),{all:3,possible:1,focus:1,decision:1,event:1,none:1,own:0,shared:0});
 assert.deepEqual(model.visible(null),[]);
});
test('the events page is routed in the suite and wired to its scripts',()=>{
 const fs=require('node:fs');
 assert.equal(require('../src/hoi4companion/web/suite-model.js').routes.events,'/events');
 const html=fs.readFileSync('src/hoi4companion/web/events.html','utf8');
 for(const script of ['/events-model.js','/events.js'])assert.ok(html.includes(script));
 assert.match(fs.readFileSync('src/hoi4companion/web/suite.html','utf8'),/data-view="events"/);
});
test('script placeholders read as the played and the other nation',()=>{
 assert.equal(model.readable(' La [FROM.GetName] propone a [ROOT.GetNameDef] un [Unknown]  patto ',{country:'SOV',other:'l\u2019altra nazione'}),'L’altra nazione propone a SOV un … patto');
 assert.equal(model.readable('La [FROM.GetName] chiede aiuto a l’[ROOT.GetNameDef]',{country:'SOV',other:'CZE'}),'CZE chiede aiuto a SOV');
 assert.equal(model.sender({triggered_by:[{kind:'focus',id:'CZE_soviet_alliance'},{kind:'focus',id:'CZE_seek_french_leadership'}]},'SOV'),'CZE');
 assert.equal(model.sender({triggered_by:[{kind:'focus',id:'CZE_a'},{kind:'decision',id:'POL_b'}]},'SOV'),null,'two senders: none named');
 assert.equal(model.sender({triggered_by:[{kind:'focus',id:'SOV_a'}]},'SOV'),null,'own focus is not a sender');
 assert.equal(model.readable(null),'');
});

test('events tell exclusive from shared and own triggers from other nations',()=>{
 const data={country:'SOV',events:[
  {id:'a',title:'Purge',receivers:['SOV'],triggered_by:[{kind:'focus',id:'SOV_purge'}]},
  {id:'b',title:'Pact',receivers:['SOV','GER'],triggered_by:[{kind:'focus',id:'GER_pact'}]},
  {id:'c',title:'Crisis',receivers:['SOV','ITA','FRA']}],possible:[{id:'d',title:'Clash',receivers:[]}]};
 assert.equal(model.scope(data.events[0],'SOV'),'own');
 assert.equal(model.scope(data.events[1],'SOV'),'shared');
 assert.equal(model.scope(data.possible[0],'SOV'),'unknown');
 assert.equal(model.scope({receivers:['GER']},'SOV'),'shared','only another nation: not exclusive to the played one');
 assert.equal(model.fromOwn(data.events[0],'SOV'),true);
 assert.equal(model.fromOwn(data.events[1],'SOV'),false);
 const c=model.counts(data);assert.equal(c.own,1);assert.equal(c.shared,2);
 assert.deepEqual(model.visible(data,{reach:'own'}).map(e=>e.id),['a']);
 assert.deepEqual(model.visible(data,{reach:'shared'}).map(e=>e.id),['b','c']);
 assert.deepEqual(model.visible(data,{possible:true}).map(e=>e.id),['a','b','c','d']);
 const html=require('node:fs').readFileSync('src/hoi4companion/web/events.html','utf8');assert.match(html,/id="eventsReach"/);
});
