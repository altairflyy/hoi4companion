'use strict';
// One guide/progression/live subscription per page. No extra polling or reader.
(()=>{
 const listeners=[];
 let guide=null,catalog=[],progress={checks:{},started:{}},live=null,error='',pending=0,revision=-1,sequence=0,server=null,queue=Promise.resolve();
 const endpoint=path=>path+'?id='+encodeURIComponent(guide.id);
 async function json(url,options){const r=await fetch(url,options),data=await r.json();if(!r.ok)throw Error(data.error||'HTTP '+r.status);return data;}
 function publish(){const view=guide?HoiGuidePresentation.evaluate(guide,window.hoiLive?.enabled?live:null,progress):null;api.value={guide,catalog,progress,view,error,pending};for(const fn of listeners)fn(api.value);}
 function remember(id){try{sessionStorage.setItem('hoi4-active-guide',id);}catch{}}
 function stored(){try{return sessionStorage.getItem('hoi4-active-guide');}catch{return null;}}
 async function select(id){
  const current=++sequence;error='';
  try{
   const definition=await json('/api/guide'+(id?'?id='+encodeURIComponent(id):''));
   const saved=await json('/api/guide/progress?id='+encodeURIComponent(definition.id));
   if(current!==sequence)return;
   guide=definition;progress=saved;revision=saved.revision;server=saved.server_id||null;remember(guide.id);publish();
  }catch(e){if(current===sequence){error='Guida non disponibile: '+e.message;publish();}}
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
   if(current===sequence){error='Conferma non salvata: '+e.message;try{progress=await json(endpoint('/api/guide/progress'));revision=progress.revision;}catch{}}
   throw e;
  }).finally(()=>{pending--;publish();});
 }
 const api=window.hoiGuide={value:null,
  subscribe(fn){listeners.push(fn);if(this.value)fn(this.value);},select,
  withGuide(href){const url=new URL(href,location.href);if(guide)url.searchParams.set('guide',guide.id);return url.pathname+url.search+url.hash;},
  async confirm(key,checked){
   if(!guide||pending)throw Error('Attendi il salvataggio in corso');
   const rows=Object.values(guide.tracks).flat(),legacy=rows.find(row=>row.key===key)?.action.params?.legacy_key,checks={...progress.checks};
   // Split a previously confirmed bulk checkpoint without losing its other
   // explicit confirmations, so one new activity can be unchecked independently.
   if(typeof legacy==='string'&&checks[legacy]===true){for(const row of rows)if(row.action.params?.legacy_key===legacy&&!Object.hasOwn(checks,row.key))checks[row.key]=true;checks[legacy]=false;}
   progress={...progress,checks:{...checks,[key]:checked}};return write({run_id:progress.run_id,checks:progress.checks});
  },
  async start(key,started){
   if(!guide||pending)throw Error('Attendi il salvataggio in corso');
   progress={...progress,started:{...progress.started,[key]:started}};return write({run_id:progress.run_id,started:progress.started});
  },
  async newRun(){const old=progress;await write({});return old;},
  async restore(old){return write({run_id:old.run_id});},
  async refreshProgress(){if(!guide||pending)return;const current=sequence,saved=await json(endpoint('/api/guide/progress'));if(current===sequence){progress=saved;revision=saved.revision;publish();}},
 };
 window.hoiLive?.subscribe(value=>{
  live=value;
  if(value?.guide_server_id&&value.guide_server_id!==server){server=value.guide_server_id;revision=-1;}
  if(guide&&value?.guide_revision!==undefined&&value.guide_revision>=revision&&!pending){
   revision=value.guide_revision;progress={...progress,...value.guide_runs?.[guide.id],server_id:server,storage:value.guide_storage||progress.storage,checks:value.guide_states?.[guide.id]||(value.guide_id===guide.id?value.guide_checks:{} )||{},started:value.guide_started?.[guide.id]||{},baselines:value.guide_baselines?.[guide.id]||{}};
  }
  // Campaign changes and timeline rewinds are reconciled once by the server.
  // A browser tab never resets shared progress from a local comparison.
  publish();
 });
 (async()=>{try{const data=await json('/api/guides');catalog=data.guides;await select(new URLSearchParams(location.search).get('guide')||stored());}catch(e){error=e.message;publish();}})();
})();
