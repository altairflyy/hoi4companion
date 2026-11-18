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
 assert.deepEqual(model.counts(data),{all:3,possible:1,guide:3,nation:3,fromGuide:0,otherBranch:0,foreign:0,historical:0,focus:1,decision:1,event:1,none:1});
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

test('the guide view hides other branches and lists guide events first',()=>{
 const data={country:'SOV',guide_focus_count:2,events:[
  {id:'a',title:'Alpha',guide_focuses:[{id:'SOV_b',name:'B'}],guide_order:1,guide_reach:'guide'},
  {id:'b',title:'Beta',guide_focuses:[],guide_reach:'other_branch'},{id:'e',title:'Epsilon',guide_focuses:[],guide_reach:'anyway'},{id:'f',title:'Phi',guide_focuses:[],guide_reach:'foreign'},{id:'h',title:'Eta',guide_focuses:[],guide_reach:'historical',historical_focuses:[{id:'GER_pact',name:'Pact'}]},
  {id:'c',title:'Gamma',guide_focuses:[{id:'SOV_a',name:'A'}],guide_order:0,guide_reach:'guide'}],possible:[{id:'d',title:'Clash'}]};
 assert.deepEqual(model.visible(data).map(e=>e.id),['c','a','e','h'],'guide view: guide events first in guide order, other branches hidden');
 assert.deepEqual(model.visible(data,{scope:'guide',possible:true}).map(e=>e.id),['c','a','e','h'],'possible events stay out of the guide view');
 assert.deepEqual(model.visible(data,{scope:'nation'}).map(e=>e.id),['a','b','e','h','c'],'events of other nations never show');
 const c=model.counts(data);assert.equal(c.guide,4);assert.equal(c.nation,5);assert.equal(c.all,4);assert.equal(c.historical,1);assert.equal(c.fromGuide,2);assert.equal(c.otherBranch,1);assert.equal(c.foreign,1);
 assert.equal(model.guideActive({...data,guide_focus_count:0}),false);
 assert.deepEqual(model.visible({...data,guide_focus_count:0}).map(e=>e.id),['a','b','e','h','c'],'without a guide every event shows, except those of other nations');
 const html=require('node:fs').readFileSync('src/hoi4companion/web/events.html','utf8');assert.match(html,/id="eventsScope"/);
});

test('focus names fall back to a readable id when the game name is a placeholder',()=>{
 assert.equal(model.focusName({id:'GER_molotov_ribbentrop_pact',name:'[GER.GetMolotovRibbentropName]'}),'GER · molotov ribbentrop pact');
 assert.equal(model.focusName({id:'CHI_x',name:'Il gruppo di volontari'}),'Il gruppo di volontari');
 assert.equal(model.focusName({id:'generic_focus'}),'generic focus');
});
