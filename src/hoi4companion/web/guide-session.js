'use strict';
// One guide/progression/live subscription per page. No extra polling or reader.
(()=>{
 const listeners=[];
 let guide=null,catalog=[],progress={checks:{},started:{}},live=null,error='',pending=0,revision=-1,sequence=0,server=null,queue=Promise.resolve();
 let activation=Promise.resolve(),activating=0,activeRevision=-1,activeSync=false,activeServer=null;
 const endpoint=path=>path+'?id='+encodeURIComponent(guide.id);
 async function json(url,options){const r=await fetch(url,options),data=await r.json();if(!r.ok)throw Error(data.error||'HTTP '+r.status);return data;}
 function publish(){const state=window.hoiLive?.enabled?live:null,view=guide?HoiGuidePresentation.evaluate(guide,state,progress):null;api.value={guide,catalog,progress,view,state,error,pending};for(const fn of listeners)fn(api.value);}
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
  const current=++sequence;error='';
  try{
   const definition=await json('/api/guide'+(id?'?id='+encodeURIComponent(id):''));
   const saved=await json('/api/guide/progress?id='+encodeURIComponent(definition.id));
   if(current!==sequence)return;
   guide=definition;progress=saved;revision=saved.revision;server=saved.server_id||null;remember(guide.id);
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
   if(current===sequence)publish();
  }catch(e){if(current===sequence){error=HoiI18n.t("Guide unavailable: ")+e.message;publish();}}
 }
 function write(payload){
  const id=guide.id,current=sequence,guard={expected_run_id:progress.run_id,expected_revision:progress.run_revision,server_id:progress.server_id};pending++;publish();
  const task=queue.then(async()=>{
   const saved=await json('/api/guide/progress?id='+encodeURIComponent(id),{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({...payload,...guard})});
   if(current===sequence){revision=saved.revision;if(pending===1)progress=saved;else progress={...saved,checks:progress.checks,started:progress.started};error='';}
   return saved;
  });
  queue=task.catch(()=>{});
  return task.catch(async e=>{
   if(current===sequence){error=HoiI18n.t("Confirmation not saved: ")+e.message;try{progress=await json(endpoint('/api/guide/progress'));revision=progress.revision;}catch{}}
   throw e;
  }).finally(()=>{pending--;publish();});
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
   progress={...progress,checks:{...checks,[key]:checked}};return write({run_id:progress.run_id,checks:progress.checks});
  },
  async start(key,started){
   if(!guide||pending)throw Error(HoiI18n.t("Wait for the current save to finish"));
   progress={...progress,started:{...progress.started,[key]:started}};return write({run_id:progress.run_id,started:progress.started});
  },
  async newRun(){const old=progress;await write({});return old;},
  async restore(old){return write({run_id:old.run_id});},
  async refreshProgress(){if(!guide||pending)return;const current=sequence,saved=await json(endpoint('/api/guide/progress'));if(current===sequence){progress=saved;revision=saved.revision;publish();}},
 };
 window.hoiLive?.subscribe(value=>{
  live=value;
  if(value?.guide_server_id&&value.guide_server_id!==server){server=value.guide_server_id;revision=-1;}
  if(value?.guide_server_id&&value.guide_server_id!==activeServer){activeServer=value.guide_server_id;activeRevision=-1;}
  const shared=value?.active_guide_id;
  if(shared)activeSync=true;
  if(!activating&&guide&&shared&&shared!==guide.id&&value.active_guide_revision>=activeRevision&&catalog.some(g=>g.id===shared)){
   activeRevision=value.active_guide_revision;select(shared,false);return;
  }
  if(guide&&value?.guide_revision!==undefined&&value.guide_revision>=revision&&!pending){
   revision=value.guide_revision;progress={...progress,...value.guide_runs?.[guide.id],server_id:server,storage:value.guide_storage||progress.storage,checks:value.guide_states?.[guide.id]||(value.guide_id===guide.id?value.guide_checks:{} )||{},started:value.guide_started?.[guide.id]||{},baselines:value.guide_baselines?.[guide.id]||{}};
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
