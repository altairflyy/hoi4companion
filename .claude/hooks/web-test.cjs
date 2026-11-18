'use strict';
// PostToolUse: after an edit of a web module, run the JS tests that load it
// (tests/test_*.cjs that mention the file); after an edit of
// translations.json, check it is valid JSON. Failures go back to Claude
// (exit 2, short stderr); success is silent. Other files are ignored.
const fs=require('fs'),path=require('path'),{spawnSync}=require('child_process');
let input='';
process.stdin.on('data',c=>input+=c).on('end',()=>{
 let call;try{call=JSON.parse(input);}catch{process.exit(0);}
 const file=String(call.tool_input?.file_path||'').replace(/\\/g,'/');
 const root=(process.env.CLAUDE_PROJECT_DIR||path.resolve(__dirname,'../..')).replace(/\\/g,'/');
 const report=text=>{process.stderr.write(text.slice(-3000)+'\n');process.exit(2);};
 if(/src\/hoi4companion\/translations\.json$/i.test(file)){
  try{JSON.parse(fs.readFileSync(file,'utf8'));}catch(e){report(`translations.json is not valid JSON: ${e.message}`);}
  process.exit(0);
 }
 const web=/src\/hoi4companion\/web\/([^/]+)\.js$/i.exec(file);
 if(!web)process.exit(0);
 const name=web[1]+'.js',dir=path.join(root,'tests');
 const tests=fs.readdirSync(dir).filter(f=>/^test_.*\.cjs$/.test(f)&&f!=='test_native_service.cjs')
  .filter(f=>{const text=fs.readFileSync(path.join(dir,f),'utf8');return text.includes('web/'+name)||text.includes(`'${name}'`)||text.includes(`"${name}"`);});
 if(!tests.length)process.exit(0);
 const run=spawnSync(process.execPath,['--test',...tests.map(f=>path.join('tests',f))],{cwd:root,encoding:'utf8',timeout:110000,maxBuffer:32<<20});
 if(run.status===0)process.exit(0);
 const out=(run.stdout||'')+(run.stderr||'');
 const lines=out.split('\n').filter(l=>/^not ok|^\s+(error|expected|actual|AssertionError|Error)|✖|ℹ (pass|fail)/i.test(l));
 report(`JS tests failed for ${name} (${tests.join(', ')}):\n${(lines.length?lines:out.split('\n').slice(-30)).join('\n')}`);
});
