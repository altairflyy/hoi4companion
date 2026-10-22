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
 const api={readDraft,editStep,normalise:c.guide,canPublish:(report,reviewed,busy)=>report?.valid===true&&reviewed===true&&!busy};
 if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiImporter=api;
})(typeof window!=='undefined'?window:globalThis);
