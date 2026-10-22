'use strict';
((root)=>{
 const c=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const integer=(v,min=0)=>Number.isSafeInteger(v)&&v>=min;
 const name=s=>s.normalize('NFKD').toLowerCase().replace(/[^\p{L}\p{N}]/gu,'');
 const compare=(a,b)=>{const x=Array.from(a,c=>c.codePointAt(0)),y=Array.from(b,c=>c.codePointAt(0));for(let i=0;i<Math.min(x.length,y.length);i++)if(x[i]!==y[i])return x[i]-y[i];return x.length-y.length;};
 function selectorFor(step){
  const v=step.action.params?.production_selector;
  if(!v||typeof v!=='object'||Array.isArray(v)||Object.keys(v).sort().join(',')!=='category,equipment_ids,variant_names'||![null,'military','naval'].includes(v.category))return null;
  for(const [key,max]of [['equipment_ids',512],['variant_names',128]])if(!Array.isArray(v[key])||v[key].length>max||v[key].some(x=>typeof x!=='string'||!x)||key==='equipment_ids'&&!v[key].length)return null;
  return {category:v.category,equipment_ids:[...new Set(v.equipment_ids)].sort(compare),variant_names:[...new Set(v.variant_names)].sort(compare)};
 }
 function relativeSpec(step){
  const p=step.action.params||{},s=selectorFor(step),q=p.assigned_factories,condition=step.completion;
  if(!s||p.category!=='production'||p.quantity_mode!=='additional'||!integer(q,1)||!condition||Object.keys(condition).length!==4||condition.op!=='fact'||condition.key!=='guide.production.'+step.key+'.added'||condition.test!=='gte'||condition.value!==q)return null;
  return {fact_key:'production:'+JSON.stringify(s),quantity:q};
 }
 function context(guide,state,baselines){
  const facts={...state.facts},details=new Map(),lines=c.fact(state,'production.lines'),country=c.fact(state,'country').value;
  const known=lines.status!=='unknown'&&Array.isArray(lines.value);
  const calculated=v=>v===null?{status:'unknown',value:null,source:'',reason:'Produzione o conteggio iniziale non verificabile'}:{status:'calculated',value:v,source:'production_adapter'};
  for(const rows of Object.values(guide.tracks))for(const step of rows){
   const p=step.action.params||{};if(p.category!=='production'||!Object.hasOwn(p,'production_selector'))continue;
   const key='guide.production.'+step.key,s=selectorFor(step),variants=new Set((s?.variant_names||[]).map(name));
   const matches=known&&s?lines.value.filter(row=>s.equipment_ids.includes(row.equipment)&&(!s.category||s.category===row.category)&&(!variants.size||variants.has(name(row.name)))):[];
   let metrics=Object.fromEntries([['assigned','assigned_factories'],['requested','requested_factories'],['active','active_factories'],['damaged','damaged_factories']].map(([label,field])=>[label,known&&s?matches.reduce((n,row)=>n+row[field],0):null]));
   if(Object.values(metrics).some(v=>v!==null&&!integer(v)))metrics=Object.fromEntries(Object.keys(metrics).map(k=>[k,null]));
   const {assigned,requested}=metrics,spec=relativeSpec(step),b=spec?baselines[step.key]:null;
   const valid=b&&b.country===country&&b.fact_key===spec.fact_key&&b.quantity===spec.quantity&&integer(b.value+spec.quantity),initial=valid?b.value:null,relative=p.quantity_mode==='additional';
   const target=relative?(valid?initial+spec.quantity:null):(integer(p.assigned_factories)?p.assigned_factories:null);
   const started=relative&&valid&&assigned!==null?requested>initial||assigned>initial:relative||assigned===null?null:requested>0||assigned>0;
   for(const [suffix,value]of [['assigned',assigned],['added',valid&&assigned!==null?assigned-initial:null],['started',started]])facts[key+'.'+suffix]=calculated(value);
   details.set(step.key,{...metrics,baseline:initial,target,lines:assigned===null?[]:matches,reason:assigned===null?'production_unavailable':relative&&!valid?'baseline_missing':''});
  }
  return {state:{...state,facts},details};
 }
 const api={context,relativeSpec};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiProductionProgress=api;
})(typeof window!=='undefined'?window:globalThis);
