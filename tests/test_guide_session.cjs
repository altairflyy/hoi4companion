'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
test('editing split legacy confirmations preserves sibling activities and rolls back rejected writes',async()=>{
 const definition={id:'g',title:'Generic',tracks:{operations:['build','train'].map(key=>({key,action:{kind:'manual',params:{legacy_key:'old'}}}))},milestones:[{key:'old',title:'Old'}]};
 let saved={schema_version:1,guide_id:'g',run_id:'run',session_id:null,checks:{old:true},revision:0},reject=false;
 const window={};const context={window,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{href:'http://localhost/walkthrough',search:''},HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js'),fetch:async(url,options)=>{
  if(options){if(reject)return {ok:false,status:400,json:async()=>({error:'Changed run'})};const body=JSON.parse(options.body);saved={...saved,...body,revision:saved.revision+1};}
  return {ok:true,json:async()=>url==='/api/guides'?{guides:[definition]}:url.startsWith('/api/guide/progress')?saved:definition};
 }};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
 await window.hoiGuide.confirm('build',false);
 assert.deepEqual(JSON.parse(JSON.stringify(saved.checks)),{old:false,build:false,train:true});
 await window.hoiGuide.start('build',true);
 assert.deepEqual(JSON.parse(JSON.stringify(saved.started)),{build:true});
 assert.equal(saved.checks.build,false);
 assert.equal(window.hoiGuide.value.view.operations.find(r=>r.key==='build').runtime_status,'active');
 reject=true;await assert.rejects(window.hoiGuide.confirm('train',false),/Changed run/);
 assert.equal(window.hoiGuide.value.progress.checks.train,true);
 assert.equal(window.hoiGuide.value.pending,0);assert.match(window.hoiGuide.value.error,/Conferma non salvata/);
 await assert.rejects(window.hoiGuide.start('build',false),/Changed run/);
 assert.equal(window.hoiGuide.value.progress.started.build,true);
});
