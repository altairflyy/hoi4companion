'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),net=require('node:net'),http=require('node:http'),{spawn,execFileSync}=require('node:child_process');
const {ROOT,temporary,write}=require('../scripts/native-tooling.cjs');
const enabled=process.platform==='win32'&&process.env.HOI4_DESKTOP_LIFECYCLE==='1';
const resources=path.join(ROOT,'src/hoi4companion'),host=process.env.HOI4_DESKTOP_EXE||path.join(resources,'_native/hoi4-desktop.exe'),backend=process.env.HOI4_BACKEND_EXE||path.join(resources,'_native/hoi4-backend.exe');
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn,ms=30000){const end=Date.now()+ms;for(;;){const value=await fn();if(value)return value;if(Date.now()>end)throw Error('Timed out waiting for desktop lifecycle');await delay(50);}}
function launch(args){const child=spawn(host,args,{cwd:ROOT,env:{...process.env,HOI4_BACKEND_EXE:backend},windowsHide:true});let stdout='',stderr='';child.stdout.on('data',b=>stdout+=b);child.stderr.on('data',b=>stderr+=b);child.on('error',e=>stderr+=String(e));return {child,get stdout(){return stdout},get stderr(){return stderr}};}
async function exit(p,ms=30000){await until(()=>p.child.exitCode!==null||p.child.signalCode!==null,ms);return p.child.exitCode;}
async function freePort(){const s=net.createServer();await new Promise((r,j)=>s.listen(0,'127.0.0.1',r).on('error',j));const port=s.address().port;await new Promise(r=>s.close(r));return port;}
async function request(port,route,body){const r=await fetch(`http://127.0.0.1:${port}${route}`,{method:body===undefined?'GET':'POST',headers:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body),signal:AbortSignal.timeout(5000)});return {status:r.status,value:await r.json()};}
async function closed(port){try{await request(port,'/api/backend');return false}catch{return true}}
test('isolated desktop headless lifecycle and foreign port protection',{skip:!enabled,timeout:150000},async()=>{
 const tmp=temporary('hoi4-desktop-lifecycle-'),data=tmp.root,port=await freePort(),owned=[];
 write(path.join(data,'settings.json'),{game_dir:path.join(data,'missing-game'),docs_dir:path.join(data,'missing-docs'),ui_language:'english'});
 const args=['--resources',resources,'--data-dir',data,'--port',String(port),'--no-browser','--no-hud'];
 const run=extra=>{const p=launch([...args,...extra]);owned.push(p);return p;};
 try{
  let p=run(['--self-check']);assert.equal(await exit(p),0,p.stderr);assert.equal(JSON.parse(p.stdout).engine,'rust-desktop');
  const first=run([]),second=run([]);await Promise.all([exit(first,70000),exit(second,70000)]);
  assert.equal(first.child.exitCode,0,first.stderr);assert.equal(second.child.exitCode,0,second.stderr);
  const statuses=[JSON.parse(first.stdout).status,JSON.parse(second.stdout).status].sort();assert.deepEqual(statuses,['activated','ready']);
  const identity=await request(port,'/api/backend');assert.equal(identity.value.engine,'rust-backend');
  assert.equal((await request(port,'/api/desktop/identity',{token:'wrong'})).status,400);
  assert.equal((await request(port,'/api/config',{ui_language:'italian'})).status,200);
  p=run(['--stop']);assert.equal(await exit(p),0,p.stderr);await until(()=>closed(port));
  p=run([]);assert.equal(await exit(p,70000),0,p.stderr);assert.equal(JSON.parse(p.stdout).status,'ready');
  assert.equal((await request(port,'/api/config')).value.settings.ui_language,'italian');
  p=run(['--stop']);assert.equal(await exit(p),0,p.stderr);await until(()=>closed(port));
  const foreign=http.createServer((req,res)=>{res.writeHead(200,{'Content-Type':'application/json'});res.end(JSON.stringify({engine:'foreign',marker:'untouched'}));});
  await new Promise((r,j)=>foreign.listen(port,'127.0.0.1',r).on('error',j));
  try{p=run([]);await until(()=>p.child.exitCode!==null);assert.match(p.stderr,/incompatible service|occupied by another program/);assert.notEqual(p.child.exitCode,0);assert.equal((await request(port,'/')).value.marker,'untouched');}
  finally{if(p.child.exitCode===null)p.child.kill();await new Promise(r=>foreign.close(r));}
 }finally{
  // Authenticate cleanup using only the capability in this private data directory.
  try{const control=JSON.parse(fs.readFileSync(path.join(data,`native-control-${port}.json`)));await request(port,'/api/shutdown',{token:control.token});await until(()=>closed(port),10000);}catch{}
  for(const p of owned)if(p.child.exitCode===null&&p.child.signalCode===null)p.child.kill();
  await delay(100);tmp.close();
 }
});

test('resident desktop job stops its backend after the owned host is terminated',{skip:!enabled||process.env.HOI4_DESKTOP_CRASH_TEST!=='1',timeout:90000},async()=>{
 const tmp=temporary('hoi4-desktop-crash-'),data=tmp.root,port=await freePort();
 write(path.join(data,'settings.json'),{game_dir:path.join(data,'missing-game'),docs_dir:path.join(data,'missing-docs')});
 const p=launch(['--resources',resources,'--data-dir',data,'--port',String(port),'--no-hud']);
 try{
  await until(async()=>{if(p.child.exitCode!==null)throw Error(`Desktop exited early: ${p.stderr}`);try{return (await request(port,'/api/backend')).value.engine==='rust-backend'}catch{return false}},70000);
  // Capture exact identities before terminating only the host process we spawned.
  await delay(2000);assert.equal(p.child.exitCode,null,'desktop should remain resident');
  const backendPid=(await request(port,'/api/backend')).value.pid;
  const ps='$root=$env:HOI4_TEST_PROFILE; @(Get-CimInstance Win32_Process | Where-Object { $_.Name -eq "msedgewebview2.exe" -and $_.CommandLine -and $_.CommandLine.Contains($root) } | Select-Object -ExpandProperty ProcessId) | ConvertTo-Json -Compress';
  const raw=execFileSync('powershell.exe',['-NoProfile','-Command',ps],{encoding:'utf8',env:{...process.env,HOI4_TEST_PROFILE:path.join(data,'webview2-profile')},windowsHide:true}).trim();
  const webviewPids=raw?JSON.parse(raw):[];assert.ok(webviewPids.length>0,'isolated WebView processes should be present');
  const alive=pid=>{try{process.kill(pid,0);return true}catch(e){return e.code==='EPERM'}};
  assert.equal(p.child.kill(),true);await exit(p);await until(()=>closed(port),10000);
  await until(()=>!alive(backendPid)&&webviewPids.every(pid=>!alive(pid)),15000);
 }finally{
  if(p.child.exitCode===null&&p.child.signalCode===null){p.child.kill();await exit(p,10000);}
  try{const control=JSON.parse(fs.readFileSync(path.join(data,`native-control-${port}.json`)));await request(port,'/api/shutdown',{token:control.token});await until(()=>closed(port),10000);}catch{}
  await delay(200);tmp.close();
 }
});
