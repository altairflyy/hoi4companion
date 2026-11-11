'use strict';
((root)=>{
 const uiText=typeof module!=='undefined'&&module.exports?require('./i18n.js').t:root.HoiI18n.t;
 const contracts=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const sections=['divisions','manpower','experience','diplomacy','states','production','equipment','resources','politics','events','decisions'];
 function evaluate(value,enabled=true){
  const sample=enabled&&value?.usable===true?value.sample:null;
  const overview=sample?.country_overview;
  const matching=!!overview&&(!overview.country||overview.country===sample.country);
  const result={current:!!sample,country:sample?.country||null,observed_at:matching?overview.observed_at:null,
   mode:!enabled?'disabled':!sample?(value?.sample?'stale':'disconnected'):!matching?'unavailable':'current',sections:{}};
  for(const key of sections){
   const strict=['experience','diplomacy'].includes(key);
   const section=matching&&(!strict||typeof sample.country==='string'&&/^[A-Z0-9]{3}$/.test(sample.country)&&overview.country===sample.country)&&overview[key];
   const qualified=!strict||section?.status==='observed'&&(key==='experience'?['army','navy','air'].every(k=>number(section[k])!==null):typeof section.at_war==='boolean');
   const status=qualified&&section&&['observed','partial'].includes(section.status)?section.status:'unavailable';
   result.sections[key]=status==='unavailable'?{status,reason:sample?(section?.reason||uiText("This data is not available from the reader yet.")):null}:{...section,status};
  }
  const equipment=result.sections.equipment;
  const logisticsCountry=typeof sample?.country==='string'&&/^[A-Z0-9]{3}$/.test(sample.country)&&overview?.country===sample.country;
  if(overview?.equipment?.logistics&&(!logisticsCountry||!contracts.validLogistics(equipment.logistics)))equipment.logistics={status:'unavailable',items:[],reason:uiText('National logistics formula not qualified')};
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
