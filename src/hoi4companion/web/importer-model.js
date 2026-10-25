'use strict';
((root)=>{
 const c=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 function countryCompatibility(condition,country,originalCountry=null,depth=0){
  if(!condition||depth>40)return null;
  if(condition.op==='tag')return typeof condition.value==='string'?condition.value===country:null;
  if(condition.op==='original_tag')return typeof condition.value==='string'&&originalCountry?condition.value===originalCountry:null;
  if(condition.op==='true')return true;if(condition.op==='false')return false;
  if(condition.op==='not'){const value=countryCompatibility(condition.arg,country,originalCountry,depth+1);return value===null?null:!value;}
  if(['all','any'].includes(condition.op)&&Array.isArray(condition.args)){
   const values=condition.args.map(arg=>countryCompatibility(arg,country,originalCountry,depth+1));
   if(condition.op==='all')return values.includes(false)?false:values.includes(null)?null:true;
   return values.includes(true)?true:values.includes(null)?null:false;
  }
  return null;
 }
 function countryPolitics(entities,countries){
  let excluded=0,unknown=0;
  const catalog=entities.filter(e=>{
   const values=countries.map(country=>countryCompatibility(e.country_condition,country,country));
   if(values.includes(true))return true;
   if(values.includes(null)){unknown++;return true;}
   excluded++;return false;
  });
  return {catalog,country_filter:{countries,basis:'static_political_conditions',coverage:'partial',unknown:'included',
   original_tag_basis:'selected_nation',excluded_entries:excluded,retained_unknown_entries:unknown,
   metadata_available:entities.some(e=>e.country_condition)}};
 }
 function countryEvents(entities,countries){
  const eligible=new Set(),results=new Map();let excluded=0,unknown=0;
  const compatible=condition=>{const values=countries.map(country=>countryCompatibility(condition,country));return values.includes(true)?true:values.includes(null)?null:false;};
  for(const e of entities)if(e.kind==='event'){
   const value=compatible(e.country_condition),previous=results.get(e.id);
   results.set(e.id,!results.has(e.id)?value:previous===true||value===true?true:previous===null||value===null?null:false);
  }
  for(const [id,value] of results){if(value===false)excluded++;else{eligible.add(id);if(value===null)unknown++;}}
  return {catalog:entities.filter(e=>e.kind==='event'?eligible.has(e.id):e.kind==='event_option'&&eligible.has(e.event_id)&&compatible(e.country_condition)!==false),
   country_filter:{countries,basis:'static_recipient_conditions',coverage:'partial',unknown:'included',excluded_events:excluded,retained_unknown_events:unknown,
    metadata_available:entities.some(e=>e.kind==='event'&&e.country_condition)}};
 }
 function catalogSections(entities,treeId=''){
  const definitions=[['focus','Focus nazionali',['focus']],['research','Ricerche tecnologiche',['technology']],
   ['production','Produzione ed equipaggiamenti',['equipment']],['construction','Costruzioni e regioni',['building','state']],
   ['projects','Progetti speciali',['project']],['politics','Politica, personaggi e decisioni',['law','idea','advisor','decision']],
   ['events','Eventi e risposte',['event','event_option']],['other','Altri identificatori',[]]];
  const sections=definitions.map(([id,title])=>({id,title,catalog_schema_version:1,catalog_scope:id,reference_language:'english',catalog:[]}));
  const byKind=new Map(definitions.flatMap(([id,,kinds])=>kinds.map(kind=>[kind,sections.find(s=>s.id===id)])));
  for(const entity of entities){
   if(entity.kind==='focus'&&treeId&&entity.scope!==treeId)continue;
   (byKind.get(entity.kind)||sections.at(-1)).catalog.push(entity);
  }
  if(treeId){
   sections[0].focus_tree_id=treeId;
   const countries=[...new Set(sections[0].catalog.flatMap(e=>e.countries||[]))].sort();
   const events=sections.find(s=>s.id==='events');
   if(countries.length){
    Object.assign(events,countryEvents(events.catalog,countries));
    const politics=sections.find(s=>s.id==='politics');Object.assign(politics,countryPolitics(politics.catalog,countries));
   }
  }
  return sections;
 }
 function eventCatalog(entities){
  const events=new Set(entities.filter(e=>e.kind==='event').map(e=>e.id)),options=new Map();
  for(const e of entities)if(e.kind==='event_option'&&events.has(e.event_id)&&typeof e.id==='string'&&e.id){
   if(!options.has(e.event_id))options.set(e.event_id,new Set());options.get(e.event_id).add(e.id);
  }
  const multiple=new Set([...options].filter(([,ids])=>ids.size>=2).map(([id])=>id));
  return {catalog_schema_version:1,catalog_scope:'events_and_options',reference_language:'english',
   event_filter:{minimum_distinct_options:2,basis:'defined_option_ids'},
   catalog:entities.filter(e=>e.kind==='event'?multiple.has(e.id):e.kind==='event_option'&&multiple.has(e.event_id))};
 }
 function draftShape(value){if(!value||typeof value!=='object'||Array.isArray(value)||!value.tracks||typeof value.tracks!=='object'||Array.isArray(value.tracks)||value.schema_version!==undefined&&value.schema_version!==1)throw Error('Definizione guida v1 necessaria');}
 function readDraft(text){
  if(typeof text!=='string'||text.length>2000000)throw Error('File JSON troppo grande');
  const value=JSON.parse(text);
  if(!value||typeof value!=='object'||Array.isArray(value))throw Error('Documento JSON non valido');
  if(value.format==='hoi4-guide-authoring'){
   if(value.authoring_schema_version!==1||!Array.isArray(value.sources)||!value.guide||typeof value.guide!=='object'||!Array.isArray(value.guide.steps))throw Error('Documento authoring v1 non valido');
   return {guide:value.guide,sources:value.sources,authoring:true,document:value};
  }
  if('import_schema_version'in value){
   if(value.import_schema_version!==1||!Array.isArray(value.sources))throw Error('Versione bozza non supportata');
   if(value.guide)draftShape(value.guide);
   return {guide:value.guide||null,sources:value.sources};
  }
  draftShape(value);return {guide:value,sources:null};
 }
 function editStep(guide,key,patch){
  const result=JSON.parse(JSON.stringify(guide)),step=Object.values(result.tracks).flat().find(row=>row.key===key);
  if(!step)throw Error('Passaggio sconosciuto');
  if(patch.entity)step.action.entity=patch.entity;
  if(patch.title!==undefined)step.title=patch.title;
  if(patch.tracking!==undefined){
   if(!['action','reminder'].includes(patch.tracking)||step.action.kind!=='manual')throw Error('Modalità monitoraggio non valida');
   if(patch.tracking==='reminder'){
    if(Object.values(result.tracks).flat().some(row=>row.depends_on?.includes(key)))throw Error('Altri passaggi dipendono da questa istruzione: rivedi prima le dipendenze.');
    delete step.completion;delete step.started_when;
   }
   step.tracking=patch.tracking;
  }
  return result;
 }
 function editConstruction(guide,key,patch){
  const result=JSON.parse(JSON.stringify(guide)),step=Object.values(result.tracks).flat().find(row=>row.key===key);
  if(!step||step.action?.kind!=='manual'||step.action.params?.category!=='construction')throw Error('Passaggio di costruzione necessario');
  const before=step.action.params;
  if(patch.state?.kind!=='state'||!/^\d+$/.test(patch.state.id)||!Number.isSafeInteger(Number(patch.state.id))||Number(patch.state.id)<1)throw Error('Scegli una regione del catalogo');
  if(patch.building?.kind!=='building'||!patch.building.id)throw Error('Scegli un edificio del catalogo');
  if(!Number.isSafeInteger(patch.quantity)||patch.quantity<1)throw Error('La quantità deve essere un intero positivo');
  if(!['additional','target_total'].includes(patch.quantity_mode))throw Error('Modalità quantità non valida');
  if(patch.province_id!=null&&(!Number.isSafeInteger(patch.province_id)||patch.province_id<1))throw Error('La provincia deve essere un ID intero positivo');
  if(typeof patch.instruction!=='string'||!patch.instruction.trim())throw Error('Inserisci l’istruzione corretta');
  const started=step.started_when,completed=step.completion;
  const counter={infrastructure:'infrastructure',arms_factory:'military_factories',industrial_complex:'civilian_factories',dockyard:'dockyards'}[before.building_id];
  const target=before.target_level??before.target_quantity??(before.quantity_mode==='target_total'?before.quantity:before.quantity_mode==='additional'&&Number.isSafeInteger(before.baseline_level)?before.baseline_level+before.quantity:undefined);
  // Rebuild only recognisable construction bindings. Preserve custom conditions.
  if(started?.op==='fact'&&started.key==='construction.queued'&&started.test==='contains'&&
   (started.value===`state:${before.state_id}:${before.building_id}`||Number.isSafeInteger(before.province_id)&&started.value===`province:${before.province_id}:${before.building_id}`))delete step.started_when;
  if(completed?.op==='fact'&&completed.test==='gte'&&
   ((completed.key===`guide.construction.${key}.added`&&completed.value===before.quantity)||
    (counter&&completed.key===`country.states.items.${before.state_id}.${counter}`&&completed.value===target)))delete step.completion;
  if(step.started_when||step.completion)throw Error('Il passaggio contiene criteri personalizzati: rivedili nell’editor JSON prima di cambiare la costruzione. Nessuna correzione applicata.');
  const params={...before,state_id:Number(patch.state.id),state_name:patch.state.name,building_id:patch.building.id,building_name:patch.building.name,quantity:patch.quantity,quantity_mode:patch.quantity_mode,instructions:[patch.instruction.trim()]};
  for(const field of ['baseline_level','target_level','target_quantity','province_id'])delete params[field];
  if(patch.province_id!=null)params.province_id=patch.province_id;
  step.action.params=params;
  return result;
 }
 const api={readDraft,editStep,editConstruction,eventCatalog,catalogSections,countryCompatibility,normalise:c.guide,canPublish:(report,reviewed,busy)=>report?.valid===true&&reviewed===true&&!busy};
 if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiImporter=api;
})(typeof window!=='undefined'?window:globalThis);
