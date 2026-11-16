'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {spawn}=require('node:child_process'),{once}=require('node:events');
const {ROOT,json,write,temporary}=require('../scripts/native-tooling.cjs');
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));

test('HUD alone maintains shared observations, releases demand, and keeps missing game facts unknown',
 {skip:process.platform!=='win32',timeout:40000},async()=>{
 const stage=temporary('hoi4-hud-demand-'),resources=path.join(ROOT,'src/hoi4companion');
 const config=path.join(stage.root,'config.json'),ready=path.join(stage.root,'ready.json');
 write(config,{path:path.join(stage.root,'progress.sqlite3'),selection_path:path.join(stage.root,'selection.json'),
  web_root:path.join(resources,'web'),reference_library:path.join(stage.root,'references'),
  guide_directory:path.join(stage.root,'guides'),association_directory:path.join(stage.root,'associations'),
  catalog:{settings:{game_dir:'',country:'AUS'},demo_file:path.join(resources,'examples/demo_focus.txt'),demo_labels:path.join(resources,'examples/demo_labels.json')},
  guides:[{id:'hud-only',title:'HUD only',tracks:{operations:[{key:'note',action:{kind:'manual'}}]}}]});
 const child=spawn(process.env.HOI4_BACKEND_EXE||path.join(resources,'_native/hoi4-backend.exe'),
  ['--config',config,'--port','0','--ready-file',ready],{cwd:ROOT,windowsHide:true,stdio:'ignore'});
 let spawnError;child.on('error',error=>spawnError=error);
 try{
  const deadline=Date.now()+20000;
  while(!fs.existsSync(ready)){
   if(spawnError)throw spawnError;if(child.exitCode!==null)throw Error('Backend exited: '+child.exitCode);
   if(Date.now()>deadline)throw Error('Backend startup timed out');await delay(25);
  }
  const url=`http://127.0.0.1:${json(ready).port}/api/overlay`;let token=0;
  async function snapshot(){
   const response=await fetch(url,{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({schema_version:1,kind:'snapshot',token:++token}),signal:AbortSignal.timeout(5000)});
   assert.equal(response.status,200);return response.json();
  }
  // No browser/SSE connection or explicit state/projection request is opened.
  let packet;
  for(let i=0;i<12;i++){
   packet=await snapshot();
   if(i>=4){assert.ok(Number.isFinite(packet.state_age_ms)&&packet.state_age_ms<2000,'HUD delivery must stay fresh without SSE');}
   assert.equal(packet.usable,false);assert.deepEqual(packet.proposed,[]);assert.deepEqual(packet.guides,[]);
   await delay(300);
  }
  // Both the demand lease and the last observation budget expire.
  await delay(4250);
  packet=await snapshot();assert.equal(packet.live_delivery_fresh,false);assert.equal(packet.usable,false);
  await delay(1200);
  packet=await snapshot();assert.ok(Number.isFinite(packet.state_age_ms)&&packet.state_age_ms<2000,'HUD must resume the shared reader');
 }finally{
  if(child.exitCode===null){const ended=once(child,'exit');child.kill();await ended;}
  stage.close();
 }
});
