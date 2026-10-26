'use strict';
// Explanations of an existing runtime evaluation, never another rule evaluator.
((root)=>{
 const uiText=typeof module!=='undefined'&&module.exports?require('./i18n.js').t:root.HoiI18n.t;
 const contracts=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const number=value=>typeof value==='number'&&Number.isFinite(value);
 const count=value=>Number.isSafeInteger(value)&&value>=0;
 const title=step=>step.name||step.title||step.key;
 function factKeys(node,result=new Set()){
  if(node?.op==='fact')result.add(node.key);
  for(const child of node?.args||[])factKeys(child,result);
  if(node?.arg)factKeys(node.arg,result);return result;
 }
 function evidence(state,key,label){const f=contracts.fact(state,key);return {key,label,value:f.value,status:f.status,source:f.source,reason:f.reason||''};}
 const label=key=>key.includes('political_power')?uiText("Political power"):key.endsWith('.military_factories')?uiText("Military factories"):key.endsWith('.civilian_factories')?uiText("Civilian factories"):key.endsWith('.infrastructure')?uiText("Infrastructure"):key.split('.').at(-1).replaceAll('_',' ');
 function nativeHref(step){const href=step.href;
  if(typeof href==='string'&&href.startsWith('/')&&!href.startsWith('//')){
   try{const parsed=new URL(href,'http://local');if(parsed.origin==='http://local'&&['/','/technologies'].includes(parsed.pathname))return parsed.pathname+parsed.search+parsed.hash;}catch{}
  }
  return step.action.kind==='research'?'/technologies':'/';
 }
 function project(guide,runtime,value,options={}){
  const state=contracts.state(value),limit=Number.isInteger(options.limit)?Math.max(1,Math.min(8,options.limit)):6;
  const result={schema_version:1,mode:runtime?.mode||'preview',country:runtime?.country||null,notice:'',items:[],hidden_count:0};
  if(runtime?.guide_id!==guide?.id){result.notice=uiText("Summary suspended: evaluation belongs to another guide.");return result;}
  if(['wrong_country','wrong_tree'].includes(runtime.mode)){result.notice=uiText("Guide suspended: game country or tree does not match.");return result;}
  if(runtime.mode!=='live'||!state.usable){result.notice=uiText("Game state unavailable. The plan remains available; previous data is not presented as current.");return result;}
  if(runtime.country!==contracts.fact(state,'country').value){result.notice=uiText("Summary suspended: evaluation and data belong to different countries.");return result;}
  guide=contracts.guide(guide); // Same legacy metadata adapter used by the existing presentation.
  const definitions=new Map(Object.values(guide.tracks||{}).flat().map(s=>[s.key,s])),rows=new Map((runtime.steps||[]).map(r=>[r.key,r]));
  const dependencyLinks=row=>(row.blocked_by||[]).map(key=>definitions.get(key)).filter(Boolean).slice(0,4).map(reference);
  const items=[];let order=0;
  const metric=(name,value)=>({label:name,value:count(value)?value:null,status:count(value)?'calculated':'unknown',source:count(value)?'runtime.production':''});
  for(const row of runtime.steps||[]){
   const step=definitions.get(row.key);if(!step||step.tracking==='reminder'||row.status==='not_applicable'||['select_focus','research','free'].includes(step.action.kind))continue;
   if(row.action_readiness?.domain==='political'&&runtime.action_sequence?.political!==row.key)continue;
   const p=step.action.params||{},category=p.category||(step.action.kind==='special_project'?'projects':'checkpoints');
   // Future blocked instructions stay in the full plan, not in the current summary.
   if(row.blocked_by?.length||row.reason==='skip_condition_unknown')continue;
   let item={key:row.key,step_key:row.key,category,title:title(step),tone:'ready',message:'',guide_goal:'',
    evidence:[],runtime_status:row.status,reason:row.reason,blocked_by:[...(row.blocked_by||[])],
    href:'#operation-'+encodeURIComponent(row.key),link_label:uiText("Show instruction"),rank:40,priority:row.priority||0,order:order++};
   const knownKeys=new Set();for(const node of [step.when,step.completion,step.started_when,guide.phases?.find(p=>p.id===step.phase)?.when])factKeys(node,knownKeys);
   item.evidence=[...knownKeys].map(key=>evidence(state,key,label(key)));
   if(row.production){
    const comparisons=(row.production.layout||[row.production]).map((detail,i)=>({
     name:row.production.layout?(p.production_targets?.[i]?.variant_name||p.production_targets?.[i]?.equipment_name||p.production_targets?.[i]?.equipment_id||uiText("Product ")+(i+1)):(p.variant_name||p.equipment_name||p.equipment_id||title(step)),
     target:count(detail.target)?detail.target:null,requested:count(detail.requested)?detail.requested:null,
     assigned:count(detail.assigned)?detail.assigned:null,active:count(detail.active)?detail.active:null,
     basis:detail.basis,reason:detail.reason||''}));
    const gap=comparisons.some(p=>p.requested!==null&&p.assigned!==null&&p.requested>p.assigned);
    if(row.status==='completed'&&!gap)continue;
    item.comparisons=comparisons;
    item.evidence=comparisons.flatMap(p=>[metric(uiText("Requested factories"),p.requested),metric(uiText("Assigned factories"),p.assigned)]);
    item.guide_goal=comparisons.map(p=>p.name+': '+(p.target===null?uiText("total cannot be verified"):p.target+uiText(" factories ")+(p.basis==='requested'?uiText("requested"):uiText("assigned")))).join(' · ');
    if(gap){item.tone='attention';item.rank=10;item.title=uiText("Production: attention required");item.reason='requested_exceeds_assigned';
     item.message=comparisons.filter(p=>p.requested!==null&&p.assigned!==null&&p.requested>p.assigned).map(p=>uiText("{0}: {1} requested, {2} assigned.",p.name,p.requested,p.assigned)).join(' ');
     item.message+=row.status==='completed'?uiText(" The guide's criterion is met; requested allocation and assigned capacity remain separate values."):uiText(" Requested factories do not yet match assigned factories.");
    }else if(comparisons.some(p=>p.assigned===null||p.target===null)){
     item.tone='unknown';item.rank=50;item.title=uiText("Production cannot be verified");item.message=uiText("A qualified count or target total is missing. Order execution is not inferred.");
    }else{item.title=row.status==='active'?uiText("Production in progress"):uiText("Production to set up");item.tone=row.status==='active'?'active':'ready';
     item.rank=row.status==='active'?20:40;item.message=uiText("Compare the guide's total with requested and assigned factories in the game.");}
   }else{
    if(row.status==='completed')continue;
    const pp=item.evidence.find(e=>e.key.includes('political_power'));
    if(row.action_readiness?.domain==='political'&&!(row.status==='unverifiable'&&pp?.status==='unknown')){
     const p=row.action_readiness.political_power;item.title=p?.affordable===false?uiText("Insufficient political power"):uiText("Next appointment or law");item.tone=p?.affordable===false?'waiting':row.availability.value===null?'unknown':'ready';item.rank=30;
     item.message=uiText("{0}. Political power: {1}; guide-declared cost: {2}.",title(step),p?.available??uiText("unverifiable"),p?.cost??uiText("not specified"));
     if(row.action_readiness.unknown.length)item.message+=uiText(" Some requirements remain unverifiable.");
     if(p?.affordable===false)item.message+=uiText(" Wait for the required resources; this action stays first in the sequence.");
    }else if(row.status==='unverifiable'&&pp?.status==='unknown'){
     item.title=uiText("Political power cannot be verified");item.tone='unknown';item.rank=30;
     item.message=uiText("The guide requires a political-power check, but the necessary data is unavailable. The condition cannot be verified.");
    }else if(row.status==='active'){
     item.tone='active';item.rank=20;
     if(row.selection?.source?.startsWith('manual_started:')){
      item.title=uiText("Start reported manually");item.evidence=[];item.message=uiText("You reported the start manually. Completion is not confirmed yet.");
     }else if(category==='construction'){
      item.title=uiText("Construction started");item.message=uiText("Construction order started. Completion is not confirmed yet.");
     }else if(category==='projects'||step.action.kind==='special_project'){
      item.title=uiText("Project in progress");item.message=uiText("Start detected using the guide's criterion. The project is not completed yet.");
     }else{item.title=uiText("Activity in progress");item.message=uiText("The runtime detects the start specified by the guide; the final criterion remains separate.");}
    }else if(row.status==='waiting'){
     item.title=uiText("Waiting for a guide condition");item.tone='waiting';item.rank=50;
     item.message=uiText("The guide's declared condition is not met yet. This does not indicate an error or a completed order.");
    }else if(row.status==='unverifiable'){
     item.title=row.reason==='catalog_unmatched'?uiText("Binding needs verification"):uiText("Activity needs verification");item.tone='unknown';item.rank=50;
     item.message=row.reason==='catalog_unmatched'?uiText("The required identity does not match the local catalog."):uiText("The runtime has insufficient evidence of the final criterion. Completion is not inferred.");
    }else{item.title=uiText("Instruction ready to check");item.message=uiText("Declared dependencies are satisfied. Actual availability in the game is not certified.");}
    item.guide_goal=title(step);
    if(category==='construction'){
     const target=row.construction?.target??(step.completion?.op==='fact'&&number(step.completion.value)?step.completion.value:null);
     item.guide_goal=(count(target)?uiText("Reach ")+target:count(p.quantity)?(p.quantity_mode==='target_total'?uiText("Reach "):uiText("Add "))+p.quantity:uiText("Check the construction target"))+(p.building_name?' '+p.building_name:'')+(p.state_name?' · '+p.state_name:'');
     if(row.construction){item.evidence.push({label:uiText("Baseline count"),value:row.construction.baseline,status:row.construction.baseline===null?'unknown':'calculated',source:'runtime.construction'},
      {label:uiText("Current count"),value:row.construction.current,status:row.construction.current===null?'unknown':'observed',source:'runtime.construction'});}
    }
   }
   items.push(item);
  }
  for(const [category,kind]of [['research','research'],['focus','select_focus']]){
   const candidates=(runtime.steps||[]).filter(r=>definitions.get(r.key)?.action.kind===kind&&!['completed','active','not_applicable'].includes(r.status));
   const parallel=runtime.parallel?.[category],ready=(parallel?.steps||[]).map(k=>rows.get(k)).filter(Boolean);
   const nextKeys=new Set(Object.values(runtime.tracks||{}).map(t=>t.next));
   const row=ready[0]||candidates.find(r=>nextKeys.has(r.key))||candidates[0];if(!row)continue;
   const step=definitions.get(row.key),research=category==='research';
   const item={key:category,step_key:row.key,category,tone:ready.length?'ready':row.status==='unverifiable'?'unknown':'waiting',
    title:research?uiText("Next research"):uiText("Next national focus"),guide_goal:research?uiText("Follow the research in its technology tab."):uiText("Follow the focus in its tree."),
    runtime_status:row.status,reason:row.reason,blocked_by:[...(row.blocked_by||[])],evidence:[],
    href:nativeHref(step),link_label:research?uiText("Open technologies"):uiText("Open focuses"),rank:ready.length?35:55,priority:row.priority||0,order:order++};
   item.message=ready.length?(research?uiText("{0} free slots; {1} research steps prescribed by the guide are ready.",parallel.capacity,ready.length):uiText("The runtime proposes the guide's prescribed focus. It is highlighted in its tree.")):
    parallel?.capacity===0?(research?uiText("Observed research slots are occupied. Upcoming technologies remain available in their tabs."):uiText("A focus is already active. The next prescribed focus remains available in its tree.")):
    row.reason==='dependency_not_completed'?(research?uiText("The next research depends on completing a previous technology or another guide step."):uiText("The next focus is waiting for its required steps to complete.")):
    row.reason==='dependency_unknown'?uiText("Completion of a previous step cannot be verified."):uiText("The runtime is waiting for a condition or verification: see the relevant tab.");
   if(row.blocked_by?.length){
    item.links=dependencyLinks(row);const names=item.links.map(l=>l.label).join(', ');
    if(names)item.message=(row.reason==='dependency_unknown'?uiText("Completion cannot be verified: "):uiText("Waiting for completion of: "))+names+'.';
    if(row.blocked_by.length>4)item.message+=uiText(" Additional prerequisites are listed in the tab.");
   }
   if(research&&parallel?.capacity===0){
    const slots=contracts.fact(state,'research.slots');
    if(slots.status!=='unknown'&&Array.isArray(slots.value))item.message=uiText("Observed slots are occupied: {0} of {1}. No new research to start; see the tabs for active research.",slots.value.filter(s=>s.technology_id).length,slots.value.length);
   }
   items.push(item);
  }
  items.sort((a,b)=>a.rank-b.rank||b.priority-a.priority||a.order-b.order);
  result.items=items.slice(0,limit).map(({rank,priority,order,...item})=>item);
  result.hidden_count=Math.max(0,items.length-result.items.length);
  if(!items.length)result.notice=uiText("No activities need additional explanation right now. The full plan remains available.");
  return result;
 }
 function reference(step){return {label:title(step),href:['research','select_focus'].includes(step.action.kind)?nativeHref(step):'#operation-'+encodeURIComponent(step.key)};}
 const api={project,reference};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiOperationalSummary=api;
})(typeof window!=='undefined'?window:globalThis);
