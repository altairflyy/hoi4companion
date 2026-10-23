'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const fixtures=JSON.parse(fs.readFileSync('tests/fixtures/campaign-progress.json','utf8'));
for(const fixture of fixtures.cases)test('campaign contract: '+fixture.name,()=>{
 const state={...fixtures.state,sample:{...fixtures.state.sample,...fixture.sample}};
 const result=runtime.evaluate(fixtures.guide,state,fixtures.progress);
 assert.equal(result.progress_status,fixture.progress_status);assert.equal(result.steps[0].status,fixture.step_status);
});
test('restored unidentified legacy progress cannot prove completions',()=>{
 const progress={...fixtures.progress,campaign_id:undefined,campaign_required:true};
 assert.equal(runtime.evaluate(fixtures.guide,fixtures.state,progress).progress_status,'campaign_unverified');
 const view=require('../src/hoi4companion/web/guide-model.js').evaluate(fixtures.guide,fixtures.state,progress);
 assert.equal(view.runtime.progress_status,'campaign_unverified');
});
test('shared browser session accepts server restart and never resets progress on its own',async()=>{
 let subscriber;const posts=[];
 let progress={...fixtures.progress,run_revision:3,revision:100,server_id:'old-server',history:[]};
 const window={hoiLive:{enabled:true,subscribe(fn){subscriber=fn;}}};
 const fetch=async(url,options)=>{
  if(options){posts.push(JSON.parse(options.body));progress={...progress,checks:posts.at(-1).checks,run_revision:progress.run_revision+1};}
  return {ok:true,json:async()=>url==='/api/guides'?{guides:[{id:'campaign-test',title:'Test'}]}:url.startsWith('/api/guide/progress')?progress:fixtures.guide};
 };
 const context={window,fetch,URL,URLSearchParams,sessionStorage:{getItem:()=>null,setItem(){}},location:{search:'',href:'http://local/walkthrough'},HoiGuidePresentation:require('../src/hoi4companion/web/guide-presentation.js')};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/guide-session.js','utf8'),context);await new Promise(setImmediate);
 await window.hoiGuide.confirm('done',false);
 assert.equal(posts[0].expected_revision,3);assert.equal(posts[0].expected_run_id,'r');assert.equal(posts[0].server_id,'old-server');
 subscriber({...fixtures.state,guide_revision:0,guide_server_id:'new-server',guide_runs:{'campaign-test':{...fixtures.progress,run_id:'restored',run_revision:8}},guide_states:{'campaign-test':{done:true}}});
 assert.equal(window.hoiGuide.value.progress.run_id,'restored');assert.equal(window.hoiGuide.value.progress.checks.done,true);
 subscriber({usable:true,sample:{country:'OTH',completed_focuses:[],researched_technologies:[]},guide_revision:0,guide_server_id:'new-server'});
 await new Promise(setImmediate);assert.equal(posts.length,1);
});
