'use strict';
// One guide/progression/live subscription per page. No extra polling or reader.
(()=>{
 const listeners=[];
 let guide=null,catalog=[],progress={checks:{},started:{}},live=null,error='',pending=0,sequence=0,server=null,queue=Promise.resolve();
 let activation=Promise.resolve(),activating=0,activeRevision=-1,activeSync=false,activeServer=null;
 let projection=null,projectionVersion=0,projectionTask=null,projectionError=false,projectionMutation=0;
 const endpoint=path=>path+'?id='+encodeURIComponent(guide.id);
 async function json(url,options){const r=await fetch(url,options),data=await r.json();if(!r.ok)throw Error(data.error||'HTTP '+r.status);return data;}
 // UX R13: the player's line readings arrive on the projection's steps, attached
 // by Rust after the evaluation. Copy them onto the same definition revision for
 // display only; statuses still come from the Rust evaluation alone.
 // The same object is kept while the readings do not change: the page resets its
 // filters and caches when the guide object changes.
 let readingMemo={definition:null,signature:'',shown:null};
 function interpreted(definition,projected){
  const readings=new Map(Object.values(projected?.tracks||{}).flat().filter(s=>s&&typeof s.key==='string'&&s.interpretation&&typeof s.interpretation==='object').map(s=>[s.key,s.interpretation]));
  if(!readings.size||!definition?.tracks){readingMemo={definition:null,signature:'',shown:null};return definition;}
  const signature=JSON.stringify([...readings]);
  if(readingMemo.definition===definition&&readingMemo.signature===signature)return readingMemo.shown;
  readingMemo={definition,signature,shown:null};
  return readingMemo.shown={...definition,tracks:Object.fromEntries(Object.entries(definition.tracks).map(([id,steps])=>[id,Array.isArray(steps)?steps.map(s=>readings.has(s?.key)?{...s,interpretation:readings.get(s.key)}:s):steps]))};
 }
 function publish(){
  const shared=!pending&&guide?.runtime_engine==='rust-guide-core'&&projection?.sequence===sequence&&projection.packet.progress.run_id===progress.run_id?projection.packet:null;
  // Between two projections the last readings of this same definition stay shown.
  const shown=shared?interpreted(guide,shared.guide):readingMemo.definition===guide&&readingMemo.shown||guide;
  const state=shared?shared.state:window.hoiLive?.enabled?live:null,view=shown?HoiGuidePresentation.evaluate(shown,state,progress,shared?.evaluation):null;
  api.value={guide:shown,catalog,progress,view,state,error,pending};for(const fn of listeners)fn(api.value);
 }
 // A projection is evaluated against the live observation its request saw. Rust reads live state only when this
 // predicate holds, so another event with the same basis cannot change the evaluation and keeps the current cards.
 const readsLive=value=>!!window.hoiLive?.enabled&&value?.usable===true&&(typeof document==='undefined'||document.visibilityState!=='hidden');
 const liveBasis=value=>JSON.stringify([readsLive(value),value?.session_id??null,value?.sample?.session_id??null,value?.sample?.country??null]);
 function requestProjection(){
  if(guide?.runtime_engine!=='rust-guide-core')return Promise.resolve();
  projectionVersion++;if(projectionTask)return projectionTask;
  projectionTask=(async()=>{
   let processed=-1;
   while(processed!==projectionVersion&&guide?.runtime_engine==='rust-guide-core'){
    processed=projectionVersion;const selected=sequence,id=guide.id,expectedServer=server,mutation=projectionMutation;
    const read=readsLive(live),basis=liveBasis(live);
    try{
     // One request is already serialized below. A cold backend may still be
     // preparing archived definitions; do not cancel it and queue another copy.
     const packet=await json('/api/guide/projection?id='+encodeURIComponent(id)+'&live='+(read?'1':'0'));
     if(selected!==sequence||mutation!==projectionMutation||basis!==liveBasis(live)||expectedServer&&server!==expectedServer)continue;
     if(packet.schema_version!==1||packet.engine!=='rust-guide-core'||packet.guide_id!==id||packet.progress?.guide_id!==id||packet.evaluation?.guide_id!==id||packet.evaluation.run_id!==null&&packet.evaluation.run_id!==packet.progress.run_id||!packet.state||!packet.server_id||expectedServer&&packet.server_id!==expectedServer||packet.definition&&(packet.definition.run_id!==packet.progress.run_id||packet.definition.status==='pinned'&&packet.definition.id!==packet.progress.definition_ref?.id))throw Error('Invalid shared guide evaluation');
     if(packet.definition_revision!==guide.runtime_definition_revision){projection=null;Promise.resolve().then(()=>select(id,false));break;}
     projection={sequence:selected,packet,basis};if(!pending)progress=packet.progress;
     server=packet.server_id;
     if(projectionError){error='';projectionError=false;}publish();
    }catch(e){if(selected===sequence&&processed===projectionVersion){projection=null;projectionError=true;error=HoiI18n.t('Shared guide evaluation unavailable: ')+e.message;publish();}}
   }
  })().finally(()=>{projectionTask=null;});
  return projectionTask;
 }
 function remember(id){
  try{localStorage.setItem('hoi4-active-guide',id);}catch{}
  try{sessionStorage.setItem('hoi4-active-guide',id);}catch{}
 }
 function stored(){
  // A new launcher tab shares the browser preference, not the old tab's session.
  let id=null;try{id=localStorage.getItem('hoi4-active-guide');}catch{}
  if(catalog.some(item=>item.id===id))return id;
  try{id=sessionStorage.getItem('hoi4-active-guide');}catch{id=null;}
  return catalog.some(item=>item.id===id)?id:null;
 }
 async function select(id,announce=true){
  const current=++sequence;error='';projection=null;projectionVersion++;
  try{
   const definition=await json('/api/guide'+(id?'?id='+encodeURIComponent(id):''));
   const saved=await json('/api/guide/progress?id='+encodeURIComponent(definition.id));
   if(current!==sequence)return;
   guide=definition;progress=saved;server=saved.server_id||null;remember(guide.id);
   if(typeof history!=='undefined'&&history.replaceState){const url=new URL(location.href);url.searchParams.set('guide',guide.id);history.replaceState(null,'',url.pathname+url.search+url.hash);}
   if(activeSync&&announce&&(typeof document==='undefined'||document.visibilityState!=='hidden')){
    activating++;
    const task=activation.catch(()=>{}).then(async()=>{
     if(current!==sequence)return;
     const selected=await json('/api/guide/active',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({guide_id:definition.id})});
     if(selected.guide_id!==definition.id)throw Error(HoiI18n.t("Active guide not confirmed by the server"));
     if(current===sequence){activeRevision=selected.revision;if(selected.server_id)activeServer=selected.server_id;}
    });activation=task;
    try{await task;}finally{activating--;}
   }
   if(current===sequence){publish();await requestProjection();}
  }catch(e){if(current===sequence){error=HoiI18n.t("Guide unavailable: ")+e.message;publish();}}
 }
 function write(payload){
  const id=guide.id,current=sequence,guard={expected_run_id:progress.run_id,expected_revision:progress.run_revision,expected_definition_id:progress.definition_ref?.id??null,server_id:progress.server_id};pending++;projectionMutation++;projection=null;projectionVersion++;publish();
  const task=queue.then(async()=>{
   const saved=await json('/api/guide/progress?id='+encodeURIComponent(id),{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({...payload,...guard})});
   if(current===sequence){if(pending===1)progress=saved;else progress={...saved,checks:progress.checks,started:progress.started,set_aside:progress.set_aside};error='';}
   return saved;
  });
  queue=task.catch(()=>{});
  return task.catch(async e=>{
   if(current===sequence){error=HoiI18n.t("Confirmation not saved: ")+e.message;try{const saved=await json('/api/guide/progress?id='+encodeURIComponent(id));if(current===sequence)progress=saved;}catch{}}
   throw e;
  }).finally(async()=>{pending--;publish();await requestProjection();});
 }
 const api=window.hoiGuide={value:null,
  subscribe(fn){listeners.push(fn);if(this.value)fn(this.value);},select,
  withGuide(href){const url=new URL(href,location.href);if(guide)url.searchParams.set('guide',guide.id);return url.pathname+url.search+url.hash;},
  async confirm(key,checked){
   if(!guide||pending)throw Error(HoiI18n.t("Wait for the current save to finish"));
   const rows=Object.values(guide.tracks).flat(),legacy=rows.find(row=>row.key===key)?.action.params?.legacy_key,checks={...progress.checks};
   // Split a previously confirmed bulk checkpoint without losing its other
   // explicit confirmations, so one new activity can be unchecked independently.
   if(typeof legacy==='string'&&checks[legacy]===true){for(const row of rows)if(row.action.params?.legacy_key===legacy&&!Object.hasOwn(checks,row.key))checks[row.key]=true;checks[legacy]=false;}
   progress={...progress,checks:{...checks,[key]:checked}};
   // Done is done: a confirmed step leaves the Set aside list in the same save.
   if(checked&&progress.set_aside?.[key]){const aside={...progress.set_aside};delete aside[key];progress={...progress,set_aside:aside};return write({run_id:progress.run_id,checks:progress.checks,set_aside:aside});}
   return write({run_id:progress.run_id,checks:progress.checks});
  },
  // Sets a step aside (or resumes it); Rust decides what that changes.
  async setAside(key,aside){
   if(!guide||pending)throw Error(HoiI18n.t("Wait for the current save to finish"));
   const next={...(progress.set_aside||{})};if(aside)next[key]=true;else delete next[key];
   progress={...progress,set_aside:next};return write({run_id:progress.run_id,set_aside:next});
  },
  async start(key,started){
   if(!guide||pending)throw Error(HoiI18n.t("Wait for the current save to finish"));
   progress={...progress,started:{...progress.started,[key]:started}};return write({run_id:progress.run_id,started:progress.started});
  },
  async newRun(){const old=progress;await write({});return old;},
  async restore(old){return write({run_id:old.run_id});},
  async refreshProgress(){if(!guide||pending)return;const current=sequence,saved=await json(endpoint('/api/guide/progress'));if(current===sequence){progress=saved;publish();await requestProjection();}},
 };
 window.hoiLive?.subscribe(value=>{
  live=value;
  if(value?.guide_server_id&&value.guide_server_id!==server)server=value.guide_server_id;
  if(value?.guide_server_id&&value.guide_server_id!==activeServer){activeServer=value.guide_server_id;activeRevision=-1;}
  const shared=value?.active_guide_id;
  if(shared)activeSync=true;
  if(!activating&&guide&&shared&&shared!==guide.id&&value.active_guide_revision>=activeRevision&&catalog.some(g=>g.id===shared)){
   activeRevision=value.active_guide_revision;select(shared,false);return;
  }
  if(guide?.runtime_engine==='rust-guide-core'){
   // Keep the current packet until its replacement arrives unless the event changes its basis: a usable flip, a
   // session or country change, or another server. Never show a packet for the wrong campaign, even briefly.
   if(projection&&(projection.basis!==liveBasis(value)||value?.guide_server_id&&value.guide_server_id!==projection.packet.server_id))projection=null;
   publish();requestProjection();return;
  }
  // Campaign changes and timeline rewinds are reconciled once by the server.
  // A browser tab never resets shared progress from a local comparison.
  publish();
 });
 (async()=>{try{const data=await json('/api/guides');catalog=data.guides;let shared=null;
  try{const selected=await json('/api/guide/active');if(catalog.some(g=>g.id===selected.guide_id)){shared=selected.guide_id;activeRevision=selected.revision;activeServer=selected.server_id||null;activeSync=true;}}catch{}
  await select(new URLSearchParams(location.search).get('guide')||shared||stored());
 }catch(e){error=e.message;publish();}})();
 if(typeof document!=='undefined'&&document.addEventListener)document.addEventListener('visibilitychange',()=>{
  if(activeSync&&guide&&document.visibilityState==='visible')select(guide.id);
 });
})();
