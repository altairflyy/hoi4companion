'use strict';
((root)=>{
 function recommended(guide,packet){
  if(packet?.usable!==true||!packet.sample?.country||guide.compatibility?.blocked)return false;
  const scope=guide.applies_to||{},sample=packet.sample;
  if(scope.country!==sample.country)return false;
  return !scope.tree_id||scope.tree_id===sample.focus_tree_id;
 }
 function counts(collection){return collection.counts||{};}
 function canAdd(collection){return collection.status==='prepared'&&((counts(collection).ready||0)>0||collection.references_status==='ready'||!collection.reviewed&&((counts(collection).needs_revision||0)>0||(counts(collection).dlc_unavailable||0)>0));}
 function pending(collection){return collection.status==='preparing'||collection.import_stage==='saving'||collection.auto_import===true&&collection.auto_finished!==true&&collection.status==='prepared';}
 function waitingMio(collection){return ['pending','waiting_for_game_exit'].includes(collection.mio_install?.status);}
 function problems(collection){return (counts(collection).needs_revision||0)>0||collection.status==='failed'||!!collection.references_error||collection.mio_install?.status==='error';}
 function legacyReady(collection){return collection.auto_import!==true&&collection.auto_finished!==true&&collection.status==='prepared'&&(canAdd(collection)||(collection.references?.mio_queues||0)>0&&!collection.mio_install);}
 function library(guides,collections){
  const saved=new Map(guides.map(g=>[g.id,g]));
  for(const collection of collections)if(collection.reviewed)for(const row of collection.items||[]){
   if(row.status==='dlc_unavailable'&&row.guide_id&&!saved.has(row.guide_id))saved.set(row.guide_id,{id:row.guide_id,title:row.title,applies_to:row.applies_to,compatibility:row.compatibility});
  }
  return [...saved.values()];
 }
 const api={recommended,counts,canAdd,library,pending,waitingMio,problems,legacyReady};
 if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiCollections=api;
})(typeof window!=='undefined'?window:globalThis);
