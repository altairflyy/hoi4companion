'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
test('a slower initial tree cannot replace the newer guide-selected tree',async()=>{
 const code=fs.readFileSync('src/hoi4companion/web/explorer.js','utf8');
 const start=code.indexOf('async function loadTree('),end=code.indexOf('\nasync function reload(',start);
 const state={tree:{id:'previous',focuses:[]},treeRequest:0,selected:null},resolvers=[],rendered=[];
 const context={state,window:{hoiLive:{enabled:true}},getJson:()=>new Promise(resolve=>resolvers.push(resolve)),setManual(){},currentFocus:id=>state.tree.focuses.find(f=>f.id===id),render:()=>rendered.push(state.tree.id),camera:{overview(){}}};
 runLocalized(code.slice(start,end),context);
 const first=context.loadTree('initial'),second=context.loadTree('guided');
 resolvers[1]({id:'guided',focuses:[{id:'B'}]});await second;
 resolvers[0]({id:'initial',focuses:[{id:'A'}]});await first;
 assert.equal(state.tree.id,'guided');assert.equal(state.selected,'B');assert.deepEqual(rendered,['guided']);
});

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}

test('settings opened during startup share the pending config and cannot save empty paths',async()=>{
 const code=fs.readFileSync('src/hoi4companion/web/explorer.js','utf8');
 const input={type:'text',value:''},cancel={type:'button',disabled:false},save={type:'submit'};
 const fields=Object.assign([input,cancel,save],{namedItem:key=>key==='game_dir'?input:null});
 const dialog={open:false,showModal(){this.open=true;}};
 const nodes={settingsModal:dialog,settingsForm:{elements:fields},formError:{}};
 let resolve,requests=0;
 const context={state:{overview:null},$:id=>nodes[id],hideTooltip(){},getJson(){
  requests++;return new Promise(done=>{resolve=done;});
 }};
 const start=code.indexOf('let overviewRequest;'),end=code.indexOf('function init(){',start);
 runLocalized(code.slice(start,end),context);
 const initial=context.loadOverview(),opening=context.openSettings();
 assert.equal(requests,1);assert.equal(dialog.open,true);
 assert.equal(input.disabled,true);assert.equal(save.disabled,true);assert.equal(cancel.disabled,false);
 resolve({settings:{game_dir:'configured-game'}});
 await Promise.all([initial,opening]);
 assert.equal(input.value,'configured-game');assert.equal(save.disabled,false);
});
