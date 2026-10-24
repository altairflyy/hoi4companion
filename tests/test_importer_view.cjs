'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const model=require('../src/hoi4companion/web/importer-model.js');
const tick=()=>new Promise(setImmediate);
const descendants=n=>[n,...(n.children||[]).flatMap(descendants)];
const definition={schema_version:1,id:'opening',title:'Opening',sources:[{id:'src',type:'manual'}],tracks:{research:[{key:'tools',title:'Tools',action:{kind:'research',entity:{kind:'technology',id:'tools'}},evidence:[{source_id:'src',basis:'explicit',quote:'Research tools.'}]}]}};
function harness(){
 class Node{
  constructor(tag=''){this.tag=tag;this.children=[];this.value='';this.hidden=false;this.checked=false;this.text='';}
  set textContent(value){this.text=String(value);this.children=[];}get textContent(){return this.text+this.children.map(n=>n.textContent||'').join('');}
  append(...nodes){this.children.push(...nodes);}replaceChildren(...nodes){this.text='';this.children=nodes;}
  click(){this.onclick?.();}remove(){}
 }
 const nodes=new Map(),get=id=>{if(!nodes.has(id))nodes.set(id,new Node());return nodes.get(id);};
 const requests=[],downloads=[],clipboard=[],catalog=[];let release;
 const source={id:'src',type:'manual',title:'Fixture',text:'Research tools.',warnings:[]};
 const report={valid:true,guide:definition,errors:[],warnings:[],resolutions:[{key:'tools',status:'resolved',candidates:[{kind:'technology',id:'tools',name:'Tools'}]}]};
 const fetch=async(path,options)=>{
  const body=options?.body?JSON.parse(options.body):null;requests.push({path,body});
  if(path.endsWith('/preview')&&release)await new Promise(resolve=>release(resolve));
  const video={id:'a'.repeat(32),status:'completed',message:'Video pronto',parts:[{name:'parte-01.mp4',bytes:149000000,start_seconds:0,end_seconds:400,url:'/api/import/video/file?id=a&name=parte-01.mp4'}],manifest_url:'/manifest',description_url:'/description'};
  const value=path==='/api/import/video'||path.startsWith('/api/import/video/status?')?video:path.endsWith('/prompt')?{prompt:fs.readFileSync('src/hoi4companion/guide-import-prompt.md','utf8')}:path.endsWith('/document')?{guide:body.document.guide,sources:body.document.sources}:path.endsWith('/status')?{ai_configured:false}:path.endsWith('/catalog')?{entities:catalog,demo:false}:path.endsWith('/source')?source:path.endsWith('/preview')?report:path.endsWith('/install')?{id:'opening',href:'/walkthrough?guide=opening'}:{};
  return {ok:true,json:async()=>value};
 };
 const document={getElementById:get,createElement:tag=>new Node(tag),body:new Node('body')};
 vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/importer.js','utf8'),{HoiImporter:model,document,fetch,
  navigator:{clipboard:{async writeText(text){clipboard.push(text);}}},Blob,URL:{createObjectURL(blob){downloads.push(blob);return 'blob:fixture';},revokeObjectURL(){}},setTimeout:fn=>fn()});
 return {get,requests,downloads,clipboard,source,report,catalog,document,setRelease:fn=>release=fn};
}

test('video preparation submits a link and exposes parts and timing manifest independently of guide import',async()=>{
 const h=harness();await tick();h.get('videoUrl').value='https://www.youtube.com/watch?v=Ht4e89B02Wo';await h.get('prepareVideo').onclick();await tick();
 assert.deepEqual(h.requests.find(r=>r.path==='/api/import/video').body,{url:'https://www.youtube.com/watch?v=Ht4e89B02Wo'});
 assert.match(h.get('videoFiles').textContent,/parte-01.mp4.*149.0 MB/);assert.match(h.get('videoFiles').textContent,/indice dei tempi/);assert.equal(h.get('prepareVideo').disabled,false);
 assert.equal(h.requests.some(r=>r.path==='/api/import/install'),false);
});

test('construction correction uses catalog selectors and revalidates the in-memory draft',async()=>{
 const h=harness();h.catalog.push({kind:'state',id:'91',name:'Lwow'},{kind:'state',id:'762',name:'Katowice'},{kind:'building',id:'infrastructure',name:'Infrastructure'});await tick();
 const row={key:'infra',action:{kind:'manual',params:{category:'construction',state_id:91,building_id:'infrastructure',quantity:1,quantity_mode:'additional',instructions:['Build in Lwow']}},completion:{op:'fact',key:'guide.construction.infra.added',test:'gte',value:1},note:'Original note',evidence:[]};
 h.report.guide={schema_version:1,id:'build',tracks:{operations:[row]}};h.report.resolutions=[];h.report.operations=[{key:'infra',monitoring:'automatic',reason:'baseline_from_live'}];
 h.get('guideJson').value=JSON.stringify(h.report.guide);await h.get('validate').onclick();
 const card=h.get('preview').children[0],editor=descendants(card).find(n=>n.className==='construction-editor');assert.ok(editor);assert.notEqual(editor.open,true);
 const selects=descendants(editor).filter(n=>n.tag==='select');assert.equal(selects[0].value,'91');selects[0].value='762';
 const instruction=descendants(editor).find(n=>n.tag==='textarea');instruction.value='Build in Katowice';
 await descendants(editor).find(n=>n.tag==='button').onclick();
 const sent=h.requests.filter(r=>r.path.endsWith('/preview')).at(-1).body.guide.tracks.operations[0];assert.equal(sent.action.params.state_id,762);assert.equal(sent.action.params.state_name,'Katowice');assert.equal(sent.completion,undefined);assert.deepEqual(sent.action.params.instructions,['Build in Katowice']);
 assert.equal(h.get('reviewed').checked,false);assert.equal(h.requests.some(r=>r.path.endsWith('/install')),false);
});

test('resolved associations are grouped closed and editors appear only on demand',async()=>{
 const h=harness();await tick();h.get('sourceText').value='Research tools.';await h.get('acquire').onclick();
 h.get('guideJson').value=JSON.stringify(definition);await h.get('validate').onclick();
 const group=h.get('preview').children.find(n=>n.tag==='details');assert.ok(group);assert.notEqual(group.open,true);
 assert.match(group.children[0].textContent,/1 associazion.*risolt.*nessuna azione richiesta/i);
 const editor=descendants(group).find(n=>n.tag==='details'&&n!==group);assert.ok(editor);assert.notEqual(editor.open,true);
 assert.match(editor.children[0].textContent,/Correggi associazione/);
 assert.doesNotMatch(h.get('preview').textContent,/Scegli \/ conferma/);
 const current=descendants(editor).find(n=>n.tag==='select');assert.ok(current.value.includes('tools'));
});

test('unresolved mappings remain visible with an actionable search',async()=>{
 const h=harness();await tick();h.report.resolutions[0].status='ambiguous';h.report.valid=false;h.report.errors=['ID ambiguo'];
 h.get('sourceText').value='Research tools.';await h.get('acquire').onclick();h.get('guideJson').value=JSON.stringify(definition);await h.get('validate').onclick();
 assert.equal(h.get('preview').children[0].tag,'article');assert.ok(descendants(h.get('preview').children[0]).some(n=>n.tag==='input'&&n.type==='search'));
});
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
