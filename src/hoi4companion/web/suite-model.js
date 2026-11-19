'use strict';
((root)=>{
 const country=typeof module==='object'&&module.exports?require('./country-model.js'):root.HoiCountryModel;
 const routes={overview:null,library:null,walkthrough:'/walkthrough',focus:'/',technologies:'/technologies',country:'/country',events:'/events',templates:'/importer#collectionTemplatesHeading',importer:'/importer',analysis:'/analysis',settings:'/'};
 function title(value){return String(value||'').replace(/\.docx$/i,'').replace(/_/g,' ').replace(/\s+/g,' ').trim();}
 function metrics(packet){const model=country.evaluate(packet),s=model.sections;
  return {current:model.current,country:model.country,divisions:country.number(s.divisions.total,{integer:true}),politicalPower:s.politics.power?.status==='observed'?country.number(s.politics.power.available,{signed:true}):null,factories:country.number(s.production.effective_factories?.military,{integer:true})};
 }
 function library(guides,tag,all,query=''){const needle=query.trim().toLocaleLowerCase();return (Array.isArray(guides)?guides:[]).filter(g=>g&&typeof g.id==='string'&&(!tag||all||!g.applies_to?.country||g.applies_to.country===tag)&&(!needle||(title(g.title)+' '+(g.applies_to?.country||'')).toLocaleLowerCase().includes(needle))).slice().sort((a,b)=>Number(b.applies_to?.country===tag)-Number(a.applies_to?.country===tag)||title(a.title).localeCompare(title(b.title)));}
 function selectable(guide,tag){return guide?.compatibility?.blocked!==true&&(!tag||!guide?.applies_to?.country||guide.applies_to.country===tag);}
 function currentProjection(packet,projection){const id=packet?.active_guide_id,run=packet?.guide_runs?.[id],campaign=packet?.sample?.campaign_identity;
  return !!(packet?.usable===true&&id&&projection?.engine==='rust-guide-core'&&projection.guide_id===id&&projection.state?.usable===true&&projection.state.session_id===packet.sample.session_id&&run?.run_id&&projection.progress?.run_id===run.run_id&&run.country===packet.sample.country&&campaign?.status==='observed'&&projection.progress.campaign_id===campaign.id&&run.campaign_id===campaign.id);
 }
 function projectionKey(packet){const sample={...packet?.sample};for(const key of ['native_tick','metrics','observed_at','age_seconds','transport'])delete sample[key];
  return JSON.stringify([packet?.usable,packet?.guide_server_id,packet?.active_guide_id,packet?.active_guide_revision,packet?.guide_revision,sample,packet?.guide_runs,packet?.guide_states,packet?.guide_started,packet?.guide_baselines]);
 }
 // Home preview without the game (UX R11): the first open steps of each track,
 // read only from a Rust evaluation in preview mode (its order, next step and
 // status). Nothing is evaluated or completed here; completed, not applicable
 // and unknown statuses are left out rather than guessed.
 const previewTracks=[['operations','Guide actions'],['focus','National focuses'],['research','Technology research']];
 const previewStatuses=new Set(['ready','active','waiting','unverifiable']);
 function preview(projection,guideId,perTrack=3){
  const e=projection?.evaluation,guide=projection?.guide;
  if(!guideId||projection?.engine!=='rust-guide-core'||projection.guide_id!==guideId||e?.guide_id!==guideId||e.mode!=='preview'||!Array.isArray(e.steps)||!e.tracks||!guide?.tracks)return null;
  const definitions=new Map(Object.values(guide.tracks).flat().filter(s=>s&&typeof s.key==='string').map(s=>[s.key,s])),rows=new Map(e.steps.map(r=>[r.key,r]));
  const groups=[];
  for(const [track,label] of previewTracks){const order=e.tracks[track]?.order,next=e.tracks[track]?.next;
   const start=Array.isArray(order)&&typeof next==='string'?order.indexOf(next):-1;if(start<0)continue;
   const items=[];
   for(const key of order.slice(start)){if(items.length>=perTrack)break;const step=definitions.get(key),row=rows.get(key);
    if(!step||!row||step.tracking==='reminder'||!previewStatuses.has(row.status))continue;
    items.push({key,track,status:row.status,reason:typeof row.reason==='string'?row.reason:'',step});}
   if(items.length)groups.push({track,label,items});
  }
  return groups;
 }
 const api={routes,title,metrics,library,selectable,currentProjection,projectionKey,preview};if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiSuiteModel=api;
})(typeof window==='object'?window:globalThis);
