'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const i18n=require('../src/hoi4companion/web/i18n.js');

test('English is default and Italian remains selectable without translating arguments',()=>{
 assert.equal(i18n.language,'english');assert.equal(i18n.t('Ricarica'),'Refresh');
 assert.equal(i18n.t('GUI reading suspended: {0}','Pausa {1} <img>'),'GUI reading suspended: Pausa {1} <img>');
 i18n.setLanguage('italian');assert.equal(i18n.t('Refresh'),'Ricarica');
 assert.equal(i18n.t('GUI reading suspended: {0}','GER_focus'),'Lettura GUI sospesa: GER_focus');
 assert.equal(i18n.t('GER_custom_focus'),'GER_custom_focus');
 assert.throws(()=>i18n.setLanguage('unsupported'));i18n.setLanguage('english');
});

test('language navigation writes only the UI preference and reloads the document',async()=>{
 class Node{constructor(){this.children=[];this.events={};}append(...nodes){this.children.push(...nodes);}setAttribute(k,v){this[k]=v;}addEventListener(k,v){this.events[k]=v;}}
 const nav=new Node(),events={},requests=[];let reloaded=0;
 const document={documentElement:{lang:'en'},querySelector:()=>nav,createElement:()=>new Node(),addEventListener:(event,fn)=>{events[event]=fn;}};
 const context={document,HOI_TRANSLATIONS:require('../src/hoi4companion/translations.json'),location:{reload(){reloaded++;}},fetch:async(url,options)=>{requests.push({url,...JSON.parse(options.body)});return {ok:true};}};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/i18n.js','utf8'),context);
 events.DOMContentLoaded();const select=nav.children[0].children[0];assert.equal(select.value,'english');
 assert.equal(select['aria-label'],'Interface language');select.value='italian';await select.events.change();
 assert.deepEqual(requests,[{url:'/api/config',ui_language:'italian'}]);assert.equal(reloaded,1);
});

test('locale changes preserve runtime targets, order, states and authored guide content',()=>{
 const runtime=require('../src/hoi4companion/web/guide-runtime.js');
 const guide={schema_version:1,id:'language-proof',tracks:{focus:[{key:'focus',action:{kind:'select_focus',entity:{kind:'focus',id:'GER_test'}},title:'Titolo originale',evidence:[]}]}};
 const state={schema_version:1,usable:true,facts:{'focus.completed':{status:'observed',value:[],source:'fixture'},'focus.active':{status:'observed',value:[],source:'fixture'}}};
 const before=JSON.stringify(guide);i18n.setLanguage('english');const en=runtime.evaluate(guide,state);
 i18n.setLanguage('italian');const it=runtime.evaluate(guide,state);i18n.setLanguage('english');
 assert.deepEqual(it,en);assert.equal(JSON.stringify(guide),before);assert.equal(en.steps[0].key,'focus');
 assert.equal(guide.tracks.focus[0].action.entity.id,'GER_test');
});
