'use strict';
((root)=>{
 const c=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const designs=typeof module!=='undefined'&&module.exports?require('./design-matching.js'):root.HoiDesignMatching;
 const integer=(v,min=0)=>Number.isSafeInteger(v)&&v>=min;
 const compare=(a,b)=>{const x=Array.from(a,c=>c.codePointAt(0)),y=Array.from(b,c=>c.codePointAt(0));for(let i=0;i<Math.min(x.length,y.length);i++)if(x[i]!==y[i])return x[i]-y[i];return x.length-y.length;};
 function selectorFor(step){
  const v=step.action.params?.production_selector;
  if(!v||typeof v!=='object'||Array.isArray(v)||Object.keys(v).sort().join(',')!=='category,equipment_ids,variant_names'||![null,'military','naval'].includes(v.category))return null;
  for(const [key,max]of [['equipment_ids',512],['variant_names',128]])if(!Array.isArray(v[key])||v[key].length>max||v[key].some(x=>typeof x!=='string'||!x)||key==='equipment_ids'&&!v[key].length)return null;
  const result={category:v.category,equipment_ids:[...new Set(v.equipment_ids)].sort(compare),variant_names:[...new Set(v.variant_names)].sort(compare)};
  if(Object.hasOwn(step.action.params,'design_reference')){const recipe=designs.configuration(step.action.params.design_reference);if(!recipe)return null;result.design=recipe;}
  return result;
 }
 function relativeSpec(step){
  const p=step.action.params||{},s=selectorFor(step),requested=Object.hasOwn(p,'requested_factories'),q=requested?p.requested_factories:p.assigned_factories,condition=step.completion;
  if(!s||p.category!=='production'||p.quantity_mode!=='additional'||!integer(q,1)||!condition||Object.keys(condition).length!==4||condition.op!=='fact'||condition.key!=='guide.production.'+step.key+'.added'||condition.test!=='gte'||condition.value!==q)return null;
  return {fact_key:(requested?'production_requested:':'production:')+designs.stableStringify(s),quantity:q};
 }
 function selectorIdentity(selector){
  const value=Object.fromEntries(Object.entries(selector).filter(([key])=>key!=='variant_names'||!Object.hasOwn(selector,'design')));
  return designs.stableStringify(value);
 }
 function context(guide,state,baselines){
  const facts={...state.facts},details=new Map(),lines=c.fact(state,'production.lines'),country=c.fact(state,'country').value;
  const known=lines.status!=='unknown'&&Array.isArray(lines.value);
  const calculated=v=>v===null?{status:'unknown',value:null,source:'',reason:'Produzione o conteggio iniziale non verificabile'}:{status:'calculated',value:v,source:'production_adapter'};
  const steps=Object.values(guide.tracks).flatMap(rows=>rows.flatMap(step=>Array.isArray(step.action.params?.production_targets)?step.action.params.production_targets.map((p,i)=>({key:step.key+'.target-'+(i+1),layout_key:step.key,action:{kind:'manual',params:p}})):step));
  // One evaluation owns this cache; never retain matches across live samples.
  const matchesCache=new Map(),military=known?lines.value.filter(row=>row.category==='military'):[];
  const selectors=new Map(steps.filter(step=>step.action.params?.category==='production').map(step=>[step.key,selectorFor(step)]));
  const priorityPeers=new Map(),priorityAllowed=new Map();
  const priorityKey=step=>JSON.stringify([step.layout_key||step.key,step.action.params.production_priority]);
  for(const step of steps)if(step.action.params?.production_priority){const key=priorityKey(step);if(!priorityPeers.has(key))priorityPeers.set(key,[]);priorityPeers.get(key).push(selectors.get(step.key));}
  const comparison=s=>{if(!known||!s)return {rows:[],complete:true,design_check:null};const key=selectorIdentity(s);if(!matchesCache.has(key))matchesCache.set(key,designs.matching(s,lines.value));return matchesCache.get(key);};
  for(const step of steps){
   const p=step.action.params||{};if(p.category!=='production'||!Object.hasOwn(p,'production_selector'))continue;
   const key='guide.production.'+step.key,s=selectors.get(step.key);
   const result=comparison(s),matches=result.rows;
   let metrics=Object.fromEntries([['assigned','assigned_factories'],['requested','requested_factories'],['active','active_factories'],['damaged','damaged_factories']].map(([label,field])=>[label,known&&s&&result.complete?matches.reduce((n,row)=>n+row[field],0):null]));
   if(Object.values(metrics).some(v=>v!==null&&!integer(v)))metrics=Object.fromEntries(Object.keys(metrics).map(k=>[k,null]));
   const {assigned,requested}=metrics,spec=relativeSpec(step),b=spec?baselines[step.key]:null;
   const valid=b&&b.country===country&&b.fact_key===spec.fact_key&&b.quantity===spec.quantity&&integer(b.value+spec.quantity),initial=valid?b.value:null,relative=p.quantity_mode==='additional';
   const requestedBasis=Object.hasOwn(p,'requested_factories'),count=requestedBasis?requested:assigned,targetValue=requestedBasis?p.requested_factories:p.assigned_factories;
   let target=relative?(valid?initial+spec.quantity:null):(integer(targetValue)?targetValue:null);
   const remaining=p.quantity_mode==='remaining';let allocated=null,remainingReason='';
   if(remaining){
    const totalFact=c.fact(state,'country.production.effective_factories.military'),total=integer(totalFact.value)?totalFact.value:null;
    target=null;
    if(known&&result.complete&&s?.category==='military'&&total!==null){
     const others=military.filter(row=>!matches.includes(row)).reduce((n,row)=>n+row.requested_factories,0);
     if(integer(others)){target=Math.max(total-others,0);allocated=matches.length===1&&military.length>0&&matches[0]===military.at(-1)&&requested>0&&requested>=target;}
    }
    if(target===null)remainingReason='remaining_capacity_unknown';
    else if(matches.length>1)remainingReason='remaining_line_ambiguous';
    else if(!matches.length)remainingReason='remaining_line_missing';
    else if(matches[0]!==military.at(-1))remainingReason='remaining_line_not_last';
   }
   const started=relative&&valid&&assigned!==null?requested>initial||assigned>initial:relative||assigned===null?null:requested>0||assigned>0;
   const priority=p.production_priority;let priorityValue=null;
   if(priority&&known&&result.complete&&s?.category==='military'){
    const group=priorityKey(step);
    if(!priorityAllowed.has(group)){
     const allowed=new Set(),uncertain=new Set();
     for(const peer of priorityPeers.get(group)){const result=comparison(peer);result.rows.forEach(r=>allowed.add(r.id));if(!result.complete&&result.design_check)result.design_check.candidates.filter(r=>r.status==='unknown').forEach(r=>uncertain.add(r.line_id));}
     priorityAllowed.set(group,{allowed,uncertain});
    }
    const {allowed,uncertain}=priorityAllowed.get(group);
    priorityValue=false;if(matches.length===1){const pos=military.indexOf(matches[0]),surrounding=priority==='first'?military.slice(0,pos):military.slice(pos+1),values=surrounding.map(r=>allowed.has(r.id)?true:uncertain.has(r.id)?null:false);priorityValue=values.includes(false)?false:values.includes(null)?null:true;}
   }
   if(priority)facts[key+'.priority_satisfied']=priorityValue===null?{status:'unknown',value:null,source:'',reason:'Production priority unavailable'}:calculated(priorityValue);
   for(const [suffix,value]of [['assigned',assigned],['requested',requested],['added',valid&&count!==null?count-initial:null],['started',started],['remaining_allocated',allocated]])facts[key+'.'+suffix]=calculated(value);
   details.set(step.key,{...metrics,baseline:initial,target,lines:assigned===null?[]:matches,basis:remaining?'remaining':requestedBasis?'requested':'assigned',reason:remaining?remainingReason:assigned===null?'production_unavailable':relative&&!valid?'baseline_missing':''});
   if(priority)Object.assign(details.get(step.key),{priority,priority_satisfied:priorityValue});
   if(result.design_check)details.get(step.key).design_check=JSON.parse(JSON.stringify(result.design_check));
  }
  for(const rows of Object.values(guide.tracks))for(const step of rows){const targets=step.action.params?.production_targets;if(Array.isArray(targets))details.set(step.key,{layout:targets.map((p,i)=>details.get(step.key+'.target-'+(i+1))||{})});}
  return {state:{...state,facts},details};
 }
 function admission(step,phases,state,results,checks){
   const blockedMilestones=[],milestoneValues=[];
   for(const milestone of step.action.params?.production_milestones||[]){const current=c.condition(milestone.condition,state,checks),historical=c.condition({op:'fact',key:'guide.campaign.milestones',test:'contains',value:milestone.token},state,checks);
    if(current!==true&&historical!==true){blockedMilestones.push(milestone.step);milestoneValues.push(current);}
   }
  const p=step.action.params||{},values=[p.production_section_when,phases.get(step.phase)?.when,step.when].filter(n=>n!==undefined).map(n=>c.condition(n,state,checks));
  if(step.skip_when!==undefined){const v=c.condition(step.skip_when,state,checks);values.push(v===null?null:!v);}
  if(values.some(v=>v!==true)){const unknown=!values.includes(false);return {status:unknown?'unverifiable':'waiting',reason:unknown?'condition_unknown':'condition_false'};}
   if(blockedMilestones.length){const knownFalse=milestoneValues.includes(false);return {status:knownFalse?'waiting':'unverifiable',reason:knownFalse?'milestone_not_completed':'milestone_unknown',blocked_by:blockedMilestones};}
  const blocked=[...(step.depends_on||[]).filter(k=>results.get(k).status!=='completed'),...(step.depends_on_started||[]).filter(k=>results.get(k).selection.value!==true&&results.get(k).completion.value!==true)];
  if(blocked.length){const unknown=blocked.some(k=>results.get(k).status==='unverifiable');return {status:unknown?'unverifiable':'waiting',reason:unknown?'dependency_unknown':'dependency_not_completed',blocked_by:blocked};}
  return null;
 }
 function deny(result,decision){
  if(!Object.hasOwn(result,'original_completion'))result.original_completion=result.completion;
  if(!Object.hasOwn(result,'original_selection'))result.original_selection=result.selection;
  Object.assign(result,decision,{production_admitted:false,completion:{status:'unknown',value:null,source:'',reason:'revision_not_admitted'},selection:{status:'unknown',value:null,source:'',reason:'revision_not_admitted'}});
 }
 function phaseReached(step,phases,state,checks){
  const p=step.action.params||{},guards=[p.production_section_when,phases.get(step.phase)?.when,step.when].filter(n=>n!==undefined);
  if(!guards.length||!guards.every(n=>c.condition(n,state,checks)===true))return false;
  return step.skip_when===undefined||c.condition(step.skip_when,state,checks)===false;
 }
 function applyRevisions(guide,state,results,checks){
  if(!state.usable)return;
  const rows=Object.values(guide.tracks).flat(),phases=new Map((guide.phases||[]).map(p=>[p.id,p]));
  const protectedKeys=new Set(rows.flatMap(s=>[...(s.depends_on||[]),...(s.depends_on_started||[])])),admitted=[];
  for(const step of rows){
   const p=step.action.params||{},revision=p.production_revision,r=results.get(step.key);
   if(!revision||r.reason==='guide_scope_mismatch')continue;
   const denied=admission(step,phases,state,results,checks);if(denied){deny(r,denied);if(!phaseReached(step,phases,state,checks))continue;}
   const targets=p.production_targets||[p],detail=r.production||{},details=p.production_targets?detail.layout||[]:[detail];
   targets.forEach((target,i)=>{const selector=selectorFor({action:{params:target}});if(selector&&target.quantity_mode!=='additional'&&details[i])admitted.push({group:revision.group,order:revision.order,step,i,p:target,selector,detail:details[i]});});
  }
  admitted.sort((a,b)=>compare(a.group,b.group)||a.order-b.order);
  for(let n=0;n<admitted.length;n++){
   const a=admitted[n];if(protectedKeys.has(a.step.key))continue;
   const replacements=admitted.slice(n+1).filter(b=>b.group===a.group&&b.order>a.order&&b.step.key!==a.step.key&&
    (selectorIdentity(b.selector)===selectorIdentity(a.selector)||a.p.quantity_mode==='remaining'&&b.p.quantity_mode==='remaining'&&a.selector.category==='military'&&b.selector.category==='military')).map(b=>b.step.key);
   if(replacements.length)a.detail.superseded_by=[...new Set(replacements)];
  }
  for(const step of rows){
   const r=results.get(step.key),p=step.action.params||{},d=r.production||{},details=p.production_targets?d.layout||[]:[d],replaced=details.filter(x=>x.superseded_by?.length);
   if(!replaced.length)continue;
   r.superseded_by=[...new Set(replaced.flatMap(x=>x.superseded_by))];
   const remaining=details.map((x,i)=>[i,x]).filter(([,x])=>!x.superseded_by?.length),completion=step.completion||{},clauses=p.production_targets&&completion.op==='all'?completion.args:[completion];
   const values=clauses.length===details.length?remaining.map(([i])=>c.condition(clauses[i],state,checks)):[null];
   r.original_completion=r.completion;
   if(remaining.length&&r.production_admitted===false)continue;
   if(!remaining.length||values.every(v=>v===true))Object.assign(r,{status:'not_applicable',reason:'production_superseded',completion:{status:'unknown',value:null,source:'',reason:'requirements_replaced'}});
   else if(values.includes(false))Object.assign(r,{status:r.selection.value===true?'active':'ready',reason:'production_partially_superseded',completion:{status:'calculated',value:false,source:'effective_production_goal'}});
   else Object.assign(r,{status:'unverifiable',reason:'production_evidence_unknown'});
  }
 }
 const api={context,selectorFor,relativeSpec,applyRevisions,admission,deny};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiProductionProgress=api;
})(typeof window!=='undefined'?window:globalThis);
