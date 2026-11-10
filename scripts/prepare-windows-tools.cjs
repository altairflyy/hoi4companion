'use strict';
// Build-time preparation of the existing pinned standalone tools. No pip/runtime.
const fs=require('node:fs'),path=require('node:path'),{ROOT,sha,write,temporary,archive,options}=require('./native-tooling.cjs');
async function text(url){const response=await fetch(url,{signal:AbortSignal.timeout(60000)});if(!response.ok)throw Error(url+': '+response.status);return response.text();}
async function download(url,file,expected){if(fs.existsSync(file)&&sha(file)===expected)return;const response=await fetch(url,{signal:AbortSignal.timeout(180000)});if(!response.ok)throw Error(url+': '+response.status);const bytes=Buffer.from(await response.arrayBuffer());fs.mkdirSync(path.dirname(file),{recursive:true});const temporary=file+'.download';fs.writeFileSync(temporary,bytes);if(sha(temporary)!==expected){fs.unlinkSync(temporary);throw Error('Checksum mismatch: '+url);}fs.renameSync(temporary,file);}
async function prepare(){
 const cache=path.join(ROOT,'.local/windows-build-downloads'),tools=path.join(ROOT,'src/hoi4companion/_native/tools');fs.mkdirSync(tools,{recursive:true});
 const version='24.15.0',name=`node-v${version}-win-x64`,base=`https://nodejs.org/dist/v${version}/`,sums=await text(base+'SHASUMS256.txt');
 const expected=sums.split('\n').find(line=>line.trim().endsWith(name+'.zip'))?.trim().split(/\s+/)[0];if(!expected)throw Error('Missing Node checksum');
 const nodeZip=path.join(cache,name+'.zip');await download(base+name+'.zip',nodeZip,expected);
 const stage=temporary('hoi4-standalone-tools-');
 try{
  archive('Extract',nodeZip,path.join(stage.root,'node'));const destination=path.join(cache,name);fs.mkdirSync(destination,{recursive:true});
  for(const leaf of ['node.exe','LICENSE','README.md'])fs.copyFileSync(path.join(stage.root,'node',name,leaf),path.join(destination,leaf));write(path.join(destination,'provenance.json'),{version,url:base+name+'.zip',sha256:expected});
  const ytVersion='2026.10.21',ytBase=`https://github.com/yt-dlp/yt-dlp/releases/download/${ytVersion}/`,ytSums=await text(ytBase+'SHA2-256SUMS');const ytHash=ytSums.split('\n').find(line=>/\s+\*?yt-dlp\.exe\s*$/.test(line))?.trim().split(/\s+/)[0];if(!ytHash)throw Error('Missing standalone yt-dlp checksum');await download(ytBase+'yt-dlp.exe',path.join(tools,'yt-dlp.exe'),ytHash);
  const metadata=JSON.parse(await text('https://pypi.org/pypi/imageio-ffmpeg/0.6.0/json'));const wheel=metadata.urls.find(file=>file.filename.endsWith('win_amd64.whl'));if(!wheel)throw Error('Pinned FFmpeg Windows archive missing');
  const wheelPath=path.join(cache,wheel.filename);await download(wheel.url,wheelPath,wheel.digests.sha256);archive('Extract',wheelPath,path.join(stage.root,'ffmpeg'));
  const binaries=path.join(stage.root,'ffmpeg/imageio_ffmpeg/binaries'),binary=fs.readdirSync(binaries).find(file=>/^ffmpeg.*\.exe$/.test(file));if(!binary)throw Error('FFmpeg executable missing');fs.copyFileSync(path.join(binaries,binary),path.join(tools,'ffmpeg.exe'));
 }finally{stage.close();}
 return tools;
}
if(require.main===module)prepare(options()).then(console.log).catch(e=>{console.error(e.message);process.exitCode=1;});
module.exports={prepare};
