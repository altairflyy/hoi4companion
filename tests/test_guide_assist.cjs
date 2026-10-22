'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const present=require('../src/hoi4companion/web/guide-presentation.js');
const guide={id:'g',title:'Fixture',applies_to:{country:'GER',tree_id:'g'},tracks:{focus:[
 {key:'f1',kind:'focus',target:'A',name:'A'},{key:'f2',kind:'focus',target:'B',name:'B'}],research:[
 {key:'r1',kind:'technology',target:'X',name:'X',href:'/technologies?folder=industry&tech=X'},
 {key:'r2',kind:'technology',target:'Y',name:'Y',href:'/technologies?folder=electronics&tech=Y'}]}};
function state(extra={}){return {usable:true,sample:{country:'GER',focus_tree_id:'g',completed_focuses:[],current_focus:null,researched_technologies:[],research_slots:[{technology_id:null}],...extra}};}
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
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide-assist.js','utf8'),context);
 return {window,host,node(id){const n=new Node();n.dataset[lane==='focus'?'focusId':'techId']=id;return n;},emit(live){const view=present.evaluate(guide,live,{checks:{}});window.hoiGuide.value={guide,view};listener({guide,view,catalog:[guide],progress:{checks:{}},pending:0});}};
}
test('focus DOM illumination advances on completion, distinguishes active, and rejects wrong scope/stale',()=>{
 const app=boot('focus'),a=app.node('A'),b=app.node('B');
 const paint=()=>app.window.hoiGuideTree.paint([a,b],'g');
 app.emit(state());paint();assert.ok(a.classes.has('guide-next'));assert.equal(a.dataset.guideStep,'f1');
 app.emit(state({completed_focuses:['A']}));paint();assert.equal(a.classes.has('guide-next'),false);assert.ok(b.classes.has('guide-next'));
 app.emit(state({current_focus:'A'}));paint();assert.ok(a.classes.has('guide-active'));assert.ok(b.classes.has('guide-upcoming'));
 app.window.hoiGuideTree.paint([a,b],'other');assert.equal(a.children.length,0);assert.equal(b.children.length,0);
 app.emit(state({country:'POL'}));paint();assert.equal(a.children.length,0);assert.equal(b.children.length,0);
 app.emit({usable:false,sample:state().sample});paint();assert.ok(a.classes.has('guide-preview'));assert.equal(a.classes.has('guide-next'),false);
});
test('technology DOM illumination honors slot capacity, and Show research resolves its folder',async()=>{
 const app=boot('research'),x=app.node('X'),y=app.node('Y');const paint=()=>app.window.hoiGuideTree.paint([x,y]);
 app.emit(state());paint();assert.ok(x.classes.has('guide-next'));assert.equal(y.children.length,0);
 app.emit(state({research_slots:[{technology_id:'X'},{technology_id:null}]}));paint();assert.ok(x.classes.has('guide-active'));assert.ok(y.classes.has('guide-next'));
 app.emit(state({research_slots:[{technology_id:'X'}]}));paint();assert.equal(y.classes.has('guide-next'),false);assert.ok(y.classes.has('guide-upcoming'));
 let selected;app.window.hoiGuideTree.connect(row=>{selected=row;});
 const button=app.host.children.flatMap(n=>n.children||[]).find(n=>n.tag==='button');await button.onclick();
 assert.equal(selected.target,'Y');assert.equal(present.folder(selected),'electronics');
});
