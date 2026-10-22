'use strict';
((root)=>{
 const sections=['divisions','manpower','states','production','equipment','resources','politics','events','decisions'];
 function evaluate(value,enabled=true){
  const sample=enabled&&value?.usable===true?value.sample:null;
  const overview=sample?.country_overview;
  const matching=!!overview&&(!overview.country||overview.country===sample.country);
  const result={current:!!sample,country:sample?.country||null,observed_at:matching?overview.observed_at:null,
   mode:!enabled?'disabled':!sample?(value?.sample?'stale':'disconnected'):!matching?'unavailable':'current',sections:{}};
  for(const key of sections){
   const section=matching&&overview[key];
   const status=section&&['observed','partial'].includes(section.status)?section.status:'unavailable';
   result.sections[key]=status==='unavailable'?{status,reason:sample?(section?.reason||'Questo dato non è ancora disponibile dal lettore.'):null}:{...section,status};
  }
  return result;
 }
 function number(value,{integer=false,signed=false}={}){
  return typeof value==='number'&&Number.isFinite(value)&&(signed||value>=0)&&(!integer||Number.isInteger(value))?(value===0?0:value):null;
 }
 function productionOutput(value,category){
  const n=number(value);if(n===null)return null;
  // Match the game's truncated unit rates; retain readable slow naval output.
  const digits=category==='naval'&&n>0&&n<.01?4:2,scale=10**digits;
  return {value:Math.trunc(n*scale)/scale,digits};
 }
 const model={evaluate,number,productionOutput};
 if(typeof module!=='undefined'&&module.exports)module.exports=model;else root.HoiCountryModel=model;
})(typeof window!=='undefined'?window:globalThis);
