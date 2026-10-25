'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const geometry=require('../src/hoi4companion/web/focus-layout.js');
function visuals(){return {status:'observed',positions:{focus_spacing:{x:96,y:130},link_spacing:{x:16,y:16},
 link_begin:{x:80,y:64},link_end:{x:80,y:0},exclusive_offset:{x:172,y:24},exclusive_offset_left:{x:12,y:24}},
 templates:{national_focus_item:{size:{width:165,height:128}},national_focus_link:{position:{x:-2,y:0},size:{width:16,height:16}},
 national_focus_exclusive_item:{position:{x:-5,y:28},children:{}}}};}
const tree=()=>({visuals:visuals(),focuses:[{id:'A',x:-2,y:0},{id:'B',x:0,y:1}]});
test('GUI metrics replace fixed dimensions and preserve relative focus coordinates',()=>{
 const input=tree(),before=JSON.stringify(input),g=geometry.layout(input);
 assert.equal(g.native,true);assert.deepEqual(g.node,{width:165,height:128});
 assert.deepEqual(g.coords.get('A'),{x:74,y:74});assert.deepEqual(g.coords.get('B'),{x:266,y:204});
 assert.equal(JSON.stringify(input),before);assert.deepEqual(geometry.layout(input),g);
});
test('missing GUI uses explicit legacy geometry; invalid coordinates cannot poison the canvas',()=>{
 const input=tree();delete input.visuals;input.focuses.push({id:'invalid',x:Infinity,y:0});
 const g=geometry.layout(input);assert.equal(g.native,false);assert.equal(g.coords.has('invalid'),false);
 assert.deepEqual(g.coords.get('B'),{x:274,y:264});assert.ok(Number.isFinite(g.width));
 const empty=geometry.layout({focuses:[]});assert.equal(empty.width,500);assert.equal(empty.height,410);
});
test('centered icon and text use independent GUI anchors without a focus-specific offset',()=>{
 const node={width:165,height:128};
 assert.deepEqual(geometry.component({position:{x:5,y:-44},orientation:'CENTER',center:true,scale:1},node),
  {x:87.5,y:20,center:true,scale:1});
 assert.equal(geometry.component({position:{x:0,y:0},orientation:'UNSUPPORTED'},node),null);
});
test('links terminate on GUI anchors and preserve upward, vertical and branching connections',()=>{
 const t=tree(),g=geometry.layout(t),a=g.coords.get('A'),b=g.coords.get('B');
 const link=geometry.edge(a,b,g,t.visuals);
 assert.deepEqual(link.points[0],{x:a.x+86,y:a.y+72});
 assert.deepEqual(link.points.at(-1),{x:b.x+86,y:b.y+8});
 assert.equal(geometry.edge(a,{x:a.x,y:a.y+130},g,t.visuals).points.length,2);
 assert.ok(geometry.edge(b,a,g,t.visuals).points.every(p=>Number.isFinite(p.x)&&Number.isFinite(p.y)));
});
test('original corner sprites retain size, missing resources use the complete SVG fallback',()=>{
 const assets=Object.fromEntries(['up_down','left_right','up_right','down_left'].map(k=>[k,{url:'/sprite/'+k}]));
 const points=[{x:0,y:0},{x:0,y:40},{x:80,y:40},{x:80,y:80}];
 const pieces=geometry.tiles(points,assets,{x:16,y:16});assert.equal(pieces.length,5);
 assert.equal(pieces[3].width,16);assert.equal(pieces[4].height,16);
 delete assets.up_right;assert.deepEqual(geometry.tiles(points,assets,{x:16,y:16}),[]);
 assert.deepEqual(geometry.tiles([{x:0,y:0},{x:5,y:5}],assets,{x:16,y:16}),[]);
});
test('exclusive pairs use GUI ports in either order and do not invent horizontal links for diagonal pairs',()=>{
 const t=tree(),g=geometry.layout(t),a={x:74,y:74},b={x:266,y:74};
 const link=geometry.exclusive(a,b,g,t.visuals);assert.equal(link.native,true);
 assert.deepEqual(link.start,{x:241,y:126});assert.deepEqual(link.end,{x:273,y:126});
 assert.deepEqual(geometry.exclusive(b,a,g,t.visuals),link);
 assert.equal(geometry.exclusive(a,{...b,y:204},g,t.visuals).native,false);
});
