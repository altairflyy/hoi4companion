'use strict';
((root)=>{
 const uiText=typeof module!=='undefined'&&module.exports?require('./i18n.js').t:root.HoiI18n.t;
 const designMatching=typeof module!=='undefined'&&module.exports?require('./design-matching.js'):root.HoiDesignMatching;
 const VERSION=1;
 const experienceCurrencies=Object.freeze({army_experience:'country.experience.army',navy_experience:'country.experience.navy',air_experience:'country.experience.air'});
 const spiritCurrencies=Object.freeze(Object.fromEntries([['army_experience',['academy_spirit','army_spirit','division_command_spirit']],['navy_experience',['naval_academy_spirit','navy_spirit','naval_command_spirit']],['air_experience',['air_force_spirit','air_force_command_spirit','air_force_academy_spirit']]].flatMap(([currency,groups])=>groups.map(group=>[group,currency]))));
 const unknown=(reason="Campo non osservato")=>({status:'unknown',value:null,source:'',reason});
 const known=(value,source='process_memory',calculated=false)=>value===null||value===undefined?unknown():({status:calculated?'calculated':'observed',value,source});
 function validType(key,value){
  if(['diplomacy.countries','diplomacy.improving_relations','diplomacy.non_aggression_pacts','intelligence.countries'].includes(key))return validForeignTags(value);
  if(key==='intelligence.active_missions')return Array.isArray(value)&&value.length<=4096&&value.every(x=>typeof x==='string'&&foreignMissions.slice(1).includes(x.split(':')[0])&&/^[A-Z0-9]{3}$/.test(x.split(':').slice(1).join(':')))&&new Set(value).size===value.length;
  if(['country.metrics.war_support','country.metrics.stability'].includes(key))return typeof value==='number'&&Number.isFinite(value)&&value>=0&&value<=1;
  if(key==='country.diplomacy.at_war')return typeof value==='boolean';
  if(Object.values(experienceCurrencies).includes(key))return typeof value==='number'&&Number.isFinite(value)&&value>=0&&value<=10000000;
  if(['country.production.effective_factories.military','country.production.effective_factories.dockyards'].includes(key))return Number.isSafeInteger(value)&&value>=0&&value<=10000000;
  if(['doctrines.selected','politics.ideas','politics.advisors','events.visible','events.visible_options','decisions.listed','decisions.executed'].includes(key))return Array.isArray(value)&&value.length<=4096&&value.every(x=>typeof x==='string'&&x.length>0&&x.length<=512)&&new Set(value).size===value.length;
  if(key==='production.lines')return validProductionLines(value);
  if(['country.intelligence.is_created','country.intelligence.in_creation'].includes(key))return typeof value==='boolean';
  if(['country','focus.tree'].includes(key))return typeof value==='string'&&value.length>0;
  if(['focus.completed','focus.active','focus.continuous','focus.ids','research.completed','research.active','research.ids','construction.queued','projects.active','projects.completed'].includes(key))return Array.isArray(value)&&value.every(x=>typeof x==='string'&&x.length>0);
  if(key==='research.slots'){
   if(!Array.isArray(value)||value.length>64||!value.every(x=>x&&typeof x==='object'&&!Array.isArray(x)&&Object.hasOwn(x,'technology_id')&&(x.technology_id===null||typeof x.technology_id==='string'&&x.technology_id.length>0)))return false;
   const indices=value.filter(x=>Object.hasOwn(x,'slot')).map(x=>x.slot);
   return indices.every(x=>Number.isSafeInteger(x)&&x>=0&&x<=64)&&new Set(indices).size===indices.length;
  }
  return true;
 }
 function validProductionLines(lines){
  if(!Array.isArray(lines)||lines.length>4096)return false;
  const ids=new Set();
  for(const row of lines){
   if(!row||typeof row!=='object'||Array.isArray(row)||['id','equipment','name'].some(key=>typeof row[key]!=='string'||!row[key])||ids.has(row.id)||!['military','naval'].includes(row.category)||['requested_factories','assigned_factories','active_factories','damaged_factories'].some(key=>!Number.isSafeInteger(row[key])||row[key]<0)||row.assigned_factories!==row.active_factories+row.damaged_factories||row.assigned_factories>row.requested_factories)return false;
   if(Object.hasOwn(row,'design')&&!designMatching.validLiveDesign(row.design))return false;
   ids.add(row.id);
  }
  return true;
 }
 function validateFacts(value,invalidReason="Dato contratto non valido"){return {...value,facts:Object.fromEntries(Object.entries(value.facts).map(([k,f])=>{
  if(f?.status==='unknown'&&f.value===null&&f.source===''&&(f.reason===undefined||typeof f.reason==='string'))return [k,{status:'unknown',value:null,source:'',...(f.reason?{reason:f.reason}:{})}];
  const item=fact(value,k);return [k,item.status==='unknown'&&value.usable?unknown(invalidReason):item];
 }))};}
 function queuedConstruction(section){
  if(!section||typeof section!=='object'||Array.isArray(section)||section.status!=='observed'||!Array.isArray(section.lines))return null;
  const keys=new Set(),identities=new Set(),integer=(value,min=0)=>Number.isSafeInteger(value)&&value>=min;
  for(const row of section.lines){
   if(!row||typeof row!=='object'||Array.isArray(row)||['id','building_id'].some(key=>typeof row[key]!=='string'||!row[key])||identities.has(row.id)||row.location_status!=='observed'||!integer(row.state_id,1)||!Object.hasOwn(row,'province_id')||row.province_id!==null&&!integer(row.province_id,1)||['amount','active_factories','built_level','repair_levels'].some(key=>!integer(row[key]))||typeof row.conversion!=='boolean')return null;
   identities.add(row.id);
    if(row.amount>0&&row.repair_levels===0&&!row.conversion){keys.add('building:'+row.building_id);keys.add('state:'+row.state_id+':'+row.building_id);if(row.province_id!==null)keys.add('province:'+row.province_id+':'+row.building_id);}
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
    if(s.country_identity_validation==='primary_selector_and_country_map'&&typeof s.session_id==='string'&&s.session_id&&typeof s.executable_sha256==='string'&&/^[0-9a-f]{64}$/.test(s.executable_sha256)&&Number.isInteger(s.native_tick)&&s.native_tick>=0&&s.native_tick<=2147483647){
     const days=Math.floor(s.native_tick/24),year=Math.floor(days/365)-5000;let day=days%365;
     if(year>=1&&year<=9999)for(const [i,length]of [31,28,31,30,31,30,31,31,30,31,30,31].entries()){
      if(day<length){f['campaign.date']=known(year*10000+(i+1)*100+day+1,'paradox_calendar_adapter',true);break;}day-=length;
     }
    }
  const campaign=s.campaign_identity;
  if(campaign?.status==='observed'&&campaign.source==='native_game_unique_id_and_seed'&&typeof campaign.id==='string'&&/^hoi4:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}:[0-9a-f]{8}$/.test(campaign.id)&&campaign.id.split(':')[1]!=='00000000-0000-0000-0000-000000000000')f['campaign.id']=known(campaign.id,campaign.source);
  f['focus.completed']=s.focus_complete===false?unknown():known(s.completed_focuses,source);
  f['research.completed']=s.research_complete===false?unknown():known(s.researched_technologies,source);
  f['focus.active']=Object.hasOwn(s,'current_focus')?known(s.current_focus?[s.current_focus]:[],source):unknown();
  f['focus.continuous']=Object.hasOwn(s,'continuous_focus')?known(s.continuous_focus?[s.continuous_focus]:[],source):unknown();
  f['research.active']=validType('research.slots',s.research_slots)?known(s.research_slots.map(x=>x.technology_id).filter(Boolean),source):unknown();
  f['research.slots']=known(s.research_slots,source);f['focus.ids']=known(s.focus_ids,source);f['research.ids']=known(s.technology_ids,source);
  const overview=s.country_overview||{},derived=new Set(['by_type','by_template','total','effective_factories','output_per_day','assigned_factories','category','balance','used','owned','controlled','name']);
  const queued=typeof s.country==='string'&&s.country&&overview.country===s.country?queuedConstruction(overview.construction):null;
  f['construction.queued']=queued===null?unknown("Coda costruzioni non qualificata"):known(queued,'construction_adapter',true);
  const projects=overview.projects,items=projects?.items,projectsKnown=Boolean(s.country)&&overview.country===s.country&&projects?.status==='observed'&&Array.isArray(items)&&items.length<=1024&&items.every(row=>row&&typeof row.id==='string'&&row.id&&typeof row.active==='boolean'&&typeof row.completed==='boolean'&&!(row.active&&row.completed))&&new Set(items.map(row=>row.id)).size===items.length;
  for(const field of ['active','completed'])f['projects.'+field]=projectsKnown?known(items.filter(row=>row[field]).map(row=>row.id),'projects_adapter',true):unknown("Progetti speciali non qualificati");
  const points=projects?.breakthroughs?.items;
  if(Boolean(s.country)&&overview.country===s.country&&projects?.breakthroughs?.status==='observed'&&Array.isArray(points)&&points.length<=64&&points.every(row=>row&&typeof row.id==='string'&&row.id&&Number.isInteger(row.available)&&row.available>=0&&row.available<=100000)&&new Set(points.map(row=>row.id)).size===points.length)for(const row of points)f['projects.points.'+row.id]=known(row.available,'breakthrough_points_adapter',true);
  const power=overview.politics?.power;
  f['country.politics.political_power']=Boolean(s.country)&&overview.country===s.country&&power?.status==='observed'&&typeof power.available==='number'&&Number.isFinite(power.available)?known(power.available,source):unknown("Potere politico non qualificato");
  const production=overview.production,productionKnown=typeof s.country==='string'&&s.country&&overview.country===s.country&&production?.status==='observed'&&validProductionLines(production.lines);
  f['production.lines']=productionKnown?known(production.lines.map(row=>({...row})),'production_adapter',true):unknown("Linee di produzione non qualificate");
  Object.assign(f,politicalFacts(typeof s.country==='string'&&s.country&&overview.country===s.country?overview:{}));
  function put(path,data,present,calculated){f[path]=present?known(data,calculated?'legacy_adapter':source,calculated):unknown();}
  function flatten(path,data,present,calculated=false){
   if(['country.diplomacy.interactions','country.intelligence.missions'].includes(path))return;
   if(Array.isArray(data)){put(path+'.count',data.length,present,true);data.forEach((child,i)=>flatten(path+'.'+(child&&typeof child==='object'?child.id??String(i):String(i)),child,present,calculated));}
   else if(data&&typeof data==='object'){present=present&&(!Object.hasOwn(data,'status')||['observed','partial'].includes(data.status));for(const [key,child]of Object.entries(data))if(!['status','reason'].includes(key))flatten(path+'.'+key,child,present,calculated||derived.has(key));}
   else put(path,data,present,calculated||(path.startsWith('country.equipment.items.')&&path.endsWith('.stockpile'))||(path.startsWith('country.production.lines.')&&path.endsWith('.id'))||(path.startsWith('country.construction.lines.')&&['position','location_status','state_name'].includes(path.split('.').at(-1))));
  }
  if(overview.country===s.country)for(const [section,data]of Object.entries(overview))if(data&&typeof data==='object'&&!Array.isArray(data)){
   const present=['experience','diplomacy','production'].includes(section)?data.status==='observed':['observed','partial'].includes(data.status);
   if(section==='manpower'){for(const [key,child]of Object.entries(data))if(!['status','reason'].includes(key))put('country.manpower.'+key,child,present,key==='available');}
     else if(section==='fortifications'){
      const rows=data.items,knownSection=data.status==='observed'&&Array.isArray(rows)&&rows.length<=20000&&rows.every(row=>row&&typeof row==='object'&&!Array.isArray(row)&&Number.isSafeInteger(row.id)&&row.id>0&&row.id<=100000&&['bunker','coastal_bunker'].every(k=>!Object.hasOwn(row,k)||Number.isSafeInteger(row[k])&&row[k]>=0&&row[k]<=1000))&&new Set(rows.map(row=>row.id)).size===rows.length;flatten('country.fortifications.items',rows||[],knownSection);
      if(knownSection&&data.borders_status==='observed'&&data.borders&&typeof data.borders==='object'&&!Array.isArray(data.borders))for(const [tag,ids]of Object.entries(data.borders)){
       if(/^[A-Z0-9]{3}$/.test(tag)&&Array.isArray(ids)&&ids.length<=512&&ids.every(n=>Number.isSafeInteger(n)&&n>0&&n<=100000)&&new Set(ids).size===ids.length)f['fortifications.borders.'+tag]=known(ids,'native_controllers_and_map_topology',true);
      }
     }
     else flatten('country.'+section,data,present);
  }
  Object.assign(f,foreignFacts(s.country&&overview.country===s.country?overview:{}));
  const metrics=overview.metrics;
  for(const metric of ['war_support','stability']){const key='country.metrics.'+metric,value=metrics?.[metric];f[key]=Boolean(s.country)&&overview.country===s.country&&metrics?.status==='observed'&&validType(key,value)?known(value,'native_effective_country_metric',true):unknown('Country metric not qualified');}
  return validateFacts({schema_version:VERSION,usable:Object.keys(s).length>0,session_id:s.session_id||null,facts:f},"Campo non osservato");
 }
 const foreignMissions=['no_mission','build_intel_network','quiet_network','counter_intelligence','root_out_resistance','boost_ideology','control_trade','diplomatic_pressure','propaganda'];
 const foreignStates=['captured','cooldown','disbanding','active','operation','killed'];
 function validForeignTags(value){return Array.isArray(value)&&value.length<=8192&&value.every(x=>typeof x==='string'&&/^[A-Z0-9]{3}$/.test(x))&&new Set(value).size===value.length;}
 function foreignFacts(overview){
  const keys=['diplomacy.countries','diplomacy.improving_relations','diplomacy.non_aggression_pacts','intelligence.countries','intelligence.active_missions'],out=Object.fromEntries(keys.map(k=>[k,unknown('Foreign interaction not qualified')]));
  const dip=overview.diplomacy,d=dip?.interactions,tags=d?.countries;
  if(dip?.status==='observed'&&d?.status==='observed'&&validForeignTags(tags)&&['improving_relations','non_aggression_pacts'].every(k=>validForeignTags(d[k])&&d[k].every(t=>tags.includes(t))))for(const k of ['countries','improving_relations','non_aggression_pacts'])out['diplomacy.'+k]=known([...d[k]].sort(),'foreign_interaction_adapter',true);
  const intel=overview.intelligence,m=intel?.missions,ids=m?.countries,rows=m?.items;
  if(intel?.status==='observed'&&m?.status==='observed'&&validForeignTags(ids)&&Array.isArray(rows)&&rows.length<=4096&&rows.every(r=>r&&typeof r==='object'&&!Array.isArray(r)&&typeof r.id==='string'&&r.id.length>0&&r.id.length<=160&&foreignMissions.includes(r.mission_id)&&foreignStates.includes(r.state)&&(r.mission_id==='no_mission'?r.target_country===null:ids.includes(r.target_country)))&&new Set(rows.map(r=>r.id)).size===rows.length){
   out['intelligence.countries']=known([...ids].sort(),'foreign_interaction_adapter',true);
   out['intelligence.active_missions']=known([...new Set(rows.filter(r=>r.state==='active'&&r.mission_id!=='no_mission').map(r=>r.mission_id+':'+r.target_country))].sort(),'foreign_interaction_adapter',true);
  }
  return out;
 }
 function politicalFacts(overview){
  const result=Object.fromEntries(['doctrines.selected','politics.ideas','politics.advisors','events.visible','events.visible_options','decisions.listed','decisions.executed'].map(key=>[key,unknown("Sezione politica non qualificata")]));
  const id=value=>typeof value==='string'&&value.length>0&&value.length<=160&&/^[A-Za-z0-9_.-]+$/.test(value);
  const publish=(key,values)=>result[key]=known([...new Set(values)].sort(),'politics_adapter',true),p=overview.politics;
  const selected=overview.doctrines?.status==='observed'?overview.doctrines.items:null;
  if(Array.isArray(selected)&&selected.length<=4096&&selected.every(row=>row&&id(row.id))&&new Set(selected.map(row=>row.id)).size===selected.length)publish('doctrines.selected',selected.map(row=>row.id));
  if(p&&['observed','partial'].includes(p.status))for(const part of ['ideas','advisors']){
   const section=p[part],rows=section?.status==='observed'?section.items:null;
   if(!Array.isArray(rows)||rows.length>4096)continue;
   if(!rows.every(row=>row&&id(row.id)&&(part==='ideas'?id(row.group)&&id(row.category)&&typeof row.is_law==='boolean':id(row.character_id)&&Number.isSafeInteger(row.slot)&&row.slot>=0&&row.slot<=2000000000)))continue;
   const ids=rows.map(row=>row.id);if(new Set(ids).size===ids.length)publish('politics.'+part,ids);
  }
  const events=overview.events,rows=events?.status==='observed'?events.visible:null;
  if(Array.isArray(rows)&&rows.length<=256){
   const instances=rows.map(row=>row?.instance_id);
   if(rows.every(row=>row&&id(row.instance_id)&&id(row.event_id))&&new Set(instances).size===instances.length){
    publish('events.visible',rows.map(row=>row.event_id));
    if(rows.every(row=>Array.isArray(row.option_ids)&&row.option_ids.length<=256&&row.option_ids.every(id)&&new Set(row.option_ids).size===row.option_ids.length))publish('events.visible_options',rows.flatMap(row=>row.option_ids.map(choice=>row.event_id+':'+choice)));
   }
  }
  const listed=overview.decisions?.status==='observed'?overview.decisions.listed_ids:null;
  if(Array.isArray(listed)&&listed.length<=4096&&listed.every(id)&&new Set(listed).size===listed.length)publish('decisions.listed',listed);
  return result;
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
  if(!record(node)||depth>16)throw Error(uiText("Invalid or excessively nested condition"));
  if(['all','any'].includes(node.op)){
   if(!Array.isArray(node.args)||node.args.length<1||node.args.length>64)throw Error(uiText("Invalid condition operands"));
   node.args.forEach(child=>validateCondition(child,depth+1));
  }else if(node.op==='not')validateCondition(node.arg,depth+1);
  else if(['fact','check'].includes(node.op)){
   if(typeof node.key!=='string'||textLength(node.key)<1||textLength(node.key)>512)throw Error(uiText("Invalid condition key"));
   if(node.op==='fact'&&(!['eq','contains','gte'].includes(node.test)||!['string','boolean','number'].includes(typeof node.value)||typeof node.value==='number'&&!jsonNumber(node.value)))throw Error(uiText("Invalid condition comparison"));
  }else throw Error(uiText("Unsupported condition operator"));
 }
 function validateAction(action){
  if(!record(action)||!['select_focus','research','special_project','manual','free'].includes(action.kind))throw Error(uiText("Unsupported action"));
  const expected={select_focus:'focus',research:'technology',special_project:'project'}[action.kind],ref=action.entity;
  if(expected){
   if(!record(ref)||ref.kind!==expected||typeof ref.id!=='string'||!ref.id.length||textLength(ref.id)>512||Object.hasOwn(ref,'scope')&&(typeof ref.scope!=='string'||!ref.scope))throw Error(uiText("Entity is incompatible with the action"));
  }else if(Object.hasOwn(action,'entity'))throw Error(uiText("Descriptive action must not have an entity"));
 }
 function identifiers(items,label,limit){
  if(!Array.isArray(items)||items.length>limit)throw Error(label+uiText(": invalid list"));
  const ids=new Set();
  for(const item of items){if(!record(item)||typeof item.id!=='string'||!item.id.length||textLength(item.id)>160||ids.has(item.id))throw Error(label+uiText(": missing or duplicate identifier"));ids.add(item.id);}
  return ids;
 }
 function jsonData(value,depth=0){
  if(depth>8)throw Error(uiText("Action parameters are nested too deeply"));
  if(value===null||typeof value==='boolean')return;
  if(typeof value==='string'&&textLength(value)<=4096)return;
  if(jsonNumber(value))return;
  if(Array.isArray(value)&&value.length<=512){value.forEach(x=>jsonData(x,depth+1));return;}
  if(record(value)&&Object.prototype.toString.call(value)==='[object Object]'&&Object.keys(value).length<=128){
   for(const [key,child]of Object.entries(value)){if(!key.length||textLength(key)>128)throw Error(uiText("Invalid parameter name"));jsonData(child,depth+1);}return;
  }
  throw Error(uiText("Invalid action parameters"));
 }
 function validUrl(value){
  if(typeof value!=='string'||!/^https?:\/\/[^/\s?#]+/.test(value)||/[\s\u0000-\u001f]/u.test(value))return false;
  try{return !!new URL(value).hostname;}catch{return false;}
 }
 function validateExtensions(guide){
  for(const step of Object.values(guide.tracks).flat())if(Object.hasOwn(step,'focus_criteria')){
   if(step.action.kind!=='select_focus')throw Error('Focus criteria on a non-focus step');
   (typeof module!=='undefined'&&module.exports?require('./focus-criteria.js'):root.HoiFocusCriteria).validate(step.focus_criteria);
  }
  if(Object.hasOwn(guide,'focus_sequence')){
   const p=guide.focus_sequence,initial=p?.initial_focuses??[];
   if(!record(p)||Object.keys(p).some(k=>!['schema_version','coverage','initial_focuses'].includes(k))||p.schema_version!==1||!['partial','complete'].includes(p.coverage)||p.coverage==='complete'&&!Object.hasOwn(p,'initial_focuses')||!Array.isArray(initial)||initial.length>4096||initial.some(x=>typeof x!=='string'||!x||textLength(x)>512)||new Set(initial).size!==initial.length)throw Error('Invalid focus sequence policy or initial focus list');
  }
  const focusPositions=new Set();
  for(const lane of Object.values(guide.tracks)){
   let previous=0;
   for(const step of lane){
    if(!Object.hasOwn(step,'focus_resolution'))continue;
    const r=step.focus_resolution;
    if(step.action.kind!=='select_focus'||!record(r)||Object.keys(r).some(k=>!['schema_version','position','query','classification','entity','locked'].includes(k))||r.schema_version!==1||!Number.isInteger(r.position)||r.position<1||r.position>4096||focusPositions.has(r.position)||r.position<=previous||typeof r.query!=='string'||!r.query||textLength(r.query)>2048||!['direct','reconstructed','probable','unresolved'].includes(r.classification)||Object.hasOwn(r,'locked')&&typeof r.locked!=='boolean')throw Error('Invalid focus source position or resolution provenance');
    validateAction({kind:'select_focus',entity:r.entity});focusPositions.add(r.position);previous=r.position;
   }
  }
  const sourceRows=Object.hasOwn(guide,'sources')?guide.sources:[],phaseRows=Object.hasOwn(guide,'phases')?guide.phases:[];
  const sources=identifiers(sourceRows,'Fonti',32),phases=identifiers(phaseRows,'Fasi',128);
  for(const source of sourceRows){if(!['youtube','web','file','manual'].includes(source.type)||Object.hasOwn(source,'url')&&!validUrl(source.url))throw Error(uiText("Invalid source"));}
  for(const phase of phaseRows){if(typeof phase.title!=='string'||!phase.title)throw Error(uiText("Missing phase title"));if(Object.hasOwn(phase,'when'))validateCondition(phase.when);}
  const rows=Object.values(guide.tracks).flat(),keys=new Set(rows.map(x=>x.key)),reminders=new Set(rows.filter(x=>x.tracking==='reminder').map(x=>x.key)),pending=new Map(),successors=new Map(rows.map(x=>[x.key,[]]));
  for(const step of rows){
   if(Object.hasOwn(step,'research_choice')){
    const r=step.research_choice,allowed=new Set(['group','query','branches','level','mode']);
    if(!['persistent','once'].includes(r?.mode??'persistent'))throw Error('Invalid research choice mode');
    if(r?.mode==='once'&&Object.hasOwn(r,'level'))throw Error('A one-off choice has per-option targets');
    if(step.action.kind!=='research'||Object.hasOwn(step,'relative_research')||!record(r)||Object.keys(r).some(k=>!allowed.has(k))||['group','query'].some(k=>typeof r[k]!=='string'||!r[k]||r[k].length>512)||!record(r.branches)||Object.keys(r.branches).length<2||Object.keys(r.branches).length>8||Object.hasOwn(r,'level')&&(!Number.isInteger(r.level)||r.level<1||r.level>128))throw Error('Invalid research branch choice');
    const ids=[];for(const [key,chain]of Object.entries(r.branches)){if(!key||key.length>160||!Array.isArray(chain)||!chain.length||chain.length>128||chain.some(x=>typeof x!=='string'||!x||x.length>512))throw Error('Invalid alternative research branch');ids.push(...chain);}
    if(new Set(ids).size!==ids.length)throw Error('Overlapping research branches');
    if(r.level&&Object.values(r.branches).some(chain=>r.level>chain.length))throw Error('Missing alternative research level');
   }
   if(Object.hasOwn(step,'relative_research_unresolved')&&(step.action.kind!=='research'||step.relative_research_unresolved!==true))throw Error('Invalid relative research state');
   if(Object.hasOwn(step,'relative_research')){const r=step.relative_research;if(step.action.kind!=='research'||!record(r)||Object.keys(r).sort().join(',')!=='chain,query'||typeof r.query!=='string'||!r.query||r.query.length>512||!Array.isArray(r.chain)||!r.chain.length||r.chain.length>128||r.chain.some(x=>typeof x!=='string'||!x||x.length>512)||new Set(r.chain).size!==r.chain.length)throw Error('Invalid relative research branch');}
   if(Object.hasOwn(step,'tracking')&&!['action','reminder'].includes(step.tracking))throw Error(uiText("Invalid tracking mode: ")+step.key);
   if(step.tracking==='reminder'&&(step.action.kind!=='manual'||Object.hasOwn(step,'completion')||Object.hasOwn(step,'started_when')))throw Error('Promemoria incompatibile con azione o completamento: '+step.key);
   const completed=Object.hasOwn(step,'depends_on')?step.depends_on:[],started=Object.hasOwn(step,'depends_on_started')?step.depends_on_started:[];
   if(!Array.isArray(completed)||!Array.isArray(started))throw Error(uiText("Invalid dependencies: ")+step.key);
   const deps=[...completed,...started];
   if(!Array.isArray(deps)||deps.length>64||deps.some(dep=>typeof dep!=='string'||!keys.has(dep)||dep===step.key)||new Set(deps).size!==deps.length)throw Error(uiText("Invalid dependencies: ")+step.key);
   if(deps.some(dep=>reminders.has(dep)))throw Error(uiText("Dependency on a reminder that cannot complete: ")+step.key);
   pending.set(step.key,deps.length);for(const dep of deps)successors.get(dep).push(step.key);
   if((['political_power_cost','resource_costs','requirements'].some(field=>Object.hasOwn(step,field)))&&(!['manual','special_project'].includes(step.action.kind)||step.tracking==='reminder'))throw Error('Requisiti di azione incompatibili');
   if(Object.hasOwn(step,'political_power_cost')&&(!jsonNumber(step.political_power_cost)||step.political_power_cost<0))throw Error(uiText("Invalid political cost"));
   if(Object.hasOwn(step,'resource_costs')&&(!step.resource_costs||typeof step.resource_costs!=='object'||Array.isArray(step.resource_costs)||Object.keys(step.resource_costs).length>64||Object.entries(step.resource_costs).some(([key,cost])=>!key||key.length>256||!jsonNumber(cost)||cost<0)))throw Error(uiText("Invalid resource costs"));
   if(Object.hasOwn(step,'requirements'))validateCondition(step.requirements);
   if(Object.hasOwn(step,'phase')&&(typeof step.phase!=='string'||!phases.has(step.phase)))throw Error(uiText("Unknown phase: ")+step.key);
   if(Object.hasOwn(step,'completion'))validateCondition(step.completion);
   if(Object.hasOwn(step,'started_when')){
    if(!['manual','special_project'].includes(step.action.kind))throw Error('Condizione di avvio incompatibile con azione: '+step.key);
    validateCondition(step.started_when);
   }
   if(Object.hasOwn(step.action,'params')){if(!record(step.action.params))throw Error(uiText("Invalid action parameters"));jsonData(step.action.params);}
   if(Object.hasOwn(step.action.params||{},'production_revision')){const p=step.action.params,r=p.production_revision;if(p.category!=='production'||!record(r)||Object.keys(r).sort().join(',')!=='group,order'||typeof r.group!=='string'||!r.group||r.group.length>160||!Number.isInteger(r.order)||r.order<0||r.order>10000000)throw Error('Invalid production revision');}
   if(Object.hasOwn(step.action.params||{},'production_section_when')){const p=step.action.params;if(!p.production_revision)throw Error('Production phase without revision');validateCondition(p.production_section_when);}
   if(Object.hasOwn(step.action.params||{},'production_milestones')){
    const p=step.action.params,values=p.production_milestones;
    if(p.category!=='production'||!Array.isArray(values)||values.length<1||values.length>64||new Set(values.map(m=>m?.step)).size!==values.length)throw Error('Invalid production milestones');
    for(const m of values){const a=rows.find(r=>r.key===m?.step),n=a?.completion;
     if(!m||Object.keys(m).sort().join(',')!=='condition,step,token'||!a||a.key===step.key||a.tracking==='reminder'||n?.op!=='fact'||!['politics.ideas','politics.advisors','research.completed','focus.completed','projects.completed','doctrines.selected'].includes(n.key)||n.test!=='contains'||typeof n.value!=='string'||!n.value||a.when||a.skip_when||a.depends_on?.length||a.depends_on_started?.length||typeof m.token!=='string'||[...m.token].length>2048||m.token!==JSON.stringify([a.key,n.key,n.value])||!m.condition||Object.keys(m.condition).sort().join(',')!==Object.keys(n).sort().join(',')||Object.keys(n).some(k=>n[k]!==m.condition[k]))throw Error('Invalid production milestone reference');
    }
   }
   for(const p of [step.action.params||{},...(Array.isArray(step.action.params?.production_targets)?step.action.params.production_targets:[])])if(p&&Object.hasOwn(p,'production_priority')&&(!['first','last'].includes(p.production_priority)||p.production_category!=='military'||p.quantity_mode==='additional'))throw Error('Invalid production priority');
   if(Object.hasOwn(step.action.params||{},'cost_currency')){const p=step.action.params;if(typeof p.cost_currency!=='string'||!Object.hasOwn(experienceCurrencies,p.cost_currency)||step.action.kind!=='manual'||typeof p.idea_id!=='string'||!p.idea_id||typeof p.idea_group!=='string'||!Object.hasOwn(spiritCurrencies,p.idea_group)||spiritCurrencies[p.idea_group]!==p.cost_currency||!Object.hasOwn(step.resource_costs||{},experienceCurrencies[p.cost_currency]))throw Error('Invalid experience currency');}
   if(Object.hasOwn(step.action.params||{},'focus_mode')&&(step.action.kind!=='select_focus'||step.action.params.focus_mode!=='continuous'))throw Error('Invalid focus mode');
   if(Object.hasOwn(step.action.params||{},'focus_optional')&&(step.action.kind!=='select_focus'||step.action.params.focus_mode!=='continuous'||typeof step.action.params.focus_optional!=='boolean'))throw Error('Invalid optional focus');
   if(Object.hasOwn(step.action.params||{},'game_date')){const v=step.action.params.game_date,m=typeof v==='string'&&v.match(/^(\d{1,4})\.(\d{1,2})\.(\d{1,2})$/);if(!m)throw Error(uiText("Invalid game date"));const [y,month,d]=m.slice(1).map(Number),days=[31,y%4===0&&(y%100!==0||y%400===0)?29:28,31,30,31,30,31,31,30,31,30,31];if(y<1||month<1||month>12||d<1||d>days[month-1])throw Error(uiText("Invalid game date"));}
   const evidence=Object.hasOwn(step,'evidence')?step.evidence:[];
   if(!Array.isArray(evidence)||evidence.length>32)throw Error(uiText("Invalid evidence"));
   for(const item of evidence){
    if(!record(item)||typeof item.source_id!=='string'||!sources.has(item.source_id)||!['explicit','visual','inferred','game_derived','unverified'].includes(item.basis))throw Error(uiText("Invalid provenance"));
    for(const field of ['timestamp_seconds','end_seconds'])if(Object.hasOwn(item,field)&&(!jsonNumber(item[field])||item[field]<0))throw Error(uiText("Invalid timestamp"));
    if(Object.hasOwn(item,'timestamp_seconds')&&Object.hasOwn(item,'end_seconds')&&item.end_seconds<item.timestamp_seconds)throw Error(uiText("Invalid source interval"));
   }
  }
  const queue=rows.filter(x=>pending.get(x.key)===0).map(x=>x.key);
  for(let cursor=0;cursor<queue.length;cursor++)for(const key of successors.get(queue[cursor])){pending.set(key,pending.get(key)-1);if(pending.get(key)===0)queue.push(key);}
  if(queue.length!==keys.size)throw Error(uiText("Cyclic guide dependencies"));
 }
 function guide(input){
  if(!record(input))throw Error(uiText("Invalid guide"));
  if(input.schema_version!==undefined&&input.schema_version!==VERSION)throw Error(uiText("Unsupported guide contract version"));
  if(input.schema!==undefined&&input.schema!==1)throw Error(uiText("Unsupported legacy guide version"));
  if(Object.hasOwn(input,'dependency_policy')&&!['legacy','explicit'].includes(input.dependency_policy))throw Error(uiText("Invalid guide dependency policy"));
  const scope=Object.hasOwn(input,'applies_to')?input.applies_to:Object.fromEntries(['country','tree_id'].filter(k=>input[k]).map(k=>[k,input[k]]));
  if(!record(scope)||Object.entries(scope).some(([k,v])=>!['country','tree_id'].includes(k)||typeof v!=='string'||!v))throw Error(uiText("Invalid guide scope"));
  if(!record(input.tracks)||Object.keys(input.tracks).length>32)throw Error(uiText("Invalid guide tracks"));
  const result={...input,schema_version:VERSION,applies_to:{...scope},tracks:{}},keys=new Set();
  for(const field of ['country','tree_id'])if(scope[field]){if(input[field]&&input[field]!==scope[field])throw Error(uiText("Conflicting guide scope"));if(!Object.hasOwn(input,field))result[field]=scope[field];}
  const actions={focus:'select_focus',technology:'research',project:'special_project',gate:'manual',free:'free'},kinds=Object.fromEntries(Object.entries(actions).map(([k,v])=>[v,k]));
  for(const [track,steps] of Object.entries(input.tracks)){
   if(!Array.isArray(steps)||steps.length>4096)throw Error(uiText("Invalid guide track"));
   const rows=steps.map((s,i)=>{
   if(!record(s))throw Error(uiText("Invalid guide step"));
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
  for(const step of steps){for(const field of ['when','priority_when','skip_when'])if(Object.hasOwn(step,field))validateCondition(step[field]);if(Object.hasOwn(step,'priority')&&(!Number.isInteger(step.priority)||Math.abs(step.priority)>100000))throw Error(uiText("Invalid guide priority"));}
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
