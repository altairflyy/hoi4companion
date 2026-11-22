'use strict';
((root)=>{
 const country=typeof module==='object'&&module.exports?require('./country-model.js'):root.HoiCountryModel;
 const routes={overview:null,library:null,walkthrough:'/walkthrough',focus:'/',technologies:'/technologies',country:'/country',events:'/events',templates:'/importer?view=references',importer:'/importer','saved-guides':'/importer?view=library',analysis:'/analysis',settings:'/'};
 function title(value){return String(value||'').replace(/\.docx$/i,'').replace(/_/g,' ').replace(/\s+/g,' ').trim();}
 function metrics(packet){const model=country.evaluate(packet),s=model.sections;
  return {current:model.current,country:model.country,divisions:country.number(s.divisions.total,{integer:true}),politicalPower:s.politics.power?.status==='observed'?country.number(s.politics.power.available,{signed:true}):null,factories:country.number(s.production.effective_factories?.military,{integer:true})};
 }
 function library(guides,tag,all,query=''){const needle=query.trim().toLocaleLowerCase();return (Array.isArray(guides)?guides:[]).filter(g=>g&&typeof g.id==='string'&&(!tag||all||!g.applies_to?.country||g.applies_to.country===tag)&&(!needle||(title(g.title)+' '+(g.applies_to?.country||'')).toLocaleLowerCase().includes(needle))).slice().sort((a,b)=>Number(b.applies_to?.country===tag)-Number(a.applies_to?.country===tag)||title(a.title).localeCompare(title(b.title)));}
 // Library cleanup (UX R15), presentation only: nothing is merged, hidden or
 // deleted. Guides are grouped by their declared nation (the observed one
 // first); entries without a nation are kept apart. A duplicate is a guide whose
 // backend definition_hash equals an earlier one in the whole library (title,
 // then id order); a guide sharing only its title is flagged as a different
 // version. A missing hash never makes a duplicate.
 function libraryGroups(guides,tag,all,query=''){
  const every=(Array.isArray(guides)?guides:[]).filter(g=>g&&typeof g.id==='string').slice().sort((a,b)=>title(a.title).localeCompare(title(b.title))||a.id.localeCompare(b.id));
  const first=new Map(),titles=new Map();
  for(const g of every){const h=typeof g.definition_hash==='string'&&g.definition_hash?g.definition_hash:null;if(h&&!first.has(h))first.set(h,g);const name=title(g.title).toLocaleLowerCase();titles.set(name,(titles.get(name)||new Set()).add(h||g.id));}
  const nations=new Map(),other=[];
  for(const guide of library(guides,tag,all,query)){const h=typeof guide.definition_hash==='string'&&guide.definition_hash?guide.definition_hash:null,original=h&&first.get(h);
   const row={guide,duplicateOf:original&&original.id!==guide.id?original:null,sameTitle:(titles.get(title(guide.title).toLocaleLowerCase())?.size||0)>1};
   const nation=typeof guide.applies_to?.country==='string'&&guide.applies_to.country?guide.applies_to.country:null;
   if(!nation){other.push(row);continue;}if(!nations.has(nation))nations.set(nation,[]);nations.get(nation).push(row);}
  const ordered=[...nations].sort(([a],[b])=>Number(b===tag)-Number(a===tag)||a.localeCompare(b)).map(([country,rows])=>({country,guides:rows}));
  return {nations:ordered,other};
 }
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
 const displayTracks=[['operations','Guide actions'],['focus','National focuses'],['research','Technology research']];
 const openStatuses=new Set(['ready','active','waiting','unverifiable']);
 function matchingEvaluation(projection,guideId,mode){
  const e=projection?.evaluation;
  if(!guideId||projection?.schema_version!==1||projection.engine!=='rust-guide-core'||
    projection.guide_id!==guideId||projection.guide?.id!==guideId||e?.guide_id!==guideId||
    e.mode!==mode||projection.progress?.guide_id!==guideId||
    (e.run_id!=null&&e.run_id!==projection.progress.run_id)||
    !Array.isArray(e.steps)||!e.tracks||!projection.guide.tracks)return null;
  return e;
 }
 // Rust nominates the next step independently for each track. A missing
 // nomination is left empty, even when the track contains other visible rows.
 function nextSteps(projection,guideId,mode){
  const e=matchingEvaluation(projection,guideId,mode);
  if(!e)return [];
  return displayTracks.flatMap(([track,label])=>{
   const key=e.tracks?.[track]?.next;
   if(typeof key!=='string')return [];
   const step=projection.guide.tracks[track]?.find(s=>s.key===key),row=e.steps.find(r=>r.key===key);
   return step&&step.tracking!=='reminder'&&row&&!row.set_aside&&openStatuses.has(row.status)?
    [{key,track,label,step,status:row.status,reason:row.reason}]:[];
  });
 }
 function preview(projection,guideId,perTrack=3){
  const e=matchingEvaluation(projection,guideId,'preview');
  if(!e)return null;
  const guide=projection.guide;
  const definitions=new Map(Object.values(guide.tracks).flat().filter(s=>s&&typeof s.key==='string').map(s=>[s.key,s])),rows=new Map(e.steps.map(r=>[r.key,r]));
  const groups=[];
  for(const [track,label] of displayTracks){const order=e.tracks[track]?.order,next=e.tracks[track]?.next;
   const start=Array.isArray(order)&&typeof next==='string'?order.indexOf(next):-1;if(start<0)continue;
   const items=[];
   for(const key of order.slice(start)){if(items.length>=perTrack)break;const step=definitions.get(key),row=rows.get(key);
    if(!step||!row||step.tracking==='reminder'||!openStatuses.has(row.status))continue;
    items.push({key,track,status:row.status,reason:typeof row.reason==='string'?row.reason:'',step});}
   if(items.length)groups.push({track,label,items});
  }
  return groups;
 }
 const api={routes,title,metrics,library,libraryGroups,selectable,currentProjection,projectionKey,preview,nextSteps};if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiSuiteModel=api;
})(typeof window==='object'?window:globalThis);
