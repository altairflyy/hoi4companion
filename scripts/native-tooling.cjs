'use strict';
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),crypto=require('node:crypto'),{execFileSync}=require('node:child_process');
const ROOT=path.resolve(__dirname,'..');
const sha=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const json=file=>JSON.parse(fs.readFileSync(file,'utf8').replace(/^\uFEFF/,''));
const write=(file,value)=>{fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,JSON.stringify(value,null,2)+'\n');};
function run(file,args=[],options={}){return execFileSync(file,args,{cwd:ROOT,windowsHide:true,encoding:'utf8',timeout:60000,maxBuffer:8*1024*1024,...options});}
// Development-only release profile: optimized, incremental, parallel codegen,
// no whole-program LTO. Production packaging keeps the Cargo.toml profile.
function fastProfile(env){env.CARGO_PROFILE_RELEASE_LTO='false';env.CARGO_PROFILE_RELEASE_CODEGEN_UNITS='16';env.CARGO_PROFILE_RELEASE_INCREMENTAL='true';return env;}
// The backend and the crates split out of it. Their local TESTS compile these
// crates without LLVM optimization (third-party dependencies stay optimized):
// measured ~17 s instead of ~59 s after an edit. Their own shared cache keeps
// the -Dev install build optimized.
const BACKEND_CRATES=['model','guides','hud','live','media','backend'];
const BACKEND_TEST_CONFIG=BACKEND_CRATES.flatMap(c=>['--config',`profile.release.package.hoi4companion-${c}.opt-level=0`]);
function powershell(script,args=[],options={}){return run('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(ROOT,'scripts',script),...args],options);}
function files(folder){return fs.readdirSync(folder,{withFileTypes:true}).flatMap(entry=>{const p=path.join(folder,entry.name);if(entry.isSymbolicLink())throw Error('Unexpected symbolic link: '+p);return entry.isDirectory()?files(p):[p];});}
function temporary(prefix){const root=fs.mkdtempSync(path.join(os.tmpdir(),prefix));return {root,close(){fs.rmSync(root,{recursive:true,force:true});}};}
function options(args=process.argv.slice(2)){const result={_:[]};for(let i=0;i<args.length;i++){const a=args[i];if(a.startsWith('--')){const key=a.slice(2).replaceAll('-','_');result[key]=args[i+1]&&!args[i+1].startsWith('--')?args[++i]:true;}else result._.push(a);}return result;}
function archive(mode,source,destination){powershell('zip-archive.ps1',['-Mode',mode,'-Source',path.resolve(source),'-Destination',path.resolve(destination)],{timeout:180000});}
module.exports={ROOT,sha,json,write,run,powershell,files,temporary,options,archive,fastProfile,BACKEND_CRATES,BACKEND_TEST_CONFIG};
