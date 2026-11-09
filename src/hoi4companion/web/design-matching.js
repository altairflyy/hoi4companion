'use strict';
((root)=>{
 const record=v=>v!==null&&typeof v==='object'&&!Array.isArray(v);
 const identifier=v=>typeof v==='string'&&v.length>=1&&v.length<=160&&!/[^A-Za-z0-9_.-]/.test(v);
 const order=(a,b)=>{const x=Array.from(a,c=>c.codePointAt(0)),y=Array.from(b,c=>c.codePointAt(0));for(let i=0;i<Math.min(x.length,y.length);i++)if(x[i]!==y[i])return x[i]-y[i];return x.length-y.length;};
 function configuration(value){
  if(!record(value)||!record(value.modules)||!record(value.upgrades)||Object.keys(value.modules).length>256||Object.keys(value.upgrades).length>256)return null;
  const modules=[],upgrades=[];
  for(const [key,module]of Object.entries(value.modules)){
   if(!identifier(key)||!identifier(module))return null;
   if(module!=='empty')modules.push([key,module]);
  }
  for(const [key,raw]of Object.entries(value.upgrades)){
   if(!identifier(key))return null;
   const level=typeof raw==='string'&&/^(?:0|[1-9][0-9]{0,4})$(?![\s\S])/.test(raw)?Number(raw):raw;
   if(!Number.isInteger(level)||level<0||level>10000)return null;
   if(level)upgrades.push([key,level]);
  }
  return {modules:Object.fromEntries(modules),upgrades:Object.fromEntries(upgrades)};
 }
 function validLiveDesign(value){
  if(!record(value))return false;
  if(value.status==='unavailable')return !Object.hasOwn(value,'modules')&&!Object.hasOwn(value,'upgrades');
  return value.status==='observed'&&record(value.upgrades)&&Object.values(value.upgrades).every(Number.isInteger)&&configuration(value)!==null;
 }
 function compare(expected,observed){
  const wanted=configuration(expected);
  if(!wanted||!validLiveDesign(observed)||observed.status!=='observed')return {status:'unknown',differences:[]};
  const actual=configuration(observed),differences=[];
  for(const kind of ['modules','upgrades'])for(const key of [...new Set([...Object.keys(wanted[kind]),...Object.keys(actual[kind])])].sort(order)){
   const fallback=kind==='modules'?null:0,before=Object.hasOwn(wanted[kind],key)?wanted[kind][key]:fallback,after=Object.hasOwn(actual[kind],key)?actual[kind][key]:fallback;
   if(before!==after)differences.push({kind,key,expected:before,actual:after});
  }
  return {status:differences.length?'different':'match',differences};
 }
 function matching(selector,lines){
  if(!selector)return {rows:[],complete:false,design_check:null};
  const candidates=lines.filter(row=>selector.equipment_ids.includes(row.equipment)&&(selector.category===null||selector.category===row.category));
  if(!Object.hasOwn(selector,'design')){
   const name=s=>s.normalize('NFKD').toLowerCase().replace(/[^\p{L}\p{N}+]/gu,''),names=new Set(selector.variant_names.map(name));
   return {rows:candidates.filter(row=>!names.size||names.has(name(row.name))),complete:true,design_check:null};
  }
  const comparisons=candidates.map(row=>({line_id:row.id,name:row.name,...compare(selector.design,row.design)})),complete=comparisons.every(row=>row.status!=='unknown'),identities=new Set(comparisons.filter(row=>row.status==='match').map(row=>row.line_id));
  return {rows:candidates.filter(row=>identities.has(row.id)),complete,design_check:{status:!complete?'unknown':identities.size?'match':comparisons.length?'different':'missing',candidates:comparisons}};
 }
 // Serialize nested map keys explicitly: JSON.stringify reorders numeric-looking
 // object keys, while Python's baseline identity sorts every key lexically.
 function stableStringify(value){
  if(Array.isArray(value))return '['+value.map(stableStringify).join(',')+']';
  if(record(value))return '{'+Object.keys(value).sort(order).map(key=>JSON.stringify(key)+':'+stableStringify(value[key])).join(',')+'}';
  return JSON.stringify(value);
 }
 const api={configuration,validLiveDesign,compare,matching,stableStringify};
 if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiDesignMatching=api;
})(typeof window!=='undefined'?window:globalThis);
