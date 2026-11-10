'use strict';
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),crypto=require('node:crypto'),{execFileSync}=require('node:child_process');
const ROOT=path.resolve(__dirname,'..');
const sha=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const json=file=>JSON.parse(fs.readFileSync(file,'utf8').replace(/^\uFEFF/,''));
const write=(file,value)=>{fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,JSON.stringify(value,null,2)+'\n');};
function run(file,args=[],options={}){return execFileSync(file,args,{cwd:ROOT,windowsHide:true,encoding:'utf8',timeout:60000,maxBuffer:8*1024*1024,...options});}
function powershell(script,args=[],options={}){return run('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(ROOT,'scripts',script),...args],options);}
function files(folder){return fs.readdirSync(folder,{withFileTypes:true}).flatMap(entry=>{const p=path.join(folder,entry.name);if(entry.isSymbolicLink())throw Error('Unexpected symbolic link: '+p);return entry.isDirectory()?files(p):[p];});}
function temporary(prefix){const root=fs.mkdtempSync(path.join(os.tmpdir(),prefix));return {root,close(){fs.rmSync(root,{recursive:true,force:true});}};}
function options(args=process.argv.slice(2)){const result={_:[]};for(let i=0;i<args.length;i++){const a=args[i];if(a.startsWith('--')){const key=a.slice(2).replaceAll('-','_');result[key]=args[i+1]&&!args[i+1].startsWith('--')?args[++i]:true;}else result._.push(a);}return result;}
function archive(mode,source,destination){powershell('zip-archive.ps1',['-Mode',mode,'-Source',path.resolve(source),'-Destination',path.resolve(destination)],{timeout:180000});}
module.exports={ROOT,sha,json,write,run,powershell,files,temporary,options,archive};
