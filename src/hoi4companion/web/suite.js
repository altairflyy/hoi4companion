'use strict';
(()=>{
 const model=HoiSuiteModel,t=HoiI18n.t,$=id=>document.getElementById(id);
 window.HoiSuiteLive=window.hoiLive;
 for(const el of document.querySelectorAll('[data-copy]'))el.textContent=t(el.dataset.copy);
 const titles={overview:'Command center',library:'Guide library',walkthrough:'Campaign plan',focus:'National focuses',technologies:'Technologies',country:'Country overview',events:'Events',templates:'Templates and references',importer:'Import ZIPs','saved-guides':'Saved guides',analysis:'Diagnostics',settings:'Settings'};
 let view='overview',catalog=[],catalogLoaded=false,packet=null,active=null,projection=null,preview=null,projectionKey='',projectionVersion=0,loading=null,queued=false,librarySignature='',libraryLoading=null;
 const format=new Intl.NumberFormat(HoiI18n.language==='italian'?'it-IT':'en-US',{maximumFractionDigits:0});
 const display=value=>value===null||value===undefined?'—':format.format(value);
 const foreground=()=>document.visibilityState!=='hidden'&&(typeof document.hasFocus!=='function'||document.hasFocus());
 function frameActivity(){window.HoiSuiteFrameActive=!!model.routes[view]&&foreground();try{$('workspaceFrame').contentWindow?.dispatchEvent(new Event('suiteviewchange'));}catch{}}
 function error(message=''){$('suiteError').textContent=message;$('suiteError').hidden=!message;}
 async function json(path,options){const response=await fetch(path,options),data=await response.json();if(!response.ok)throw Error(data.error||'HTTP '+response.status);return data;}
 function route(key){if(!Object.hasOwn(model.routes,key))key='overview';const previous=view;if(previous!==key){projectionVersion++;queued=false;}view=key;
  if(previous==='settings'&&view!=='settings')$('workspaceFrame').contentDocument?.getElementById('settingsModal')?.close();
  frameActivity();
  for(const button of document.querySelectorAll('.suite-sidebar [data-view]')){if(button.dataset.view===key)button.setAttribute('aria-current','page');else button.removeAttribute('aria-current');}
  $('viewTitle').textContent=t(titles[key]);$('overviewView').hidden=key!=='overview';$('libraryView').hidden=key!=='library';$('frameView').hidden=!model.routes[key];
  if(model.routes[key]){const url=new URL(model.routes[key],location.origin);url.searchParams.set('suite','1');if($('workspaceFrame').src!==url.href){$('frameStatus').textContent=t('Loading workspace…');$('workspaceFrame').src=url.href;}else if(key==='settings')$('workspaceFrame').contentDocument?.getElementById('navSettings')?.click();}
  if(key==='library')loadLibrary();
  if(key==='overview')refreshProjection();
 }
 function navigate(key){if(!Object.hasOwn(model.routes,key))key='overview';if(location.hash.slice(1)===key)route(key);else location.hash=key;}
 document.addEventListener('click',event=>{const button=event.target.closest('[data-view]');if(button){event.preventDefault();navigate(button.dataset.view);}});
 window.addEventListener('hashchange',()=>route(location.hash.slice(1)));
 $('workspaceFrame').addEventListener('load',()=>{
  const frame=$('workspaceFrame');try{const doc=frame.contentDocument;if(!doc||frame.contentWindow.location.origin!==location.origin)return;
   doc.documentElement.classList.add('suite-embedded');if(!doc.querySelector('link[data-suite-style]')){const style=doc.createElement('link');style.rel='stylesheet';style.href='/suite-embedded.css';style.dataset.suiteStyle='true';doc.head.append(style);}
   // Retain existing graph/search/guide behavior. Only duplicate navigation is hidden.
   $('frameStatus').textContent='';
   doc.querySelector('.collection-links')?.addEventListener('click',event=>{
    const link=event.target.closest('a[data-suite-view]');
    if(!link||event.defaultPrevented||event.button!==0||event.altKey||event.ctrlKey||event.metaKey||event.shiftKey)return;
    event.preventDefault();navigate(link.dataset.suiteView);
   });
   doc.getElementById('settingsModal')?.addEventListener('close',()=>{
    if(view==='settings')document.querySelector('.suite-sidebar [data-view="settings"]').focus();
   });
   if(view==='settings')doc.getElementById('navSettings')?.click();
  }catch{$('frameStatus').textContent=t('Workspace unavailable. Open the classic workspace to continue.');}
 });
 function drawLibrary(){const tag=packet?.usable?packet.sample?.country:null,all=$('allCountries').checked;
  const groups=model.libraryGroups(catalog,tag,all,$('guideSearch').value),count=groups.nations.reduce((n,g)=>n+g.guides.length,groups.other.length);$('libraryCount').textContent=format.format(catalog.length);$('guideCards').replaceChildren();
  $('libraryNotice').textContent=!catalog.length?t('Import a collection to populate your library.'):!count?t('No guides match this view.'):tag&&!all?t('Recommended for your observed country. Missing DLCs remain visible and unavailable.'):'—';
  const card=({guide,duplicateOf,sameTitle})=>{const card=document.createElement('article');card.className='guide-card';card.dataset.blocked=String(guide.compatibility?.blocked===true);card.dataset.active=String(guide.id===active?.guide_id);card.dataset.duplicate=String(!!duplicateOf);
   const country=document.createElement('span');country.className='guide-country';country.textContent=guide.applies_to?.country||t('Not a national guide');const title=document.createElement('h3');title.textContent=model.title(guide.title);
   const info=document.createElement('p');info.textContent=guide.compatibility?.blocked?t('Missing DLC'):!model.selectable(guide,tag)?t('Different country'):guide.id===active?.guide_id?t('Active guide'):t('Guide ready');
   const parts=[country,title,info];
   if(duplicateOf||sameTitle){const flag=document.createElement('p');flag.className='guide-flag';flag.textContent=duplicateOf?t('Duplicate: same content as “{0}”. Nothing is removed.',model.title(duplicateOf.title)):t('Same title as another guide, different content: check the version before choosing.');parts.push(flag);}
   const button=document.createElement('button');button.type='button';button.textContent=t(guide.id===active?.guide_id?'Continue campaign plan':'Use this guide');button.disabled=!model.selectable(guide,tag);button.addEventListener('click',async()=>{button.disabled=true;try{error();await json('/api/guide/active',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({guide_id:guide.id})});active=await json('/api/guide/active');projectionKey='';await refreshProjection();navigate('walkthrough');}catch(e){error(e.message);button.disabled=false;}});
   card.append(...parts,button);return card;};
  const section=(heading,note,rows)=>{const group=document.createElement('section');group.className='library-group';const h=document.createElement('h2');h.textContent=heading;const small=document.createElement('small');small.textContent=note;h.append(small);
   const cards=document.createElement('div');cards.className='guide-cards';cards.append(...rows.map(card));group.append(h,cards);$('guideCards').append(group);};
  for(const nation of groups.nations)section(nation.country,format.format(nation.guides.length),nation.guides);
  if(groups.other.length)section(t('Other documents'),t('Not national guides: videos, notes, instructions.'),groups.other);
 }
 function loadLibrary(){if(libraryLoading)return libraryLoading;libraryLoading=(async()=>{try{const data=await json('/api/guides');catalog=data.guides||[];catalogLoaded=true;drawLibrary();if(active)drawRoute();}catch(e){error(e.message);}})().finally(()=>libraryLoading=null);return libraryLoading;}
 function drawRoute(){const name=active?.title||catalog.find(g=>g.id===active?.guide_id)?.title;
  $('activeGuideTitle').textContent=name?model.title(name):t('Choose a guide to begin');$('activeGuideShort').textContent=name?model.title(name):t('No guide selected');$('routeItems').replaceChildren();
  const current=model.currentProjection(packet,projection);
  const groups=!current&&!packet?.usable?model.preview(preview,active?.guide_id):null;
  if(groups?.length){drawPreview(groups);return;}
  if(!current){$('routeStatus').textContent=t('Waiting for current game data');$('routeNotice').textContent=t(packet?.usable?'Guide progress is unavailable; no steps are inferred.':'Open HOI4 to qualify the current campaign. Your library stays available.');return;}
  const summary=HoiOperationalSummary.project(projection.guide,projection.evaluation,projection.state,{limit:4});
  $('routeStatus').textContent=t(projection.evaluation.mode==='live'?'Current campaign':'Guide ready');$('routeNotice').textContent=summary.notice||t('Readiness follows the guide conditions; unknown data never completes a step.');
  const definitions=new Map(Object.values(projection.guide.tracks||{}).flat().map(step=>[step.key,step]));
  for(const item of summary.items){const step=definitions.get(item.step_key);routeItem(item.href||'/walkthrough',item.title,step?.name||step?.title||item.guide_goal||item.title,item.message||'');}
 }
 function routeItem(href,label,name,note){const path=new URL(href,location.origin);if(path.origin!==location.origin)return;const link=document.createElement('a');link.className='route-item';link.href=path.pathname+path.search+path.hash;
  link.addEventListener('click',event=>{event.preventDefault();const key=path.pathname==='/technologies'?'technologies':path.pathname==='/'?'focus':'walkthrough';navigate(key);const target=new URL(link.href,location.origin);target.searchParams.set('suite','1');$('workspaceFrame').src=target.href;});
  const dot=document.createElement('i');dot.setAttribute('aria-hidden','true');const text=document.createElement('div');const category=document.createElement('small');category.textContent=label;const title=document.createElement('strong');title.textContent=name;const message=document.createElement('p');message.textContent=note;text.append(category,title,message);const arrow=document.createElement('span');arrow.textContent='→';arrow.setAttribute('aria-hidden','true');link.append(dot,text,arrow);$('routeItems').append(link);
 }
 // UX R11: without the game the home previews the first steps of the active
 // guide with the statuses of the Rust preview evaluation, never a JS verdict.
 const previewStatus={ready:'Ready',active:'In progress',waiting:'Waiting',unverifiable:'To check in game'};
 function drawPreview(groups){$('routeStatus').textContent=t('Preview without the game');$('routeNotice').textContent=t('Preview: the first steps of the active guide. Statuses come from the guide engine without game data; no step counts as done until HOI4 is open.');
  for(const group of groups)for(const item of group.items){const step=item.step;
   const name=window.HoiActionPlan?.composedTitle(step.reading)||step.name||step.title||item.key;
   const href=item.track==='research'?'/technologies':item.track==='focus'?(typeof step.href==='string'&&step.href.startsWith('/')&&!step.href.startsWith('//')?step.href:'/'):'/walkthrough#operation-'+encodeURIComponent(item.key);
   routeItem(href,t(group.label)+' · '+t(previewStatus[item.status]),name,HoiI18n.reason?.(item.reason)||'');}
 }
 async function refreshProjection(){if(view!=='overview'||!foreground())return;if(loading){queued=true;return loading;}const version=++projectionVersion;
  loading=(async()=>{try{const identity=await json('/api/guide/active');if(version!==projectionVersion)return;active=identity;
    if(!identity.guide_id){projection=null;preview=null;drawRoute();return;}
    if(!packet?.usable){projection=null;let result=null;try{result=await json('/api/guide/projection?id='+encodeURIComponent(identity.guide_id)+'&live=0');}catch{}if(version!==projectionVersion)return;
     preview=result?.schema_version===1?result:null;drawRoute();return;}
    preview=null;
    const result=await json('/api/guide/projection?id='+encodeURIComponent(identity.guide_id)+'&live='+(packet?.usable?'1':'0'));
    if(version!==projectionVersion||packet?.active_guide_id&&result.guide_id!==packet.active_guide_id)return;
    if(result.schema_version!==1||result.engine!=='rust-guide-core'||result.guide_id!==identity.guide_id||result.evaluation?.guide_id!==identity.guide_id||result.progress?.guide_id!==identity.guide_id)throw Error(t('Guide progress is unavailable; no steps are inferred.'));
    projection=result;drawRoute();if(view==='library')drawLibrary();
   }catch(e){projection=null;preview=null;drawRoute();if(e.message==='No guide available'){if(!catalogLoaded)await loadLibrary();if(catalogLoaded&&!catalog.length)return;}error(e.message);}
  })().finally(()=>{loading=null;if(queued){queued=false;refreshProjection();}});return loading;
 }
 window.hoiLive.subscribe(value=>{packet=value;if(value?.active_guide_id&&value.active_guide_id!==active?.guide_id){projectionVersion++;projection=null;preview=null;active={guide_id:value.active_guide_id,title:catalog.find(g=>g.id===value.active_guide_id)?.title};drawRoute();}const m=model.metrics(value);$('countryTag').textContent=m.country||'—';$('metricCountry').textContent=m.country||'—';$('metricDivisions').textContent=display(m.divisions);$('metricPP').textContent=display(m.politicalPower);$('metricFactories').textContent=display(m.factories);$('readerBadge').dataset.connected=String(m.current);$('readerText').textContent=t(m.current?'Reader connected':'Waiting for HOI4');$('campaignHint').textContent=t(m.current?'Current campaign':'No qualified game state');$('hudStatus').textContent=value?.usable?(value.overlay?.message||t('No new contextual indication')):t('Waiting for current game data');
  const signature=JSON.stringify([value?.active_guide_id,m.country,value?.active_guide_revision]);if(signature!==librarySignature){librarySignature=signature;drawLibrary();}
  const key=model.projectionKey(value);
  if(!value?.usable){projection=null;drawRoute();}if(key!==projectionKey){projectionKey=key;refreshProjection();}
 });
 $('guideSearch').addEventListener('input',drawLibrary);$('allCountries').addEventListener('change',drawLibrary);
 window.addEventListener('pageshow',event=>{if(event.persisted&&document.visibilityState!=='hidden')loadLibrary();});
 function activity(){frameActivity();if(!foreground()){projectionVersion++;queued=false;}else if(view==='overview')refreshProjection();}
 document.addEventListener('visibilitychange',activity);window.addEventListener('blur',activity);window.addEventListener('focus',activity);
 route(location.hash.slice(1)||'overview');loadLibrary();if(!loading)refreshProjection();
})();
