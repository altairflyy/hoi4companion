'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
test('a slower initial tree cannot replace the newer guide-selected tree',async()=>{
 const code=fs.readFileSync('src/hoi4companion/web/explorer.js','utf8');
 const start=code.indexOf('async function loadTree('),end=code.indexOf('\nasync function reload(',start);
 const state={tree:{id:'previous',focuses:[]},treeRequest:0,selected:null},resolvers=[],rendered=[];
 const context={state,window:{hoiLive:{enabled:true}},getJson:()=>new Promise(resolve=>resolvers.push(resolve)),setManual(){},currentFocus:id=>state.tree.focuses.find(f=>f.id===id),render:()=>rendered.push(state.tree.id),requestAnimationFrame:fn=>fn(),centerFocus(){},URLSearchParams,location:{search:''}};
 vm.runInNewContext(code.slice(start,end),context);
 const first=context.loadTree('initial'),second=context.loadTree('guided');
 resolvers[1]({id:'guided',focuses:[{id:'B'}]});await second;
 resolvers[0]({id:'initial',focuses:[{id:'A'}]});await first;
 assert.equal(state.tree.id,'guided');assert.equal(state.selected,'B');assert.deepEqual(rendered,['guided']);
});
