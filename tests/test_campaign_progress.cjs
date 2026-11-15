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
 let subscriber;const posts=[];
 let progress={...fixtures.progress,run_revision:3,revision:100,server_id:'old-server',history:[]};
 const window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}};
 const fetch=async(url,options)=>{if(url==='/api/guide/active')return {ok:true,json:async()=>options?{guide_id:JSON.parse(options.body).guide_id,revision:1}:{}};
  if(options){posts.push(JSON.parse(options.body));progress={...progress,checks:posts.at(-1).checks,run_revision:progress.run_revision+1};}
  return {ok:true,json:async()=>url==='/api/guides'?{guides:[{id:'campaign-test',title:'Test'}]}:url.startsWith('/api/guide/progress')?progress:fixtures.guide};
 };
 const context={window,fetch,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{search:'',href:'http://local/walkthrough'},HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js')};
 runLocalized(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
 await window.hoiGuide.confirm('done',false);
 assert.equal(posts[0].expected_revision,3);assert.equal(posts[0].expected_run_id,'r');assert.equal(posts[0].server_id,'old-server');
 subscriber({...fixtures.state,guide_revision:0,guide_server_id:'new-server',guide_runs:{'campaign-test':{...fixtures.progress,run_id:'restored',run_revision:8}},guide_states:{'campaign-test':{done:true}}});
 assert.equal(window.hoiGuide.value.progress.run_id,'restored');assert.equal(window.hoiGuide.value.progress.checks.done,true);
 subscriber({usable:true,sample:{country:'OTH',completed_focuses:[],researched_technologies:[]},guide_revision:0,guide_server_id:'new-server'});
 await new Promise(setImmediate);assert.equal(posts.length,1);
});

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}
