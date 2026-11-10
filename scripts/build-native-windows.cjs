'use strict';
// Explicit resource allowlist: application data and development sources stay out.
const fs=require('node:fs'),path=require('node:path'),{ROOT,sha,json,write,run,powershell,files,temporary,options,archive}=require('./native-tooling.cjs');
const PACKAGE=path.join(ROOT,'src/hoi4companion');
const RESOURCE_FOLDERS=['web','examples','licenses','rules'];
const RESOURCE_FILES=['guide-import-prompt.md','guide_language.json','authoring-actions-v1.json','foreign-tasks-v1.json','live-profiles-v1.json','live-abi-v1.json','design-profile-v1.json','country-functions-v1.json','translations.json'];
function locate(name,explicit){
 const candidates=[explicit,path.join(PACKAGE,'_native/tools',name)];
 if(name==='ffmpeg.exe'){const d=path.join(ROOT,'.local/media-tools/imageio_ffmpeg/binaries');if(fs.existsSync(d))candidates.push(...fs.readdirSync(d).filter(n=>/^ffmpeg.*\.exe$/.test(n)).sort().map(n=>path.join(d,n)));}
 if(name==='yt-dlp.exe')candidates.push(path.join(require('node:os').homedir(),'Downloads',name));
 const found=candidates.find(p=>p&&fs.existsSync(p)&&fs.statSync(p).isFile());if(!found)throw Error('Missing standalone utility: '+name+'; pass its explicit path');return path.resolve(found);
}
function build(args=options()){
 if(process.platform!=='win32')throw Error('Windows x64 build required');
 if(!args.skip_native)for(const script of ['build-backend.ps1','build-overlay.ps1'])powershell(script,[],{timeout:900000});
 const native=path.join(PACKAGE,'_native'),backend=path.resolve(args.backend_directory||native),overlay=path.resolve(args.overlay_executable||path.join(native,'overlay/hoi4-overlay.exe'));
 for(const file of [path.join(backend,'hoi4-desktop.exe'),path.join(backend,'hoi4-backend.exe'),path.join(backend,'hoi4companion_backend.dll'),path.join(native,'hoi4companion_parser.dll'),overlay,path.join(native,'cwtools/Companion.Cwtools.exe')])if(!fs.existsSync(file))throw Error('Missing native component: '+file);
 const node=path.resolve(args.node||path.join(ROOT,'.local/windows-build-downloads/node-v24.15.0-win-x64/node.exe'));
 if(!fs.existsSync(node))throw Error('Pinned on-demand Node utility missing; run prepare-windows-tools.cjs or pass --node');
 const tools={'ffmpeg.exe':locate('ffmpeg.exe',args.ffmpeg),'yt-dlp.exe':locate('yt-dlp.exe',args.ytdlp),'node.exe':node};
 const version=json(path.join(ROOT,'package.json')).version,output=path.join(ROOT,`dist/HOI4Companion-${version}-native-windows-x64.zip`),stage=temporary('hoi4-native-package-');
 const copy=(from,to)=>{fs.mkdirSync(path.dirname(to),{recursive:true});fs.copyFileSync(from,to);};
 let manifest;
 try{
  const destination=path.join(stage.root,'HOI4Companion'),resources=path.join(destination,'resources');fs.mkdirSync(resources,{recursive:true});
  copy(path.join(backend,'hoi4-desktop.exe'),path.join(destination,'HOI4 Companion.exe'));
  for(const name of RESOURCE_FOLDERS)fs.cpSync(path.join(PACKAGE,name),path.join(resources,name),{recursive:true,filter:p=>!/(?:__pycache__|\.(?:pyc|pdb))$/.test(p)});
  for(const name of RESOURCE_FILES)copy(path.join(PACKAGE,name),path.join(resources,name));
  fs.mkdirSync(path.join(resources,'guides'));const target=path.join(resources,'_native');
  for(const name of ['hoi4-backend.exe','hoi4companion_backend.dll'])copy(path.join(backend,name),path.join(target,name));
  copy(path.join(native,'hoi4companion_parser.dll'),path.join(target,'hoi4companion_parser.dll'));copy(overlay,path.join(target,'overlay/hoi4-overlay.exe'));
  write(path.join(target,'overlay/provenance.json'),{schema_version:1,engine:'rust-native-hud',native_hud_protocol:1,files:{'hoi4-overlay.exe':sha(overlay)}});
  fs.cpSync(path.join(native,'cwtools'),path.join(target,'cwtools'),{recursive:true,filter:p=>! /\.(?:pdb|log)$/.test(p)});
  const utilities={},toolsTarget=path.join(target,'tools');
  for(const [name,file] of Object.entries(tools)){
   copy(file,path.join(toolsTarget,name));utilities[name]={sha256:sha(file),version:run(file,[name==='ffmpeg.exe'?'-version':'--version']).split(/\r?\n/)[0],on_demand:true};
   if(name==='ffmpeg.exe')fs.writeFileSync(path.join(toolsTarget,'ffmpeg-license.txt'),run(file,['-L']));
  }
  fs.writeFileSync(path.join(toolsTarget,'yt-dlp-license.txt'),'yt-dlp standalone utility: https://github.com/yt-dlp/yt-dlp/blob/master/LICENSE\nThe main yt-dlp project is Unlicense/public domain; standalone builds retain third-party licenses.\n');
  if(fs.existsSync(path.join(path.dirname(node),'LICENSE')))copy(path.join(path.dirname(node),'LICENSE'),path.join(toolsTarget,'node-LICENSE.txt'));
  write(path.join(toolsTarget,'manifest.json'),utilities);
  for(const name of ['README.md','LICENSE','START HERE.txt'])copy(path.join(ROOT,name),path.join(destination,name));fs.cpSync(path.join(ROOT,'docs'),path.join(destination,'docs'),{recursive:true});
  const entries=Object.fromEntries(files(destination).sort().map(p=>[path.relative(destination,p).replaceAll('\\','/'),sha(p)]));
  const forbidden=/python(?:w|3|313)?\.(?:exe|dll)|companion runtime\.exe|\.(?:py|pyc|pyd|sqlite3|pdb)$|settings\.json$|\.env$|\/(?:guides|guide-videos|sprite-cache)\//i;
  for(const name of Object.keys(entries))if(forbidden.test(name))throw Error('Forbidden runtime/user data in native package: '+name);
  manifest={schema_version:1,version,architecture:'rust-backend-rust-hud-javascript-ui',resident_python:false,data_directory:'%LOCALAPPDATA%/HOI4Companion',git_revision:run('git',['rev-parse','HEAD']).trim(),working_tree_dirty:!!run('git',['status','--porcelain']).trim(),files:entries,utilities};write(path.join(destination,'manifest.json'),manifest);
  const stagedZip=path.join(stage.root,'package.zip');archive('Create',destination,stagedZip);fs.mkdirSync(path.dirname(output),{recursive:true});copy(stagedZip,output);
 }finally{stage.close();}
 const result={zip:output,bytes:fs.statSync(output).size,sha256:sha(output),resident_python:false,files:Object.keys(manifest.files).length};fs.writeFileSync(output+'.sha256',result.sha256+'  '+path.basename(output)+'\n');return result;
}
if(require.main===module)try{console.log(JSON.stringify(build(),null,2));}catch(e){console.error(e.message);process.exitCode=1;}
module.exports={build};
