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

test('source refresh is an explicit optional video preparation request',async()=>{
 const h=harness();await tick();h.get('videoUrl').value='https://www.youtube.com/watch?v=Ht4e89B02Wo';h.get('videoRefreshSource').checked=true;
 await h.get('prepareVideo').onclick();assert.equal(h.requests.find(r=>r.path==='/api/import/video').body.refresh_source,true);
});

test('thematic event download includes exact answer to event links independently of selected focus tree',async()=>{
 const h=harness();h.catalog.push({kind:'focus',id:'focus',scope:'another_tree'},
  {kind:'event',id:'example.1',name:'Invitation',localized_name:'Invito'},
  {kind:'event_option',id:'example.1.a',event_id:'example.1',name:'Accept',localized_name:'Accetta'},
  {kind:'event_option',id:'example.1.b',event_id:'example.1',name:'Decline',localized_name:'Rifiuta'});
 await tick();h.get('treeScope').value='unrelated_tree';h.get('treeScope').onchange();h.get('catalogSection').value='events';h.get('catalogSection').onchange();h.get('downloadSection').onclick();
 const value=JSON.parse(await h.downloads[0].text());
 assert.equal(value.catalog_scope,'events');assert.equal(value.reference_language,'english');
 assert.equal(value.event_filter.minimum_distinct_options,2);
 assert.equal(value.catalog.length,3);
 assert.equal(value.catalog.find(e=>e.id==='example.1.a').event_id,'example.1');
 assert.equal(value.catalog.find(e=>e.id==='example.1.a').localized_name,'Accetta');
 assert.equal(h.requests.some(r=>r.path.endsWith('/install')||r.path.endsWith('/extract')),false);
 const html=fs.readFileSync('src/hoi4companion/web/importer.html','utf8');
 assert.match(html,/id="downloadSection"/);
});

test('thematic catalogs partition every entity once, retain reference fields, and keep unknown kinds',()=>{
 const catalog=['focus','technology','equipment','building','state','project','law','idea','advisor','decision','event','event_option','future_kind'].map((kind,i)=>({kind,id:String(i),scope:kind==='focus'?'tree':undefined,event_id:kind==='event_option'?'10':undefined}));
 const before=JSON.stringify(catalog),sections=model.catalogSections(catalog);
 assert.equal(sections.flatMap(s=>s.catalog).length,catalog.length);
 for(const entry of catalog)assert.equal(sections.filter(s=>s.catalog.includes(entry)).length,1);
 assert.deepEqual(sections.find(s=>s.id==='research').catalog.map(e=>e.kind),['technology']);
 assert.deepEqual(sections.find(s=>s.id==='production').catalog.map(e=>e.kind),['equipment']);
 assert.deepEqual(sections.find(s=>s.id==='construction').catalog.map(e=>e.kind),['building','state']);
 assert.deepEqual(sections.find(s=>s.id==='politics').catalog.map(e=>e.kind),['law','idea','advisor','decision']);
 assert.equal(sections.find(s=>s.id==='events').catalog[1].event_id,'10');
 assert.equal(sections.find(s=>s.id==='other').catalog[0].kind,'future_kind');
 assert.equal(JSON.stringify(catalog),before);
});

test('section exports scope only focuses, preserve all other topics, and permit single-answer events',async()=>{
 const h=harness();h.catalog.push({kind:'focus',id:'a',scope:'selected'},{kind:'focus',id:'b',scope:'other'},
  {kind:'technology',id:'tech',upgrade_successors:['next']},{kind:'equipment',id:'rifles',family_id:'infantry'},
  {kind:'event',id:'single'},{kind:'event_option',id:'single.a',event_id:'single'});
 await tick();h.get('treeScope').value='selected';h.get('treeScope').onchange();
 h.get('catalogSection').value='focus';h.get('catalogSection').onchange();h.get('downloadSection').onclick();
 let value=JSON.parse(await h.downloads.at(-1).text());assert.deepEqual(value.catalog.map(e=>e.id),['a']);assert.equal(value.reference_language,'english');
 h.get('catalogSection').value='research';h.get('catalogSection').onchange();h.get('downloadSection').onclick();
 value=JSON.parse(await h.downloads.at(-1).text());assert.deepEqual(value.catalog,[h.catalog[2]]);
 h.get('catalogSection').value='events';h.get('catalogSection').onchange();assert.equal(h.get('downloadSection').disabled,true);
 h.get('eventsMultiple').checked=false;h.get('eventsMultiple').onchange();assert.equal(h.get('downloadSection').disabled,false);h.get('downloadSection').onclick();
 value=JSON.parse(await h.downloads.at(-1).text());assert.deepEqual(value.catalog.map(e=>e.id),['single','single.a']);assert.equal(value.event_filter,undefined);
 h.get('downloadCatalog').onclick();value=JSON.parse(await h.downloads.at(-1).text());assert.equal(value.catalog.length,5);
 assert.equal(h.requests.some(r=>r.path.endsWith('/install')||r.path.endsWith('/extract')),false);
});

test('event export removes single-answer events, duplicate IDs and orphan answers do not count as choices',()=>{
 const catalog=[{kind:'event',id:'single'},{kind:'event_option',id:'single.a',event_id:'single'},
  {kind:'event_option',id:'single.a',event_id:'single'},
  {kind:'event',id:'empty'},{kind:'event',id:'multi'},
  {kind:'event_option',id:'multi.a',event_id:'multi'},
  {kind:'event_option',id:'multi.b',event_id:'multi'},
  {kind:'event_option',id:'orphan.a',event_id:'orphan'},
  {kind:'event_option',id:'orphan.b',event_id:'orphan'}];
 const before=JSON.stringify(catalog),result=model.eventCatalog(catalog);
 assert.deepEqual(result.catalog.map(e=>e.id),['multi','multi.a','multi.b']);
 assert.equal(result.event_filter.basis,'defined_option_ids');
 assert.equal(JSON.stringify(catalog),before);
});

test('country event filtering uses recipient conditions, retains uncertainty, and preserves parent-answer links',()=>{
 const tag=value=>({op:'tag',value}),catalog=[{kind:'focus',id:'start',scope:'german_focus',countries:['GER']},
  {kind:'event',id:'foreign_name',country_condition:tag('GER')},
  {kind:'event_option',id:'foreign.a',event_id:'foreign_name',country_condition:tag('GER')},
  {kind:'event_option',id:'foreign.b',event_id:'foreign_name',country_condition:tag('FRA')},
  {kind:'event',id:'GER_wrong',country_condition:tag('FRA')},
  {kind:'event_option',id:'wrong.a',event_id:'GER_wrong'},
  {kind:'event',id:'uncertain',country_condition:{op:'any',args:[tag('FRA'),{op:'unknown'}]}},
  {kind:'event_option',id:'uncertain.a',event_id:'uncertain'},
  {kind:'event',id:'shared',country_condition:{op:'true'}},
  {kind:'event_option',id:'shared.a',event_id:'shared'},
  {kind:'event',id:'legacy_no_metadata'},{kind:'event_option',id:'legacy.a',event_id:'legacy_no_metadata'}];
 const before=JSON.stringify(catalog),sections=model.catalogSections(catalog,'german_focus'),events=sections.find(s=>s.id==='events');
 assert.deepEqual(events.catalog.map(e=>e.id),['foreign_name','foreign.a','uncertain','uncertain.a','shared','shared.a','legacy_no_metadata','legacy.a']);
 assert.deepEqual(events.country_filter.countries,['GER']);assert.equal(events.country_filter.excluded_events,1);assert.equal(events.country_filter.retained_unknown_events,2);
 assert.equal(model.catalogSections(catalog).find(s=>s.id==='events').catalog.length,11);
 assert.equal(JSON.stringify(catalog),before);
 // A shared focus tree may represent several countries: include matches to any of them.
 catalog[0].countries.push('FRA');assert.equal(model.catalogSections(catalog,'german_focus').find(s=>s.id==='events').catalog.length,11);
});

test('country condition evaluation keeps unknown scopes and supports conjunction and negation',()=>{
 const tag=value=>({op:'tag',value});
 assert.equal(model.countryCompatibility({op:'all',args:[tag('FRA'),{op:'unknown'}]},'GER'),false);
 assert.equal(model.countryCompatibility({op:'any',args:[tag('FRA'),{op:'unknown'}]},'GER'),null);
 assert.equal(model.countryCompatibility({op:'not',arg:tag('FRA')},'GER'),true);
 assert.equal(model.countryCompatibility({op:'not',arg:{op:'unknown'}},'GER'),null);
 assert.equal(model.countryCompatibility({op:'future_operator'},'GER'),null);
});

test('politics export filters declared national constraints without filtering by ID prefixes',()=>{
 const tag=value=>({op:'tag',value}),original=value=>({op:'original_tag',value});
 const catalog=[{kind:'focus',id:'g',scope:'germany',countries:['GER']},
  {kind:'law',id:'global',country_condition:{op:'unknown'}},
  {kind:'advisor',id:'GER_foreign',country_condition:original('FRA')},
  {kind:'advisor',id:'FRA_shared',country_condition:{op:'unknown'},countries:['FRA']},
  {kind:'idea',id:'GER_name',country_condition:tag('FRA')},
  {kind:'decision',id:'local',country_condition:original('GER')},
  {kind:'decision',id:'not_local',country_condition:original('FRA')},
  {kind:'idea',id:'unknown_legacy'}];
 const before=JSON.stringify(catalog),s=model.catalogSections(catalog,'germany').find(s=>s.id==='politics');
 assert.deepEqual(s.catalog.map(e=>e.id),['global','FRA_shared','local','unknown_legacy']);
 assert.equal(s.country_filter.excluded_entries,3);assert.equal(s.country_filter.retained_unknown_entries,3);
 assert.equal(s.country_filter.original_tag_basis,'selected_nation');
 assert.equal(model.countryCompatibility(original('FRA'),'GER'),null);
 assert.equal(model.catalogSections(catalog).find(s=>s.id==='politics').catalog.length,7);
 assert.equal(JSON.stringify(catalog),before);
});

test('the politics selector, summary and complete export share the active national filter',async()=>{
 const h=harness();h.catalog.push({kind:'focus',id:'g',scope:'germany',countries:['GER']},
  {kind:'law',id:'shared',country_condition:{op:'unknown'}},
  {kind:'advisor',id:'german',country_condition:{op:'original_tag',value:'GER'}},
  {kind:'decision',id:'french',country_condition:{op:'original_tag',value:'FRA'}});
 await tick();h.get('treeScope').value='germany';h.get('treeScope').onchange();h.get('catalogSection').value='politics';h.get('catalogSection').onchange();
 assert.match(h.get('catalogSummary').textContent,/2 voci/);assert.match(h.get('catalogSummary').textContent,/Nazione: GER/);
 h.get('downloadSection').onclick();let value=JSON.parse(await h.downloads.at(-1).text());assert.deepEqual(value.catalog.map(e=>e.id),['shared','german']);
 h.get('downloadCatalog').onclick();value=JSON.parse(await h.downloads.at(-1).text());assert.equal(value.catalog.length,3);
});

test('legacy political metadata blocks only the affected thematic export and complete export',async()=>{
 const h=harness();h.catalog.push({kind:'focus',id:'g',scope:'germany',countries:['GER']},{kind:'advisor',id:'old'});
 await tick();h.get('treeScope').value='germany';h.get('treeScope').onchange();
 assert.equal(h.get('downloadSection').disabled,false);assert.equal(h.get('downloadCatalog').disabled,true);assert.equal(h.get('catalogWarning').hidden,false);
 h.get('catalogSection').value='politics';h.get('catalogSection').onchange();assert.equal(h.get('downloadSection').disabled,true);h.get('downloadSection').onclick();assert.equal(h.downloads.length,0);
 h.get('treeScope').value='';h.get('treeScope').onchange();assert.equal(h.get('downloadSection').disabled,false);
});

test('selected country filters thematic and complete exports before counting multiple responses',async()=>{
 const h=harness(),tag=value=>({op:'tag',value});h.catalog.push({kind:'focus',id:'start',scope:'tree',countries:['GER']},
  {kind:'event',id:'e',country_condition:tag('GER')},{kind:'event_option',id:'e.a',event_id:'e'},
  {kind:'event_option',id:'e.b',event_id:'e',country_condition:tag('FRA')},
  {kind:'event',id:'f',country_condition:tag('FRA')},{kind:'event_option',id:'f.a',event_id:'f'});
 await tick();h.get('treeScope').value='tree';h.get('treeScope').onchange();h.get('catalogSection').value='events';h.get('catalogSection').onchange();
 assert.equal(h.get('downloadSection').disabled,true);h.get('eventsMultiple').checked=false;h.get('eventsMultiple').onchange();h.get('downloadSection').onclick();
 let value=JSON.parse(await h.downloads.at(-1).text());assert.deepEqual(value.catalog.map(e=>e.id),['e','e.a']);
 h.get('downloadCatalog').onclick();value=JSON.parse(await h.downloads.at(-1).text());assert.deepEqual(value.catalog.map(e=>e.id),['start','e','e.a']);
});

test('event menu and summary count the downloaded events and answers after both filters',async()=>{
 const h=harness(),tag=value=>({op:'tag',value});h.catalog.push({kind:'focus',id:'g',scope:'tree',countries:['GER']},
  {kind:'event',id:'e',country_condition:tag('GER')},{kind:'event_option',id:'e.a',event_id:'e'},
  {kind:'event_option',id:'e.b',event_id:'e'},
  {kind:'event',id:'single',country_condition:tag('GER')},{kind:'event_option',id:'single.a',event_id:'single'},
  {kind:'event',id:'f',country_condition:tag('FRA')},{kind:'event_option',id:'f.a',event_id:'f'},
  {kind:'event_option',id:'f.b',event_id:'f'});
 await tick();const label=()=>h.get('catalogSection').children.find(n=>n.value==='events').textContent;
 assert.match(label(),/eventi: 2.*risposte: 4/);
 h.get('treeScope').value='tree';h.get('treeScope').onchange();h.get('catalogSection').value='events';h.get('catalogSection').onchange();
 assert.match(label(),/eventi: 1.*risposte: 2/);assert.match(h.get('catalogSummary').textContent,/eventi: 1.*risposte: 2/);
 h.get('eventsMultiple').checked=false;h.get('eventsMultiple').onchange();
 assert.match(label(),/eventi: 2.*risposte: 3/);assert.match(h.get('catalogSummary').textContent,/eventi: 2.*risposte: 3/);
 h.get('downloadSection').onclick();const file=JSON.parse(await h.downloads.at(-1).text());assert.equal(file.catalog.length,5);
});

test('old server catalog visibly marks national filtering inactive and does not export a falsely scoped event catalog',async()=>{
 const h=harness();h.catalog.push({kind:'focus',id:'g',scope:'tree',countries:['GER']},
  {kind:'event',id:'e'},{kind:'event_option',id:'e.a',event_id:'e'},{kind:'event_option',id:'e.b',event_id:'e'});
 await tick();h.get('treeScope').value='tree';h.get('treeScope').onchange();h.get('catalogSection').value='events';h.get('catalogSection').onchange();
 assert.equal(h.get('catalogWarning').hidden,false);assert.match(h.get('catalogWarning').textContent,/Filtro nazionale non attivo/);
 assert.doesNotMatch(h.get('catalogSummary').textContent,/Nazione non determinabile/);
 assert.equal(h.get('downloadSection').disabled,true);h.get('downloadSection').onclick();assert.equal(h.downloads.length,0);
 assert.equal(h.get('downloadCatalog').disabled,true);
 h.get('treeScope').value='';h.get('treeScope').onchange();assert.equal(h.get('catalogWarning').hidden,true);
 assert.equal(h.get('downloadSection').disabled,false);h.get('downloadSection').onclick();assert.equal(h.downloads.length,1);
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
