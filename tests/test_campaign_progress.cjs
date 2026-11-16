'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const fixtures=JSON.parse(fs.readFileSync('tests/fixtures/campaign-progress.json','utf8'));
const fixture=require('./presentation-fixtures.cjs');
test('restored unidentified legacy progress cannot prove completions',()=>{
 const {guide,state,progress,evaluation}=fixture('campaign: restored unidentified progress');
 const view=require('../src/hoi4companion/web/guide-model.js').evaluate(guide,state,{progress,evaluation});
 assert.equal(view.runtime.progress_status,'campaign_unverified');
});
test('shared browser session accepts server restart and never resets progress on its own',async()=>{
 let subscriber,serverId='old-server';const posts=[];
 let progress={...fixtures.progress,run_revision:3,revision:100,server_id:'old-server',history:[]};
 const guide={...fixtures.guide,runtime_engine:'rust-guide-core',runtime_definition_revision:'a'.repeat(64)};
 const packet=()=>({schema_version:1,engine:'rust-guide-core',server_id:serverId,guide_id:guide.id,definition_revision:guide.runtime_definition_revision,
  state:{schema_version:1,usable:false,facts:{}},progress,evaluation:{schema_version:1,guide_id:guide.id,run_id:progress.run_id,steps:[]}});
 const window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}};
 const fetch=async(url,options)=>{if(url==='/api/guide/active')return {ok:true,json:async()=>options?{guide_id:JSON.parse(options.body).guide_id,revision:1}:{}};
  if(options){posts.push(JSON.parse(options.body));progress={...progress,checks:posts.at(-1).checks,run_revision:progress.run_revision+1};}
  return {ok:true,json:async()=>url==='/api/guides'?{guides:[{id:'campaign-test',title:'Test'}]}:url.startsWith('/api/guide/projection')?packet():url.startsWith('/api/guide/progress')?progress:guide};
 };
 const context={window,fetch,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{search:'',href:'http://local/walkthrough'},HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js')};
 const settle=async()=>{for(let i=0;i<4;i++)await new Promise(setImmediate);};
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await settle();
 await window.hoiGuide.confirm('done',false);
 assert.equal(posts[0].expected_revision,3);assert.equal(posts[0].expected_run_id,'r');assert.equal(posts[0].server_id,'old-server');
 // The restarted server restored another run; the page takes it from the server's packet.
 serverId='new-server';progress={...fixtures.progress,run_id:'restored',run_revision:8,server_id:'new-server',checks:{done:true}};
 subscriber({...fixtures.state,guide_server_id:'new-server'});await settle();
 assert.equal(window.hoiGuide.value.progress.run_id,'restored');assert.equal(window.hoiGuide.value.progress.checks.done,true);
 subscriber({usable:true,sample:{country:'OTH',completed_focuses:[],researched_technologies:[]},guide_server_id:'new-server'});
 await settle();assert.equal(posts.length,1);
});

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}
