'use strict';
((root)=>{
 const c=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 function draftShape(value){if(!value||typeof value!=='object'||Array.isArray(value)||!value.tracks||typeof value.tracks!=='object'||Array.isArray(value.tracks)||value.schema_version!==undefined&&value.schema_version!==1)throw Error('Definizione guida v1 necessaria');}
 function readDraft(text){
  if(typeof text!=='string'||text.length>2000000)throw Error('File JSON troppo grande');
  const value=JSON.parse(text);
  if(!value||typeof value!=='object'||Array.isArray(value))throw Error('Documento JSON non valido');
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
  if(typeof patch.instruction!=='string'||!patch.instruction.trim())throw Error('Inserisci l’istruzione corretta');
  const started=step.started_when,completed=step.completion;
  const counter={infrastructure:'infrastructure',arms_factory:'military_factories',industrial_complex:'civilian_factories',dockyard:'dockyards'}[before.building_id];
  const target=before.target_level??before.target_quantity??(before.quantity_mode==='target_total'?before.quantity:before.quantity_mode==='additional'&&Number.isSafeInteger(before.baseline_level)?before.baseline_level+before.quantity:undefined);
  // Rebuild only recognisable construction bindings. Preserve custom conditions.
  if(started?.op==='fact'&&started.key==='construction.queued'&&started.test==='contains'&&started.value===`state:${before.state_id}:${before.building_id}`)delete step.started_when;
  if(completed?.op==='fact'&&completed.test==='gte'&&
   ((completed.key===`guide.construction.${key}.added`&&completed.value===before.quantity)||
    (counter&&completed.key===`country.states.items.${before.state_id}.${counter}`&&completed.value===target)))delete step.completion;
  if(step.started_when||step.completion)throw Error('Il passaggio contiene criteri personalizzati: rivedili nell’editor JSON prima di cambiare la costruzione. Nessuna correzione applicata.');
  const params={...before,state_id:Number(patch.state.id),state_name:patch.state.name,building_id:patch.building.id,building_name:patch.building.name,quantity:patch.quantity,quantity_mode:patch.quantity_mode,instructions:[patch.instruction.trim()]};
  for(const field of ['baseline_level','target_level','target_quantity'])delete params[field];
  step.action.params=params;
  return result;
 }
 const api={readDraft,editStep,editConstruction,normalise:c.guide,canPublish:(report,reviewed,busy)=>report?.valid===true&&reviewed===true&&!busy};
 if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiImporter=api;
})(typeof window!=='undefined'?window:globalThis);
