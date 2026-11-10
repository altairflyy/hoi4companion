'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),{spawn}=require('node:child_process'),{once}=require('node:events');
const {ROOT,json,write,temporary}=require('../scripts/native-tooling.cjs'),{zip,word,png}=require('./native-fixtures.cjs');
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
test('native service import contracts, evidence, collection persistence and campaign isolation',{skip:process.platform!=='win32',timeout:90000},async()=>{
 const stage=temporary('hoi4-native-contracts-'),root=stage.root,resources=path.join(ROOT,'src/hoi4companion');
 const config={path:path.join(root,'progress.sqlite3'),selection_path:path.join(root,'selection.json'),web_root:path.join(resources,'web'),reference_library:path.join(root,'references'),guide_directory:path.join(root,'guides'),guide_directories:[path.join(root,'guides')],association_directory:path.join(root,'associations'),shutdown_token:'a'.repeat(64),catalog:{settings:{game_dir:'',country:'AUS'},demo_file:path.join(resources,'examples/demo_focus.txt'),demo_labels:path.join(resources,'examples/demo_labels.json')},guides:[{id:'original',title:'Original',tracks:{operations:[{key:'note',action:{kind:'manual'}}]}}]};
 write(path.join(root,'config.json'),config);let child,port;
 async function stop(){if(child&&child.exitCode===null){const exited=once(child,'exit');child.kill();await exited;}child=null;}
 async function start(index){
  const ready=path.join(root,`ready-${index}.json`);child=spawn(process.env.HOI4_BACKEND_EXE||path.join(resources,'_native/hoi4-backend.exe'),['--config',path.join(root,'config.json'),'--port','0','--ready-file',ready],{cwd:ROOT,windowsHide:true,stdio:'ignore'});
  let spawnError;child.on('error',error=>spawnError=error);
  const deadline=Date.now()+20000;while(!fs.existsSync(ready)){if(spawnError)throw spawnError;if(child.exitCode!==null)throw Error('Native service exited: '+child.exitCode);if(Date.now()>deadline)throw Error('Native service startup timed out');await delay(25);}port=json(ready).port;
 }
 async function api(url,body,extra={}){
  const binary=Buffer.isBuffer(body),response=await fetch(`http://127.0.0.1:${port}${url}`,{method:body===undefined?'GET':'POST',headers:{'Content-Type':binary?'application/octet-stream':'application/json',...extra},body:body===undefined?undefined:binary?body:JSON.stringify(body),signal:AbortSignal.timeout(30000)});return {status:response.status,value:await response.json()};
 }
 async function prepared(id){const deadline=Date.now()+20000;while(true){const {value}=await api('/api/import/collection?id='+id);if(value.status!=='preparing')return value;if(Date.now()>deadline)throw Error('Collection preparation timed out');await delay(30);}}
 try{
  await start(1);const original=(await api('/api/guide/active')).value;
  // A saved preference is not the live nation. No game is configured here.
  const overlay=(await api('/api/overlay',{schema_version:1,kind:'snapshot',token:1})).value;
  assert.deepEqual(overlay.guides,[]);assert.equal(overlay.guide_filter.status,'unavailable');assert.equal(overlay.guide_filter.country,null);
  const waitingSnapshot=(await api('/api/overlay',{schema_version:1,kind:'observe',token:3,view:[1,1,0,0,1280,720]})).value;
  assert.equal(waitingSnapshot.usable,false);assert.deepEqual(waitingSnapshot.guides,[]);assert.equal(waitingSnapshot.guide_filter.status,'unavailable');
  assert.equal(waitingSnapshot.guide_id,'original');assert.equal(overlay.guide_id,'original');
  assert.equal((await api('/api/overlay',{schema_version:1,kind:'select',token:2,guide_id:'original'})).status,400);
  assert.deepEqual((await api('/api/guide/active')).value,original);
  assert.ok((await api('/api/guides')).value.guides.some(g=>g.id==='original'));
  const image=png(),templates=zip({'Templates/OFF.png':image,'Armor/designs.txt':'tank_designs={category=land equipment={name="Example" target_variant={type=medium modules={gun=cannon}}}}'});
  const refs=await api('/api/import/collections?name=templates.zip',templates);assert.equal(refs.status,200);const refsReady=await prepared(refs.value.id);assert.equal(refsReady.references.designs,1);assert.equal(refsReady.references.images,1);
  assert.equal((await api('/api/import/collection/commit',{id:refs.value.id,reviewed:false})).status,400);
  assert.equal((await api('/api/import/collection/commit',{id:refs.value.id,reviewed:true})).value.references_status,'installed');
  const payload=zip({'Country Guides/Ready.docx':word(['Create Agency']),'Country Guides/Unresolved.docx':word([],[],['Unknown National Focus']),'Country Guides/Tools/Abbreviations.docx':word()});
  assert.equal((await api('/api/import/collections',payload,{Origin:'https://external.example'})).status,403);
  const upload=await api('/api/import/collections?name=guides.zip',payload);assert.equal(upload.status,200);const id=upload.value.id,ready=await prepared(id);assert.equal(ready.counts.total,2);assert.equal(ready.counts.ready,1);assert.equal(ready.counts.needs_revision,1);
  const committed=await api('/api/import/collection/commit',{id,reviewed:true});assert.equal(committed.status,200);assert.equal(committed.value.counts.installed,1);assert.equal(committed.value.counts.needs_revision,1);assert.equal(committed.value.reviewed,true);
  assert.deepEqual((await api('/api/guide/active')).value,original);
  const filename=path.join(root,'guides',fs.readdirSync(path.join(root,'guides'))[0]),saved=fs.readFileSync(filename);assert.equal((await api('/api/import/collections?name=duplicate.zip',payload)).value.id,id);await api('/api/import/collection/commit',{id,reviewed:true});assert.deepEqual(fs.readFileSync(filename),saved);
  const pending=committed.value.items.find(row=>row.status==='needs_revision');assert.equal((await api(`/api/import/collection/item?id=${id}&key=${pending.key}`)).status,200);assert.equal((await api('/api/import/collection?id=../escape')).status,400);
  const source=(await api('/api/import/source',{text:'Prepare the front.'})).value;
  const guide={id:'evidence-check',title:'Evidence check',sources:[{id:source.id,type:'manual'}],tracks:{operations:[{key:'note',tracking:'reminder',action:{kind:'manual',params:{category:'army',instructions:['Prepare the front.']}},evidence:[{source_id:source.id,basis:'explicit',quote:'Prepare the front.'}]}]}};
  assert.equal((await api('/api/import/preview',{guide,sources:[source]})).value.valid,true);
  const cycle=structuredClone(guide);cycle.tracks.operations[0].depends_on=['note'];assert.equal((await api('/api/import/preview',{guide:cycle,sources:[source]})).value.valid,false);
  const badEvidence=structuredClone(guide);badEvidence.tracks.operations[0].evidence[0].source_id='missing';assert.equal((await api('/api/import/preview',{guide:badEvidence,sources:[source]})).value.valid,false);
  await stop();await start(2);const restored=(await api('/api/import/collections')).value.collections;assert.equal(restored.length,2);assert.equal(restored.find(row=>row.id===id).counts.installed,1);assert.equal((await api('/api/reference-library')).value.designs.length,1);assert.ok((await api('/api/guides')).value.guides.some(row=>row.id===JSON.parse(saved).id));
  assert.equal((await api('/api/shutdown',{token:'invalid'})).status,400);assert.equal((await api('/api/backend')).status,200);
  const exit=once(child,'exit');assert.equal((await api('/api/shutdown',{token:'a'.repeat(64)})).status,200);await exit;assert.equal(child.exitCode,0);
 }finally{await stop();stage.close();}
});
