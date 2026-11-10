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
 function library(guides,collections){
  const saved=new Map(guides.map(g=>[g.id,g]));
  for(const collection of collections)if(collection.reviewed)for(const row of collection.items||[]){
   if(row.status==='dlc_unavailable'&&row.guide_id&&!saved.has(row.guide_id))saved.set(row.guide_id,{id:row.guide_id,title:row.title,applies_to:row.applies_to,compatibility:row.compatibility});
  }
  return [...saved.values()];
 }
 const api={recommended,counts,canAdd,library};
 if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiCollections=api;
})(typeof window!=='undefined'?window:globalThis);
