'use strict';
// Read-only audit. The proposed admission check is a diagnostic shadow policy,
// never imported by the application or its evaluator.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),{spawnSync}=require('node:child_process');
const {ROOT,json,temporary}=require('./native-tooling.cjs');
const benchmark=require('./benchmark-guides.cjs');
const digest=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const PROTECTED={
 'tests/fixtures/guide-benchmark-baseline.json':'62d158fd1fcb9c9277bf1a895615b29922670984be375ec9af4a3e04947a6ae9',
 'tests/fixtures/guide-benchmark-development.json':'86872b340546e9e7ed533a835b6436438c488c10f68f372a76edc84a0d1afc6e',
 'tests/fixtures/guide-benchmark-holdout.json':'2b34515cbec0881943a7d67907129ba21397f9f961a2b70bf25eacc51ca42ea6'
};
const IDENTITIES={decision_id:'decision',advisor_id:'advisor',law_id:'law',idea_id:'idea',project_id:'project',doctrine_id:'doctrine',event_id:'event',event_option_id:'event_option'};
const NAMES=new Set(['decision','advisor','law','idea','project','doctrine','event_option']);
function tally(rows,field){const counts={};for(const row of rows){const key=typeof field==='function'?field(row):row[field];counts[key]=(counts[key]||0)+1;}return counts;}
function protectedHashes(){const hashes={};for(const [name,expected] of Object.entries(PROTECTED)){const actual=digest(path.join(ROOT,name));if(actual!==expected)throw Error('Protected benchmark input changed: '+name);hashes[name]=actual;}return hashes;}
function sourceQuery(step){
 const instructions=step.action?.params?.instructions;
 const raw=Array.isArray(instructions)&&instructions.length===1?instructions[0]:step.title;
 if(typeof raw!=='string'||!raw.trim())return {raw:'',query:'',replayable:false,reason:'no_source_piece'};
 let query=raw.trim().replace(/^ASAP(?:\s*:\s*|\s+)/i,'');
 // Decorations recognized by the existing named-command grammar; keep them in
 // the audit record, never propagate these thresholds to another source clause.
 query=query.replace(/\s*\(\s*(?:before|after|when|if|on)\s+[^()]+\)\s*$/i,'').replace(/\s*\(\s*\d{1,6}\s*(?:PP|AE|NE|AFE)\s*\)\s*$/i,'').trim();
 const replayable=query.length>0&&!/[\r\n:&]/.test(query);
 return {raw,query,replayable,reason:replayable?'literal_source_piece':'compound_or_typed_command_not_replayed'};
}
function references(guide){
 const rows=[];
 for(const [track,steps] of Object.entries(guide.tracks||{}))for(const step of steps){
  const p=step.action?.params||{},refs=[];
  if(step.action?.entity?.id)refs.push({...step.action.entity,field:'action.entity'});
  for(const [field,kind] of Object.entries(IDENTITIES))if(typeof p[field]==='string')refs.push({kind,id:p[field],field});
  for(const ref of refs.filter((r,i)=>refs.findIndex(v=>v.kind===r.kind&&v.id===r.id)===i))rows.push({guide_id:guide.id,country:guide.applies_to?.country||null,step_key:step.key,track,reference:ref,step,source:sourceQuery(step)});
 }
 return rows;
}
function mechanicalAudit(guides,entries){
 const byId=new Map();for(const entry of entries){const key=entry.kind+':'+entry.id;if(!byId.has(key))byId.set(key,[]);byId.get(key).push(entry);}
 const counts={event_options_checked:0,event_parent_conflicts:0,advisors_checked:0,advisor_role_conflicts:0,advisor_person_conflicts:0,decisions_checked:0,targeted_decision_conflicts:0},issues=[];
 for(const guide of guides)for(const step of Object.values(guide.tracks||{}).flat()){
  const p=step.action?.params||{};
  if(p.event_option_id){counts.event_options_checked++;const options=byId.get('event_option:'+p.event_option_id)||[];if(!p.event_id||!options.some(o=>o.event_id===p.event_id)){counts.event_parent_conflicts++;issues.push({guide_id:guide.id,step_key:step.key,status:'event_parent_conflict',event_id:p.event_id,option_id:p.event_option_id});}}
  if(p.advisor_id){counts.advisors_checked++;const advisors=byId.get('advisor:'+p.advisor_id)||[];if(p.role&&advisors.length&&!advisors.some(a=>!a.slot||a.slot===p.role)){counts.advisor_role_conflicts++;issues.push({guide_id:guide.id,step_key:step.key,status:'advisor_role_conflict',advisor:p.advisor_id,role:p.role});}if(p.person_id&&advisors.length&&advisors.every(a=>a.character_id&&a.character_id!==p.person_id)){counts.advisor_person_conflicts++;issues.push({guide_id:guide.id,step_key:step.key,status:'advisor_person_conflict',advisor:p.advisor_id,person_id:p.person_id});}}
  if(p.decision_id){counts.decisions_checked++;const candidates=byId.get('decision:'+p.decision_id)||[];if(candidates.some(d=>d.targeted===true)&&p.target_country&&p.target_state!==undefined){counts.targeted_decision_conflicts++;issues.push({guide_id:guide.id,step_key:step.key,status:'multiple_decision_target_kinds',decision:p.decision_id});}}
 }
 return {status:'mechanical_consistency_not_semantic_accuracy',counts,issues};
}
function probesFor(rows){
 const probes=[];
 for(const row of rows)if(NAMES.has(row.reference.kind)&&row.source.replayable){
  const p=row.step.action?.params||{},request={query:row.source.query,kinds:[row.reference.kind],country:row.country,typos:false};
  if(row.reference.scope)request.scope=row.reference.scope;
  if(row.reference.kind==='advisor'&&p.role)request.filters={slot:p.role};
  if(row.reference.kind==='event_option'&&p.event_id)request.filters={event_id:p.event_id};
  const key=[row.guide_id,row.step_key,row.reference.kind,row.reference.id].join(':');probes.push({key,request});
  if(row.reference.kind==='decision'){
   probes.push({key:key+':other-families',request:{query:row.source.query,kinds:['advisor','idea','decision','event_option'],country:row.country,typos:false}});
   // Query separately: an exact name in another kind must not hide a prefix
   // candidate because Catalog::matches prioritizes tiers across a combined pool.
   probes.push({key:key+':event-options',request:{query:row.source.query,kinds:['event_option'],country:row.country,typos:false}});
  }
 }
 return probes;
}
function shadowAdmission(row,result){
 if(row.reference.kind!=='decision')return {status:'out_of_scope',reason:'Only new implicit named-decision admission is proposed.'};
 if(!row.source.replayable||!result)return {status:'not_measured',reason:'Original command fragment cannot be safely replayed by this diagnostic.'};
 const matches=result.matches||[],bound=matches.find(m=>m.kind==='decision'&&m.id===row.reference.id);
 if(matches.length!==1)return {status:'requires_review',reason:matches.length?'ambiguous_current_candidates':'no_current_candidate',saved_binding_reproduced:!!bound};
 if(!bound)return {status:'requires_review',reason:'replay_selects_different_identity',saved_binding_reproduced:false};
 const strong=['exact_id','exact_normalized_name','exact_normalized_alias','exact_vocabulary_alias'].includes(bound.match_evidence);
 return {status:strong?'strong_source_evidence':'requires_review',reason:strong?bound.match_evidence:'implicit_match_has_no_exact_source_identity',saved_binding_reproduced:true};
}
function narrowAdmission(row,result,eventOptions,origin='unknown'){
 if(origin==='explicit')return {status:'out_of_scope_explicit_choice',reason:'Do not turn a declared/user-selected identity into an implicit match.'};
 if(origin!=='implicit')return {status:'not_measured',reason:'Generated ID presence does not establish whether the source choice was explicit.'};
 const strict=shadowAdmission(row,result);
 if(strict.status==='not_measured'||strict.status==='out_of_scope')return strict;
 if(strict.status==='strong_source_evidence')return {status:'unchanged_strong_source',reason:strict.reason};
 if(strict.reason!=='implicit_match_has_no_exact_source_identity')return strict;
 if(!eventOptions)return {status:'not_measured',reason:'Missing separate event-option candidate probe.'};
 return eventOptions.matches.length?{status:'requires_review',reason:'weak_implicit_decision_has_event_option_rivals'}:{status:'unchanged_weak_no_event_rival',reason:'No event-option rival; not a certification that this identity is correct.'};
}
function report(snapshot,probeOutput,protectedFiles){
 const guides=snapshot.guides.filter(g=>g.definition).map(g=>g.definition),rows=guides.flatMap(references),byKey=new Map(probeOutput.results.map(r=>[r.key,r]));
 const decisions=rows.filter(r=>r.reference.kind==='decision').map(row=>{
  const key=[row.guide_id,row.step_key,row.reference.kind,row.reference.id].join(':'),result=byKey.get(key),cross=byKey.get(key+':other-families'),events=byKey.get(key+':event-options');
  return {guide_id:row.guide_id,country:row.country,step_key:row.step_key,source_text:row.source.raw,query:row.source.query,query_basis:row.source.reason,source_quote:row.step.evidence?.[0]?.quote||null,saved_id:row.reference.id,legacy_matches:result?.matches||[],shadow_admission:shadowAdmission(row,result),narrow_shadow_admission:narrowAdmission(row,result,events,'implicit'),event_option_rival_count:events?.matches.length||0,event_option_rivals:(events?.matches||[]).slice(0,12),cross_family_candidate_count:cross?.matches.length||0,cross_family_candidates:(cross?.matches||[]).slice(0,12)};
 });
 const replayRows=rows.filter(r=>NAMES.has(r.reference.kind)&&r.source.replayable).map(row=>{const key=[row.guide_id,row.step_key,row.reference.kind,row.reference.id].join(':'),result=byKey.get(key),bound=result?.matches.find(m=>m.kind===row.reference.kind&&m.id===row.reference.id);return {kind:row.reference.kind,status:bound?.match_evidence||'saved_identity_not_reproduced_by_source_piece_probe'};});
 return {schema_version:1,status:'diagnostic_only_no_production_policy_applied',protected_inputs:protectedFiles,snapshot_input_identity:snapshot.benchmark_input.sha256,scope:{documents:guides.length,prepared_steps:guides.reduce((n,g)=>n+Object.values(g.tracks).flat().length,0),entity_references:rows.length,identity_replay_probes:probeOutput.results.length},limitations:['A strong exact source label is not proof that an action is available or correct in the campaign.','A failed source-piece replay is not proof of an incorrect saved binding; compound grammar and relative entities need their original compiler.','original_tag remains unknown unless independently established; foreign ID prefixes never prove conflict.','Shadow admission applies to future new implicit decision associations, not saved guide rewrites.','Narrow shadow results assume a new resolution known to be implicit. Stored ID presence is not evidence of explicit user provenance.'],mechanical:mechanicalAudit(guides,snapshot.catalog.entries),source_replay:{by_kind:tally(replayRows,'kind'),evidence:tally(replayRows,'status')},decision_shadow_policy:{scope:'All 182 existing decision associations, source-fragment replay where attributable. No automatic corrections.',counts:tally(decisions,d=>d.shadow_admission.status),reasons:tally(decisions,d=>d.shadow_admission.reason),narrow_policy_counts:tally(decisions,d=>d.narrow_shadow_admission.status),cases:decisions}};
}
function main(argv){
 const args={};for(let i=0;i<argv.length;i++){if(!argv[i].startsWith('--'))throw Error('Expected --option');args[argv[i].slice(2)]=argv[++i];}
 if(!args.snapshot||!args.output)throw Error('Usage: node scripts/audit-guide-bindings.cjs --snapshot VERIFIED.json --output NEW.json [--data-dir DIR]');
 if(fs.existsSync(args.output))throw Error('Refusing to overwrite an audit report: '+args.output);
 const protectedFiles=protectedHashes(),snapshot=json(args.snapshot),dataDir=path.resolve(args['data-dir']||path.join(ROOT,'.local'));
 if(!snapshot.benchmark_input||benchmark.inputFingerprint(dataDir).sha256!==snapshot.benchmark_input.sha256)throw Error('Frozen runtime input identity does not match the current corpus/settings/library/associations.');
 if(snapshot.error_count||snapshot.guides.some(g=>!g.definition))throw Error('A complete runtime snapshot is required; partial captures are not treated as the whole corpus.');
 const owned=temporary('hoi4-binding-probes-');
 try{
  const input=path.join(owned.root,'input.json'),output=path.join(owned.root,'output.json'),rows=snapshot.guides.flatMap(g=>references(g.definition));
  fs.writeFileSync(input,JSON.stringify({...snapshot.catalog,vocabulary:json(path.join(ROOT,'src/hoi4companion/guide_language.json')),probes:probesFor(rows)}));
  const executable=process.env.HOI4_BINDING_PROBE_EXE||path.join(ROOT,'.local/rust-core-tests-target/release/examples/binding_probe.exe');
  const result=spawnSync(executable,[input,output],{windowsHide:true,encoding:'utf8',timeout:120000,maxBuffer:1024*1024});
  if(result.error)throw result.error;if(result.status)throw Error(result.stderr||'Native resolver replay failed');
  if(benchmark.inputFingerprint(dataDir).sha256!==snapshot.benchmark_input.sha256)throw Error('Inputs changed during audit.');
  const audit=report(snapshot,json(output),protectedFiles);audit.native_probe_sha256=digest(executable);audit.protected_inputs_unchanged=JSON.stringify(protectedHashes())===JSON.stringify(protectedFiles);
  fs.writeFileSync(args.output,JSON.stringify(audit,null,2)+'\n',{flag:'wx'});
 }finally{owned.close();}
}
module.exports={PROTECTED,protectedHashes,sourceQuery,references,mechanicalAudit,probesFor,shadowAdmission,narrowAdmission,report};
if(require.main===module)try{main(process.argv.slice(2));}catch(error){console.error(error.message);process.exitCode=1;}
