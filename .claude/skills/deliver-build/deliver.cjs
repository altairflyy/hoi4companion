'use strict';
// Rebuild and restart the companion so the user never swaps binaries:
// stop -> wait for every hoi4-* process to exit -> build backend (and the
// overlay when its sources or translations are newer than its exe) -> check
// the installed backend is the fresh build -> start -> wait for HTTP.
// Never touches HOI4 itself. Usage: node deliver.cjs [--overlay] [--no-start]
const {spawnSync}=require('child_process'),fs=require('fs'),path=require('path'),crypto=require('crypto');
const ROOT=path.resolve(__dirname,'../../..');
const args=new Set(process.argv.slice(2));
const NATIVE=path.join(ROOT,'src/hoi4companion/_native');
const log=(...a)=>console.log('[deliver]',...a);
const fail=message=>{console.error('[deliver] FAILED:',message);process.exit(1);};
const ps=(script,extra=[],{tolerate=false}={})=>{
 const r=spawnSync('powershell',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(ROOT,'scripts',script),...extra],{cwd:ROOT,encoding:'utf8',maxBuffer:64<<20});
 if(r.status!==0&&!tolerate)fail(`${script} exited ${r.status}\n${(r.stderr||r.stdout||'').split('\n').slice(-15).join('\n')}`);
 return r.stdout;
};
const running=()=>{
 const r=spawnSync('tasklist',['/FO','CSV','/NH'],{encoding:'utf8'});
 return (r.stdout||'').split('\n').filter(l=>/"hoi4-(backend|desktop|overlay)\.exe"/i.test(l)).length;
};
const sleep=ms=>Atomics.wait(new Int32Array(new SharedArrayBuffer(4)),0,0,ms);
const newest=dir=>{let t=0;const walk=p=>{for(const e of fs.readdirSync(p,{withFileTypes:true})){const q=path.join(p,e.name);if(e.isDirectory()){if(e.name!=='target')walk(q);}else t=Math.max(t,fs.statSync(q).mtimeMs);}};walk(dir);return t;};
const mtime=p=>fs.existsSync(p)?fs.statSync(p).mtimeMs:0;
const sha=p=>fs.existsSync(p)?crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex'):null;

(async()=>{
 // Stop fails when nothing is running; only processes left alive matter.
 if(running()){log('stopping companion');ps('avvia-tutto.ps1',['-Stop'],{tolerate:true});}
 else log('companion not running');
 for(let i=0;i<60&&running();i++)sleep(1000);
 if(running())fail('hoi4-* processes still running after 60 s; binaries would not be replaced');

 log('building backend');
 ps('build-backend.ps1');
 const built=path.join(ROOT,'.local/rust-backend-target/release/hoi4-backend.exe');
 const installed=path.join(NATIVE,'hoi4-backend.exe');
 if(fs.existsSync(built)&&sha(built)!==sha(installed))fail('installed hoi4-backend.exe differs from the fresh build');

 const overlayExe=path.join(NATIVE,'overlay/hoi4-overlay.exe');
 const overlaySources=Math.max(newest(path.join(ROOT,'native/overlay/src')),mtime(path.join(ROOT,'src/hoi4companion/translations.json')));
 if(args.has('--overlay')||overlaySources>mtime(overlayExe)){
  log('building overlay (sources or translations newer than its exe)');
  ps('build-overlay.ps1');
 }else log('overlay up to date, skipped');
 for(const exe of [installed,overlayExe])log(path.relative(ROOT,exe),new Date(mtime(exe)).toLocaleString());

 if(args.has('--no-start'))return log('done (not started)');
 log('starting companion');
 // avvia-tutto returns as soon as the resident desktop host is launched.
 // Its stdio must not be a pipe: the host would keep it open and hang us.
 const start=spawnSync('powershell',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(ROOT,'scripts/avvia-tutto.ps1')],{cwd:ROOT,stdio:'ignore',windowsHide:true,timeout:120000});
 if(start.status!==0)fail(`avvia-tutto.ps1 exited ${start.status}${start.error?' ('+start.error.message+')':''}`);
 for(let i=0;i<150;i++){
  try{const r=await fetch('http://127.0.0.1:8766/events',{signal:AbortSignal.timeout(2000)});if(r.ok)return log(`up on :8766 after ~${i*2}s`);}catch{}
  sleep(2000);
 }
 fail('companion did not answer on :8766 within 300 s');
})();
