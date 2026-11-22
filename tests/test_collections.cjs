'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const model=require('../src/hoi4companion/web/collection-model.js');
const importer=require('../src/hoi4companion/web/importer-model.js');
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

function harness({initial=[],view='import'}={}){
 class Node{
  constructor(){this.children=[];this.value='';this.hidden=false;this.text='';this.classList={add(){}};this.events={};}
  append(...nodes){this.children.push(...nodes);}replaceChildren(...nodes){this.children=nodes;}
  set textContent(v){this.text=String(v);this.children=[];}get textContent(){return this.text+this.children.map(n=>n.textContent||'').join('');}
  setAttribute(k,v){this[k]=v;}removeAttribute(k){delete this[k];}addEventListener(k,fn){this.events[k]=fn;}
 }
 const nodes=new Map(),get=id=>{if(!nodes.has(id))nodes.set(id,new Node());return nodes.get(id);};
 const collection={id:'a'.repeat(64),status:'prepared',auto_import:true,auto_finished:true,import_stage:'complete',reviewed:true,name:'Country guides.zip',counts:{total:2,processed:2,installed:1,needs_revision:1},items:[{key:'b'.repeat(64),guide_id:'pending',title:'Needs work',status:'needs_revision',issues:['Unresolved focus']},{title:'Already saved',guide_id:'saved',status:'installed',interpretation:{verification:{automatic:2,manual:1,uninterpreted:1,reminder:0},first_uninterpreted:'unreadable'}}]};
 let data=initial,requests=[],fail=false,failCommit=false,commits=0,timer,subscribed;const events={};
 const fetch=async(url,options)=>{requests.push({url,options});if(fail)throw Error('offline');let result={};
  if(url==='/api/import/collections')result={collections:data};
  else if(url.startsWith('/api/import/collections?')){data=[collection];result=collection;}
  else if(url==='/api/guides')result={guides:[{id:'saved',title:'A saved guide',applies_to:{country:'HUN'}}]};
  else if(url==='/api/import/collection/commit'){commits++;if(failCommit)throw Error('publication interrupted');const c=data.find(c=>c.id===JSON.parse(options.body).id);Object.assign(c,{reviewed:true,auto_finished:true,import_stage:'complete',counts:{...c.counts,ready:0,installed:1}});result=c;}
  return {ok:true,json:async()=>result};};
 const document={hidden:false,getElementById:get,createElement:()=>new Node(),addEventListener(k,fn){events[k]=fn;}};
 const window={hoiLive:{subscribe(fn){subscribed=fn;}},hoiImportEditor:{initialize:async()=>{},refreshReferences:async()=>{get('referenceGallery').open=true;}}};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/collections.js','utf8'),{document,window,URL,location:{href:'http://local/importer?view='+view},fetch,HoiCollections:model,HoiImporter:importer,HoiGuideAvailability:{notice:()=>''},HoiI18n:{t:(s,...args)=>s.replace(/\{(\d+)\}/g,(_,i)=>args[i])},setTimeout:fn=>{timer=fn;return 1;},clearTimeout(){timer=null;}});
 return {get,requests,collection,events,setData:v=>data=v,setFail:v=>fail=v,setFailCommit:v=>failCommit=v,commits:()=>commits,poll:()=>timer?.(),subscribed};
}
const tick=()=>new Promise(setImmediate);

test('ZIP import completes without a publication button; only real corrections are listed',async()=>{
 const h=harness();await tick();h.get('guideZipFiles').files=[{name:'country.zip',size:10}];await h.get('guideZipFiles').onchange();
 assert.equal(h.commits(),0);assert.match(h.get('collectionJobs').textContent,/Import complete|items to correct/);
 const [heading,report,progress]=h.get('collectionJobs').children[0].children;
 assert.equal(heading.textContent,'Country guides.zip');
 assert.match(report.textContent,/2 steps can be checked automatically, 1 to confirm, 1 not interpreted/);
 assert.equal(report.children[2].href,'/associations?guide=saved&step=unreadable');
 assert.match(progress.textContent,/Read archive/);
 assert.doesNotMatch(h.get('collectionJobs').textContent,/No further confirmation/);
 assert.match(h.get('collections').textContent,/Needs work/);assert.doesNotMatch(h.get('collections').textContent,/Already saved/);
 assert.equal(h.get('collectionLibrary').hidden,true);assert.equal(h.get('collectionTemplates').hidden,true);
 assert.equal(h.requests.some(r=>r.url==='/api/guide/active'||r.url==='/api/import/catalog'),false);
 assert.equal(h.get('guideZipFiles').value,'');
});
test('all three ZIP slots request durable automatic processing, without MIO installation clicks',async()=>{
 const h=harness();await tick();
 for(const [id,name] of [['guideZipFiles','guides.zip'],['templateZipFiles','templates.zip'],['advancedZipFiles','advanced.zip']]){
  h.get(id).files=[{name,size:20}];const upload=h.get(id).onchange();assert.equal(h.get(id).disabled,true);await upload;assert.equal(h.get(id).disabled,false);
  assert.ok(h.requests.some(r=>r.url==='/api/import/collections?automatic=1&name='+name));
 }assert.equal(h.commits(),0);assert.equal(h.requests.some(r=>r.url==='/api/reference-library/mio-install'),false);
});
test('drop zones use the same automatic archive flow',async()=>{
 const h=harness();await tick();for(const [id,name] of [['guideZipDrop','g.zip'],['templateZipDrop','t.zip'],['advancedZipDrop','a.zip']]){
  let prevented=false;await h.get(id).events.drop({preventDefault(){prevented=true;},dataTransfer:{files:[{name,size:30}]}});assert.equal(prevented,true);
  assert.ok(h.requests.some(r=>r.url==='/api/import/collections?automatic=1&name='+name));
 }
});
test('prepared legacy jobs are finished once with automatic validation, not a claimed human review',async()=>{
 const old={id:'old',name:'Old.zip',status:'prepared',counts:{ready:1}};const h=harness({initial:[old]});await tick();
 assert.equal(h.commits(),1);assert.equal(JSON.parse(h.requests.find(r=>r.url==='/api/import/collection/commit').options.body).automatic,true);
 await h.events.visibilitychange();assert.equal(h.commits(),1);
});
test('progress follows server stages; MIO game-exit wait is not an error or a second import',async()=>{
 const c={id:'pending',name:'MIO.zip',auto_import:true,status:'preparing',import_stage:'validating',counts:{total:3,processed:1}};
 const h=harness({initial:[c]});await tick();assert.match(h.get('collectionJobs').textContent,/1 \/ 3 guides checked/);
 Object.assign(c,{status:'prepared',auto_finished:true,import_stage:'complete',references:{mio_queues:2},mio_install:{status:'waiting_for_game_exit'}});
 await h.poll();assert.match(h.get('collectionJobs').textContent,/Scheduled automatically when HOI4 closes/);assert.equal(h.get('collectionReview').hidden,true);assert.equal(h.commits(),0);
});
test('DLC requirements alone never open a revision panel',async()=>{
 const h=harness({initial:[{id:'dlc',status:'prepared',reviewed:true,auto_finished:true,counts:{dlc_unavailable:3},items:[{status:'dlc_unavailable'}]}]});await tick();assert.equal(h.get('collectionReview').hidden,true);
});
test('saved library and reference views are separate from import, retaining their existing routes',async()=>{
 const h=harness({view:'library'});await tick();assert.equal(h.get('collectionLibrary').hidden,false);assert.equal(h.get('collectionUpload').hidden,true);assert.match(h.get('collectionGuides').textContent,/A saved guide/);
 const r=harness({view:'references'});await tick();assert.equal(r.get('collectionTemplates').hidden,false);assert.equal(r.get('referenceGallery').open,true);
});
test('network failure is visible and a later page refresh can recover',async()=>{
 const h=harness();await tick();h.setFail(true);await h.events.visibilitychange();assert.equal(h.get('collectionError').hidden,false);assert.match(h.get('collectionError').textContent,/offline/);
 assert.equal(h.get('guideZipFiles').disabled,false);h.setFail(false);await h.events.visibilitychange();assert.equal(h.get('collectionError').hidden,true);
});

test('interrupted legacy publication does not remain stuck saving and can be retried',async()=>{
 const h=harness();await tick();const old={id:'legacy',name:'Old.zip',status:'prepared',counts:{ready:1}};
 h.setData([old]);h.setFailCommit(true);await h.events.visibilitychange();assert.equal(h.get('collectionError').hidden,false);assert.equal(model.pending(old),false);assert.equal(h.get('guideZipFiles').disabled,false);
 h.setFailCommit(false);await h.events.visibilitychange();assert.equal(h.commits(),2);assert.equal(old.auto_finished,true);assert.equal(h.get('collectionError').hidden,true);
});

test('invalid ZIP selections fail locally without uploading and clear the chooser',async()=>{
 const h=harness();await tick();for(const file of [{name:'guide.docx',size:10},{name:'empty.zip',size:0},{name:'large.zip',size:200000001}]){
  h.get('guideZipFiles').files=[file];await h.get('guideZipFiles').onchange();assert.equal(h.get('collectionError').hidden,false);assert.equal(h.get('guideZipFiles').value,'');
 }assert.equal(h.requests.some(r=>r.options?.method==='POST'),false);
});
test('markup has separate ZIP slots but no duplicate review, refresh or publish buttons',()=>{
 const html=fs.readFileSync('src/hoi4companion/web/importer.html','utf8');for(const id of ['guideZipFiles','templateZipFiles','advancedZipFiles'])assert.match(html,new RegExp('for="'+id+'"'));
 assert.doesNotMatch(html,/id="(?:collectionAdd|collectionRefresh|stageUpload|stageReview|stageLibrary)"/);assert.match(html,/id="collectionProgress"[^>]*hidden/);
});
