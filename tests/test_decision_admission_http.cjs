'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),{spawn}=require('node:child_process'),{once}=require('node:events');
const {ROOT,json,write,temporary,sha}=require('../scripts/native-tooling.cjs');
const {zip,word}=require('./native-fixtures.cjs');
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
function fixture(){
 const cases=json(path.join(__dirname,'fixtures/guide-binding-probe-cases-v1.json')).cases;
 const second=json(path.join(__dirname,'fixtures/guide-binding-safety-audit-v1.json')).decision_shadow_policy.cases.find(c=>c.guide_id==='word-67736ac669bdcbf1');
 const sov=structuredClone(cases.find(c=>c.country==='SOV'));
 sov.source_guide=second.guide_id;sov.source_step_key=second.step_key;cases.push(sov);return cases;
}
const rows=g=>Object.values(g.tracks).flat();
const decision=g=>rows(g).find(r=>r.action?.params?.decision_id);
const record=g=>decision(g)?.action.params.decision_resolution;

// Small, deterministic script installation: genuine parser/catalog paths, no game or mod files.
function condition(node){
 switch(node?.op){
  case 'tag':case 'original_tag':return `${node.op} = ${node.value}`;
  case 'all':case 'any':return `${node.op==='all'?'AND':'OR'} = { ${(node.args||[]).map(condition).join(' ')} }`;
  case 'not':return `NOT = { ${condition(node.arg)} }`;
  case 'true':return '';
  case 'false':return 'NOT = { always = yes }';
  default:return 'has_country_flag = integration_unknown';
 }
}
function gameFixture(root){
 const entries=new Map();for(const c of fixture())for(const e of c.entries)entries.set(e.kind+':'+e.id,e);
 entries.set('decision:war_bonds',{kind:'decision',id:'war_bonds',name:'War Bonds',group:'generic',country_condition:{op:'unknown'}});
 const game=path.join(root,'script-installation');
 fs.mkdirSync(path.join(game,'common/national_focus'),{recursive:true});
 const put=(name,text)=>{const file=path.join(game,name);fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,text);};
 let labels='l_english:\n ENG:0 "United Kingdom"\n SOV:0 "Soviet Union"\n FRA:0 "France"\n NOR:0 "Norway"\n SPR:0 "Spain"\n POL:0 "Poland"\n';
 put('common/national_focus/anchors.txt',['ENG','SOV','FRA','NOR','SPR','POL'].map(tag=>`focus_tree = { id = QA_${tag} country = { factor = 0 modifier = { add = 10 tag = ${tag} } } focus = { id = QA_${tag}_first x = 0 y = 0 cost = 1 } focus = { id = QA_${tag}_second x = 0 y = 1 cost = 1 prerequisite = { focus = QA_${tag}_first } } }`).join('\n'));
 const groups=new Map();
 for(const e of entries.values()){
  labels+=` ${e.id}:0 ${JSON.stringify(e.name)}\n`;
  if(e.kind==='decision'){
   const group=e.group||'generic';if(!groups.has(group))groups.set(group,[]);
   groups.get(group).push(`${e.id} = { available = { ${condition(e.country_condition)} } }`);
  }
 }
 put('common/decisions/decisions.txt',[...groups].map(([group,items])=>`${group} = { ${items.join('\n')} }`).join('\n'));
 put('events/events.txt',[...entries.values()].filter(e=>e.kind==='event').map(e=>
  `country_event = { id = ${e.id} title = ${e.id} trigger = { ${condition(e.country_condition)} } ${[...entries.values()].filter(o=>o.kind==='event_option'&&o.event_id===e.id).map(o=>`option = { name = ${o.id} trigger = { ${condition(o.country_condition)} } }`).join(' ')} }`).join('\n'));
 put('localisation/english/integration_l_english.yml',labels);
 return game;
}

test('decision safety across ZIP, HTTP, collection publication, persisted provenance and reload',
 {skip:process.platform!=='win32',timeout:300000},async t=>{
 const stage=temporary('hoi4-decision-http-'),root=stage.root,resources=path.join(ROOT,'src/hoi4companion');
 const exe=process.env.HOI4_BACKEND_EXE||path.join(resources,'_native/hoi4-backend.exe');
 const original={id:'original',title:'Existing guide',tracks:{operations:[{key:'note',action:{kind:'manual'},completion:{op:'check',key:'note'}}]}};
 const oldFile=path.join(root,'guides','old-guide.json');
 const historical={id:'old-guide',title:'Historical guide',applies_to:{country:'ENG'},tracks:{operations:[{key:'old-key',action:{kind:'manual',params:{decision_id:'SOV_propaganda_defend_moscow'}}}]}};
 write(oldFile,historical);const oldBytes=fs.readFileSync(oldFile);
 const config={path:path.join(root,'progress.sqlite3'),selection_path:path.join(root,'selection.json'),web_root:path.join(resources,'web'),reference_library:path.join(root,'references'),guide_directory:path.join(root,'guides'),guide_directories:[path.join(root,'guides')],association_directory:path.join(root,'associations'),shutdown_token:'b'.repeat(64),catalog:{settings:{game_dir:gameFixture(root),docs_dir:path.join(root,'game-documents'),mods:'off',country:'ENG',language:'english'}},guides:[original]};
 write(path.join(root,'config.json'),config);
 let child,port,launch=0;
 const saved=new Map(),origins=['explicit_id','inferred','user_reviewed','legacy_unknown'];
 async function api(url,body){
  const binary=Buffer.isBuffer(body),r=await fetch(`http://127.0.0.1:${port}${url}`,{method:body===undefined?'GET':'POST',headers:{'Content-Type':binary?'application/octet-stream':'application/json'},body:body===undefined?undefined:binary?body:JSON.stringify(body),signal:AbortSignal.timeout(30000)});
  return {status:r.status,value:await r.json()};
 }
 async function start(){
  const ready=path.join(root,`ready-${++launch}.json`);let output='';
  child=spawn(exe,['--config',path.join(root,'config.json'),'--port','0','--ready-file',ready],{cwd:ROOT,windowsHide:true,stdio:['ignore','pipe','pipe']});
  child.stdout.on('data',b=>output+=b);child.stderr.on('data',b=>output+=b);let error;child.on('error',e=>error=e);
  const deadline=Date.now()+90000;while(!fs.existsSync(ready)){if(error)throw error;if(child.exitCode!==null)throw Error(`Backend exited ${child.exitCode}: ${output}`);if(Date.now()>deadline)throw Error('Backend readiness timeout: '+output);await delay(25);}port=json(ready).port;
 }
 async function stop(){
  if(child?.exitCode===null){const exited=once(child,'exit');try{const r=await api('/api/shutdown',{token:config.shutdown_token});assert.equal(r.status,200);await exited;}catch(e){child.kill();await exited;throw e;}}child=null;
 }
 async function collection(id,automatic=false){
  const deadline=Date.now()+120000;
  while(true){const r=await api('/api/import/collection?id='+id);assert.equal(r.status,200);assert.notEqual(r.value.status,'failed',JSON.stringify(r.value));if(automatic?r.value.auto_finished===true:r.value.status==='prepared')return r.value;if(Date.now()>deadline)throw Error('Collection timeout '+JSON.stringify(r.value));await delay(25);}
 }
 async function report(c,item){const r=await api(`/api/import/collection/item?id=${c.id}&key=${item.key}`);assert.equal(r.status,200);return r.value;}
 async function preview(packet){const r=await api('/api/import/preview',{guide:packet.guide,sources:packet.sources});assert.equal(r.status,200);return r.value;}
 async function packet(origin,id){
  const source=(await api('/api/import/source',{text:`Source for ${id}: War Bonds and We shall…`})).value;
  const evidence=[{source_id:source.id,basis:'explicit',quote:source.text}];
  if(origin==='explicit_id'){
   const r=await api('/api/import/document',{document:{format:'hoi4-guide-authoring',authoring_schema_version:1,sources:[source],guide:{id,title:id,applies_to:{country:'ENG'},dependency_policy:'explicit',steps:[{key:'d',type:'decision',target:{id:'SOV_propaganda_defend_moscow'},evidence}]}}});assert.equal(r.status,200,JSON.stringify(r.value));return r.value;
  }
  const params=origin==='inferred'?{decision_name:'War Bonds'}:{decision_id:'SOV_propaganda_defend_moscow'};
  if(origin==='user_reviewed')params.decision_resolution={schema_version:1,id:params.decision_id,origin,query:'We shall…'};
  return {sources:[source],guide:{id,title:id,applies_to:{country:'ENG'},dependency_policy:'explicit',sources:[source],tracks:{operations:[{key:'d',action:{kind:'manual',params},evidence}]}}};
 }
 function remember(guide){const file=path.join(root,'guides',guide.id+'.json');assert.ok(fs.existsSync(file));saved.set(guide.id,fs.readFileSync(file));return file;}
 function unchanged(){assert.deepEqual(fs.readFileSync(oldFile),oldBytes);for(const [id,bytes]of saved)assert.deepEqual(fs.readFileSync(path.join(root,'guides',id+'.json')),bytes,id);}
 try{
  await start();t.diagnostic('Backend SHA-256: '+sha(exe));
  const check=await api('/api/guide/checks?id=original',{key:'note',checked:true});assert.equal(check.status,200,JSON.stringify(check.value));
  const baseline=(await api('/api/guide/progress?id=original')).value;
  const active=(await api('/api/guide/active')).value;
  const archiveEntries={};
  for(const [i,c]of fixture().entries())archiveEntries[`Country Guides/${c.country} case ${i}.docx`]=word([c.query,`Context ${c.source_guide}`],[],[`QA_${c.country}_first`,`QA_${c.country}_second`]);
  archiveEntries['Country Guides/ENG literal ID.docx']=word(['war_bonds'],[],['QA_ENG_first','QA_ENG_second']);
  const archive=zip(archiveEntries);
  let manual,automatic;
  await t.test('full Word ZIP extraction requires review for all four flagged cases',async()=>{
   const upload=await api('/api/import/collections?name=decision-cases.zip',archive);assert.equal(upload.status,200);manual=await collection(upload.value.id);
   assert.equal(manual.counts.total,9);assert.equal(manual.counts.needs_revision,4,JSON.stringify({items:manual.items,first:await report(manual,manual.items[0])}));assert.equal(manual.counts.ready,5);
   for(const [i,c]of fixture().entries()){
    const item=manual.items.find(row=>row.entry.endsWith(`case ${i}.docx`));assert.ok(item);const p=await report(manual,item);
    assert.equal(decision(p.guide).action.params.decision_id,c.saved_id);assert.equal(record(p.guide).origin,'inferred');
    assert.equal(p.valid,c.expected_shadow_status!=='requires_review',JSON.stringify(p.errors));
    if(!p.valid){assert.equal(p.decision_validation[0].status,'ambiguous');assert.equal((await api('/api/import/install',{guide:p.guide,sources:p.sources,reviewed:true})).status,400);}
   }
   const literal=await report(manual,manual.items.find(row=>row.entry.endsWith('literal ID.docx')));assert.equal(record(literal.guide).origin,'explicit_id');
   assert.equal((await api('/api/import/collection/commit',{id:manual.id,reviewed:false})).status,400);
   const committed=await api('/api/import/collection/commit',{id:manual.id,reviewed:true});assert.equal(committed.status,200);assert.equal(committed.value.counts.installed,5);assert.equal(committed.value.counts.needs_revision,4);
   for(const item of committed.value.items.filter(row=>row.status==='installed')){const p=await report(manual,item);remember(p.guide);assert.deepEqual(record(json(path.join(root,'guides',p.guide.id+'.json'))),record(p.guide));}
  });
  await t.test('re-preview and reviewed=true cannot launder ambiguous inference',async()=>{
   for(const item of manual.items.filter(row=>row.status==='needs_revision')){
    let p=await report(manual,item);const expected=structuredClone(record(p.guide)),id=decision(p.guide).action.params.decision_id;
    for(let n=0;n<3;n++){const sources=p.sources||[...p.guide.sources];p=await preview({guide:JSON.parse(JSON.stringify(p.guide)),sources});assert.equal(p.valid,false);assert.deepEqual(record(p.guide),expected);assert.equal(decision(p.guide).action.params.decision_id,id);}
   }
   const duplicate=await api('/api/import/collections?name=repeated.zip',archive);assert.equal(duplicate.value.id,manual.id);
   const again=await api('/api/import/collection/commit',{id:manual.id,reviewed:true});assert.equal(again.value.counts.needs_revision,4);unchanged();
  });
  await t.test('automatic ZIP publication installs foreign controls and withholds ambiguities',async()=>{
   const entries={};for(const [name,bytes]of Object.entries(archiveEntries))entries[name]=Buffer.from(bytes);
   entries['Country Guides/ENG extra.docx']=word(['Create Agency']);
   const payload=zip(entries),uploaded=await api('/api/import/collections?automatic=1&name=automatic.zip',payload);assert.equal(uploaded.status,200);automatic=await collection(uploaded.value.id,true);
   assert.equal(automatic.counts.needs_revision,4);assert.equal(automatic.counts.installed,6);assert.equal(automatic.review_mode,'automatic');
   // Previously installed controls must retain their exact files.
   unchanged();const duplicate=await api('/api/import/collections?automatic=1&name=repeat-auto.zip',payload);assert.equal(duplicate.value.id,automatic.id);await collection(automatic.id,true);unchanged();
  });
  await t.test('HTTP document/preview/install preserves all four provenance states',async()=>{
   for(const origin of origins){
    const input=await packet(origin,'http-'+origin),p=await preview(input);assert.equal(p.valid,true,JSON.stringify(p.errors));assert.equal(record(p.guide).origin,origin);
    const second=await preview({guide:JSON.parse(JSON.stringify(p.guide)),sources:input.sources});assert.deepEqual(record(second.guide),record(p.guide));
    assert.equal((await api('/api/import/install',{guide:second.guide,sources:input.sources,reviewed:false})).status,400);
    const installed=await api('/api/import/install',{guide:second.guide,sources:input.sources,reviewed:true});assert.equal(installed.status,200,JSON.stringify(installed.value));
    const file=remember(p.guide);assert.deepEqual(record(json(file)),record(p.guide));
    const loaded=await api('/api/guide?id='+p.guide.id);assert.equal(loaded.status,200);assert.equal(record(loaded.value).origin,origin);
    assert.equal((await api('/api/import/install',{guide:second.guide,sources:input.sources,reviewed:true})).status,400);unchanged();
   }
  });
  // Older persisted collection reports can contain any provenance state. Seed only
  // isolated reports using real HTTP previews; do not pretend Word encodes review metadata.
  async function historicalCollection(mode){
   const entries={};for(const origin of origins)entries[`Country Guides/ENG ${mode} ${origin}.docx`]=word([`Context ${mode} ${origin}`]);
   const uploaded=await api('/api/import/collections?name='+mode+'.zip',zip(entries));assert.equal(uploaded.status,200);const c=await collection(uploaded.value.id);
   const reports=[];for(const origin of origins){const input=await packet(origin,mode+'-'+origin),p=await preview(input);assert.equal(p.valid,true,JSON.stringify(p.errors));reports.push({...p,sources:input.sources});}
   return {c,reports};
  }
  const seededManual=await historicalCollection('persisted-manual'),seededAutomatic=await historicalCollection('persisted-auto');
  await stop();
  for(const [seed,auto]of [[seededManual,false],[seededAutomatic,true]]){
   const manifestPath=path.join(root,'collections',seed.c.id,'manifest.json'),manifest=json(manifestPath);
   manifest.items.forEach((item,i)=>{const p=seed.reports[i];write(path.join(root,'collections',seed.c.id,'reports',item.key+'.json'),p);item.guide_id=p.guide.id;item.status='ready';});
   manifest.status='prepared';manifest.auto_import=auto;manifest.auto_finished=false;manifest.import_stage='saving';write(manifestPath,manifest);
  }
  await t.test('restart and manual/automatic commit retain persisted report provenance',async()=>{
   await start();
   const committed=await api('/api/import/collection/commit',{id:seededManual.c.id,reviewed:true});assert.equal(committed.status,200);assert.equal(committed.value.counts.installed,4);
   const auto=await collection(seededAutomatic.c.id,true);assert.equal(auto.counts.installed,4);
   for(const seed of [seededManual,seededAutomatic])for(const p of seed.reports){const file=remember(p.guide);assert.deepEqual(record(json(file)),record(p.guide));}
   unchanged();
  });
  await t.test('reload retains guide IDs, legacy file bytes, active selection and saved checks',async()=>{
   await stop();await start();unchanged();
   const restored=(await api('/api/guide/progress?id=original')).value;
   for(const key of ['run_id','checks','started','construction','research_bindings','production'])if(baseline[key]!==undefined)assert.deepEqual(restored[key],baseline[key],key);
   assert.equal(restored.checks.note,true);
   assert.equal(record((await api('/api/guide?id=old-guide')).value),undefined);
   assert.equal((await api('/api/guide/active')).value.guide_id,active.guide_id);
   for(const [id,bytes]of saved){const loaded=await api('/api/guide?id='+id);assert.equal(loaded.status,200);const persisted=JSON.parse(bytes);if(record(persisted))assert.deepEqual(record(loaded.value),record(persisted));}
   for(const item of manual.items.filter(row=>row.status==='needs_revision')){const p=await report(manual,item);assert.equal(record(p.guide).origin,'inferred');assert.equal((await preview({guide:p.guide,sources:p.sources})).valid,false);}
  });
  await t.test('four original corpus DOCX files traverse the real installation catalog and ZIP HTTP pipeline',
   {skip:!process.env.HOI4_DECISION_CORPUS_ZIP||!process.env.HOI4_DECISION_GAME_DIR},async()=>{
    await stop();const actual=path.join(root,'original-corpus');
    for(const [key,suffix]of [['path','progress.sqlite3'],['selection_path','selection.json'],['reference_library','references'],['guide_directory','guides'],['association_directory','associations']])config[key]=path.join(actual,suffix);
    config.guide_directories=[config.guide_directory];config.catalog.settings.game_dir=process.env.HOI4_DECISION_GAME_DIR;config.catalog.settings.docs_dir=path.join(actual,'game-documents');write(path.join(root,'config.json'),config);
    const sourceBytes=fs.readFileSync(process.env.HOI4_DECISION_CORPUS_ZIP);await start();
    const upload=await api('/api/import/collections?automatic=1&name=original-four.zip',sourceBytes);assert.equal(upload.status,200);const c=await collection(upload.value.id,true);assert.equal(c.counts.total,4);
    const cases=fixture().filter(c=>c.expected_shadow_status==='requires_review');
    const records=new Map();
    for(const expected of cases){
     const item=c.items.find(row=>row.guide_id===expected.source_guide);assert.ok(item,JSON.stringify(c.items));
     const p=await report(c,item),matched=p.decision_validation?.find(r=>r.decision_id===expected.saved_id&&r.status==='ambiguous');
     assert.ok(matched,JSON.stringify({guide_id:item.guide_id,validation:p.decision_validation,errors:p.errors}));
     assert.equal(item.status,'needs_revision');assert.equal(matched.provenance.origin,'inferred');assert.equal(matched.provenance.query,expected.query);
     records.set(item.key,matched.provenance);
     const again=await preview({guide:p.guide,sources:p.sources});assert.equal(again.valid,false);assert.deepEqual(again.decision_validation.find(r=>r.decision_id===expected.saved_id).provenance,matched.provenance);
     assert.equal(fs.existsSync(path.join(actual,'guides',item.guide_id+'.json')),false);
     assert.equal((await api('/api/import/install',{guide:p.guide,sources:p.sources,reviewed:true})).status,400);
    }
    const manualCommit=await api('/api/import/collection/commit',{id:c.id,reviewed:true});assert.equal(manualCommit.status,200);assert.equal(manualCommit.value.counts.installed,0);assert.equal(manualCommit.value.counts.needs_revision,4);
    assert.equal((await api('/api/import/collections?automatic=1&name=repeated-original.zip',sourceBytes)).value.id,c.id);
    for(const item of c.items){const p=await report(c,item);assert.deepEqual(p.decision_validation.find(r=>r.status==='ambiguous').provenance,records.get(item.key));}
    const mines=fixture().find(row=>row.country==='NOR'),src=(await api('/api/import/source',{text:mines.query})).value;
    const foreign=await preview({sources:[src],guide:{id:'real-norwegian-reference',title:'Norwegian reference',applies_to:{country:'NOR'},sources:[src],tracks:{operations:[{key:'mines',action:{kind:'manual',params:{decision_name:mines.query}},evidence:[{source_id:src.id,basis:'explicit',quote:src.text}]}]}}});
    assert.equal(foreign.valid,true,JSON.stringify(foreign.errors));assert.equal(decision(foreign.guide).action.params.decision_id,'SWE_repair_the_mines');assert.equal(record(foreign.guide).origin,'inferred');
    assert.equal(c.counts.installed,0);assert.deepEqual(fs.readFileSync(process.env.HOI4_DECISION_CORPUS_ZIP),sourceBytes);unchanged();
    t.diagnostic('Original corpus: four DOCX IDs verified; all four ambiguous decisions withheld by actual ZIP/HTTP automatic import.');
   });
 }finally{
  try{await stop();}finally{stage.close();assert.equal(fs.existsSync(root),false);}
 }
});
