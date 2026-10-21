'use strict';
((root)=>{
 const model={
  focusStates(value,treeId,ids){const result=Object.fromEntries(ids.map(id=>[id,'unknown']));const s=value?.usable?value.sample:null;if(!s||s.focus_tree_id!==treeId)return result;const done=new Set(s.completed_focuses||[]);for(const id of ids){if(done.has(id))result[id]='completed';else if(s.current_focus===id)result[id]='active';}return result;},
  technologyState(value,id){const s=value?.usable?value.sample:null;if(!s)return 'unknown';if(s.research_slots.some(slot=>slot.technology_id===id))return 'active';if(s.researched_technologies.includes(id))return 'completed';return s.technology_ids.includes(id)?'unresearched':'unknown';}
 };
 if(typeof module!=='undefined'&&module.exports)module.exports=model;else root.HoiLiveModel=model;
})(typeof window!=='undefined'?window:globalThis);
