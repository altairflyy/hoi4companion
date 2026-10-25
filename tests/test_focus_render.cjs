'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
test('focus view composes GUI art, preserves guide targets and replaces nodes when switching to a legacy tree',()=>{
 class Element{
  constructor(tag){this.tag=tag;this.children=[];this.dataset={};this.attributes={};this.style={setProperty(k,v){this[k]=v;}};this.classList={add(){},toggle(){}};}
  append(...children){this.children.push(...children);}replaceChildren(...children){this.children=children;}
  setAttribute(k,v){this.attributes[k]=v;}addEventListener(k,v){this['on'+k]=v;}
 }
 const elements=new Map(),get=id=>{if(!elements.has(id))elements.set(id,new Element('div'));return elements.get(id);};
 const asset=name=>({url:'/api/sprite/'+name+'.png',width:24,height:24});
 const fixture={id:'tree',focuses:['A','B'].map((id,i)=>({id,x:i*2,y:0,name:id,cost:10,prerequisites:[],mutually_exclusive:[i?'A':'B'],sprite:asset(id)})),
  visuals:{status:'observed',positions:{focus_spacing:{x:96,y:130},exclusive_offset:{x:172,y:24},exclusive_offset_left:{x:12,y:24}},
   states:{active:{...asset('current'),width:185,height:48}},templates:{
    national_focus_item:{size:{width:165,height:128},children:{
     symbol:{position:{x:5,y:-44},orientation:'CENTER',center:true,scale:1},
     bg:{position:{x:5,y:40},orientation:'UPPER_LEFT'},
     name:{position:{x:15,y:58},orientation:'UPPER_LEFT',width:147,height:20}}},
    national_focus_exclusive_item:{position:{x:-5,y:28},children:{mid:{sprite:asset('mid')},link1:{sprite:asset('line'),position:{x:16,y:10}},link2:{sprite:asset('line'),position:{x:16,y:10}}}}}}};
 let painted=0;
 const context={fixture,HoiFocusLayout:require('../src/hoi4companion/web/focus-layout.js'),
  window:{hoiLive:{enabled:true},hoiGuideTree:{paint(nodes,id){painted++;assert.equal(nodes.length,2);assert.equal(id,'tree');}}},
  document:{getElementById:get,createElement:tag=>new Element(tag),createElementNS:(ns,tag)=>new Element(tag),querySelectorAll:()=>get('nodes').children},
  fetch(){throw Error('Rendering must not request more data');}};
 const source=fs.readFileSync('src/hoi4companion/web/explorer.js','utf8');
 vm.runInNewContext(source.slice(0,source.lastIndexOf('init();')),context);
 vm.runInNewContext("state.tree=fixture; statuses=()=>({A:'active',B:'unknown'}); renderInspector=()=>{}; render();",context);
 let a=get('nodes').children[0];assert.equal(a.style.width,'165px');assert.equal(a.style.height,'128px');
 assert.equal(a.children[0].style.left,'87.5px');assert.equal(a.children[0].style.top,'20px');
 assert.equal(a.children[1].style.width,'185px');assert.equal(a.children[1].children[0].src,'/api/sprite/current.png');
 assert.ok(get('wires').children.some(n=>n.attributes.href==='/api/sprite/mid.png'));
 assert.match(get('assetInfo').textContent,/GUI locale/);assert.equal(painted,1);
 // Green texture frames must never make an unverified path look completed.
 fixture.focuses[1].y=1;fixture.focuses[1].prerequisites=[['A']];
 Object.assign(fixture.visuals.positions,{link_begin:{x:80,y:64},link_end:{x:80,y:0},link_spacing:{x:16,y:16}});
 fixture.visuals.templates.national_focus_link={position:{x:-2,y:0},size:{width:16,height:16}};
 fixture.visuals.link_states=Object.fromEntries(['neutral','completed'].map(state=>[state,
  Object.fromEntries(['up_down','left_right','up_right','down_left'].map(shape=>[shape,asset(state+'-'+shape)]))]));
 vm.runInNewContext('render();',context);
 assert.ok(get('wires').children.some(n=>n.attributes.class?.includes('focus-link-unverified')));
 assert.ok(get('wires').children.some(n=>n.attributes.href?.includes('neutral-')));
 vm.runInNewContext("statuses=()=>({A:'completed',B:'completed'});render();",context);
 assert.ok(get('wires').children.some(n=>n.attributes.href?.includes('completed-')));
 assert.equal(get('wires').children.some(n=>n.attributes.class?.includes('focus-link-unverified')),false);
 vm.runInNewContext('delete fixture.visuals; render();',context);
 a=get('nodes').children[0];assert.equal(a.style.width,'174px');assert.equal(painted,4);
 assert.match(get('assetInfo').textContent,/ripiego/);
});
