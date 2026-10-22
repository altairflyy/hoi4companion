'use strict';
((root)=>{
 const VERSION=1;
 const unknown=(reason='Campo non osservato')=>({status:'unknown',value:null,source:'',reason});
 const known=(value,source='process_memory',calculated=false)=>value===null||value===undefined?unknown():({status:calculated?'calculated':'observed',value,source});
 function validType(key,value){
  if(key==='production.lines')return validProductionLines(value);
  if(['country.intelligence.is_created','country.intelligence.in_creation'].includes(key))return typeof value==='boolean';
  if(['country','focus.tree'].includes(key))return typeof value==='string'&&value.length>0;
  if(['focus.completed','focus.active','focus.continuous','focus.ids','research.completed','research.active','research.ids','construction.queued'].includes(key))return Array.isArray(value)&&value.every(x=>typeof x==='string'&&x.length>0);
  if(key==='research.slots')return Array.isArray(value)&&value.every(x=>x&&typeof x==='object'&&Object.hasOwn(x,'technology_id')&&(x.technology_id===null||typeof x.technology_id==='string'&&x.technology_id.length>0));
  return true;
 }
 function validProductionLines(lines){
  if(!Array.isArray(lines)||lines.length>4096)return false;
  const ids=new Set();
  for(const row of lines){
   if(!row||typeof row!=='object'||Array.isArray(row)||['id','equipment','name'].some(key=>typeof row[key]!=='string'||!row[key])||ids.has(row.id)||!['military','naval'].includes(row.category)||['requested_factories','assigned_factories','active_factories','damaged_factories'].some(key=>!Number.isSafeInteger(row[key])||row[key]<0)||row.assigned_factories!==row.active_factories+row.damaged_factories||row.assigned_factories>row.requested_factories)return false;
   ids.add(row.id);
  }
  return true;
 }
 function validateFacts(value,invalidReason='Dato contratto non valido'){return {...value,facts:Object.fromEntries(Object.entries(value.facts).map(([k,f])=>{
  if(f?.status==='unknown'&&f.value===null&&f.source===''&&(f.reason===undefined||typeof f.reason==='string'))return [k,{status:'unknown',value:null,source:'',...(f.reason?{reason:f.reason}:{})}];
  const item=fact(value,k);return [k,item.status==='unknown'&&value.usable?unknown(invalidReason):item];
 }))};}
 function queuedConstruction(section){
  if(!section||typeof section!=='object'||Array.isArray(section)||section.status!=='observed'||!Array.isArray(section.lines))return null;
  const keys=new Set(),identities=new Set(),integer=(value,min=0)=>Number.isSafeInteger(value)&&value>=min;
  for(const row of section.lines){
   if(!row||typeof row!=='object'||Array.isArray(row)||['id','building_id'].some(key=>typeof row[key]!=='string'||!row[key])||identities.has(row.id)||row.location_status!=='observed'||!integer(row.state_id,1)||!Object.hasOwn(row,'province_id')||row.province_id!==null&&!integer(row.province_id,1)||['amount','active_factories','built_level','repair_levels'].some(key=>!integer(row[key]))||typeof row.conversion!=='boolean')return null;
   identities.add(row.id);
   if(row.amount>0&&row.repair_levels===0&&!row.conversion){keys.add('building:'+row.building_id);keys.add('state:'+row.state_id+':'+row.building_id);}
  }
  return [...keys].sort((a,b)=>{const left=Array.from(a,c=>c.codePointAt(0)),right=Array.from(b,c=>c.codePointAt(0));for(let i=0;i<Math.min(left.length,right.length);i++)if(left[i]!==right[i])return left[i]-right[i];return left.length-right.length;});
 }
 function state(value){
  if(value?.schema_version!==undefined){
   if(value.schema_version!==VERSION||value.usable!==true||!value.facts||typeof value.facts!=='object'||Array.isArray(value.facts))return {schema_version:VERSION,usable:false,session_id:null,facts:{}};
   return validateFacts({schema_version:VERSION,usable:true,session_id:value.session_id??null,facts:value.facts});
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
  const queued=typeof s.country==='string'&&s.country&&overview.country===s.country?queuedConstruction(overview.construction):null;
  f['construction.queued']=queued===null?unknown('Coda costruzioni non qualificata'):known(queued,'construction_adapter',true);
  const production=overview.production,productionKnown=typeof s.country==='string'&&s.country&&overview.country===s.country&&production?.status==='observed'&&validProductionLines(production.lines);
  f['production.lines']=productionKnown?known(production.lines.map(row=>({...row})),'production_adapter',true):unknown('Linee di produzione non qualificate');
  function put(path,data,present,calculated){f[path]=present?known(data,calculated?'legacy_adapter':source,calculated):unknown();}
  function flatten(path,data,present,calculated=false){
   if(Array.isArray(data)){put(path+'.count',data.length,present,true);data.forEach((child,i)=>flatten(path+'.'+(child&&typeof child==='object'?child.id??String(i):String(i)),child,present,calculated));}
   else if(data&&typeof data==='object'){for(const [key,child]of Object.entries(data))if(!['status','reason'].includes(key))flatten(path+'.'+key,child,present,calculated||derived.has(key));}
   else put(path,data,present,calculated||(path.startsWith('country.equipment.items.')&&path.endsWith('.stockpile'))||(path.startsWith('country.production.lines.')&&path.endsWith('.id'))||(path.startsWith('country.construction.lines.')&&['position','location_status','state_name'].includes(path.split('.').at(-1))));
  }
  if(overview.country===s.country)for(const [section,data]of Object.entries(overview))if(data&&typeof data==='object'&&!Array.isArray(data)){
   const present=['observed','partial'].includes(data.status);
   if(section==='manpower'){for(const [key,child]of Object.entries(data))if(!['status','reason'].includes(key))put('country.manpower.'+key,child,present,key==='available');}
   else flatten('country.'+section,data,present);
  }
  return validateFacts({schema_version:VERSION,usable:Object.keys(s).length>0,session_id:s.session_id||null,facts:f},'Campo non osservato');
 }
 function fact(state,key){const f=state?.schema_version===VERSION&&state?.usable===true?state.facts?.[key]:null;return f&&['observed','calculated'].includes(f.status)&&typeof f.source==='string'&&f.source&&f.value!==null&&f.value!==undefined&&validType(key,f.value)&&!(typeof f.value==='number'&&!Number.isFinite(f.value))?f:unknown(f?.status==='unknown'&&f.value===null&&f.source===''&&typeof f.reason==='string'?f.reason:undefined);}
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
 const record=x=>x!==null&&typeof x==='object'&&!Array.isArray(x);
 const textLength=x=>[...x].length;
 const jsonNumber=x=>typeof x==='number'&&Number.isFinite(x)&&(!Number.isInteger(x)||Number.isSafeInteger(x));
 function validateCondition(node,depth=0){
  if(!record(node)||depth>16)throw Error('Condizione non valida o troppo profonda');
  if(['all','any'].includes(node.op)){
   if(!Array.isArray(node.args)||node.args.length<1||node.args.length>64)throw Error('Operandi condizione non validi');
   node.args.forEach(child=>validateCondition(child,depth+1));
  }else if(node.op==='not')validateCondition(node.arg,depth+1);
  else if(['fact','check'].includes(node.op)){
   if(typeof node.key!=='string'||textLength(node.key)<1||textLength(node.key)>512)throw Error('Chiave condizione non valida');
   if(node.op==='fact'&&(!['eq','contains','gte'].includes(node.test)||!['string','boolean','number'].includes(typeof node.value)||typeof node.value==='number'&&!jsonNumber(node.value)))throw Error('Confronto condizione non valido');
  }else throw Error('Operatore condizione non supportato');
 }
 function validateAction(action){
  if(!record(action)||!['select_focus','research','special_project','manual','free'].includes(action.kind))throw Error('Azione non supportata');
  const expected={select_focus:'focus',research:'technology',special_project:'project'}[action.kind],ref=action.entity;
  if(expected){
   if(!record(ref)||ref.kind!==expected||typeof ref.id!=='string'||!ref.id.length||textLength(ref.id)>512||Object.hasOwn(ref,'scope')&&(typeof ref.scope!=='string'||!ref.scope))throw Error('Entità incompatibile con azione');
  }else if(Object.hasOwn(action,'entity'))throw Error('Azione descrittiva senza entità richiesta');
 }
 function identifiers(items,label,limit){
  if(!Array.isArray(items)||items.length>limit)throw Error(label+': elenco non valido');
  const ids=new Set();
  for(const item of items){if(!record(item)||typeof item.id!=='string'||!item.id.length||textLength(item.id)>160||ids.has(item.id))throw Error(label+': identificatore mancante o duplicato');ids.add(item.id);}
  return ids;
 }
 function jsonData(value,depth=0){
  if(depth>8)throw Error('Parametri azione troppo profondi');
  if(value===null||typeof value==='boolean')return;
  if(typeof value==='string'&&textLength(value)<=4096)return;
  if(jsonNumber(value))return;
  if(Array.isArray(value)&&value.length<=512){value.forEach(x=>jsonData(x,depth+1));return;}
  if(record(value)&&Object.prototype.toString.call(value)==='[object Object]'&&Object.keys(value).length<=128){
   for(const [key,child]of Object.entries(value)){if(!key.length||textLength(key)>128)throw Error('Nome parametro non valido');jsonData(child,depth+1);}return;
  }
  throw Error('Parametri azione non validi');
 }
 function validUrl(value){
  if(typeof value!=='string'||!/^https?:\/\/[^/\s?#]+/.test(value)||/[\s\u0000-\u001f]/u.test(value))return false;
  try{return !!new URL(value).hostname;}catch{return false;}
 }
 function validateExtensions(guide){
  const sourceRows=Object.hasOwn(guide,'sources')?guide.sources:[],phaseRows=Object.hasOwn(guide,'phases')?guide.phases:[];
  const sources=identifiers(sourceRows,'Fonti',32),phases=identifiers(phaseRows,'Fasi',128);
  for(const source of sourceRows){if(!['youtube','web','file','manual'].includes(source.type)||Object.hasOwn(source,'url')&&!validUrl(source.url))throw Error('Fonte non valida');}
  for(const phase of phaseRows){if(typeof phase.title!=='string'||!phase.title)throw Error('Titolo fase mancante');if(Object.hasOwn(phase,'when'))validateCondition(phase.when);}
  const rows=Object.values(guide.tracks).flat(),keys=new Set(rows.map(x=>x.key)),reminders=new Set(rows.filter(x=>x.tracking==='reminder').map(x=>x.key)),pending=new Map(),successors=new Map(rows.map(x=>[x.key,[]]));
  for(const step of rows){
   if(Object.hasOwn(step,'tracking')&&!['action','reminder'].includes(step.tracking))throw Error('Modalità monitoraggio non valida: '+step.key);
   if(step.tracking==='reminder'&&(step.action.kind!=='manual'||Object.hasOwn(step,'completion')||Object.hasOwn(step,'started_when')))throw Error('Promemoria incompatibile con azione o completamento: '+step.key);
   const deps=Object.hasOwn(step,'depends_on')?step.depends_on:[];
   if(!Array.isArray(deps)||deps.length>64||deps.some(dep=>typeof dep!=='string'||!keys.has(dep)||dep===step.key)||new Set(deps).size!==deps.length)throw Error('Dipendenze non valide: '+step.key);
   if(deps.some(dep=>reminders.has(dep)))throw Error('Dipendenza da promemoria non completabile: '+step.key);
   pending.set(step.key,deps.length);for(const dep of deps)successors.get(dep).push(step.key);
   if(Object.hasOwn(step,'phase')&&(typeof step.phase!=='string'||!phases.has(step.phase)))throw Error('Fase sconosciuta: '+step.key);
   if(Object.hasOwn(step,'completion'))validateCondition(step.completion);
   if(Object.hasOwn(step,'started_when')){
    if(!['manual','special_project'].includes(step.action.kind))throw Error('Condizione di avvio incompatibile con azione: '+step.key);
    validateCondition(step.started_when);
   }
   if(Object.hasOwn(step.action,'params')){if(!record(step.action.params))throw Error('Parametri azione non validi');jsonData(step.action.params);}
   const evidence=Object.hasOwn(step,'evidence')?step.evidence:[];
   if(!Array.isArray(evidence)||evidence.length>32)throw Error('Evidenze non valide');
   for(const item of evidence){
    if(!record(item)||typeof item.source_id!=='string'||!sources.has(item.source_id)||!['explicit','visual','inferred','game_derived','unverified'].includes(item.basis))throw Error('Provenienza non valida');
    for(const field of ['timestamp_seconds','end_seconds'])if(Object.hasOwn(item,field)&&(!jsonNumber(item[field])||item[field]<0))throw Error('Timestamp non valido');
    if(Object.hasOwn(item,'timestamp_seconds')&&Object.hasOwn(item,'end_seconds')&&item.end_seconds<item.timestamp_seconds)throw Error('Intervallo fonte non valido');
   }
  }
  const queue=rows.filter(x=>pending.get(x.key)===0).map(x=>x.key);
  for(let cursor=0;cursor<queue.length;cursor++)for(const key of successors.get(queue[cursor])){pending.set(key,pending.get(key)-1);if(pending.get(key)===0)queue.push(key);}
  if(queue.length!==keys.size)throw Error('Dipendenze della guida cicliche');
 }
 function guide(input){
  if(!record(input))throw Error('Guida non valida');
  if(input.schema_version!==undefined&&input.schema_version!==VERSION)throw Error('Versione contratto guida non supportata');
  if(input.schema!==undefined&&input.schema!==1)throw Error('Versione guida legacy non supportata');
  const scope=Object.hasOwn(input,'applies_to')?input.applies_to:Object.fromEntries(['country','tree_id'].filter(k=>input[k]).map(k=>[k,input[k]]));
  if(!record(scope)||Object.entries(scope).some(([k,v])=>!['country','tree_id'].includes(k)||typeof v!=='string'||!v))throw Error('Ambito guida non valido');
  if(!record(input.tracks)||Object.keys(input.tracks).length>32)throw Error('Tracce guida non valide');
  const result={...input,schema_version:VERSION,applies_to:{...scope},tracks:{}},keys=new Set();
  for(const field of ['country','tree_id'])if(scope[field]){if(input[field]&&input[field]!==scope[field])throw Error('Ambito guida in conflitto');if(!Object.hasOwn(input,field))result[field]=scope[field];}
  const actions={focus:'select_focus',technology:'research',project:'special_project',gate:'manual',free:'free'},kinds=Object.fromEntries(Object.entries(actions).map(([k,v])=>[v,k]));
  for(const [track,steps] of Object.entries(input.tracks)){
   if(!Array.isArray(steps)||steps.length>4096)throw Error('Traccia guida non valida');
   const rows=steps.map((s,i)=>{
   if(!record(s))throw Error('Passaggio guida non valido');
   const raw=Object.hasOwn(s,'action')?s.action:{kind:actions[s.kind],...(['focus','technology','project'].includes(s.kind)?{entity:{kind:s.kind,id:s.target}}:{})};
   validateAction(raw);const action={...raw,...(raw.entity?{entity:{...raw.entity}}:{})};
   const key=Object.hasOwn(s,'key')?s.key:`${track}-${i+1}`;
   if(typeof key!=='string'||!key||keys.has(key))throw Error('Chiave passaggio assente o duplicata');keys.add(key);
   if(action.entity?.kind==='focus'&&scope.tree_id){if(action.entity.scope!==undefined&&action.entity.scope!==scope.tree_id)throw Error('Ambito focus in conflitto');action.entity.scope=scope.tree_id;}
   const r={...s,key,action,kind:kinds[action.kind]};
   if(action.entity)r.target=action.entity.id;
   if(s.gate&&!Object.hasOwn(r,'when'))r.when={op:'check',key:s.gate};
   if(s.available_gate&&s.priority_after){
    if(!Object.hasOwn(r,'priority'))r.priority=100;
    if(!Object.hasOwn(r,'priority_when'))r.priority_when={op:'all',args:[{op:'fact',key:'focus.completed',test:'contains',value:s.priority_after},{op:'check',key:s.available_gate}]};
   }
   return r;
   });
   Object.defineProperty(result.tracks,track,{value:rows,enumerable:true,writable:true,configurable:true});
  }
  const steps=Object.values(result.tracks).flat();
  for(const s of steps)if(s.optional_before&&!Object.hasOwn(s,'skip_when'))s.skip_when={op:'any',args:[{op:'fact',key:'focus.completed',test:'contains',value:s.optional_before},...steps.filter(p=>p.target===s.optional_before&&p.priority_when).map(p=>p.priority_when)]};
  for(const step of steps){for(const field of ['when','priority_when','skip_when'])if(Object.hasOwn(step,field))validateCondition(step[field]);if(Object.hasOwn(step,'priority')&&(!Number.isInteger(step.priority)||Math.abs(step.priority)>100000))throw Error('Priorità guida non valida');}
  for(const [key,value]of Object.entries({milestones:[],assumptions:[],title:input.id??'Guida',subtitle:''}))if(!Object.hasOwn(result,key))result[key]=value;
  validateExtensions(result);
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
