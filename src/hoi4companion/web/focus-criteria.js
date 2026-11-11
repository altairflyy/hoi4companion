'use strict';
((root)=>{
 const record=x=>!!x&&typeof x==='object'&&!Array.isArray(x);
 function validate(r){
  if(!record(r)||Object.keys(r).sort().join(',')!=='available,branch,conflicts,mutex,prerequisites,schema_version'||r.schema_version!==1)throw Error('Invalid focus criteria metadata');
  let budget=0;
  function visit(n,depth=0){
   if(++budget>4096||depth>32||!record(n))throw Error('Invalid focus criterion');
   const keys=Object.keys(n).sort().join(',');
   if(['all','any'].includes(n.op)){if(keys!=='args,op'||!Array.isArray(n.args))throw Error('Invalid focus criteria group');for(const x of n.args)visit(x,depth+1);}
   else if(n.op==='not'){if(keys!=='arg,op')throw Error('Invalid focus criteria negation');visit(n.arg,depth+1);}
   else if(n.op==='if'){if(keys!=='condition,op,then')throw Error('Invalid focus criteria implication');visit(n.condition,depth+1);visit(n.then,depth+1);}
   else if(n.op==='const'){if(keys!=='op,value'||typeof n.value!=='boolean')throw Error('Invalid focus constant');}
   else if(n.op==='unknown'){if(Object.keys(n).some(k=>!['op','trigger','value'].includes(k))||typeof n.trigger!=='string'||Object.hasOwn(n,'value')&&typeof n.value!=='string')throw Error('Invalid unknown focus predicate');}
   else if(n.op==='compare'){if(keys!=='fact,op,test,trigger,value'||typeof n.fact!=='string'||typeof n.trigger!=='string'||!['>','<','>=','<=','=','!=','contains'].includes(n.test)||!['string','boolean','number'].includes(typeof n.value)||typeof n.value==='number'&&!Number.isFinite(n.value)||['>','<','>=','<='].includes(n.test)&&typeof n.value!=='number'||n.test==='contains'&&typeof n.value!=='string')throw Error('Invalid focus comparison');}
   else throw Error('Unsupported focus criteria operator');
  }
  visit(r.available);visit(r.branch);
  for(const field of ['mutex','conflicts'])if(!Array.isArray(r[field])||r[field].length>4096||r[field].some(x=>typeof x!=='string'||!x))throw Error('Invalid focus exclusions');
  if(!Array.isArray(r.prerequisites)||r.prerequisites.length>128||r.prerequisites.some(g=>!Array.isArray(g)||!g.length||g.length>512||g.some(x=>typeof x!=='string'||!x)))throw Error('Invalid focus prerequisite groups');
 }
 function assess(r,state){
  const checks=[],fact=key=>{const f=state?.facts?.[key];return state?.usable&&['observed','calculated'].includes(f?.status)&&f.source?f.value:null;};
  const group=(v,op)=>op==='all'?(v.includes(false)?false:v.includes(null)?null:true):(v.includes(true)?true:v.includes(null)?null:false);
  function evaluate(n){
   if(n.op==='const')return n.value;
   if(['all','any'].includes(n.op))return group(n.args.map(evaluate),n.op);
   if(n.op==='not'){const v=evaluate(n.arg);return v===null?null:!v;}
   if(n.op==='if'){const a=evaluate(n.condition),b=evaluate(n.then);return a===false||b===true?true:a===true?b:null;}
   if(n.op==='unknown'){checks.push({trigger:n.trigger,value:null,reason:'unsupported_trigger'});return null;}
   const actual=fact(n.fact),expected=n.value,test=n.test;let value=null;
   if(test==='contains')value=Array.isArray(actual)&&actual.every(x=>typeof x==='string')?actual.includes(expected):null;
   else if(actual!==null&&actual!==undefined){
    if(['>','<','>=','<='].includes(test)){if(typeof actual==='number'&&Number.isFinite(actual))value={'>':actual>expected,'<':actual<expected,'>=':actual>=expected,'<=':actual<=expected}[test];}
    else if(typeof actual===typeof expected)value=test==='='?actual===expected:actual!==expected;
   }
   checks.push({trigger:n.trigger,fact:n.fact,test,required:expected,actual:actual??null,value});return value;
  }
  const available=evaluate(r.available),branch=evaluate(r.branch),raw=fact('focus.completed'),done=Array.isArray(raw)&&raw.every(x=>typeof x==='string')?raw:null;
  const prerequisites=done===null?null:r.prerequisites.every(g=>g.some(p=>done.includes(p))),excluded=done===null?null:r.mutex.some(p=>done.includes(p));
  const value=group([available,branch,prerequisites,excluded===null?null:!excluded],'all');
  return {value,available,branch,prerequisites,excluded,checks,reason:excluded?'focus_excluded':prerequisites===false?'focus_prerequisites_false':value===false?'focus_requirements_false':value===null?'focus_requirements_unknown':'focus_requirements_satisfied'};
 }
 function alternatives(rows,results,tracks,capacity){
  const defs=new Map(rows.map(([,s])=>[s.key,s]));
  const keys=Object.values(tracks).flatMap(t=>t.order).filter(k=>defs.get(k).action.kind==='select_focus');
  const pending=keys.filter(k=>!['completed','not_applicable','active'].includes(results.get(k).status)&&!defs.get(k).action.params?.focus_optional);
  if(!pending.length)return null;
  const primary=pending[0],blocked=results.get(primary),result={primary,blocked:blocked.focus_readiness||{},steps:[]};
  if(capacity!==1||blocked.focus_readiness?.value!==false)return result;
  const protectedIds=new Set(pending.map(k=>defs.get(k).action.entity.id));
  for(const key of pending.slice(1)){const row=defs.get(key),item=results.get(key),rules=row.focus_criteria;
   if(row.action.entity.id===defs.get(primary).action.entity.id&&(row.action.entity.scope??null)===(defs.get(primary).action.entity.scope??null))continue;
   if(!rules||item.status!=='ready'||item.focus_readiness?.value!==true||rules.conflicts.some(k=>protectedIds.has(k)))continue;
   result.steps.push(key);break;
  }return result;
 }
 function operational(r,state){const rules={schema_version:1,available:r?.available,branch:{op:'const',value:true},prerequisites:[],mutex:[],conflicts:[]};try{validate(rules);}catch{return {value:null,checks:[{value:null,reason:'invalid_operational_criteria'}]};}return assess(rules,{...state,facts:{...state.facts,'focus.completed':{status:'calculated',source:'no_focus_dependency',value:[]}}});}
 const api={validate,assess,alternatives,operational};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiFocusCriteria=api;
})(typeof window!=='undefined'?window:globalThis);
