'use strict';
// Server jobs own preparation and publication; the page only shows progress.
(()=>{
 const $=id=>document.getElementById(id),t=(...args)=>HoiI18n.t(...args),model=HoiCollections;
 const node=(tag,text)=>{const n=document.createElement(tag);if(text!==undefined)n.textContent=text;return n;};
 const url=new URL(globalThis.location?.href||'http://local/importer');
 const page=url.searchParams.get('view')||(url.hash==='#collectionTemplatesHeading'?'references':'import');
 let collections=[],guides=[],busy=false,timer=null,live=null;
 const recent=new Set(),attempted=new Set();
 function failure(error){$('collectionError').hidden=false;$('collectionError').textContent=error.message||String(error);}
 async function request(url,body){const response=await fetch(url,body===undefined?undefined:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const result=await response.json();if(!response.ok)throw Error(result.error||t('Request failed'));return result;}
 function controls(){for(const id of ['guideZipFiles','templateZipFiles','advancedZipFiles'])$(id).disabled=busy;}
 async function run(task){if(busy)return;busy=true;controls();$('collectionError').hidden=true;try{await task();}catch(e){failure(e);}finally{busy=false;controls();render();schedule();}}
 function renderGuides(){
  if(page!=='library')return;
  const q=$('collectionSearch').value.trim().toLocaleLowerCase(),view=model.library(guides,collections).filter(g=>(g.title+' '+(g.applies_to?.country||'')).toLocaleLowerCase().includes(q));
  view.sort((a,b)=>Number(model.recommended(b,live))-Number(model.recommended(a,live))||a.title.localeCompare(b.title));$('collectionGuides').replaceChildren();
  $('collectionRecommendations').textContent=live?.usable===true?t('Current country: {0}. Matching guides appear first; your active guide is not changed.',live.sample.country):t('Load a campaign to see matching guides. You can browse your saved library at any time.');
  for(const guide of view){const card=node('article');card.className='collection-guide';card.append(node('h3',guide.title||guide.id),node('p',guide.applies_to?.country||t('Country not specified')));
   if(model.recommended(guide,live))card.append(node('strong',t('Matches your campaign')));
   if(guide.compatibility?.blocked){card.classList.add('guide-dlc-unavailable');card.append(node('p',HoiGuideAvailability.notice(guide.compatibility,t)));}
   else{const link=node('a',t('Open guide →'));link.href='/walkthrough?guide='+encodeURIComponent(guide.id);card.append(link);}$('collectionGuides').append(card);}
  if(!view.length)$('collectionGuides').append(node('p',guides.length?t('No guides match your search.'):t('No saved guides yet. Import a guide ZIP to get started.')));
 }
 function taskCard(collection){
  const c=model.counts(collection),card=node('article');card.className='import-job';card.append(node('h3',collection.name));
  const preparing=collection.status==='preparing',pending=model.pending(collection),failed=collection.status==='failed';
  const steps=node('ol');steps.className='import-steps';const current=failed?-1:collection.import_stage==='reading'?0:preparing?1:pending?2:3;
  for(const [i,label] of ['Read archive','Recognize and validate contents','Save valid contents','Install MIO presets'].entries()){
   const item=node('li');item.className='import-step '+(i<current?'done':i===current?'current':'waiting');item.append(node('span',t(label)));
   let detail=i<current?t('Done'):i===current?t('In progress'):t('Waiting');if(i===1&&preparing&&c.total)detail=t('{0} / {1} guides checked',c.processed||0,c.total);
   if(i===3&&!pending&&!failed){const mio=collection.mio_install;detail=model.waitingMio(collection)?t('Scheduled automatically when HOI4 closes'):mio?.status==='error'?t('Needs attention'):t('Done');
    if(!(collection.references?.mio_queues||0))detail=t('Not present in this archive');item.className='import-step '+(model.waitingMio(collection)?'waiting':mio?.status==='error'?'error':'done');}
   if(failed){item.className='import-step waiting';detail=t('Interrupted');}item.append(node('small',detail));steps.append(item);
  }card.append(steps);
  if(pending){const progress=node('progress');progress.setAttribute('aria-label',t('Import progress'));if(preparing&&c.total){progress.max=c.total;progress.value=c.processed||0;}card.append(progress);}
  if(!pending&&!failed){card.append(node('p',t('{0} guides saved · {1} designs · {2} images · {3} MIO presets',c.installed||0,collection.references?.designs||0,collection.references?.images||0,collection.references?.mio_queues||0)));
   if(c.dlc_unavailable)card.append(node('p',t('{0} guides require unavailable DLCs; they remain disabled in the library.',c.dlc_unavailable)));
   card.append(node('p',model.problems(collection)?t('Import finished with items to correct below. Valid contents are already saved.'):model.waitingMio(collection)?t('Contents saved. MIO installation is queued; no further import click is needed.'):t('Import complete. No further confirmation is needed.')));}
  return card;
 }
 function render(){
  const tasks=collections.filter(c=>recent.has(c.id)||model.pending(c)||model.waitingMio(c));$('collectionProgress').hidden=page!=='import'||!tasks.length;$('collectionJobs').replaceChildren(...tasks.map(taskCard));
  $('collections').replaceChildren();const problems=collections.filter(model.problems);$('collectionReview').hidden=page!=='import'||!problems.length;
  for(const collection of problems){const section=node('article');section.className='collection-review';section.append(node('h3',collection.name));
   if(collection.error||collection.references_error)section.append(node('p',collection.error||collection.references_error));
   if(collection.mio_install?.status==='error')section.append(node('p',t('MIO installation failed: {0}. Correct the problem and retry.',collection.mio_install.error)));
   if(collection.status==='failed'||collection.references_error||collection.mio_install?.status==='error'){const retry=node('button',t('Retry import'));retry.type='button';retry.disabled=busy;
    retry.onclick=()=>run(async()=>{recent.add(collection.id);attempted.delete(collection.id);if(collection.status==='failed'||collection.references_error)await request('/api/import/collection/retry',{id:collection.id});else await request('/api/import/collection/commit',{id:collection.id,automatic:true});await refresh();});section.append(retry);}
   for(const row of collection.items||[])if(row.status==='needs_revision'){const item=node('div');item.className='collection-row';const text=node('div');text.append(node('strong',row.title),node('p',(row.issues||[]).slice(0,2).join(' · ')));item.append(text);
    if(row.guide_id){const edit=node('button',t('Correct guide'));edit.type='button';edit.disabled=busy;edit.onclick=()=>run(async()=>{const report=await request('/api/import/collection/item?id='+collection.id+'&key='+row.key);$('advancedImport').open=true;await window.hoiImportEditor.review(report);});item.append(edit);}section.append(item);}
   $('collections').append(section);
  }
  if(!busy)$('collectionStatus').textContent=collections.some(model.pending)?t('Import continues in the background, even if you leave this page.'):t('Choose an archive to import. Saved contents do not need to be uploaded again.');renderGuides();
 }
 async function refresh(){
  collections=(await request('/api/import/collections')).collections;render();
  // Migrate already-prepared legacy jobs once; new jobs finish on the server.
  for(const collection of collections)if(model.legacyReady(collection)&&!attempted.has(collection.id)){
   attempted.add(collection.id);recent.add(collection.id);const stage=collection.import_stage;collection.import_stage='saving';render();
   try{await request('/api/import/collection/commit',{id:collection.id,automatic:true});collections=(await request('/api/import/collections')).collections;}
   catch(error){collection.import_stage=stage;attempted.delete(collection.id);throw error;}
  }
  if(page==='library')guides=(await request('/api/guides')).guides;render();
 }
 function schedule(){clearTimeout(timer);if(!document.hidden&&collections.some(c=>model.pending(c)||model.waitingMio(c)))timer=setTimeout(()=>run(refresh),collections.some(model.pending)?1500:5000);}
 async function upload(files){if(!files.length)return;await run(async()=>{
  for(const file of files){if(!/\.zip$/i.test(file.name)||!file.size||file.size>200000000)throw Error(t('Choose non-empty ZIP files up to 200 MB each.'));
   $('collectionStatus').textContent=t('Reading {0}…',file.name);$('collectionProgress').hidden=false;const pending=node('p',t('Uploading and checking {0}…',file.name));pending.setAttribute('role','status');$('collectionJobs').append(pending);
   const response=await fetch('/api/import/collections?automatic=1&name='+encodeURIComponent(file.name),{method:'POST',headers:{'Content-Type':'application/octet-stream'},body:file});const result=await response.json();if(!response.ok)throw Error(result.error||t('Request failed'));recent.add(result.id);
  }await refresh();
 });}
 for(const [inputId,dropId] of [['guideZipFiles','guideZipDrop'],['templateZipFiles','templateZipDrop'],['advancedZipFiles','advancedZipDrop']]){
  $(inputId).onchange=async()=>{const files=Array.from($(inputId).files);try{await upload(files);}finally{$(inputId).value='';}};$(dropId).addEventListener('dragover',e=>e.preventDefault());$(dropId).addEventListener('drop',e=>{e.preventDefault();if(!busy)return upload(Array.from(e.dataTransfer.files));});
 }
 $('collectionSearch').oninput=renderGuides;$('advancedImport').addEventListener('toggle',()=>{if($('advancedImport').open)window.hoiImportEditor.initialize().catch(failure);});
 $('collectionReferences').append($('showReferences'),$('referenceStatus'),$('referenceGallery'));
 $('collectionUpload').hidden=page!=='import';$('collectionLibrary').hidden=page!=='library';$('collectionTemplates').hidden=page!=='references';
 if(page!=='import'){$('collectionPageTitle').textContent=t(page==='library'?'Saved guides':'Templates and references');$('collectionIntro').hidden=true;$('collectionStatus').hidden=true;}
 window.hoiLive?.subscribe(value=>{live=value;renderGuides();});document.addEventListener('visibilitychange',()=>{if(document.hidden)clearTimeout(timer);else return run(refresh);});
 run(async()=>{await refresh();if(page==='references')await window.hoiImportEditor.refreshReferences();});
})();
