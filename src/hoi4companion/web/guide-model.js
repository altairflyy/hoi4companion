'use strict';
((root)=>{
 function evaluate(guide,value,local={}){
  const raw=value?.usable?value.sample:null;
  const mode=!raw?'preview':raw.country!==guide.country?'wrong_country':raw.focus_tree_id!==guide.tree_id?'wrong_tree':'live';
  const sample=mode==='live'?raw:null,checks=local.checks||{};
  const completed=new Set(sample?.completed_focuses||[]),researched=new Set(sample?.researched_technologies||[]);
  const activeTech=new Set((sample?.research_slots||[]).map(s=>s.technology_id).filter(Boolean));
  const unity=guide.tracks.focus.find(s=>s.available_gate);
  const prioritize=!!sample&&unity&&completed.has(unity.priority_after)&&!completed.has(unity.target)&&checks[unity.available_gate]===true;
  function status(step){
   if(step.kind==='gate'||step.kind==='project')return checks[step.key]?'confirmed':'manual';
   if(step.kind==='free')return 'free';
   const isFocus=step.kind==='focus',done=isFocus?completed:researched;
   if(sample&&done.has(step.target))return 'completed';
   if(sample&&(isFocus?sample.current_focus===step.target:activeTech.has(step.target)))return 'active';
   if(isFocus&&step.optional_before&&sample&&(prioritize||completed.has(step.optional_before)))return 'skipped';
   if(step.catalog_match===false)return 'unavailable';
   if(step.gate&&!checks[step.gate])return 'waiting';
   return sample?'pending':'planned';
  }
  const rows={};for(const [track,steps]of Object.entries(guide.tracks))rows[track]=steps.map((s,i)=>({...s,order:i+1,status:status(s)}));
  const settled=r=>['completed','confirmed','skipped'].includes(r.status);
  let focusNext=rows.focus.find(r=>!settled(r)&&r.status!=='active')||null;
  if(prioritize)focusNext=rows.focus.find(r=>r.key===unity.key);
  const researchQueue=rows.research.filter(r=>!['project','gate'].includes(r.kind)&&!settled(r)&&r.status!=='active');
  const first=researchQueue[0]||null,suggestions=[];
  for(const row of researchQueue){if(['waiting','unavailable','free'].includes(row.status))break;suggestions.push(row);if(suggestions.length>=5)break;}
  const projects=rows.research.filter((r,i)=>r.kind==='project'&&!settled(r)&&rows.research.slice(0,i).filter(p=>p.kind!=='project').every(p=>settled(p)||p.status==='active'));
  const slots=sample?.research_slots||[];
  return {mode,country:raw?.country||null,focus:{rows:rows.focus,next:focusNext,active:rows.focus.filter(r=>r.status==='active'),completed:rows.focus.filter(settled).length,
   outside:sample?.current_focus&&!guide.tracks.focus.some(r=>r.target===sample.current_focus)?sample.current_focus:null},
   research:{rows:rows.research,next:first,suggestions,projects,active:rows.research.filter(r=>r.status==='active'),completed:rows.research.filter(settled).length,
    free_slots:sample?slots.filter(s=>!s.technology_id).length:null,slots,
    outside:[...activeTech].filter(id=>!guide.tracks.research.some(r=>r.target===id))}};
 }
 const model={evaluate};if(typeof module!=='undefined'&&module.exports)module.exports=model;else root.HoiGuideModel=model;
})(typeof window!=='undefined'?window:globalThis);
