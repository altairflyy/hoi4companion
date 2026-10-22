'use strict';
// Presentation adapter: all decisions belong to the pure, shared runtime.
((root)=>{
 const common=typeof module!=='undefined'&&module.exports;
 const c=common?require('./contracts.js'):root.HoiContracts;
 const runtime=common?require('./guide-runtime.js'):root.HoiGuideRuntime;
 const settled=row=>row.runtime_status==='completed'||row.runtime_reason==='skip_condition_true';
 function evaluate(input,value,local={}){
  const guide=c.guide(input),state=c.state(value);
  const progress=local.progress||{schema_version:1,guide_id:guide.id||'',run_id:local.run_id||'legacy',session_id:local.session_id||null,checks:local.checks||{},started:local.started||{},baselines:local.baselines||{}};
  const result=runtime.evaluate(guide,state,progress),byKey=new Map(result.steps.map(row=>[row.key,row]));
  function row(step,index){
   const decision=byKey.get(step.key),kind=step.action.kind;
   let status;
   if(['manual','special_project'].includes(kind))status=decision.status==='completed'?'confirmed':'manual';
   else if(kind==='free')status='free';
   else if(['completed','active'].includes(decision.status))status=decision.status;
   else if(decision.reason==='skip_condition_true')status='skipped';
   else if(decision.reason==='catalog_unmatched')status='unavailable';
   else if(['entity_scope_mismatch','condition_false','condition_unknown','phase_condition_false','phase_condition_unknown'].includes(decision.reason)||Object.hasOwn(step,'depends_on')&&decision.blocked_by.length)status='waiting';
   else status=result.mode==='live'?'pending':'planned';
   return {...step,...(step.completion?{completion_condition:step.completion}:{}),...decision,
    order:index+1,status,runtime_status:decision.status,runtime_reason:decision.reason,
    provenance:decision.completion_basis==='manual'?'manual':'calculated'};
  }
  const tracks={};
  for(const [id,steps]of Object.entries(guide.tracks)){
   const rows=steps.map(row),mapped=new Map(rows.map(r=>[r.key,r]));
   const next=result.mode.startsWith('wrong_')?rows.find(r=>!settled(r)):mapped.get(result.tracks[id].next);
   Object.defineProperty(tracks,id,{value:{rows,next:next||null,active:rows.filter(r=>r.runtime_status==='active'),completed:rows.filter(settled).length},enumerable:true});
  }
  const empty=()=>({rows:[],next:null,active:[],completed:0}),focus=tracks.focus||empty(),research=tracks.research||empty();
  const mapped=new Map(research.rows.map(row=>[row.key,row]));
  const queue=(result.tracks.research?.order||[]).map(key=>mapped.get(key)).filter(row=>!settled(row)&&row.runtime_status!=='active'&&!['manual','special_project'].includes(row.action.kind));
  research.next=queue[0]||null;
  research.suggestions=result.mode==='live'?result.parallel.research.steps.map(key=>mapped.get(key)).filter(Boolean):[];
  if(result.mode!=='live')for(const candidate of queue){if(['waiting','unavailable','free'].includes(candidate.status))break;research.suggestions.push(candidate);if(research.suggestions.length>=5)break;}
  research.projects=research.rows.filter((row,i)=>row.action.kind==='special_project'&&!settled(row)&&
   (Object.hasOwn(row,'depends_on')?row.runtime_status==='ready'||row.runtime_reason==='manual_confirmation_required':research.rows.slice(0,i).filter(p=>p.action.kind!=='special_project').every(p=>settled(p)||p.runtime_status==='active')));
  const context=result.mode==='live'?state:null,slots=c.fact(context,'research.slots').value;
  research.slots=Array.isArray(slots)?slots:[];research.free_slots=result.parallel.research.capacity;
  const activeTech=c.fact(context,'research.active').value,activeFocus=c.fact(context,'focus.active').value;
  research.outside=(Array.isArray(activeTech)?activeTech:[]).filter(id=>!research.rows.some(row=>row.target===id));
  focus.outside=(Array.isArray(activeFocus)?activeFocus:[]).find(id=>!focus.rows.some(row=>row.target===id))||null;
  return {schema_version:c.VERSION,mode:result.mode,country:result.country,tracks,focus,research,runtime:result};
 }
 const model={evaluate};if(typeof module!=='undefined'&&module.exports)module.exports=model;else root.HoiGuideModel=model;
})(typeof window!=='undefined'?window:globalThis);
