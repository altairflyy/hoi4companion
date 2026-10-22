'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const model=require('../src/hoi4companion/web/importer-model.js');
const tick=()=>new Promise(setImmediate);
const definition={schema_version:1,id:'opening',title:'Opening',sources:[{id:'src',type:'manual'}],tracks:{research:[{key:'tools',title:'Tools',action:{kind:'research',entity:{kind:'technology',id:'tools'}},evidence:[{source_id:'src',basis:'explicit',quote:'Research tools.'}]}]}};
function harness(){
 class Node{
  constructor(tag=''){this.tag=tag;this.children=[];this.value='';this.hidden=false;this.checked=false;this.text='';}
  set textContent(value){this.text=String(value);this.children=[];}get textContent(){return this.text+this.children.map(n=>n.textContent||'').join('');}
  append(...nodes){this.children.push(...nodes);}replaceChildren(...nodes){this.text='';this.children=nodes;}
  click(){this.onclick?.();}remove(){}
 }
 const nodes=new Map(),get=id=>{if(!nodes.has(id))nodes.set(id,new Node());return nodes.get(id);};
 const requests=[],downloads=[],clipboard=[];let release;
 const source={id:'src',type:'manual',title:'Fixture',text:'Research tools.',warnings:[]};
 const report={valid:true,guide:definition,errors:[],warnings:[],resolutions:[{key:'tools',status:'resolved',candidates:[{kind:'technology',id:'tools',name:'Tools'}]}]};
 const fetch=async(path,options)=>{
  const body=options?.body?JSON.parse(options.body):null;requests.push({path,body});
  if(path.endsWith('/preview')&&release)await new Promise(resolve=>release(resolve));
  const value=path.endsWith('/prompt')?{prompt:fs.readFileSync('src/hoi4companion/guide-import-prompt.md','utf8')}:path.endsWith('/document')?{guide:body.document.guide,sources:body.document.sources}:path.endsWith('/status')?{ai_configured:false}:path.endsWith('/catalog')?{entities:[],demo:false}:path.endsWith('/source')?source:path.endsWith('/preview')?report:path.endsWith('/install')?{id:'opening',href:'/walkthrough?guide=opening'}:{};
  return {ok:true,json:async()=>value};
 };
 const document={getElementById:get,createElement:tag=>new Node(tag),body:new Node('body')};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/importer.js','utf8'),{HoiImporter:model,document,fetch,
  navigator:{clipboard:{async writeText(text){clipboard.push(text);}}},Blob,URL:{createObjectURL(blob){downloads.push(blob);return 'blob:fixture';},revokeObjectURL(){}},setTimeout:fn=>fn()});
 return {get,requests,downloads,clipboard,source,report,document,setRelease:fn=>release=fn};
}
test('controller acquires, previews, edits, reviews and installs without auto AI or auto install',async()=>{
 const h=harness();await tick();assert.equal(h.get('extract').disabled,true);
 h.get('sourceText').value='Research tools.';await h.get('acquire').onclick();
 assert.equal(h.get('sourceCount').textContent,'1 fonti acquisite');assert.equal(h.get('packet').disabled,false);
 h.get('guideJson').value=JSON.stringify(definition);h.get('guideJson').oninput();await h.get('validate').onclick();
 assert.match(h.get('validation').textContent,/superata/);assert.equal(h.get('preview').children.length,1);
 assert.equal(h.get('install').disabled,true);h.get('reviewed').checked=true;h.get('reviewed').onchange();
 assert.equal(h.get('install').disabled,false);
 h.get('guideJson').oninput();assert.equal(h.get('install').disabled,true);
 await h.get('validate').onclick();h.get('reviewed').checked=true;h.get('reviewed').onchange();
 h.get('exportGuide').onclick();assert.equal(h.downloads.length,1);
 await h.get('install').onclick();assert.equal(h.get('openGuide').href,'/walkthrough?guide=opening');
 assert.equal(h.get('openGuide').hidden,false);
 assert.equal(h.requests.filter(r=>r.path.endsWith('/install')).length,1);
 assert.equal(h.requests.some(r=>r.path.endsWith('/extract')),false);
});

test('ID choices display installed localisation while retaining the English reference',async()=>{
 const h=harness();await tick();
 h.report.resolutions[0].candidates=[{kind:'technology',id:'tools',name:'Basic Machine Tools',localized_name:'Macchine utensili di base'}];
 h.get('sourceText').value='Research tools.';await h.get('acquire').onclick();
 h.get('guideJson').value=JSON.stringify(definition);await h.get('validate').onclick();
 assert.match(h.get('preview').textContent,/Macchine utensili di base/);
 assert.doesNotMatch(h.get('preview').textContent,/Basic Machine Tools/);
});

test('preview explains an association derived from the authored focus prerequisites',async()=>{
 const h=harness();await tick();
 Object.assign(h.report.resolutions[0],{matching_basis:'guide_prerequisites',supporting_focus_ids:['previous_focus'],
  excluded_candidates:[{entity:{id:'other_focus',name:'Other focus'},reason:'route_mutual_exclusion',conflicting_focus_ids:['previous_focus']}]});
 h.get('sourceText').value='Research tools.';await h.get('acquire').onclick();
 h.get('guideJson').value=JSON.stringify(definition);await h.get('validate').onclick();
 assert.match(h.get('preview').textContent,/Percorso della guida: previous_focus/);
 assert.match(h.get('preview').textContent,/Escluso dal percorso: Other focus \(other_focus\)/);
});

test('preview separates import review from live monitoring and groups source notices',async()=>{
 const h=harness();await tick();
 h.report.operations=[{key:'agency',title:'Agency',monitoring:'automatic',automatic_start:true,automatic_completion:true},
  {key:'build',title:'Factories',monitoring:'start_only',reason:'additional_baseline_missing'},
  {key:'policy',title:'Policy',monitoring:'manual'}];
 h.report.warnings=['agency: osservazione visiva dichiarata dall’IA esterna, da confrontare con la fonte'];
 h.get('sourceText').value='Research tools.';await h.get('acquire').onclick();
 h.get('guideJson').value=JSON.stringify(definition);await h.get('validate').onclick();
 assert.match(h.get('monitoringSummary').textContent,/1 automatiche.*1 con solo avvio.*1 da confermare/);
 assert.match(h.get('validation').textContent,/pronta per essere aggiunta/);
 assert.equal(h.get('issues').children.length,0);
 assert.equal(h.get('sourceWarnings').children.length,1);
 assert.match(h.get('reviewSummary').textContent,/1 istruzioni.*video/);
});
test('an edit during validation invalidates the pending report and blocks publishing',async()=>{
 const h=harness();await tick();h.get('sourceText').value='Research tools.';await h.get('acquire').onclick();
 let release;h.setRelease(resolve=>{release=resolve;});h.get('guideJson').value=JSON.stringify(definition);
 const pending=h.get('validate').onclick();await tick();h.get('guideJson').value='{}';h.get('guideJson').oninput();
 release();await pending;assert.equal(h.get('install').disabled,true);assert.equal(h.get('preview').children.length,0);
 assert.match(h.get('validation').textContent,/modificata/);
});
test('standalone prompt copies and downloads without first acquiring a source or calling AI',async()=>{
 const h=harness();await tick();assert.equal(h.get('copyPrompt').disabled,false);
 await h.get('copyPrompt').onclick();assert.match(h.clipboard[0],/hoi4-guide-import/);
 h.get('downloadPrompt').onclick();assert.equal(await h.downloads[0].text(),h.clipboard[0]);
 assert.equal(h.requests.some(r=>r.path.endsWith('/source')||r.path.endsWith('/extract')||r.path.endsWith('/packet')),false);
});
test('external AI file loads sources and previews directly with no preliminary acquisition',async()=>{
 const h=harness();await tick();const document={format:'hoi4-guide-import',import_schema_version:1,guide:definition,sources:[h.source]};
 h.get('draftFile').files=[{size:1200,text:async()=>JSON.stringify(document)}];
 await h.get('draftFile').onchange();assert.match(h.get('validation').textContent,/superata/);
 assert.equal(h.requests.filter(r=>r.path.endsWith('/document')).length,1);
 assert.equal(h.requests.some(r=>r.path.endsWith('/source')||r.path.endsWith('/extract')),false);
 h.get('guideJson').value=JSON.stringify(document);h.get('guideJson').oninput();
 await h.get('validate').onclick();assert.match(h.get('validation').textContent,/superata/);
});
