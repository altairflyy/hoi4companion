'use strict';
((root)=>{
 const VERSION=1;
 const unknown=(reason='Campo non osservato')=>({status:'unknown',value:null,source:'',reason});
 const known=(value,source='process_memory',calculated=false)=>value===null||value===undefined?unknown():({status:calculated?'calculated':'observed',value,source});
 function validType(key,value){
  if(['country','focus.tree'].includes(key))return typeof value==='string'&&value.length>0;
  if(['focus.completed','focus.active','focus.continuous','focus.ids','research.completed','research.active','research.ids'].includes(key))return Array.isArray(value)&&value.every(x=>typeof x==='string'&&x.length>0);
  if(key==='research.slots')return Array.isArray(value)&&value.every(x=>x&&typeof x==='object'&&Object.hasOwn(x,'technology_id')&&(x.technology_id===null||typeof x.technology_id==='string'&&x.technology_id.length>0));
  return true;
 }
 function validateFacts(value){return {...value,facts:Object.fromEntries(Object.entries(value.facts).map(([k,f])=>[k,fact(value,k)]))};}
 function state(value){
  if(value?.schema_version!==undefined){
   if(value.schema_version!==VERSION||value.usable!==true||!value.facts||typeof value.facts!=='object'||Array.isArray(value.facts))return {schema_version:VERSION,usable:false,session_id:null,facts:{}};
   return validateFacts(value);
  }
  const s=value?.usable===true&&value.sample&&typeof value.sample==='object'&&!Array.isArray(value.sample)?value.sample:{};
  const source=value?.source||'process_memory',f={};
  f.country=known(s.country,source);f['focus.tree']=known(s.focus_tree_id,'legacy_adapter',true);
  f['focus.completed']=s.focus_complete===false?unknown():known(s.completed_focuses,source);
  f['research.completed']=s.research_complete===false?unknown():known(s.researched_technologies,source);
  f['focus.active']=Object.hasOwn(s,'current_focus')?known(s.current_focus?[s.current_focus]:[],source):unknown();
  f['focus.continuous']=Object.hasOwn(s,'continuous_focus')?known(s.continuous_focus?[s.continuous_focus]:[],source):unknown();
  f['research.active']=validType('research.slots',s.research_slots)?known(s.research_slots.map(x=>x.technology_id).filter(Boolean),source):unknown();
  f['research.slots']=known(s.research_slots,source);f['focus.ids']=known(s.focus_ids,source);f['research.ids']=known(s.technology_ids,source);
  const overview=s.country_overview||{},derived=new Set(['by_type','by_template','total','effective_factories','output_per_day','assigned_factories','category','balance','used','owned','controlled','name']);
  function put(path,data,present,calculated){f[path]=present?known(data,calculated?'legacy_adapter':source,calculated):unknown();}
  function flatten(path,data,present,calculated=false){
   if(Array.isArray(data)){put(path+'.count',data.length,present,true);data.forEach((child,i)=>flatten(path+'.'+(child&&typeof child==='object'?child.id??String(i):String(i)),child,present,calculated));}
   else if(data&&typeof data==='object'){for(const [key,child]of Object.entries(data))if(!['status','reason'].includes(key))flatten(path+'.'+key,child,present,calculated||derived.has(key));}
   else put(path,data,present,calculated||(path.startsWith('country.equipment.items.')&&path.endsWith('.stockpile'))||(path.startsWith('country.production.lines.')&&path.endsWith('.id')));
  }
  if(overview.country===s.country)for(const [section,data]of Object.entries(overview))if(data&&typeof data==='object'&&!Array.isArray(data)){
   const present=['observed','partial'].includes(data.status);
   if(section==='manpower'){for(const [key,child]of Object.entries(data))if(!['status','reason'].includes(key))put('country.manpower.'+key,child,present,key==='available');}
   else flatten('country.'+section,data,present);
  }
  return validateFacts({schema_version:VERSION,usable:Object.keys(s).length>0,session_id:s.session_id||null,facts:f});
 }
 function fact(state,key){const f=state?.schema_version===VERSION&&state?.usable===true?state.facts?.[key]:null;return f&&['observed','calculated'].includes(f.status)&&typeof f.source==='string'&&f.source&&f.value!==null&&f.value!==undefined&&validType(key,f.value)&&!(typeof f.value==='number'&&!Number.isFinite(f.value))?f:unknown();}
 function condition(node,s,checks={},depth=0){
  if(!node||depth>16||typeof node!=='object')return null;
  if(node.op==='all'||node.op==='any'){
   if(!Array.isArray(node.args)||!node.args.length||node.args.length>64)return null;
   const values=node.args.map(x=>condition(x,s,checks,depth+1));
   return node.op==='all'?(values.includes(false)?false:values.includes(null)?null:true):(values.includes(true)?true:values.includes(null)?null:false);
  }
  if(node.op==='not'){const v=condition(node.arg,s,checks,depth+1);return v===null?null:!v;}
  if(node.op==='check')return Object.hasOwn(checks,node.key)&&typeof checks[node.key]==='boolean'?checks[node.key]:null;
  if(node.op!=='fact')return null;
  const f=fact(s,node.key);if(f.status==='unknown')return null;
  if(node.test==='contains')return Array.isArray(f.value)?f.value.includes(node.value):null;
  if(node.test==='eq')return ['number','string','boolean'].includes(typeof node.value)&&typeof f.value===typeof node.value?f.value===node.value:null;
  if(node.test==='gte')return typeof f.value==='number'&&Number.isFinite(f.value)&&typeof node.value==='number'&&Number.isFinite(node.value)?f.value>=node.value:null;
  return null;
 }
 function guide(input){
  if(input.schema_version!==undefined&&input.schema_version!==VERSION)throw Error('Versione contratto guida non supportata');
  if(input.schema!==undefined&&input.schema!==1)throw Error('Versione guida legacy non supportata');
  const result={...input,schema_version:VERSION,applies_to:input.applies_to||Object.fromEntries(['country','tree_id'].filter(k=>input[k]).map(k=>[k,input[k]])),tracks:{}};
  const actions={focus:'select_focus',technology:'research',project:'special_project',gate:'manual',free:'free'},kinds=Object.fromEntries(Object.entries(actions).map(([k,v])=>[v,k]));
  for(const [track,steps] of Object.entries(input.tracks||{}))result.tracks[track]=steps.map((s,i)=>{
   const action=s.action||{kind:actions[s.kind],...(['focus','technology','project'].includes(s.kind)?{entity:{kind:s.kind,id:s.target}}:{})};
   const r={...s,key:s.key||`${track}-${i+1}`,action,kind:kinds[action.kind],target:action.entity?.id||s.target};
   if(s.gate&&!r.when)r.when={op:'check',key:s.gate};
   if(s.available_gate&&s.priority_after&&!r.priority_when){r.priority=s.priority??100;r.priority_when={op:'all',args:[{op:'fact',key:'focus.completed',test:'contains',value:s.priority_after},{op:'check',key:s.available_gate}]};}
   return r;
  });
  const steps=Object.values(result.tracks).flat();
  for(const s of steps)if(s.optional_before&&!s.skip_when)s.skip_when={op:'any',args:[{op:'fact',key:'focus.completed',test:'contains',value:s.optional_before},...steps.filter(p=>p.target===s.optional_before&&p.priority_when).map(p=>p.priority_when)]};
  return result;
 }
 function checkKeys(node,result=new Set()){
  if(node?.op==='check')result.add(node.key);
  for(const child of node?.args||[])checkKeys(child,result);
  if(node?.arg)checkKeys(node.arg,result);
  return [...result];
 }
 const api={VERSION,state,fact,condition,guide,checkKeys};
 if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiContracts=api;
})(typeof window!=='undefined'?window:globalThis);
