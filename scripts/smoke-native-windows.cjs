'use strict';
const fs=require('node:fs'),path=require('node:path'),net=require('node:net'),assert=require('node:assert/strict');
const {ROOT,sha,json,write,run,temporary,options,archive}=require('./native-tooling.cjs');
async function freePort(){const server=net.createServer();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const port=server.address().port;await new Promise(resolve=>server.close(resolve));return port;}
async function smoke(input,output){
 const file=path.resolve(input),stage=temporary('hoi4-native-smoke-'),result={schema_version:1,archive:file,sha256:sha(file),game_started:false,live_verified:false,hud_visual_alignment_verified:false};
 let running=false,launch;
 try{
  archive('Extract',file,path.join(stage.root,'extracted'));const app=path.join(stage.root,'extracted/HOI4Companion'),manifest=json(path.join(app,'manifest.json'));
  for(const [name,expected] of Object.entries(manifest.files)){assert.equal(sha(path.join(app,name)),expected,'Package hash: '+name);assert.ok(!/\.(?:py|pyc|pyd)$|python(?:w|3|313)?\.(?:exe|dll)$/i.test(name));}
  result.manifest_files_verified=Object.keys(manifest.files).length;result.resident_python=manifest.resident_python;assert.equal(result.resident_python,false);
  const data=path.join(stage.root,'profile');fs.mkdirSync(data);write(path.join(data,'settings.json'),{game_dir:path.join(data,'unconfigured'),docs_dir:path.join(data,'documents'),ui_language:'english'});
  const env=Object.fromEntries(Object.entries(process.env).filter(([key])=>! /^(?:HOI4|PYTHON)/i.test(key)));env.PATH=path.join(process.env.SystemRoot,'System32');env.HOI4COMPANION_DATA_DIR=data;
  const executable=path.join(app,'HOI4 Companion.exe'),hud=path.join(app,'resources/_native/overlay/hoi4-overlay.exe');
  const norm=p=>{try{return fs.realpathSync(p).toLowerCase();}catch{const d=path.dirname(p),b=path.basename(p);return (fs.existsSync(d)?path.join(fs.realpathSync(d),b):path.resolve(p)).toLowerCase();}};
  const call=(exe,args)=>run(exe,args,{cwd:app,env,timeout:60000});const checked=JSON.parse(call(executable,['--self-check']));assert.equal(norm(checked.configuration.path),norm(path.join(data,'guide-progress.sqlite3')));assert.ok(norm(checked.configuration.ffmpeg).startsWith(norm(app)+path.sep));result.startup_self_check=true;
  const sourceFile=path.join(data,'export-source.txt'),exported=path.join(data,'export.json');fs.writeFileSync(sourceFile,Buffer.from('name="café"','latin1'));
  call(path.join(app,'resources/_native/hoi4-backend.exe'),['--export-script',sourceFile,'--script-encoding','windows-1252','--json-duplicates','group','--output',exported]);assert.equal(json(exported).name,'café');result.native_jomini_export_verified=true;
  const port=await freePort();launch=(...extra)=>JSON.parse(call(executable,['--port',String(port),'--no-browser','--no-hud',...extra]));
  const api=async(url,body)=>{const response=await fetch(`http://127.0.0.1:${port}${url}`,{method:body===undefined?'GET':'POST',headers:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body),signal:AbortSignal.timeout(30000)});const text=await response.text();assert.equal(response.status,200,url+': '+text.slice(0,500));return response.headers.get('content-type').includes('application/json')?JSON.parse(text):text;};
  launch();running=true;assert.equal((await api('/api/backend')).resident_python,false);assert.deepEqual((await api('/api/guides')).guides,[]);result.clean_install_has_no_personal_guides=true;
  for(const page of ['/','/walkthrough','/importer','/technologies','/analysis','/associations'])assert.match(await api(page),/<html/i);result.web_routes_served=6;
  assert.match((await api('/api/import/prompt')).prompt,/hoi4-guide-authoring/);
  const source=await api('/api/import/source',{text:'Prepare the front.'});
  const guide={id:'distribution-proof',title:'Distribution proof',sources:[{id:source.id,type:'manual'}],tracks:{operations:[{key:'note',tracking:'reminder',action:{kind:'manual',params:{category:'army',instructions:['Prepare the front.']}},evidence:[{source_id:source.id,basis:'explicit',quote:source.text}]}]}};
  const preview=await api('/api/import/preview',{guide,sources:[source]});assert.equal(preview.valid,true,JSON.stringify(preview.errors));assert.equal((await api('/api/import/install',{guide,sources:[source],reviewed:true})).id,guide.id);
  await api('/api/guide/active',{guide_id:guide.id});assert.equal((await api('/api/guide/projection?live=0')).guide_id,guide.id);result.guide_preview_install_selection_verified=true;
  const native=JSON.parse(call(hud,['--native-protocol-check','--port',String(port)]));assert.equal(native.resident_python,false);assert.ok(native.binary_frame_verified&&native.frame_cache_verified);assert.equal(native.snapshot.guide_id,guide.id);result.native_hud_protocol_and_binary_frames_verified=true;
  const analysis=await api('/api/script-analysis');assert.equal(analysis.demo,true);assert.equal(analysis.status,'partial');assert.equal(analysis.rules.adapter,'cwtools-subset-v2');assert.equal(analysis.runtime_evaluated,false);assert.ok(analysis.coverage.documents>0&&Array.isArray(analysis.diagnostics));result.script_analysis_endpoint_verified=true;
  launch('--stop');running=false;await new Promise(resolve=>setTimeout(resolve,250));launch();running=true;assert.equal((await api('/api/guide/active')).guide_id,guide.id);result.installed_guide_survives_restart=true;
  result.owned_stop_verified=launch('--stop').stopping===true;running=false;result.external_youtube_download_verified=false;result.provider_api_called=false;
 }finally{if(running)launch('--stop');stage.close();}
 write(output,result);return result;
}
if(require.main===module){const args=options();const version=json(path.join(ROOT,'package.json')).version;smoke(args._[0]||path.join(ROOT,`dist/HOI4Companion-${version}-native-windows-x64.zip`),args.output||path.join(ROOT,'.local/native-windows-smoke-proof.json')).then(result=>console.log(JSON.stringify(result,null,2))).catch(e=>{console.error(e.stack);process.exitCode=1;});}
module.exports={smoke};
