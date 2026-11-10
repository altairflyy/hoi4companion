'use strict';
// Collection orchestration only. The existing importer owns compilation/editing.
(()=>{
 const $=id=>document.getElementById(id),t=(...args)=>HoiI18n.t(...args),model=HoiCollections;
 const node=(tag,text)=>{const n=document.createElement(tag);if(text!==undefined)n.textContent=text;return n;};
 let collections=[],guides=[],busy=false,timer=null,live=null;
 const expanded=new Map();
 const labels={pending:'Preparing',ready:'Ready to add',installed:'In your library',dlc_unavailable:'DLC unavailable',needs_revision:'Needs correction'};
 function failure(error){$('collectionError').hidden=false;$('collectionError').textContent=error.message||String(error);}
 async function request(url,body){const response=await fetch(url,body===undefined?undefined:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const result=await response.json();if(!response.ok)throw Error(result.error||t('Request failed'));return result;}
 function controls(){for(const id of ['guideZipFiles','templateZipFiles','collectionRefresh'])$(id).disabled=busy;if(busy)$('collectionAdd').disabled=true;}
 async function run(task){if(busy)return;busy=true;controls();$('collectionError').hidden=true;try{await task();}catch(e){failure(e);}finally{busy=false;controls();render();schedule();}}
 function stage(id){for(const key of ['stageUpload','stageReview','stageLibrary']){if(key===id)$(key).setAttribute('aria-current','step');else $(key).removeAttribute('aria-current');}}
 function renderGuides(){
  const q=$('collectionSearch').value.trim().toLocaleLowerCase(),view=model.library(guides,collections).filter(g=>(g.title+' '+(g.applies_to?.country||'')).toLocaleLowerCase().includes(q));
  view.sort((a,b)=>Number(model.recommended(b,live))-Number(model.recommended(a,live))||a.title.localeCompare(b.title));
  $('collectionGuides').replaceChildren();
  $('collectionRecommendations').textContent=live?.usable===true?t('Current country: {0}. Matching guides appear first; your active guide is not changed.',live.sample.country):t('Load a campaign to see matching guides. You can browse your saved library at any time.');
  for(const guide of view){
   const card=node('article');card.className='collection-guide';
   card.append(node('h3',guide.title||guide.id));
   card.append(node('p',guide.applies_to?.country||t('Country not specified')));
   if(model.recommended(guide,live))card.append(node('strong',t('Matches your campaign')));
   if(guide.compatibility?.blocked){card.classList.add('guide-dlc-unavailable');card.append(node('p',HoiGuideAvailability.notice(guide.compatibility,t)));}
   else {const link=node('a',t('Open guide →'));link.href='/walkthrough?guide='+encodeURIComponent(guide.id);card.append(link);}
   $('collectionGuides').append(card);
  }
  if(!view.length)$('collectionGuides').append(node('p',guides.length?t('No guides match your search.'):t('Your saved guides will appear here after review.')));
 }
 function render(){
  $('collections').replaceChildren();
  for(const collection of collections){
   const section=node('article');section.className='collection-review';
   section.append(node('h3',collection.name));
   const c=model.counts(collection);
   const status=node('p',t('{0} / {1} guides prepared · {2} ready · {3} saved · {4} DLC unavailable · {5} need correction',c.processed||0,c.total||0,c.ready||0,c.installed||0,c.dlc_unavailable||0,c.needs_revision||0));status.setAttribute('role','status');section.append(status);
   if(collection.references)section.append(node('p',t('{0} equipment designs · {1} reference images',collection.references.designs,collection.references.images)));
   if(collection.error||collection.references_error)section.append(node('p',collection.error||collection.references_error));
   if(collection.status==='failed'){const retry=node('button',t('Resume preparation'));retry.type='button';retry.disabled=busy;retry.onclick=()=>run(async()=>{await request('/api/import/collection/retry',{id:collection.id});await refresh();});section.append(retry);}
   if(collection.status==='preparing'){const progress=node('progress');progress.max=c.total||1;progress.value=c.processed||0;progress.setAttribute('aria-label',t('Collection preparation'));section.append(progress);}
   const details=node('details'),count=c.needs_revision||0;
   details.append(node('summary',count?t('Review {0} guides that need correction',count):t('View guides and compatibility')));details.open=expanded.get(collection.id)??(count>0&&collections.length===1&&c.total<10);details.addEventListener('toggle',()=>expanded.set(collection.id,details.open));
   for(const row of collection.items||[]){
    const item=node('div');item.className='collection-row';if(row.status==='dlc_unavailable')item.classList.add('guide-dlc-unavailable');
    const text=node('div');text.append(node('strong',row.title),node('span',t(labels[row.status]||row.status)));
    if(row.status==='dlc_unavailable')text.append(node('p',HoiGuideAvailability.notice(row.compatibility,t)));
    if(row.status==='needs_revision')text.append(node('p',(row.issues||[]).slice(0,2).join(' · ')));
    item.append(text);
    if(row.status==='needs_revision'&&row.guide_id){const edit=node('button',t('Correct guide'));edit.type='button';edit.disabled=busy;edit.onclick=()=>run(async()=>{
     const report=await request('/api/import/collection/item?id='+collection.id+'&key='+row.key);
     $('advancedImport').open=true;await window.hoiImportEditor.review(report);
    });item.append(edit);}
    details.append(item);
   }
   section.append(details);$('collections').append(section);
  }
  if(!collections.length)$('collections').append(node('p',t('Choose your ZIP archives above. No need to select each guide separately.')));
  $('collectionAdd').disabled=busy||collections.some(c=>c.status==='preparing')||!collections.some(model.canAdd);
  stage(collections.some(c=>c.status==='preparing'||model.canAdd(c))?'stageReview':guides.length||collections.some(c=>c.reviewed)?'stageLibrary':'stageUpload');
  renderGuides();
 }
 async function refresh(includeLibrary=false){
  const result=await request('/api/import/collections');collections=result.collections;
  if(includeLibrary)guides=(await request('/api/guides')).guides;
  render();
 }
 function schedule(){clearTimeout(timer);if(!document.hidden&&collections.some(c=>c.status==='preparing'))timer=setTimeout(async()=>{try{await refresh();}catch(e){failure(e);}schedule();},1500);}
 async function upload(files){await run(async()=>{
  try{for(const file of files){
   if(!/\.zip$/i.test(file.name)||!file.size||file.size>200000000)throw Error(t('Choose non-empty ZIP files up to 200 MB each.'));
   $('collectionStatus').textContent=t('Storing {0}…',file.name);
   const response=await fetch('/api/import/collections?name='+encodeURIComponent(file.name),{method:'POST',headers:{'Content-Type':'application/octet-stream'},body:file});
   const result=await response.json();if(!response.ok)throw Error(result.error||t('Request failed'));
  }}catch(error){await refresh();throw error;}
  await refresh();$('collectionStatus').textContent=t('Preparing your collection in the background. You can leave this page; the archives are already saved.');stage('stageReview');
 });}
 $('collectionAdd').onclick=()=>run(async()=>{
  $('collectionStatus').textContent=t('Adding reviewed content to your local library…');
  for(const collection of [...collections].sort((a,b)=>Number(b.references_status==='ready')-Number(a.references_status==='ready')))if(model.canAdd(collection))await request('/api/import/collection/commit',{id:collection.id,reviewed:true});
  await refresh(true);$('collectionStatus').textContent=t('Library saved. You can use it without uploading the ZIP again.');stage('stageLibrary');
 });
 for(const [inputId,dropId] of [['guideZipFiles','guideZipDrop'],['templateZipFiles','templateZipDrop']]){
  $(inputId).onchange=()=>upload(Array.from($(inputId).files));
  $(dropId).addEventListener('dragover',e=>{e.preventDefault();});
  $(dropId).addEventListener('drop',e=>{e.preventDefault();if(!busy)return upload(Array.from(e.dataTransfer.files));});
 }
 $('collectionRefresh').onclick=()=>run(async()=>{await refresh(true);});
 $('collectionSearch').oninput=renderGuides;
 $('advancedImport').addEventListener('toggle',()=>{if($('advancedImport').open)window.hoiImportEditor.initialize().catch(failure);});
 $('collectionReferences').append($('showReferences'),$('referenceStatus'),$('referenceGallery'));
 window.hoiLive?.subscribe(value=>{live=value;renderGuides();});
 document.addEventListener('visibilitychange',()=>{if(document.hidden)clearTimeout(timer);else run(async()=>{await refresh(true);});});
 run(async()=>{await refresh(true);$('collectionStatus').textContent=t('Your library is stored on this computer.');});
})();
