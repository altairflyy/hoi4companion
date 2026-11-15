'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const present=require('../src/hoi4companion/web/guide-presentation.js');
const fixture=require('./presentation-fixtures.cjs');
const guide={id:'g',title:'Fixture',applies_to:{country:'GER',tree_id:'g'},tracks:{focus:[
 {key:'f1',kind:'focus',target:'A',name:'A'},{key:'f2',kind:'focus',target:'B',name:'B'}],research:[
 {key:'r1',kind:'technology',target:'X',name:'X',href:'/technologies?folder=industry&tech=X'},
 {key:'r2',kind:'technology',target:'Y',name:'Y',href:'/technologies?folder=electronics&tech=Y'}]}};
function boot(lane){
 class Node{
  constructor(tag){this.tag=tag;this.children=[];this.dataset={};this.attrs={};this.classes=new Set();this.classList={toggle:(c,v)=>v?this.classes.add(c):this.classes.delete(c),add:c=>this.classes.add(c)};}
  set className(value){this.classes=new Set(value.split(' '));}get className(){return [...this.classes].join(' ');}
  append(...nodes){for(const n of nodes){n.parent=this;this.children.push(n);}}
  replaceChildren(...nodes){this.children=[];this.append(...nodes);}
  setAttribute(key,value){this.attrs[key]=value;}getAttribute(key){return this.attrs[key]||null;}
  addEventListener(type,fn){this['on'+type]=fn;}
  querySelector(selector){return this.children.find(n=>n.classes?.has(selector.slice(1)))||null;}
  remove(){this.parent.children=this.parent.children.filter(n=>n!==this);}
 }
 const host=new Node();host.dataset.lane=lane;let listener;
 const window={dispatchEvent(){},hoiGuide:{subscribe(fn){listener=fn;},withGuide:x=>x}};
 const context={window,document:{getElementById:()=>host,createElement:tag=>new Node(tag),querySelectorAll:()=>[]},CustomEvent:class{},HoiContracts:require('../src/hoi4companion/web/contracts.js')};
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide-assist.js','utf8'),context);
 return {window,host,node(id){const n=new Node();n.dataset[lane==='focus'?'focusId':'techId']=id;return n;},emit(name){const {state:live,progress,evaluation}=fixture('assist: '+name),view=present.evaluate(guide,live,{progress},evaluation);window.hoiGuide.value={guide,view};listener({guide,view,catalog:[guide],progress:{checks:{}},pending:0});}};
}
test('focus DOM illumination advances on completion, distinguishes active, and rejects wrong scope/stale',()=>{
 const app=boot('focus'),a=app.node('A'),b=app.node('B');
 const paint=()=>app.window.hoiGuideTree.paint([a,b],'g');
 app.emit('start');paint();assert.ok(a.classes.has('guide-next'));assert.equal(a.dataset.guideStep,'f1');
 app.emit('focus completed');paint();assert.equal(a.classes.has('guide-next'),false);assert.ok(b.classes.has('guide-next'));
 app.emit('focus active');paint();assert.ok(a.classes.has('guide-active'));assert.ok(b.classes.has('guide-upcoming'));
 app.window.hoiGuideTree.paint([a,b],'other');assert.equal(a.children.length,0);assert.equal(b.children.length,0);
 app.emit('wrong country');paint();assert.equal(a.children.length,0);assert.equal(b.children.length,0);
 app.emit('stale');paint();assert.ok(a.classes.has('guide-preview'));assert.equal(a.classes.has('guide-next'),false);
});
test('technology DOM illumination honors slot capacity, and Show research resolves its folder',async()=>{
 const app=boot('research'),x=app.node('X'),y=app.node('Y');const paint=()=>app.window.hoiGuideTree.paint([x,y]);
 app.emit('start');paint();assert.ok(x.classes.has('guide-next'));assert.equal(y.children.length,0);
 app.emit('research active with free slot');paint();assert.ok(x.classes.has('guide-active'));assert.ok(y.classes.has('guide-next'));
 app.emit('research slots busy');paint();assert.equal(y.classes.has('guide-next'),false);assert.ok(y.classes.has('guide-upcoming'));
 let selected;app.window.hoiGuideTree.connect(row=>{selected=row;});
 const button=app.host.children.flatMap(n=>n.children||[]).find(n=>n.tag==='button');await button.onclick();
 assert.equal(selected.target,'Y');assert.equal(present.folder(selected),'electronics');
});

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}
