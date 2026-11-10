'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const model=require('../src/hoi4companion/web/collection-model.js');
test('recommendations require fresh country and tree; DLC never becomes a recommendation',()=>{
 const guide={applies_to:{country:'HUN',tree_id:'hungary'}};
 const packet={usable:true,sample:{country:'HUN',focus_tree_id:'hungary'}};
 assert.equal(model.recommended(guide,packet),true);
 for(const value of [null,{...packet,usable:false},{usable:true,sample:{country:'GER',focus_tree_id:'hungary'}},{usable:true,sample:{country:'HUN'}}])assert.equal(model.recommended(guide,value),false);
 assert.equal(model.recommended({...guide,compatibility:{blocked:true}},packet),false);
});
test('batch confirmation waits for preparation, permits revalidation and keeps DLC separate',()=>{
 assert.equal(model.canAdd({status:'preparing',counts:{ready:2}}),false);
 assert.equal(model.canAdd({status:'prepared',counts:{ready:2}}),true);
 assert.equal(model.canAdd({status:'prepared',counts:{needs_revision:1}}),true);
 assert.equal(model.canAdd({status:'prepared',counts:{dlc_unavailable:3}}),true);
 assert.equal(model.canAdd({status:'prepared',reviewed:true,counts:{dlc_unavailable:3,needs_revision:1}}),false);
 assert.equal(model.canAdd({status:'prepared',references_status:'ready'}),true);
});
test('reviewed unavailable guides stay gray in the saved library without duplicating installed IDs',()=>{
 const row={guide_id:'blocked',title:'Blocked',status:'dlc_unavailable',compatibility:{blocked:true}};
 assert.equal(model.library([],[{items:[row]}]).length,0);
 assert.equal(model.library([],[{reviewed:true,items:[row]}])[0].compatibility.blocked,true);
 assert.equal(model.library([{id:'blocked',title:'Already installed'}],[{reviewed:true,items:[row]}]).length,1);
});
function harness(){
 class Node{
  constructor(){this.children=[];this.value='';this.hidden=false;this.text='';this.classList={add(){}};this.events={};}
  append(...nodes){this.children.push(...nodes);}replaceChildren(...nodes){this.children=nodes;}
  set textContent(value){this.text=String(value);this.children=[];}get textContent(){return this.text+this.children.map(n=>n.textContent||'').join('');}
  setAttribute(key,value){this[key]=value;}removeAttribute(key){delete this[key];}addEventListener(key,fn){this.events[key]=fn;}
 }
 const nodes=new Map(),get=id=>{if(!nodes.has(id))nodes.set(id,new Node());return nodes.get(id);};
 const collection={id:'a'.repeat(64),status:'prepared',name:'Country guides.zip',counts:{total:2,processed:2,ready:1,needs_revision:1},items:[{key:'b'.repeat(64),guide_id:'pending',title:'Needs work',status:'needs_revision',issues:['Unresolved focus']} ]};
 let data=[],requests=[],fail=false,commits=0;
 const fetch=async(url,options)=>{
  requests.push({url,options});if(fail)throw Error('offline');
  let result={};
  if(url==='/api/import/collections')result={collections:data};
  else if(url.startsWith('/api/import/collections?')){data=[collection];result=collection;}
  else if(url==='/api/guides')result={guides:[]};
  else if(url==='/api/import/collection/commit'){commits++;collection.counts={...collection.counts,ready:0,installed:1};result=collection;}
  return {ok:true,json:async()=>result};
 };
 let subscribed;
 const document={hidden:false,getElementById:get,createElement:()=>new Node(),addEventListener(){}};
 const window={hoiLive:{subscribe(fn){subscribed=fn;}},hoiImportEditor:{initialize:async()=>{}}};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/collections.js','utf8'),{document,window,fetch,HoiCollections:model,HoiGuideAvailability:{notice:()=>''},HoiI18n:{t:(s,...args)=>s.replace(/\{(\d+)\}/g,(_,i)=>args[i])},setTimeout:()=>1,clearTimeout(){}});
 return {get,requests,collection,setData:v=>data=v,setFail:v=>fail=v,commits:()=>commits,subscribed};
}
const tick=()=>new Promise(setImmediate);
test('one ZIP prepares all guides; one confirmation retains unresolved rows and never selects a guide',async()=>{
 const h=harness();await tick();h.get('guideZipFiles').files=[{name:'country.zip',size:10}];await h.get('guideZipFiles').onchange();await tick();
 assert.match(h.get('collections').textContent,/Needs work/);
 assert.equal(h.get('collectionAdd').disabled,false);
 await h.get('collectionAdd').onclick();
 assert.equal(h.commits(),1);assert.match(h.get('collections').textContent,/Needs work/);
 assert.equal(h.requests.some(r=>r.url==='/api/guide/active'),false);
 assert.equal(h.requests.some(r=>r.url==='/api/import/catalog'),false);
});
test('template ZIP slot uses persistent preparation and waits for the shared review',async()=>{
 const h=harness();await tick();h.get('templateZipFiles').files=[{name:'templates.zip',size:20}];
 const uploading=h.get('templateZipFiles').onchange();
 assert.equal(h.get('guideZipFiles').disabled,true);assert.equal(h.get('templateZipFiles').disabled,true);
 await uploading;
 assert.equal(h.get('guideZipFiles').disabled,false);assert.equal(h.get('templateZipFiles').disabled,false);
 assert.ok(h.requests.some(r=>r.url==='/api/import/collections?name=templates.zip'&&r.options.body.name==='templates.zip'));
 assert.equal(h.commits(),0);assert.equal(h.requests.some(r=>r.url==='/api/import/templates'),false);
});
test('both dedicated drop zones accept ZIPs and share the existing collection workflow',async()=>{
 const h=harness();await tick();
 for(const [id,name] of [['guideZipDrop','guides.zip'],['templateZipDrop','templates.zip']]){
  let prevented=false;await h.get(id).events.drop({preventDefault(){prevented=true;},dataTransfer:{files:[{name,size:30}]}});
  assert.equal(prevented,true);assert.ok(h.requests.some(r=>r.url==='/api/import/collections?name='+name));
 }
 assert.equal(h.commits(),0);
});
test('two separately labelled ZIP inputs are present instead of a combined selector',()=>{
 const html=fs.readFileSync('src/hoi4companion/web/importer.html','utf8');
 for(const id of ['guideZipFiles','templateZipFiles']){
  assert.match(html,new RegExp('for="'+id+'"'));assert.match(html,new RegExp('<input id="'+id+'" type="file" accept=".zip,application/zip" multiple>'));
 }
 assert.doesNotMatch(html,/id="collectionFiles"/);
});
test('network failure is visible and the refresh action remains usable',async()=>{
 const h=harness();await tick();h.setFail(true);await h.get('collectionRefresh').onclick();
 assert.equal(h.get('collectionError').hidden,false);assert.match(h.get('collectionError').textContent,/offline/);
 assert.equal(h.get('collectionRefresh').disabled,false);
 h.setFail(false);await h.get('collectionRefresh').onclick();assert.equal(h.get('collectionError').hidden,true);
});
