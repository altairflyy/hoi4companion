'use strict';
// Offline diagnostics only. Heuristic tags below never enter import or runtime code.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),{spawnSync}=require('node:child_process');
const {ROOT,json,temporary}=require('./native-tooling.cjs');
const VERSION=1,SEED='same-author-actions-v1',sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const TOOLING_SHA256=sha(Buffer.concat([fs.readFileSync(__filename),fs.readFileSync(path.join(__dirname,'benchmark-guide-sources.ps1'))]));
const lines=text=>String(text).split(/\r\n|[\n\r\x0b\x0c\x1c-\x1e\u0085\u2028\u2029]/u).filter(s=>s.trim());
const norm=text=>String(text).normalize('NFKC').replace(/\s+/g,' ').trim();
const sorted=value=>Array.isArray(value)?value.map(sorted):value&&typeof value==='object'?Object.fromEntries(Object.keys(value).sort().map(k=>[k,sorted(value[k])])):value;
const hashJson=value=>sha(JSON.stringify(sorted(value)));
const sum=(rows,fn)=>rows.reduce((n,row)=>n+fn(row),0);
const tally=(rows,fn)=>{const out={};for(const row of rows)for(const key of [fn(row)].flat())out[key]=(out[key]||0)+1;return Object.fromEntries(Object.entries(out).sort());};
function capturedParagraphs(text){
 // Acquired text loses paragraph boundaries. These records are physical lines,
 // never silently presented as original DOCX paragraphs.
 const result=[];let priority=false,rowIndex=0;
 for(const block of String(text).split('[Table row]\n').slice(1)){
  rowIndex++;const cells=[...block.matchAll(/\[Column (\d+)\]\n([\s\S]*?)(?=\[Column \d+\]\n|$)/g)];
  const headings=cells.map(m=>norm(m[2]).toUpperCase().replace(/\*$/,''));
  if(headings.join('|')==='ACTIONS|RESEARCH PRIORITY|FOCUS PRIORITY'){priority=true;continue;}
  if(cells.length!==3){priority=false;continue;}
  if(priority)for(const [i,text] of lines(cells[0][2]).entries())result.push({row_index:rowIndex,column_index:1,paragraph_index:null,line_index:i+1,text,coordinate_kind:'captured_text_line'});
 }
 return result;
}
function tags(text){
 const families=[],patterns={
  production:/\b(?:MIL|NAV|AIR)\s*:|\b(?:production|factories?\s+(?:on|to)|convoys?|MIOs?|dockyards?)\b/i,
  construction:/\b(?:build|construct|repair|INFRAs?|CIVs?|MILs?|forts?|railways?|supply hubs?|facilit(?:y|ies))\b/i,
  politics:/\b(?:LAW\s*:|PP\b|advisor|adviser|jurist|lawyer|bureaucrat|conscription|economy|trade law|politic|hire|appoint|expert|LPF|Ratsu)/i,
  army:/\b(?:ARMY\s*:|recruit|train|exercise|divisions?|INF\b|CAV\b|template|battalions?|generals?|field marshal|fallback|frontline|front line|army|garrison)/i,
  diplomacy:/\b(?:relations?|non.aggression|NAP\b|pact|guarantee|justify|faction|invite|diplomatic|lend.lease|access|join.*war)/i,
  intelligence:/\b(?:agenc(?:y|ies)|sp(?:y|ies)|operatives?|network|intelligence|infiltrat|propaganda)/i,
  events:/\b(?:event|choose|select.*option|reject kings|accept.*offer|popup|pop.up)/i,
  doctrine:/\b(?:doctrines?|spirit|mastery|grand battleplan|superior firepower)/i,
  special_projects:/\b(?:special project|flame(?:thrower)?\s*(?:tank|panzer)|engineering vehicles|scientist|breakthrough)/i,
  focus_research:/\b(?:focus|research|tech(?:nology)?|UP\b)/i,
  campaign_strategy:/\b(?:war|peace|annex|capitulat|invad|naval invasion|defen[cs]|attack|conquer|encircl|offensive|push|rush|landings?)/i
 };
 for(const [family,re] of Object.entries(patterns))if(re.test(text))families.push(family);
 const syntax=[];
 if(/\b(?:on|after|before|when|once|at|until)\b/i.test(text))syntax.push('trigger');
 if(/\b(?:on|after|when|once|before)\b[^:]*:/i.test(text))syntax.push('trigger_header');
 if((text.match(/\b(?:on|after|when|once|before|until|if)\b/gi)||[]).length>1)syntax.push('nested_or_multiple_conditions');
 if(/\b(?:and|then|or)\b|[+;&]/i.test(text))syntax.push('conjunction');
 if(/\b\d+\s*PP\b/i.test(text))syntax.push('pp_cost');
 if(/\b[A-Z]{2,}\b/.test(text))syntax.push('abbreviation');
 if(/(?:…|\.{3})/.test(text))syntax.push('ellipsis');
 if(/\b(?:all|remaining|->|maximum)\b|->/i.test(text))syntax.push('relative_quantity');
 if(/\b(?:not|never|ignore|without|do not|don't)\b/i.test(text))syntax.push('negation_or_exception');
 const advice=/^\s*(?:good (?:job|luck)|well done|congratulations|have fun|enjoy|this (?:is|was) (?:a|the) guide)[!.\s]*$/i.test(text);
 const header=/^\s*(?:on|after|when|once|before|until|at)\b.+(?:[:…]|\.{3})\s*$/i.test(text);
 return {families:families.length?families:['unclassified'],syntax,disposition:advice?'advice_candidate':header?'trigger_header_candidate':families.length?'action_candidate':'unclassified'};
}
function automatic(node){
 if(!node||typeof node!=='object')return false;
 if(node.op==='check')return false;
 if(node.op==='fact')return true;
 if(['all','any'].includes(node.op))return Array.isArray(node.args)&&node.args.length>0&&node.args.every(automatic);
 if(node.op==='not')return automatic(node.arg);
 return false;
}
function payload(step){
 const p=step.action?.params||{};
 const fields=['operation_id','decision_id','decision_name','advisor_id','advisor_name','law_id','law_name','idea_id','idea_name','person_id','person_name','building_id','building_name','production_targets','production_selector','doctrine_id','doctrine_name','project_id','project_name','event_id','event_option_id','agency_name','template_name','template_id','division_template','unit_template'];
 const present=fields.filter(k=>p[k]!==undefined&&p[k]!==null&&p[k]!==''&&(!Array.isArray(p[k])||p[k].length));
 if(step.action?.kind&& !['manual','free'].includes(step.action.kind))present.push('action.kind');
 return present;
}
function stepsOf(guide){return Object.entries(guide.tracks||{}).flatMap(([track,rows])=>rows.map((step,index)=>({...step,_track:track,_index:index})));}
function countryCondition(node,context){
 if(!node||typeof node!=='object')return null;
 if(node.op==='const')return typeof node.value==='boolean'?node.value:null;
 if(['tag','original_tag'].includes(node.op))return typeof context[node.op]==='string'?context[node.op]===node.value:null;
 if(node.op==='not'){const value=countryCondition(node.arg,context);return value===null?null:!value;}
 if(['all','any'].includes(node.op)&&Array.isArray(node.args)&&node.args.length){const values=node.args.map(v=>countryCondition(v,context));return node.op==='all'?values.includes(false)?false:values.includes(null)?null:true:values.includes(true)?true:values.includes(null)?null:false;}
 return null;
}
function bindingAudit(guides,catalog){
 if(!catalog?.entries)return {status:'not_measured',reason:'No frozen catalog was supplied.'};
 const index=new Map();for(const entry of catalog.entries){const key=entry.kind+':'+entry.id;if(!index.has(key))index.set(key,[]);index.get(key).push(entry);}
 const counts={},issues=[],byKind={};
 for(const guide of guides)for(const step of stepsOf(guide)){
  const p=step.action?.params||{},refs=[];
  if(step.action?.entity?.id)refs.push({kind:step.action.entity.kind,id:step.action.entity.id,field:'action.entity',scope:step.action.entity.scope});
  for(const [field,kind] of Object.entries({decision_id:'decision',advisor_id:'advisor',law_id:'law',idea_id:'idea',project_id:'project',doctrine_id:'doctrine',event_id:'event',event_option_id:'event_option'}))if(typeof p[field]==='string')refs.push({kind,id:p[field],field});
  for(const ref of refs.filter((r,i)=>refs.findIndex(other=>other.kind===r.kind&&other.id===r.id)===i)){
   let candidates=index.get(ref.kind+':'+ref.id)||[];
   if(ref.scope)candidates=candidates.filter(e=>!e.scope||e.scope===ref.scope);
   if(ref.kind==='advisor'&&p.role)candidates=candidates.filter(e=>!e.slot||e.slot===p.role);
   const statuses=candidates.map(e=>countryCondition(e.country_condition,{tag:guide.applies_to?.country,original_tag:guide.applies_to?.country}));
   const status=!candidates.length?'id_absent_or_scope_mismatch':candidates.length>1?'multiple_catalog_definitions':statuses.every(s=>s===false)?'contradiction_under_declared_origin':candidates.every(e=>!e.country_condition)?'country_metadata_not_exposed':statuses.some(s=>s===null)?'country_gate_unknown':'no_static_country_contradiction';
   counts[status]=(counts[status]||0)+1;byKind[ref.kind]=(byKind[ref.kind]||0)+1;
   if(['id_absent_or_scope_mismatch','multiple_catalog_definitions','contradiction_under_declared_origin'].includes(status))issues.push({guide_id:guide.id,country:guide.applies_to?.country||null,step_key:step.key,title:step.title,reference:ref,status,candidates:candidates.map(e=>({kind:e.kind,id:e.id,name:e.name,slot:e.slot,scope:e.scope,country_condition:e.country_condition})),quote:step.evidence?.[0]?.quote||null});
  }
 }
 return {status:'static_diagnostic_not_entity_accuracy',context_assumption:'For contradiction probes only, declared guide country is assumed to be the original starting country as well as current tag. Neither is live evidence.',unknown_policy:'Unknown predicates remain unknown; a foreign-looking ID prefix is never a contradiction.',counts,by_kind:byKind,issues};
}
function primarySource(guide){return (guide.sources||[]).find(s=>s.type==='file'&&s.title===guide.title)||(guide.sources||[]).find(s=>s.type==='file'&&s.text?.includes('[Table row]'))||(guide.sources||[]).find(s=>s.type==='file');}
function identitySnapshot(guide){
 return stepsOf(guide).map(s=>({key:s.key,track:s._track,index:s._index,source_key:s.action?.params?.source_action?.key||null,source_ordinal:s.action?.params?.source_action?.ordinal||null,evidence_sha256:hashJson(s.evidence||[]),semantics_sha256:hashJson(Object.fromEntries(['action','when','completion','started_when','depends_on','dependency_policy','depends_on_policy','tracking','relative_research','research_choice'].filter(k=>s[k]!==undefined).map(k=>[k,s[k]])))}));
}
function archiveRequests(dataDir,guides){
 const byGuide=new Map(),directory=path.join(dataDir,'collections');
 if(fs.existsSync(directory))for(const id of fs.readdirSync(directory).sort()){
  const file=path.join(directory,id,'manifest.json');if(!fs.existsSync(file))continue;const m=json(file);
  for(const item of m.items||[])if(item.guide_id&&item.entry&&!byGuide.has(item.guide_id))byGuide.set(item.guide_id,{guide_id:item.guide_id,archive:path.join(directory,id,'source.zip'),entry:item.entry});
 }
 return guides.filter(g=>byGuide.has(g.id)).map(g=>byGuide.get(g.id));
}
function extractOriginals(dataDir,guides){
 const requests=archiveRequests(dataDir,guides),stage=temporary('hoi4-guide-benchmark-');
 try{
  const input=path.join(stage.root,'requests.json');fs.writeFileSync(input,JSON.stringify(requests));
  const result=spawnSync('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(ROOT,'scripts/benchmark-guide-sources.ps1'),'-Requests',input],{encoding:'utf8',windowsHide:true,timeout:120000,maxBuffer:16*1024*1024});
  if(result.error)throw result.error;if(result.status)throw Error(result.stderr||'Original-source extraction failed');
  return new Map(JSON.parse(result.stdout.replace(/^\uFEFF/,'')).map(row=>[row.guide_id,row]));
 }finally{stage.close();}
}
function inputFingerprint(dataDir){
 const items=[];
 for(const directory of ['guides','guide-associations']){const root=path.join(dataDir,directory);if(fs.existsSync(root))for(const name of fs.readdirSync(root).filter(n=>n.endsWith('.json')).sort())items.push([directory+'/'+name,sha(fs.readFileSync(path.join(root,name)))]);}
 for(const file of ['settings.json','reference-library/manifest.json']){const p=path.join(dataDir,file);items.push([file,fs.existsSync(p)?sha(fs.readFileSync(p)):null]);}
 items.push(['vocabulary',sha(fs.readFileSync(path.join(ROOT,'src/hoi4companion/guide_language.json')))]);
 return {sha256:hashJson(items),files:items};
}
function captureRuntime(dataDir,executable){
 const stage=temporary('hoi4-guide-benchmark-');
 try{
  if(!fs.existsSync(executable))throw Error('Build the offline example first: cargo build --release --locked --manifest-path native/backend/Cargo.toml --example guide_benchmark (use the existing backend target directory).');
  const before=inputFingerprint(dataDir),output=path.join(stage.root,'runtime.json'),config=path.join(stage.root,'config.json');
  fs.writeFileSync(config,JSON.stringify({guides_dir:path.join(dataDir,'guides'),association_directory:path.join(dataDir,'guide-associations'),settings_path:path.join(dataDir,'settings.json'),vocabulary_path:path.join(ROOT,'src/hoi4companion/guide_language.json'),library_dir:path.join(dataDir,'reference-library'),output}));
  const result=spawnSync(executable,[config],{windowsHide:true,encoding:'utf8',timeout:300000,maxBuffer:1024*1024});
  if(result.error)throw result.error;if(result.status)throw Error(result.stderr||'Offline Rust preparation failed');
  if(before.sha256!==inputFingerprint(dataDir).sha256)throw Error('Guide inputs changed during capture; discard this mixed snapshot and retry.');
  const runtime=json(output);runtime.benchmark_input=before;runtime.benchmark_executable_sha256=sha(fs.readFileSync(executable));return runtime;
 }finally{stage.close();}
}
function groupSteps(guide,source){
 const groups=new Map();
 for(const step of stepsOf(guide)){
  if(step._track!=='operations'&&!step.action?.params?.source_action)continue;
  const evidence=(step.evidence||[]).filter(e=>e.source_id===source.id&&typeof e.quote==='string');
  if(!evidence.length)continue;
  const reference=step.action?.params?.source_action;
  const key=reference?.key||step.key?.replace(/(?:(?:--operation-|--note-)\d+)+$/,'');
  if(!groups.has(key))groups.set(key,{key,steps:[],quote:evidence[0].quote,ordinal:reference?.ordinal||null});
  groups.get(key).steps.push(step);
 }
 return [...groups.values()].sort((a,b)=>a.key.localeCompare(b.key,undefined,{numeric:true}));
}
function attributeGroups(paragraphs,groups){
 const flat=[],starts=[];
 for(const [index,p] of paragraphs.entries()){starts.push(flat.length);for(const text of lines(p.text))flat.push({text:text.trim(),paragraph:index});}
 const assignments=[],ambiguous=new Set();
 for(const group of groups){
  const quote=lines(group.quote).map(s=>s.trim()),candidates=[];
  if(!quote.length)continue;
  for(let start=0;start+quote.length<=flat.length;start++)if(quote.every((text,i)=>flat[start+i].text===text))candidates.push(start);
  const ordinal=Number.isInteger(group.ordinal)?group.ordinal-1:-1;
  const p=paragraphs[ordinal],anchored=p?.coordinate_kind==='original_docx'&&lines(p.text).length===quote.length&&candidates.includes(starts[ordinal]);
  if(anchored||candidates.length===1){const start=anchored?starts[ordinal]:candidates[0];assignments.push({group,start,end:start+quote.length,source_lines:quote,basis:anchored?'original_paragraph_ordinal_and_quote':'unique_complete_quote_span'});}
  else if(candidates.length>1)for(const start of candidates)for(let i=start;i<start+quote.length;i++)ambiguous.add(i);
 }
 // Competing parent groups must not silently share a source occurrence.
 const collisions=new Set();for(let a=0;a<assignments.length;a++)for(let b=a+1;b<assignments.length;b++)if(assignments[a].start<assignments[b].end&&assignments[b].start<assignments[a].end){collisions.add(a);collisions.add(b);}
 for(const i of collisions)for(let k=assignments[i].start;k<assignments[i].end;k++)ambiguous.add(k);
 const result=new Map();for(const [i,assignment] of assignments.entries())if(!collisions.has(i))for(let k=assignment.start;k<assignment.end;k++)result.set(k,assignment);
 return {assignments:result,ambiguous};
}
function diagnoseGuide(guide,definition,original){
 const source=primarySource(guide),groups=source?groupSteps(definition,source):[],captured=source?capturedParagraphs(source.text):[];
 const originalLines=original?.paragraphs?.flatMap(p=>lines(p.text))||[];
 const verifiedOriginal=!!original&&!original.error&&JSON.stringify(originalLines.map(s=>s.trim()))===JSON.stringify(captured.map(p=>p.text.trim()))&&(!guide.id.startsWith('word-')||guide.id==='word-'+original.document_sha256?.slice(0,16));
 const paragraphs=verifiedOriginal?original.paragraphs.map(p=>({...p,coordinate_kind:'original_docx'})):captured;
 let lineOrdinal=0;const records=[],attribution=attributeGroups(paragraphs,groups);
 for(const [index,p] of paragraphs.entries()){
  const physical=lines(p.text);
  for(const [within,text] of physical.entries()){
   const attributed=attribution.assignments.get(lineOrdinal),sourceAmbiguous=attribution.ambiguous.has(lineOrdinal),group=attributed?.group;
   const singleLine=attributed&&attributed.end-attributed.start===1;
   const uniquelyAttached=group?.steps.every(s=>{
    const instructions=(s.action?.params?.instructions||[s.title||'']).flatMap(lines).map(norm);
    return attributed.source_lines.filter(line=>instructions.some(part=>part&&(norm(line)===part||norm(line).includes(part)&&part.length>=4))).length===1;
   });
   lineOrdinal++;const matching=singleLine?group.steps:group?.steps.filter(s=>{
    const instructions=s.action?.params?.instructions||[s.title||''];
    return instructions.some(t=>lines(t).some(part=>norm(part)===norm(text)||norm(text).includes(norm(part))&&norm(part).length>=4));
   })||[];
   const typed=matching.filter(s=>payload(s).length),unknown=matching.filter(s=>!payload(s).length);
   const full=typed.some(s=>(s.action?.params?.instructions||[s.title||'']).some(t=>norm(t)===norm(text)));
   const fields=tags(text),status=sourceAmbiguous?'unmapped':full&&!unknown.length?'typed_payload_present':typed.length?'partially_typed':matching.length?'untyped':'unmapped';
   records.push({id:'action-'+sha([guide.id,source?.content_sha256||'',String(lineOrdinal),text].join('\0')).slice(0,20),guide_id:guide.id,country:guide.applies_to?.country||null,guide_title:guide.title,source_id:source?.id||null,source_sha256:source?.content_sha256||null,source_coordinate:{kind:p.coordinate_kind,table_index:p.table_index||null,row_index:p.row_index,column_index:p.column_index,paragraph_index:p.paragraph_index,physical_line_index:within+1,actions_line_ordinal:lineOrdinal},text,block_text:p.text,paragraph_ordinal:index+1,previous:paragraphs[index-1]?.text||null,next:paragraphs[index+1]?.text||null,...fields,diagnostic_status:status,source_attribution:sourceAmbiguous?'ambiguous':attributed?.basis||'unmapped',assertion_attribution_complete:!!attributed&&!sourceAmbiguous&&(singleLine||uniquelyAttached),linked_step_keys:matching.map(s=>s.key),observed_steps:matching,link_basis:'A complete source quote must have a unique contiguous span or match its original paragraph ordinal. Negative/count assertions also require complete line attachment; unmatched/normalized group members are not silently discarded.',payload_fields:[...new Set(typed.flatMap(payload))].sort(),verification:{automatic_start:matching.some(s=>automatic(s.started_when)),automatic_completion:matching.some(s=>automatic(s.completion)),explicit_manual_check:matching.some(s=>JSON.stringify(s.completion||{}).includes('"check"')),reminder_only:matching.length>0&&matching.every(s=>s.tracking==='reminder')},trigger:{source_detected:fields.syntax.includes('trigger'),declared_timing:matching.map(s=>s.action?.params?.timing).filter(Boolean),runtime_condition_keys:matching.flatMap(s=>['when','skip_when','priority_when'].filter(k=>s[k]).map(k=>({step_key:s.key,field:k,condition:s[k]})))}});
  }
 }
 return {records,summary:{id:guide.id,title:guide.title,country:guide.applies_to?.country||null,installed_steps:stepsOf(guide).length,runtime_steps:stepsOf(definition).length,actions_lines:records.length,original_paragraphs:verifiedOriginal?paragraphs.length:null,source_status:original?.error?'original_read_error':verifiedOriginal?'original_docx_verified':original?'original_text_mismatch':'captured_text_only',source_error:original?.error||null,original_entry:original?.archive_entry||null,document_sha256:original?.document_sha256||null,diagnostic_status:tally(records,r=>r.diagnostic_status),families:tally(records,r=>r.families),verification:{automatic_start:records.filter(r=>r.verification.automatic_start).length,automatic_completion:records.filter(r=>r.verification.automatic_completion).length},opaque_multiline_reminders:stepsOf(definition).filter(s=>s.tracking==='reminder'&&s.action?.kind==='manual'&&(s.action?.params?.instructions||[]).some(t=>lines(t).length>1)).length,source_missing:!source,step_identity:identitySnapshot(definition)}};
}
function analyze(guides,{runtime=null,originals=new Map()}={}){
 const runtimeById=new Map((runtime?.guides||[]).map(r=>[r.id,r])),records=[],perGuide=[];
 for(const guide of guides){const native=runtimeById.get(guide.id),missing=runtime&&!native?.definition,result=diagnoseGuide(guide,missing?{...guide,tracks:{}}:native?.definition||guide,originals.get(guide.id));
  if(missing){result.summary.runtime_steps=null;result.summary.opaque_multiline_reminders=null;result.summary.verification={automatic_start:null,automatic_completion:null};result.summary.diagnostic_status={not_measured:result.records.length};for(const record of result.records)record.diagnostic_status='not_measured';}
  records.push(...result.records);perGuide.push({...result.summary,runtime_status:runtime?native?.definition?'prepared':native?.error||'missing':'not_measured'});
 }
 const snapshots=guides.map(g=>({id:g.id,hash:hashJson(g)})).sort((a,b)=>a.id.localeCompare(b.id));
 const report={schema_version:VERSION,scope:'same_author_installed_collection',dataset_sha256:hashJson(snapshots),interpretation_layer:runtime?'rust_runtime_prepared':'installed_definitions_only',quality_accuracy:{status:'not_measured',reason:'Heuristic and payload-presence counters are not correctness labels. Human-approved expected interpretations are required.'},input_guides:snapshots,metrics:{installed_documents:guides.length,country_guides:guides.filter(g=>g.applies_to?.country).length,countries:[...new Set(guides.map(g=>g.applies_to?.country).filter(Boolean))].sort(),installed_steps:sum(guides,g=>stepsOf(g).length),installed_tracks:tally(guides.flatMap(stepsOf),s=>s._track),runtime_steps:runtime?sum(perGuide,g=>g.runtime_steps):null,runtime_errors:runtime?perGuide.filter(g=>g.runtime_status!=='prepared').length:null,actions_physical_lines:records.length,original_docx_verified:perGuide.filter(g=>g.source_status==='original_docx_verified').length,opaque_multiline_reminder_steps:sum(perGuide,g=>g.opaque_multiline_reminders),diagnostic_status:tally(records,r=>r.diagnostic_status),source_intent_candidates:tally(records,r=>r.disposition),families:tally(records,r=>r.families),syntax:tally(records,r=>r.syntax),verification:{automatic_start:records.filter(r=>r.verification.automatic_start).length,automatic_completion:records.filter(r=>r.verification.automatic_completion).length,explicit_manual_check:records.filter(r=>r.verification.explicit_manual_check).length,reminder_only:records.filter(r=>r.verification.reminder_only).length}},per_guide:perGuide};
 report.entity_resolution=bindingAudit(runtime?runtime.guides.filter(g=>g.definition).map(g=>g.definition):guides,runtime?.catalog);
 report.quality_accuracy.categories={correctly_structured:null,partially_understood:null,unrecognized_actionable:null,genuine_advice_or_context:null};
 report.measurement_coverage={documents:guides.length,runtime_successful:runtime?perGuide.filter(g=>g.runtime_status==='prepared').length:null,source_lines_measured:records.filter(r=>r.diagnostic_status!=='not_measured').length,source_lines_not_measured:records.filter(r=>r.diagnostic_status==='not_measured').length,ambiguous_source_attribution:records.filter(r=>r.source_attribution==='ambiguous').length,lines_with_complete_assertion_attachment:records.filter(r=>r.assertion_attribution_complete&& !['not_measured','unmapped'].includes(r.diagnostic_status)).length};
 if(runtime&&report.metrics.runtime_errors){report.metrics.runtime_steps_measured=report.metrics.runtime_steps;report.metrics.runtime_steps=null;report.metrics.opaque_multiline_reminder_steps_measured=report.metrics.opaque_multiline_reminder_steps;report.metrics.opaque_multiline_reminder_steps=null;report.metrics.verification_measured=report.metrics.verification;report.metrics.verification=null;}
 report.family_hotspots={status:'heuristic_triage_not_verified_failure_counts',overlapping_families:true,counts:Object.fromEntries([...new Set(records.flatMap(r=>r.families))].sort().map(family=>[family,tally(records.filter(r=>r.families.includes(family)&&r.disposition==='action_candidate'),r=>r.diagnostic_status)]))};
 const prepared=runtime?runtime.guides.filter(g=>g.definition).map(g=>g.definition):guides;
 const timingSteps=prepared.flatMap(stepsOf).filter(s=>s.action?.params?.timing);
 report.triggers={source_lines_with_trigger_candidates:records.filter(r=>r.trigger.source_detected).length,steps_with_declared_timing:timingSteps.length,steps_with_unknown_timing_placeholder:timingSteps.filter(s=>JSON.stringify(s.when||{}).includes('guide.conditions.')).length,scope_accuracy:null,scope_status:'Requires approved source-level scope annotations; counts do not assert correct scope.'};
 report.verification_scope='Counts measure declared fact/check predicates, not whether the native reader can observe them in a real campaign. Structural payload presence is not verified interpretation correctness.';
 return {report,records};
}
function compare(baseline,current){
 if(baseline.dataset_sha256!==current.dataset_sha256)return {comparable:false,reason:'Corpus identity changed; do not call changes parser regressions.'};
 if(baseline.interpretation_layer!==current.interpretation_layer)return {comparable:false,reason:'Interpretation layers differ.'};
 if(current.interpretation_layer==='rust_runtime_prepared'&&[baseline,current].some(r=>r.input_fingerprints?.runtime_input_identity_status!=='verified_before_and_after_capture'))return {comparable:false,reason:'An external runtime snapshot has no verified capture identity.'};
 if(current.interpretation_layer==='rust_runtime_prepared')for(const key of ['input_identity','vocabulary','catalog_sha256','tooling_sha256'])if(!baseline.input_fingerprints?.[key]||!current.input_fingerprints?.[key]||baseline.input_fingerprints[key]!==current.input_fingerprints[key])return {comparable:false,reason:'Frozen inputs changed or are missing: '+key+'. Do not attribute differences to parser changes alone.'};
 const sourceStates=report=>report.per_guide.map(g=>[g.id,g.source_status,g.document_sha256]).sort((a,b)=>a[0].localeCompare(b[0]));
 if(hashJson(sourceStates(baseline))!==hashJson(sourceStates(current)))return {comparable:false,reason:'Original-source verification or document identity changed.'};
 const byId=new Map(baseline.per_guide.map(g=>[g.id,g])),changes=[];
 for(const guide of current.per_guide){const old=byId.get(guide.id);for(const key of ['actions_lines','runtime_steps','opaque_multiline_reminders'])if(guide[key]!==old[key])changes.push({guide_id:guide.id,metric:key,before:old[key],after:guide[key]});for(const key of new Set([...Object.keys(old.diagnostic_status),...Object.keys(guide.diagnostic_status)]))if((old.diagnostic_status[key]||0)!==(guide.diagnostic_status[key]||0))changes.push({guide_id:guide.id,metric:'diagnostic_status.'+key,before:old.diagnostic_status[key]||0,after:guide.diagnostic_status[key]||0});
  const oldKeys=new Map((old.step_identity||[]).map(s=>[s.key,s])),newKeys=new Map((guide.step_identity||[]).map(s=>[s.key,s]));
  for(const key of new Set([...oldKeys.keys(),...newKeys.keys()])){const a=oldKeys.get(key),b=newKeys.get(key);if(!a||!b||hashJson(a)!==hashJson(b))changes.push({guide_id:guide.id,metric:'step_identity',step_key:key,status:!a?'new':!b?'removed':'changed',changed_fields:a&&b?Object.keys(b).filter(field=>JSON.stringify(a[field])!==JSON.stringify(b[field])):[]});}
 }
 const oldIssues=new Set((baseline.entity_resolution?.issues||[]).map(i=>hashJson([i.guide_id,i.step_key,i.reference,i.status])));
 for(const issue of current.entity_resolution?.issues||[])if(!oldIssues.has(hashJson([issue.guide_id,issue.step_key,issue.reference,issue.status])))changes.push({guide_id:issue.guide_id,metric:'entity_issue',step_key:issue.step_key,status:issue.status,reference:issue.reference});
 return {comparable:true,changes,gate:'Diagnostic changes require review; more typed payloads do not prove improved interpretation.'};
}
function sample(records,dataset,size=400){
 const byGuide=new Map();for(const row of records){if(!byGuide.has(row.guide_id))byGuide.set(row.guide_id,[]);byGuide.get(row.guide_id).push(row);}
 const guideIds=[...byGuide.keys()].sort((a,b)=>sha(SEED+a).localeCompare(sha(SEED+b))),holdout=new Set(guideIds.slice(0,Math.max(1,Math.round(guideIds.length*.2)))),selected=[],seen=new Set();
 function choose(pool,count,split){const familyCounts={},syntaxCounts={};for(let i=0;i<count&&pool.length;i++){
   pool.sort((a,b)=>{
    const score=r=>r.families.reduce((s,f)=>s+1/(1+(familyCounts[f]||0)),0)+r.syntax.reduce((s,f)=>s+1/(1+(syntaxCounts[f]||0)),0)+(r.block_text.includes('\n')?.5:0);
    return score(b)-score(a)||sha(SEED+a.id).localeCompare(sha(SEED+b.id));
   });const row=pool.shift(),normalized=norm(row.text).toLowerCase();if(seen.has(normalized)){i--;continue;}seen.add(normalized);for(const f of row.families)familyCounts[f]=(familyCounts[f]||0)+1;for(const f of row.syntax)syntaxCounts[f]=(syntaxCounts[f]||0)+1;selected.push({...row,split});
  }}
 const holdoutSize=Math.round(size*.2),devSize=size-holdoutSize;
 // Whole guides stay in one split. Identical text is also excluded across splits.
 for(const split of ['holdout','development']){const ids=guideIds.filter(id=>holdout.has(id)===(split==='holdout')),target=split==='holdout'?holdoutSize:devSize;
  for(const id of ids)choose([...byGuide.get(id)],Math.min(8,Math.floor(target/ids.length)),split);
  const used=new Set(selected.map(r=>r.id));choose(records.filter(r=>ids.includes(r.guide_id)&&!used.has(r.id)),target-selected.filter(r=>r.split===split).length,split);
 }
 if(selected.length!==size)throw Error(`Only ${selected.length} distinct representative lines available (requested ${size})`);
 return {schema_version:VERSION,dataset_sha256:dataset,seed:SEED,status:'proposed_not_human_verified',selection:'Source-only family/syntax stratification; whole-guide split and exact normalized text deduplication. No parser success used in selection.',holdout_policy:'Do not tune rules against holdout annotations. Report development and holdout independently.',cases:selected.map(row=>({id:row.id,split:row.split,guide_id:row.guide_id,country:row.country,guide_title:row.guide_title,source:{id:row.source_id,sha256:row.source_sha256,coordinate:row.source_coordinate},text:row.text,context:{block:row.block_text,previous:row.previous,next:row.next},proposed_expected:{disposition:row.disposition==='advice_candidate'?'advice':row.disposition==='trigger_header_candidate'?'trigger_header':row.disposition==='action_candidate'?'actionable':'needs_interpretation',families:row.families,syntax:row.syntax,actions:[],entities:[],trigger_scope:'requires_annotation',justification:'Source-pattern proposal only. Preserve every clause, negation, quantity and ordering constraint; do not infer a country-specific ID from a prefix or infer completion from missing observations.'},review:{status:'pending',annotation_origin:'heuristic_proposal',human_verified:false,required:['atomic actions and arguments','entity identity or explicit unresolved status','trigger scope and clause relationships','advice versus actionable distinction']}}))};
}
function assertionResult(assertion,record){
 const steps=record?.observed_steps||[];
 if(assertion.kind==='no_structured_action')return !steps.some(s=>payload(s).length);
 if(assertion.kind==='step_count'&&Number.isInteger(assertion.equals)&&assertion.equals>=0)return steps.length===assertion.equals;
 if(assertion.kind==='field_equals'&&Object.hasOwn(assertion,'equals')&&typeof assertion.path==='string'&&assertion.path&&!assertion.path.split('.').some(k=>!k||['__proto__','prototype','constructor'].includes(k))){
  const matches=steps.map(step=>assertion.path.split('.').reduce((value,key)=>value&&Object.hasOwn(value,key)?value[key]:undefined,step)).map(v=>JSON.stringify(sorted(v))===JSON.stringify(sorted(assertion.equals)));
  if(assertion.quantifier!==undefined&&!['any','all'].includes(assertion.quantifier))return null;
  return assertion.quantifier==='all'?matches.length>0&&matches.every(Boolean):matches.some(Boolean);
 }
 return null;
}
function quality(gold,records,split='development'){
 const selected=gold.cases.filter(c=>c.split===split),approved=selected.filter(c=>c.review?.human_verified===true&&c.review?.status==='approved'&&c.review?.reviewed_by&&c.review?.reviewed_at),byId=new Map(records.map(r=>[r.id,r]));
 const results=approved.map(c=>{const assertions=c.acceptance||[],actual=byId.get(c.id),admitted=actual&&['typed_payload_present','partially_typed','untyped'].includes(actual.diagnostic_status)&&actual.source_attribution!=='ambiguous'&&actual.assertion_attribution_complete===true,checks=admitted?assertions.map(a=>assertionResult(a,actual)):[];return {id:c.id,guide_id:c.guide_id,present:!!actual,classification:!admitted||!checks.length||checks.includes(null)?'not_measured':checks.every(Boolean)?'meets_approved_assertions':checks.some(Boolean)?'partially_meets_approved_assertions':'fails_approved_assertions',checks};});
 const measured=results.filter(r=>r.classification!=='not_measured');
 return {split,total_cases:selected.length,matched_source_cases:selected.filter(c=>byId.has(c.id)).length,human_approved:approved.length,pending:selected.length-approved.length,assertion_coverage:measured.length,accuracy:measured.length?measured.filter(r=>r.classification==='meets_approved_assertions').length/measured.length:null,status:measured.length?'approved_assertion_results_not_full_strategy_correctness':'not_measured_no_approved_executable_assertions',counts:tally(results,r=>r.classification),cases:results};
}
function main(argv){
 if(argv.includes('--help')){console.log('node scripts/benchmark-guides.cjs [--data-dir DIR] [--capture-runtime | --runtime SNAPSHOT.json] [--runtime-output NEW-SNAPSHOT.json] [--verify-sources] [--output REPORT.json] [--records-output DETAILS.json] [--baseline BASELINE.json] [--gold CASES.json --split development|holdout] [--sample-output NEW.json --holdout-output NEW-HOLDOUT.json]');return;}
 const args={};for(let i=0;i<argv.length;i++){if(!argv[i].startsWith('--'))throw Error('Expected --option');args[argv[i].slice(2)]=argv[i+1]&&!argv[i+1].startsWith('--')?argv[++i]:true;}
 if(args.runtime&&args['capture-runtime'])throw Error('Choose either --runtime or --capture-runtime');
 if(args.split&&!['development','holdout'].includes(args.split))throw Error('Choose development or holdout');
 const dataDir=path.resolve(args['data-dir']||path.join(ROOT,'.local')),directory=path.join(dataDir,'guides');
 const files=fs.readdirSync(directory).filter(n=>n.endsWith('.json')).sort(),guides=files.map(n=>json(path.join(directory,n))),inputIdentity=inputFingerprint(dataDir),runtime=args['capture-runtime']?captureRuntime(dataDir,process.env.HOI4_BENCHMARK_EXE||path.join(ROOT,'.local/rust-backend-target/release/examples/guide_benchmark.exe')):args.runtime?json(args.runtime):null;
 if(runtime?.benchmark_input&&runtime.benchmark_input.sha256!==inputIdentity.sha256)throw Error('Runtime snapshot input identity does not match the installed guides, associations, settings, library or vocabulary.');
 const originals=args['verify-sources']?extractOriginals(dataDir,guides):new Map(),{report,records}=analyze(guides,{runtime,originals});
 report.input_fingerprints={input_identity:inputIdentity.sha256,tooling_sha256:TOOLING_SHA256,vocabulary:sha(fs.readFileSync(path.join(ROOT,'src/hoi4companion/guide_language.json'))),production_sources:hashJson(fs.readdirSync(path.join(ROOT,'native/backend/src')).filter(n=>n.endsWith('.rs')).sort().map(n=>[n,sha(fs.readFileSync(path.join(ROOT,'native/backend/src',n)))])),catalog_sha256:runtime?hashJson(runtime.catalog):null,runtime_executable_sha256:runtime?.benchmark_executable_sha256||null,runtime_input_identity_status:runtime?.benchmark_input?'verified_before_and_after_capture':runtime?'unbound_external_snapshot':'not_measured'};
 report.preparation={elapsed_ms:runtime?.elapsed_ms||null,live_game_access:false,application_started:false};
 if(args.baseline)report.comparison=compare(json(args.baseline),report);
 if(args.gold){const gold=json(args.gold);report.gold_coverage=gold.dataset_sha256===report.dataset_sha256?quality(gold,records,args.split||'development'):{status:'corpus_mismatch'};}
 if(args['runtime-output']){
  if(!runtime?.benchmark_input)throw Error('--runtime-output requires a verified --capture-runtime snapshot');
  fs.writeFileSync(args['runtime-output'],JSON.stringify(runtime)+'\n',{flag:'wx'});
 }
 if(args['sample-output']){
  const packet=sample(records,report.dataset_sha256);
  const outputs=args['holdout-output']?[[args['sample-output'],{...packet,cases:packet.cases.filter(c=>c.split==='development')}],[args['holdout-output'],{...packet,cases:packet.cases.filter(c=>c.split==='holdout')}]]:[[args['sample-output'],packet]];
  for(const [file] of outputs)if(fs.existsSync(file))throw Error('Refusing to overwrite annotations: '+file);
  for(const [file,value] of outputs)fs.writeFileSync(file,JSON.stringify(value,null,2)+'\n',{flag:'wx'});
 }
 if(args['records-output'])fs.writeFileSync(args['records-output'],JSON.stringify({dataset_sha256:report.dataset_sha256,records},null,2)+'\n');
 const output=JSON.stringify(report,null,2)+'\n';if(args.output)fs.writeFileSync(args.output,output);else process.stdout.write(output);
}
module.exports={lines,norm,hashJson,capturedParagraphs,tags,automatic,payload,stepsOf,countryCondition,bindingAudit,identitySnapshot,primarySource,archiveRequests,inputFingerprint,captureRuntime,attributeGroups,diagnoseGuide,analyze,compare,sample,assertionResult,quality};
if(require.main===module)try{main(process.argv.slice(2));}catch(error){console.error(error.message);process.exitCode=1;}
