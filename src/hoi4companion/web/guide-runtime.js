'use strict';
((root)=>{
 const c=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const unknown=(reason='not_observed')=>({status:'unknown',value:null,source:'',reason});
 const calculated=(value,source)=>value===null?unknown():({status:'calculated',value,source});
 const settled=row=>['completed','not_applicable'].includes(row.status);
 function compareIds(a,b){
  const left=Array.from(a,c=>c.codePointAt(0)),right=Array.from(b,c=>c.codePointAt(0));
  for(let i=0;i<Math.min(left.length,right.length);i++)if(left[i]!==right[i])return left[i]-right[i];
  return left.length-right.length;
 }
 function progressFor(guide,state,input){
  if(input===null||input===undefined)return {status:'none',run_id:null,checks:{}};
  if(typeof input!=='object'||Array.isArray(input)||input.schema_version!==1||typeof input.guide_id!=='string'||typeof input.run_id!=='string'||!input.run_id.trim()||[...input.run_id].length>160||input.session_id!==undefined&&input.session_id!==null&&(typeof input.session_id!=='string'||!input.session_id.trim()))throw Error('Avanzamento guida non valido');
  const provided=Object.hasOwn(input,'checks')?input.checks:{};
  if(!provided||typeof provided!=='object'||Array.isArray(provided)||Object.values(provided).some(x=>typeof x!=='boolean'))throw Error('Conferme della partita non valide');
  if(input.guide_id!==(guide.id||''))return {status:'wrong_guide',run_id:null,checks:{}};
  if(input.session_id&&!state.usable)return {status:'session_unverified',run_id:null,checks:{}};
  if(input.session_id&&input.session_id!==state.session_id)return {status:'wrong_session',run_id:null,checks:{}};
  const allowed=new Set((guide.milestones||[]).map(x=>x.key));
  for(const rows of Object.values(guide.tracks))for(const step of rows){
   if(['manual','special_project'].includes(step.action.kind))allowed.add(step.key);
   for(const field of ['when','priority_when','skip_when','completion'])for(const key of c.checkKeys(step[field]))allowed.add(key);
  }
  for(const phase of guide.phases||[])for(const key of c.checkKeys(phase.when))allowed.add(key);
  return {status:'accepted',run_id:input.run_id,checks:Object.fromEntries(Object.entries(provided).filter(([key])=>allowed.has(key)))};
 }
 function evaluate(input,value,progress=null){
  const guide=c.guide(input),state=c.state(value),scope=guide.applies_to;
  const country=c.fact(state,'country').value,tree=c.fact(state,'focus.tree').value;
  const mode=!state.usable?'preview':scope.country&&country!==null&&country!==scope.country?'wrong_country':scope.tree_id&&tree!==null&&tree!==scope.tree_id?'wrong_tree':scope.country&&country===null||scope.tree_id&&tree===null?'preview':'live';
  const context=mode==='live'?state:{...state,usable:false},run=progressFor(guide,state,progress);
  const checks=['wrong_country','wrong_tree'].includes(mode)?{}:run.checks;
  const phases=new Map((guide.phases||[]).map(x=>[x.id,x]));
  const condition=node=>c.condition(node,context,checks);
  const membership=(key,target)=>{const f=c.fact(context,key);return f.status==='unknown'||!Array.isArray(f.value)?unknown():calculated(f.value.includes(target),'membership:'+key);};
  const definitions=new Map(),results=new Map(),indices=new Map(),barriers=new Map();let index=0;
  const trackEntries=Object.entries(guide.tracks).sort(([a],[b])=>compareIds(a,b));
  for(const [track,rows]of trackEntries){
   const manual=[];
   for(const step of rows){
    definitions.set(step.key,step);indices.set(step.key,index++);barriers.set(step.key,Object.hasOwn(step,'depends_on')?[]:[...manual]);
    const kind=step.action.kind,isNative=['select_focus','research'].includes(kind),isFocus=kind==='select_focus';
    let completion=unknown(),basis='free';
    if(Object.hasOwn(step,'completion')){completion=calculated(condition(step.completion),'declared_completion');basis='condition';}
    else if(['manual','special_project'].includes(kind)){completion=calculated(Object.hasOwn(checks,step.key)?checks[step.key]:null,'check:'+step.key);basis='manual';}
    else if(isNative){completion=membership(isFocus?'focus.completed':'research.completed',step.target);basis='native_membership';}
    const selection=isNative?membership(isFocus?'focus.active':'research.active',step.target):unknown();
    const priorityValue=step.priority_when?condition(step.priority_when):true;
    results.set(step.key,{key:step.key,track,status:'unverifiable',reason:'not_evaluated',blocked_by:[],completion,selection,completion_basis:basis,priority:priorityValue===true?(step.priority||0):0,priority_known:priorityValue!==null,availability:unknown('dynamic_availability_not_evaluated')});
    if(kind==='manual')manual.push(step.key);
   }
  }
  const indegree=new Map(),successors=new Map([...definitions.keys()].map(key=>[key,[]]));
  for(const [key,step]of definitions){const deps=step.depends_on||[];indegree.set(key,deps.length);for(const dep of deps)successors.get(dep).push(key);}
  const queue=[...definitions.keys()].filter(key=>indegree.get(key)===0);
  for(let cursor=0;cursor<queue.length;cursor++){
   const key=queue[cursor],step=definitions.get(key),row=results.get(key),kind=step.action.kind;
   const native=['select_focus','research'].includes(kind),entityScope=kind==='select_focus'?step.action.entity.scope:null;
   const set=(status,reason)=>{row.status=status;row.reason=reason;};
   const testGate=(node,falseReason,unknownReason)=>{const result=condition(node);if(result!==true){set(result===false?'waiting':'unverifiable',result===false?falseReason:unknownReason);return false;}return true;};
   if(['wrong_country','wrong_tree'].includes(mode)){row.completion=unknown();row.selection=unknown();set('not_applicable','guide_scope_mismatch');}
   else if(entityScope&&tree!==null&&tree!==entityScope){row.completion=unknown();row.selection=unknown();set('not_applicable','entity_scope_mismatch');}
   else if(entityScope&&tree===null){row.completion=unknown();row.selection=unknown();set('unverifiable','entity_scope_unknown');}
   else if(row.completion.value===true)set('completed','completion_true');
   else if(row.selection.value===true)set('active','selection_true');
   else if(native&&mode!=='live')set('unverifiable','live_state_unavailable');
   else if(step.skip_when&&condition(step.skip_when)!==false)set(condition(step.skip_when)===true?'not_applicable':'unverifiable',condition(step.skip_when)===true?'skip_condition_true':'skip_condition_unknown');
   else if(step.phase&&phases.get(step.phase).when&&!testGate(phases.get(step.phase).when,'phase_condition_false','phase_condition_unknown')){}
   else if(step.when&&!testGate(step.when,'condition_false','condition_unknown')){}
   else if(step.catalog_match===false)set('unverifiable','catalog_unmatched');
   else{
    const deps=step.depends_on||[];
    const explicit=deps.filter(dep=>results.get(dep).status!=='completed');
    const implicit=barriers.get(key).filter(dep=>results.get(dep).completion.value!==true);
    row.blocked_by=[...new Set([...explicit,...implicit])];
    if(row.blocked_by.length){
     const missing=explicit.some(dep=>results.get(dep).status==='unverifiable')||implicit.some(dep=>results.get(dep).completion.value===null);
     set(missing?'unverifiable':'waiting',missing?'dependency_unknown':'dependency_not_completed');
    }else if(native&&row.selection.value===null)set('unverifiable','selection_unknown');
    else if(row.completion.value===null&&kind!=='free')set('unverifiable',['manual','special_project'].includes(kind)?'manual_confirmation_required':'completion_unknown');
    else set('ready',kind==='free'?'free_choice':'prerequisites_satisfied');
   }
   for(const other of successors.get(key)){indegree.set(other,indegree.get(other)-1);if(indegree.get(other)===0)queue.push(other);}
  }
  const compare=(a,b)=>results.get(b).priority-results.get(a).priority||indices.get(a)-indices.get(b);
  const steps=[...results.values()],tracks={},candidates=new Set();
  for(const [track,rows]of trackEntries){
   const keys=rows.map(x=>x.key),order=[...keys].sort(compare),next=order.find(key=>!settled(results.get(key))&&results.get(key).status!=='active')||null;
   Object.defineProperty(tracks,track,{value:{keys,order,next},enumerable:true});
   for(const key of order){const row=results.get(key);if(settled(row)||row.status==='active')continue;if(definitions.get(key).action.kind==='special_project')continue;if(row.status!=='ready'){if(['dependency_not_completed','dependency_unknown'].includes(row.reason))continue;break;}candidates.add(key);}
  }
  const focus=c.fact(context,'focus.active'),slots=c.fact(context,'research.slots');
  const capacity={focus:Array.isArray(focus.value)?(focus.value.length?0:1):null,research:Array.isArray(slots.value)?slots.value.filter(x=>!x.technology_id).length:null};
  const ready=steps.filter(x=>x.status==='ready').map(x=>x.key).sort(compare),parallel={};
  for(const [lane,kind]of [['focus','select_focus'],['research','research']])parallel[lane]={capacity:capacity[lane],steps:capacity[lane]===null?[]:ready.filter(key=>candidates.has(key)&&definitions.get(key).action.kind===kind).slice(0,capacity[lane])};
  return {schema_version:1,guide_id:guide.id||'',run_id:run.run_id,progress_status:run.status,mode,country,steps,tracks,ready,parallel};
 }
 const api={evaluate};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiGuideRuntime=api;
})(typeof window!=='undefined'?window:globalThis);
