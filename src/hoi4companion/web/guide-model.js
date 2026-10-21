'use strict';
((root)=>{
 const c=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const settled=r=>['completed','confirmed','skipped'].includes(r.status);
 function evaluate(input,value,local={}){
  const guide=c.guide(input),state=c.state(value),checks=local.checks||{};
  const country=c.fact(state,'country').value,tree=c.fact(state,'focus.tree').value,scope=guide.applies_to;
  const mode=!state.usable?'preview':scope.country&&country!==scope.country?'wrong_country':scope.tree_id&&tree!==scope.tree_id?'wrong_tree':'live';
  const live=mode==='live',context=live?state:{...state,usable:false};
  const members=key=>{const f=c.fact(context,key);return new Set(Array.isArray(f.value)?f.value:[]);};
  const completed=members('focus.completed'),researched=members('research.completed'),activeFocus=members('focus.active'),activeTech=members('research.active');
  function row(step,index){
   const kind=step.action.kind,isFocus=kind==='select_focus',done=isFocus?completed:researched,active=isFocus?activeFocus:activeTech;
   const scoped=live&&(!isFocus||!step.action.entity.scope||step.action.entity.scope===tree);
   const membership=(key,set)=>{const f=c.fact(scoped?context:null,key);return f.status==='unknown'?f:{status:'calculated',value:set.has(step.target),source:'membership:'+key};};
   const completion=['select_focus','research'].includes(kind)?membership(isFocus?'focus.completed':'research.completed',done):{status:'unknown',value:null,source:''};
   const selection=['select_focus','research'].includes(kind)?membership(isFocus?'focus.active':'research.active',active):{status:'unknown',value:null,source:''};
   let status,provenance='calculated';
   if(kind==='manual'||kind==='special_project'){status=checks[step.key]===true?'confirmed':'manual';provenance='manual';}
   else if(kind==='free')status='free';
   else if(completion.value===true)status='completed';
   else if(selection.value===true)status='active';
   else if(live&&!scoped)status='waiting';
   else if(live&&step.skip_when&&c.condition(step.skip_when,context,checks)===true)status='skipped';
   else if(step.catalog_match===false)status='unavailable';
   else if(step.when&&c.condition(step.when,context,checks)!==true)status='waiting';
   else status=live?'pending':'planned';
   return {...step,order:index+1,status,provenance,completion,selection,availability:{status:'unknown',value:null,source:'',reason:'Una priorità o conferma manuale non prova la disponibilità del motore.'}};
  }
  function ordered(rows){
   const candidates=rows.filter(r=>!settled(r)&&r.status!=='active');
   const priorities=live?candidates.filter(r=>r.priority_when&&c.condition(r.priority_when,context,checks)===true).sort((a,b)=>(b.priority||0)-(a.priority||0)):[];
   const promoted=new Set(priorities);
   return [...priorities,...candidates.filter(r=>!promoted.has(r))];
  }
  const tracks={};
  for(const [id,steps] of Object.entries(guide.tracks)){
   const rows=steps.map(row);
   tracks[id]={rows,next:ordered(rows)[0]||null,active:rows.filter(r=>r.status==='active'),completed:rows.filter(settled).length};
  }
  const empty=()=>({rows:[],next:null,active:[],completed:0}),focus=tracks.focus||empty(),research=tracks.research||empty();
  const researchQueue=ordered(research.rows.filter(r=>!['special_project','manual'].includes(r.action.kind)));
  research.next=researchQueue[0]||null;research.suggestions=[];
  for(const r of researchQueue){if(['waiting','unavailable','free'].includes(r.status))break;research.suggestions.push(r);if(research.suggestions.length>=5)break;}
  research.projects=research.rows.filter((r,i)=>r.action.kind==='special_project'&&!settled(r)&&research.rows.slice(0,i).filter(p=>p.action.kind!=='special_project').every(p=>settled(p)||p.status==='active'));
  const slots=c.fact(context,'research.slots').value;research.slots=Array.isArray(slots)?slots:[];
  research.free_slots=live&&Array.isArray(slots)?slots.filter(s=>!s.technology_id).length:null;
  research.outside=[...activeTech].filter(id=>!research.rows.some(r=>r.target===id));
  focus.outside=[...activeFocus].find(id=>!focus.rows.some(r=>r.target===id))||null;
  return {schema_version:c.VERSION,mode,country,tracks,focus,research};
 }
 const model={evaluate};if(typeof module!=='undefined'&&module.exports)module.exports=model;else root.HoiGuideModel=model;
})(typeof window!=='undefined'?window:globalThis);
