'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
function context(){const source=fs.readFileSync(path.join(__dirname,'../src/hoi4companion/web/technologies.js'),'utf8');const c=vm.createContext({document:{},console,HoiI18n:require('../src/hoi4companion/web/i18n.js')});vm.runInContext(source.slice(0,source.indexOf('let request=0;')),c);return c;}
test('cornered research regions draw one enclosing frame instead of a tiled grid',()=>{
 const c=context(),children=[];
 c.document={createElement:()=>({style:{}}),getElementById:()=>({replaceChildren:()=>children.splice(0),append:n=>children.push(n)})};
 vm.runInContext(`state.tree={visuals:{layers:[{width:1340,height:550,sprite:{url:'/frame.png',kind:'corneredtilespritetype',border:{x:32,y:32}},repeat:true},{width:100,height:200,sprite:{url:'/stripes.png',kind:'corneredtilespritetype',border:{x:0,y:0}},repeat:true}]}};renderBackdrop();`,c);
 assert.equal(children[0].style.borderImageSlice,'32 32 fill');assert.equal(children[0].style.backgroundRepeat,undefined);
 assert.equal(children[0].style.width,'1340px');assert.equal(children[1].style.backgroundRepeat,'repeat');
});
test('native geometry is not expanded and unmapped catalog data stays out of the game sheet',()=>{
 const c=context();vm.runInContext(`state.tree={id:'f',layout:{captions:[]},technologies:[
  {id:'A',name:'A',start_year:1936,in_folder:true,folders:[{id:'f',layout:{x:100,y:200,direction:'UP'}}],paths:[]},
  {id:'B',name:'B',start_year:1937,in_folder:true,folders:[{id:'f',layout:{x:100,y:340,direction:'UP'}}],paths:[]},
  {id:'C',name:'C',start_year:1938,in_folder:true,folders:[{id:'f',x:0}],paths:[]}]};layout(sectionNodes());`,c);
 const coords=JSON.parse(vm.runInContext('JSON.stringify(Object.fromEntries(state.coords))',c));
 assert.equal(coords.B.x,coords.A.x);assert.equal(coords.B.y-coords.A.y,140);assert.equal(coords.A.x,100);assert.equal(coords.A.y,200);assert.equal(coords.C,undefined);
});

test('connectors use each native rectangle and keep the game orthogonal routing',()=>{
 const c=context();
 const path=vm.runInContext("wirePath({x:10,y:20,width:183,height:84},{x:400,y:20,width:72,height:72},'LEFT')",c);
 assert.ok(path.startsWith('M193,62 H'));assert.ok(path.endsWith(' H400'));assert.ok(!path.includes(' C'));
 const vertical=vm.runInContext("wirePath({x:10,y:20,width:72,height:72},{x:10,y:200,width:72,height:72},'UP')",c);
 assert.ok(vertical.startsWith('M46,92 V'));assert.ok(vertical.endsWith(' V200'));
});
test('branch selection keeps only matching declared GUI group and exposes fallback',()=>{
 const c=context();vm.runInContext(`state.tree={id:'f',technologies:[
 {id:'A',in_folder:true,folders:[{id:'f',layout:{section_id:'root'}}]},
 {id:'B',in_folder:true,folders:[{id:'f'}]},
 {id:'child',in_folder:false,folders:[]}]};state.section='root';`,c);
 assert.equal(vm.runInContext('sectionNodes().map(t=>t.id).join()',c),'A');
 vm.runInContext("state.section='unmapped'",c);assert.equal(vm.runInContext('sectionNodes().map(t=>t.id).join()',c),'B');
 vm.runInContext("state.section='all'",c);assert.equal(vm.runInContext('sectionNodes().map(t=>t.id).join()',c),'A,B');
});
