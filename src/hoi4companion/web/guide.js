'use strict';
(()=>{
 const $=id=>document.getElementById(id),el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
 const categories={all:HoiI18n.t("All"),construction:HoiI18n.t("Factories and infrastructure"),production:HoiI18n.t("Production"),projects:HoiI18n.t("Special projects"),politics:HoiI18n.t("Politics and laws"),characters:HoiI18n.t("Characters"),army:HoiI18n.t("Armed forces"),templates:HoiI18n.t("Templates and vehicles"),checkpoints:HoiI18n.t("Events and conditions")};
 let value=null,filter='all',undo=null,lastGuide=null,dependencyRows=new Map(),unplacedOperations=new Set();
 const cardCache=new Map(),blockCache=new Map();let filterKey='',summaryKey='';
 function sync(parent,nodes){
  if(!parent.childNodes||!parent.insertBefore){parent.replaceChildren(...nodes);return;}
  nodes.forEach((node,i)=>{if(parent.childNodes[i]!==node)parent.insertBefore(node,parent.childNodes[i]||null);});
  while(parent.childNodes.length>nodes.length)parent.lastChild.remove();
 }
 function revealPlan(){filter='all';$('hideDone').checked=false;$('operationsPlan').open=true;render(value);}
 const guideReferences=typeof HoiGuideReferences==='undefined'?null:HoiGuideReferences.create({document,t:message=>HoiI18n.t(message)});
 function referenceText(tag,cls,text,row){const node=el(tag,cls);if(guideReferences)guideReferences.append(node,text,row);else node.textContent=text;return node;}
 function dependencyLink(target){const a=el('a','',target.label);a.href=target.href.startsWith('#')?target.href:window.hoiGuide.withGuide(target.href);if(target.href.startsWith('#'))a.onclick=revealPlan;return a;}
 const expandedFrames=new Set(),frameCache=new Map(),mediaReferences=new Map(),expandedDependencies=new Set();
 let visualReferenceSteps=new Set();
 function dependencyItems(keys,row){
  const list=el('ul','summary-dependencies');
  for(const key of keys){const target=dependencyRows.get(key),li=el('li');if(target){const ref=HoiOperationalSummary.reference(target);ref.label+=(row.depends_on_started||[]).includes(key)?HoiI18n.t(" (start)"):HoiI18n.t(" (completion)");li.append(dependencyLink(ref));}else li.append(el('span','',HoiI18n.t("Unresolved step · ")+key));list.append(li);}
  return list;
 }
 function dependencyList(row,node){
  const keys=row.blocked_by;if(!keys?.length)return;
  node.append(el('p','operation-wait',['dependency_unknown','milestone_unknown'].includes(row.runtime_reason)?HoiI18n.t("Prerequisites to verify:"):HoiI18n.t("Waiting for prerequisites:")),dependencyItems(keys.slice(0,3),row));
  if(keys.length<=3)return;
  const more=el('details','operation-dependency-more'),id=value.guide.id+':'+row.key;
  more.append(el('summary','',HoiI18n.t("{0} more prerequisites",keys.length-3)));
  let rendered=false;
  const reveal=()=>{if(more.open){expandedDependencies.add(id);if(!rendered){more.append(dependencyItems(keys.slice(3),row));rendered=true;}}else expandedDependencies.delete(id);};
  more.ontoggle=reveal;more.open=expandedDependencies.has(id);if(more.open)reveal();node.append(more);
 }
 function sourceFrame(row){
  const id=value.guide.id,reference=row.frame_reference,key=JSON.stringify([id,row.key,reference?.url||'',reference?.seconds??null]),details=el('details','operation-frame');
  details.append(el('summary','',reference?HoiI18n.t("Show guide frame · {0}",clock(reference.seconds)):HoiI18n.t("Show source references")));let displayed=false;
  details.ontoggle=async()=>{
   if(!details.open){expandedFrames.delete(key);return;}expandedFrames.add(key);if(displayed)return;displayed=true;
   const caption=el('p','',HoiI18n.t("Source frame, not the current game.")),imageArea=el('div','operation-image-area'),list=el('ul','operation-media-references');details.append(caption,imageArea,list);
   let requested=0,defaultReference=null,frameLoaded=false;
   const describe=ref=>{const basis=ref?.basis==='visual'?HoiI18n.t("Declared visual observation"):ref?.basis==='declared'?HoiI18n.t("Frame specified by the guide"):ref?.basis==='explicit'?HoiI18n.t("Spoken reference · image not verified"):HoiI18n.t("Source reference · image not verified");return basis+(ref?' · '+clock(ref.seconds):'')+' · '+HoiI18n.t("Source frame, not the current game.");};
   const showFrame=async ref=>{
    const current=++requested,frameKey=JSON.stringify([id,row.key,ref?.id||'',ref?.url||reference?.url||'',ref?.seconds??reference?.seconds??null]);
    const args='id='+encodeURIComponent(id)+'&key='+encodeURIComponent(row.key)+(ref?'&ref='+encodeURIComponent(ref.id):'');
    try{
     if(!frameCache.has(frameKey))frameCache.set(frameKey,(async()=>{const response=await fetch('/api/guide/frame?'+args);if(!response.ok){let error;try{error=await response.json();}catch{}throw Error(error?.error||HoiI18n.t("Source references are unavailable; the original source link remains available."));}return URL.createObjectURL(await response.blob());})());
     const url=await frameCache.get(frameKey);if(current!==requested)return;
     const img=el('img'),link=el('a','operation-image-link',HoiI18n.t("Open frame at full size"));img.alt=HoiI18n.t("Source visual reference for ")+name(row);img.src=url;link.href=url;link.target='_blank';link.rel='noopener noreferrer';imageArea.replaceChildren(img,link);frameLoaded=true;caption.textContent=describe(ref||defaultReference);
    }catch(e){frameCache.delete(frameKey);if(current===requested){frameLoaded=false;imageArea.replaceChildren();caption.textContent=e.message;}}
   };
   const referencesKey=JSON.stringify([id,row.key,reference,row.evidence,(value.guide.source_inventory?.items||[]).filter(item=>item.step_keys?.includes(row.key))]);
   const loadReferences=async()=>{
    try{
     if(!mediaReferences.has(referencesKey))mediaReferences.set(referencesKey,(async()=>{const response=await fetch('/api/guide/media?id='+encodeURIComponent(id)+'&key='+encodeURIComponent(row.key));if(!response.ok)throw Error('media unavailable');return response.json();})());
     const report=await mediaReferences.get(referencesKey);
     if(!Array.isArray(report?.references))throw Error('invalid media references');
     defaultReference=report.references.find(ref=>ref.id===report.default_reference)||null;if(frameLoaded&&requested===1)caption.textContent=describe(defaultReference);
     for(const ref of report.references){
      if(!ref||typeof ref.id!=='string'||!Number.isFinite(ref.seconds)||ref.seconds<0)continue;
      const item=el('li'),basis=ref.basis==='visual'?HoiI18n.t("Declared visual observation"):ref.basis==='declared'?HoiI18n.t("Frame specified by the guide"):ref.basis==='explicit'?HoiI18n.t("Spoken reference · image not verified"):HoiI18n.t("Source reference · image not verified");
      item.append(el('p','',basis+' · '+clock(ref.seconds)+(ref.source_title?' · '+ref.source_title:'')));
      const actions=el('div','operation-media-actions'),show=el('button','operation-show-frame',HoiI18n.t("Show this frame")),clip=el('button','operation-watch-clip',HoiI18n.t("Watch context clip"));show.dataset.reference=clip.dataset.reference=ref.id;show.type=clip.type='button';show.onclick=()=>showFrame(ref);clip.onclick=()=>openClip(id,row,ref);actions.append(show,clip);
      const url=ref.url?sourceLink(ref.seconds,ref.url):null;if(url){const link=el('a','',HoiI18n.t("Open the original video ↗"));link.href=url;link.target='_blank';link.rel='noopener noreferrer';actions.append(link);}item.append(actions);list.append(item);
     }
     if(report.truncated)list.append(el('li','',HoiI18n.t("Some source references are omitted; refine the guide to inspect the rest.")));
    }catch{mediaReferences.delete(referencesKey);list.append(el('li','',HoiI18n.t("Source references are unavailable; the original source link remains available.")));}
   };
   await Promise.all([reference?showFrame(null):Promise.resolve(),loadReferences()]);
  };
  details.open=expandedFrames.has(key);return details;
 }
 function closeClip(){const video=$('sourceMediaVideo');video.pause();video.removeAttribute('src');video.load();}
 function openClip(id,row,ref){
  const dialog=$('sourceMediaDialog'),video=$('sourceMediaVideo'),original=$('sourceMediaOriginal');
  video.pause();video.removeAttribute('src');video.load();
  $('sourceMediaTitle').textContent=HoiI18n.t("Context clip · {0}",name(row));
  $('sourceMediaDescription').textContent=HoiI18n.t("The clip is contextual and may not show the action. The reference occurs at {0} in the original video.",clock(ref.seconds));
  $('sourceMediaStatus').textContent=HoiI18n.t("Preparing the local video clip…");
  const href=ref.url?sourceLink(ref.seconds,ref.url):null;original.hidden=!href;if(href)original.href=href;
  video.onerror=()=>{$('sourceMediaStatus').textContent=HoiI18n.t("The local clip is not available. Open the original video or prepare it in Import guide.");};
  video.onloadedmetadata=()=>{$('sourceMediaStatus').textContent='';};
  video.src='/api/guide/clip?id='+encodeURIComponent(id)+'&key='+encodeURIComponent(row.key)+'&ref='+encodeURIComponent(ref.id);
  video.preload='metadata';video.controls=true;if(!dialog.open)dialog.showModal();
 }
 $('sourceMediaClose').onclick=()=>$('sourceMediaDialog').close();
 $('sourceMediaDialog').onclose=closeClip;
 if(location.pathname==='/hud'){document.body.classList.add('hud-view');const token=new URLSearchParams(location.search).get('instance')||'';document.title='HOI4 Companion HUD '+(/^[a-f0-9]{32}$/.test(token)?token:'');}
 const sourceLink=(seconds,referenceUrl=null)=>{try{const url=new URL(referenceUrl||value.guide.source?.url||value.guide.sources?.[0]?.url);if(!['http:','https:'].includes(url.protocol))return null;if(Number.isFinite(seconds))url.searchParams.set('t',String(Math.floor(seconds)));return url.href;}catch{return null;}};
 const clock=seconds=>Math.floor(seconds/60)+':'+String(Math.floor(seconds%60)).padStart(2,'0');
 const name=row=>row.name||row.title||row.key;
 function badge(row){if(row.runtime_reason==='production_superseded')return HoiI18n.t('Superseded');if(row.tracking==='reminder')return row.runtime_status==='waiting'?HoiI18n.t("Reminder · upcoming"):row.runtime_status==='unverifiable'?HoiI18n.t("Reminder · timing needs verification"):HoiI18n.t("Reminder");return row.runtime_status==='completed'?HoiI18n.t("Completed"):row.runtime_status==='not_applicable'?HoiI18n.t("Not applicable"):row.runtime_status==='waiting'?HoiI18n.t("Upcoming"):row.runtime_status==='active'?(row.start_basis==='live'?HoiI18n.t("In progress · start detected live"):row.start_basis==='condition'?HoiI18n.t("In progress · criterion verified"):HoiI18n.t("Started · in progress (manual)")):row.started_when?(row.selection?.value===false?HoiI18n.t("No order detected"):HoiI18n.t("Start cannot be verified")):row.runtime_reason==='manual_confirmation_required'||row.runtime_status==='ready'?HoiI18n.t("Check in the game"):HoiI18n.t("Timing needs verification");}
 function confirm(key,text){const label=el('label','checkpoint-check'),input=el('input');input.type='checkbox';input.checked=value.progress.checks[key]===true;input.disabled=value.pending>0;input.setAttribute('data-check',key);input.addEventListener('change',()=>window.hoiGuide.confirm(key,input.checked).catch(()=>{}));label.append(input,el('span','',text));return label;}
 function controls(row,box){
  if(row.superseded_by?.length){box.append(el('p','operation-copy',HoiI18n.t('Superseded targets are no longer requested; this does not prove the earlier setup was completed.')),dependencyItems(row.superseded_by,row));if(row.runtime_reason==='production_superseded')return;}
  if(row.tracking==='reminder'){box.append(el('p','operation-copy',HoiI18n.t("Method reminder: no confirmation required; it does not block other steps.")));return;}
  if(row.started_when&&row.runtime_status!=='completed'){
   box.append(el('p','operation-start-note',row.runtime_status==='active'?HoiI18n.t("Start detected using the guide's condition. Starting alone does not prove completion."):HoiI18n.t("Automatic start detection: waiting for the guide's condition to be met.")));
  }
  if(['manual','special_project'].includes(row.action.kind)&&!row.started_when&&row.action_readiness?.domain!=='political'&&row.selection&&row.runtime_status!=='completed'){
   const started=value.progress.started?.[row.key]===true,button=el('button','operation-start',started?HoiI18n.t("Undo reported start"):HoiI18n.t("I issued the order"));
   button.type='button';button.disabled=value.pending>0;button.setAttribute('data-start',row.key);button.setAttribute('aria-pressed',String(started));button.onclick=()=>window.hoiGuide.start(row.key,!started).catch(()=>{});box.append(button);
   if(row.runtime_status==='active')box.append(el('p','operation-start-note',HoiI18n.t("Order reported manually; completion still needs verification. Starting alone does not release completion dependencies.")));
  }
  if(['manual','special_project'].includes(row.action.kind)&&!row.completion_condition)box.append(confirm(row.key,HoiI18n.t("Completed · ")+(row.confirmation||HoiI18n.t("final criterion verified in the game"))));
  const phase=value.guide.phases?.find(p=>p.id===row.phase);
  const keys=new Set([row.when,row.priority_when,row.skip_when,row.started_when,phase?.when].flatMap(node=>HoiContracts.checkKeys(node)));
  if(row.completion_condition)for(const key of HoiContracts.checkKeys(row.completion_condition))if(key!==row.action.params?.legacy_key)keys.add(key);
  const finalKeys=new Set(HoiContracts.checkKeys(row.completion_condition));
  for(const key of keys)box.append(confirm(key,finalKeys.has(key)?HoiI18n.t("Final criterion verified in the game"):key===row.key?HoiI18n.t("Completed · ")+(row.confirmation||HoiI18n.t("final criterion verified in the game")):HoiI18n.t("Condition verified in the game")));
 }
 function cachedCard(row,highlight=false){
  const key=(highlight?'next:':'plan:')+row.key,p=value.progress,phase=value.guide.phases?.find(item=>item.id===row.phase);
  const keys=new Set([row.when,row.priority_when,row.skip_when,row.started_when,row.completion_condition,phase?.when].flatMap(node=>HoiContracts.checkKeys(node)));keys.add(row.key);
  const hud=value.state?.overlay,diagnostics=(hud?.items||[]).filter(item=>item.step_keys?.includes(row.key)).map(item=>item.message);
  const signature=JSON.stringify([row,value.view.mode,value.pending,[...keys].map(k=>[k,p.checks?.[k],p.started?.[k]]),value.view.model.runtime.action_sequence?.political,unplacedOperations.has(row.key),hud?.guide_id,hud?.status,diagnostics]);
  const previous=cardCache.get(key);if(previous?.signature===signature)return previous.node;
  const node=card(row,highlight);
  if(previous?.node.querySelectorAll){const old=[...previous.node.querySelectorAll('details')],fresh=[...node.querySelectorAll('details')];old.forEach((detail,i)=>{if(detail.open&&fresh[i]?.querySelector('summary')?.textContent===detail.querySelector('summary')?.textContent)fresh[i].open=true;});}
  cardCache.set(key,{signature,node});return node;
 }
 function card(row,highlight=false){const node=el('article','operation-card'+(row.tracking==='reminder'?' reminder':'')+(row.event_observation?.visible?' event-visible':'')+(highlight?' highlighted':'')+(row.runtime_status==='completed'?' completed':'')+(row.runtime_status==='active'?' in-progress':''));node.id=(highlight?'next-operation-':'operation-')+row.key;node.dataset.step=row.key;
  const operational=HoiActionPlan.describe(row,{usable:value.view.mode==='live',guide_id:value.guide.id,expected_guide_id:value.guide.id,overlay:value.state?.overlay,political_key:value.view.model.runtime.action_sequence?.political});
  const hint=row.action.params?.category_hint;
  const label=(categories[row.category]||HoiI18n.t("INSTRUCTION"))+(hint&&hint!==row.category?' · '+hint:'');
  const top=el('div','operation-top');top.append(el('span','eyebrow',label),el('span','badge',badge(row)));node.append(top);
  const repeatedTitle=row.tracking==='reminder'&&row.instructions?.some(text=>text.replace(/\s+/g,' ').startsWith(name(row).replace(/\s+/g,' ')));
  if(!repeatedTitle)node.append(referenceText('h3','',operational.title||name(row),row));
  if(row.action.params?.timing||!row.action.params?.source_action)node.append(referenceText('p','operation-timing',HoiI18n.t("When: ")+row.timing,row));
  if(unplacedOperations.has(row.key)&&!row.action.params?.source_action)node.append(el('p','operation-copy',HoiI18n.t("Chronological position is unverified: consult this instruction's timing and conditions.")));
  if(row.game_date)node.append(el('p','operation-timing',HoiI18n.t("Date shown in the guide: ")+row.game_date+HoiI18n.t(" · chronological reference, not an automatic deadline")));
  if(operational.access)node.append(el('p','operation-access',operational.access));
  if(operational.instructions.length){const list=el('ol','operation-instructions');for(const line of operational.instructions)for(const part of line.split('\n').filter(s=>s.trim()))list.append(referenceText('li','',part.replace(/^\s*[-•]\s*/,''),row));node.append(list);}
  const related=guideReferences?.related(row,[operational.title||name(row),...operational.instructions,row.timing]);if(related)node.append(related);
  if(row.tracking!=='reminder')node.append(el('p','operation-monitoring',operational.monitoring),el('p','operation-hud operation-hud-'+operational.hud.status,operational.hud.message));
  if(row.operational_glossary?.length){const glossary=el('dl','operation-glossary');for(const term of row.operational_glossary)glossary.append(el('dt','',term.code),el('dd','',HoiI18n.t(term.meaning)));node.append(glossary);}
  if(row.evidence?.some(e=>e.quote)){
   const original=el('details','operation-original');original.append(el('summary','',HoiI18n.t('Original action and references')));
   for(const quote of new Set(row.evidence.map(e=>e.quote).filter(Boolean)))original.append(referenceText('blockquote','',quote,row));node.append(original);
  }
  const refs=[row.action.params?.design_reference,...(row.action.params?.production_targets||[]).map(p=>p.design_reference)].filter(Boolean);
  for(const ref of row.action.params?.reference_images||[]){const details=el('details','design-reference'),image=el('img');details.append(el('summary','',HoiI18n.t('Design reference')+' · '+ref.name));image.src='/api/reference-library/image?id='+encodeURIComponent(ref.id);image.alt=ref.name;image.loading='lazy';image.style.maxWidth='100%';details.append(image);node.append(details);}
  for(const ref of refs){const details=el('details','design-reference');details.append(el('summary','',HoiI18n.t('Design reference')+' · '+ref.name));if(ref.image_id){const image=el('img');image.src='/api/reference-library/image?id='+encodeURIComponent(ref.image_id);image.alt=ref.name;image.loading='lazy';image.style.maxWidth='100%';details.append(image);}details.append(el('pre','',JSON.stringify({modules:ref.modules,upgrades:ref.upgrades},null,2)));node.append(details);}
  if(row.event_observation){const e=row.event_observation,p=row.action.params;node.append(el('p','operation-observations',e.visible===true?HoiI18n.t("EVENT OPEN · ")+p.event_id:e.visible===false?HoiI18n.t("Event is not currently open."):HoiI18n.t("Event visibility cannot be verified.")));if(p.event_option_id)node.append(el('p','event-choice',HoiI18n.t("Recommended choice: ")+(p.event_option_name||p.event_option||p.event_option_id)+(e.option_available===true?HoiI18n.t(" · available in the event window"):e.option_available===false?HoiI18n.t(" · not among the current choices"):HoiI18n.t(" · availability cannot be verified"))));}
  if(row.action_readiness&&row.runtime_status!=='completed'){
   const r=row.action_readiness,p=r.political_power;
   if(p)node.append(el('p','operation-observations',HoiI18n.t("Political power: {0} · effective cost: {1} · guide threshold: {2}{3}",p.available??HoiI18n.t("unverifiable"),p.effective_cost??HoiI18n.t("unverifiable"),p.guide_threshold??HoiI18n.t("not specified"),p.affordable===false?' · insufficiente: mantieni questo ordine nella sequenza':'')));
   const resourceNames={'projects.points.specialization_land':HoiI18n.t("Land breakthrough points"),'projects.points.specialization_air':HoiI18n.t("Air breakthrough points"),'projects.points.specialization_naval':HoiI18n.t("Naval breakthrough points"),'projects.points.specialization_nuclear':HoiI18n.t("Physics breakthrough points")};
   for(const resource of r.resources||[])node.append(el('p','operation-observations',`${resourceNames[resource.key]||resource.key}: ${resource.available??HoiI18n.t("unverifiable")} / ${resource.cost}${resource.affordable===false?HoiI18n.t(" · insufficient: HUD waiting"):''}`));
   const reasons={project_cost_unknown:HoiI18n.t("Project point cost is not qualified."),resource_balance_unknown:HoiI18n.t("Resource balance is not qualified: HUD waiting."),insufficient_resource:HoiI18n.t("Insufficient points: the HUD border remains off."),political_power_unknown:HoiI18n.t("Current political power is not qualified."),political_cost_unknown:HoiI18n.t("The effective political cost is not qualified; a guide threshold alone cannot confirm affordability."),action_requirements_unknown:HoiI18n.t("Dynamic requirements cannot be verified."),commander_assignment_unobserved:HoiI18n.t("The live reader does not expose commander assignment to units."),facility_compatibility_unknown:HoiI18n.t("Facility compatibility cannot be verified yet."),compatible_facility_unavailable:HoiI18n.t("No compatible idle facility with an assigned scientist is available."),action_requirements_false:HoiI18n.t("Declared requirements are not met.")};
   for(const reason of [...r.blocked,...r.unknown])if(reasons[reason])node.append(el('p','operation-copy',reasons[reason]));
  }
  if(row.note&&!row.evidence?.some(e=>e.quote===row.note)&&!/^Abbreviations \(/.test(row.note))node.append(el('p','operation-copy',row.note));
  const productionDetails=row.production?.layout||[row.production];
  for(const detail of productionDetails){
   const check=detail?.design_check;if(!check)continue;
   const status={match:'Design configuration matches',different:'Design configuration differs',unknown:'Design configuration not verifiable',missing:'Required design is not in production'}[check.status];
   if(status)node.append(el('p','operation-observations',HoiI18n.t(status)));
   for(const candidate of check.candidates||[]){
    if(!candidate.differences?.length)continue;
    const differences=el('details','design-differences');differences.append(el('summary','',candidate.name+' · '+HoiI18n.t('Configuration differences')));
    const list=el('ul');for(const d of candidate.differences)list.append(el('li','',HoiI18n.t('Required {0}: {1} · observed: {2}',d.key,String(d.expected??HoiI18n.t('Empty')),String(d.actual??HoiI18n.t('Empty')))));
    differences.append(list);node.append(differences);
   }
  }
  productionDetails.forEach((p,i)=>{if(!p?.priority)return;const target=row.action.params?.production_targets?.[i]||row.action.params;
   const product=target.variant_name||target.equipment_name||target.equipment_id||'';
   node.append(el('p','operation-observations',HoiI18n.t('Production priority · {0}: {1} · {2}',product,HoiI18n.t(p.priority==='first'?'Leading group':'Last military line'),HoiI18n.t(p.priority_satisfied===true?'Order verified':p.priority_satisfied===false?'Reorder the production lines':'Order unverifiable'))));});
  if(row.fortifications){const f=row.fortifications;node.append(el('p','operation-observations',HoiI18n.t('Fortifications: {0} / {1} provinces at level {2}',f.completed_provinces??HoiI18n.t('unverifiable'),f.total_provinces??HoiI18n.t('unverifiable'),f.target_level)));}
  if(row.construction){const c=row.construction;node.append(el('p','operation-observations',c.target===null?HoiI18n.t("Baseline not recorded yet: open the guide before issuing the order."):HoiI18n.t("Buildings in the state: {0} / {1} · baseline: {2}",c.current??HoiI18n.t("unverifiable"),c.target,c.baseline)));}
  if(row.production?.layout){const table=el('table','production-targets'),head=el('tr');for(const label of [HoiI18n.t("Product"),HoiI18n.t("Target"),HoiI18n.t("Requested live"),HoiI18n.t("Available live")])head.append(el('th','',label));table.append(head);row.production.layout.forEach((p,i)=>{const target=row.action.params.production_targets[i],line=el('tr');for(const text of [target.variant_name||target.equipment_name||target.equipment_id,p.superseded_by?.length?HoiI18n.t('Superseded'):target.quantity_mode==='remaining'?HoiI18n.t('ALL remaining factories'):p.target??'—',p.requested??'—',p.assigned??'—'])line.append(el('td','',String(text)));table.append(line);});node.append(table,el('p','operation-copy',HoiI18n.t("Factory targets include requested factories that are not available yet. The objective is not based only on active factories.")));}
  else if(row.production){const p=row.production,target=p.target===null?HoiI18n.t("target not determined"):p.target;
   node.append(el('p','operation-observations',HoiI18n.t("Production · {0}: {1} / {2} · requested: {3} · available: {4} · active: {5} · damaged: {6}",p.basis==='requested'?HoiI18n.t('requested factories'):HoiI18n.t('assigned factories'),p.basis==='requested'?(p.requested??HoiI18n.t("unverifiable")):(p.assigned??HoiI18n.t("unverifiable")),target,p.requested??'—',p.assigned??'—',p.active??'—',p.damaged??'—')));
   if(p.baseline!==null)node.append(el('p','operation-copy',HoiI18n.t("Initially assigned: ")+p.baseline));
   if(p.reason==='baseline_missing')node.append(el('p','operation-copy',HoiI18n.t("The baseline must be recorded before increasing the requested factories.")));
   if(p.lines.length){const list=el('ul','operation-observations');for(const line of p.lines)list.append(el('li','',`${line.name} · ${line.equipment_name||line.equipment} · ${line.category==='naval'?HoiI18n.t("dockyards"):HoiI18n.t("military factories")}: ${line.assigned_factories} assegnate`));node.append(list);}
  }
  if(row.construction_observations?.length){const list=el('ul','operation-observations');for(const line of row.construction_observations)list.append(el('li','',HoiI18n.t("{0} · {1}: {2} queued · {3} active factories",line.name||line.building_id,line.state_name||'regione '+line.state_id,line.amount,line.active_factories)));node.append(el('p','eyebrow',HoiI18n.t("LIVE ORDERS")),list);}
  dependencyList(row,node);
  controls(row,node);
  const reference=row.frame_reference;
  if(reference||visualReferenceSteps.has(row.key)||(row.evidence||[]).some(e=>Number.isFinite(e.timestamp_seconds)))node.append(sourceFrame(row));
  const href=sourceLink(reference?.seconds,reference?.url);if(href){const a=el('a','operation-source',HoiI18n.t("Source")+(reference?' · video '+clock(reference.seconds):'')+' ↗');a.href=href;a.target='_blank';a.rel='noopener noreferrer';node.append(a);}return node;
 }
 function links(){const {guide,view}=value;const focus=view.lanes.focus.next,tech=view.lanes.research.next;
  const scope=guide.applies_to?.tree_id;const baseFocus=scope?'/?tree='+encodeURIComponent(scope):'/';
  $('editAssociations').href=window.hoiGuide.withGuide('/associations');
  $('focusTreeLink').href=window.hoiGuide.withGuide(focus?.href||baseFocus);$('researchTreeLink').href=window.hoiGuide.withGuide(tech?.href||'/technologies');$('hudLink').href=window.hoiGuide.withGuide('/walkthrough');
  for(const a of document.querySelectorAll('.topbar a'))a.href=window.hoiGuide.withGuide(a.getAttribute('href'));
 }
 function renderSummary(){
  const result=HoiOperationalSummary.project(value.guide,value.view.model.runtime,value.state),cards=$('summaryCards');
  $('summaryNotice').textContent=result.notice;$('summaryNotice').hidden=!result.notice;
  $('summaryRemaining').hidden=!result.hidden_count;$('summaryRemaining').textContent=result.hidden_count?HoiI18n.t("Another {0} activities are listed in the full plan below.",result.hidden_count):'';
  const hud=value.state?.overlay,liveOK=value.view.mode==='live';
  $('hudDiagnosticMessage').textContent=!liveOK?HoiI18n.t("Game state cannot be verified: previous HUD guidance is not presented as current."):hud?.guide_id&&hud.guide_id!==value.guide.id?HoiI18n.t("Diagnostics belong to another guide."):hud?.message||HoiI18n.t("HUD diagnostics unavailable. Start the HUD control panel to check its status.");
  const hudItems=$('hudDiagnosticItems');hudItems.replaceChildren();
  if(liveOK&&hud?.status==='observed'&&hud.guide_id===value.guide.id){
   for(const item of hud.items||[]){const li=el('li','',item.message);
    if(item.panel==='research'&&item.folder&&item.code==='different_folder'){
     const a=el('a','',HoiI18n.t(" Open this tab in Companion"));a.href=window.hoiGuide.withGuide('/technologies?folder='+encodeURIComponent(item.folder));li.append(a);
    }hudItems.append(li);
   }
  }
  const signature=JSON.stringify(result);if(signature===summaryKey)return;summaryKey=signature;cards.replaceChildren();
  const tones={attention:HoiI18n.t("Attention"),active:HoiI18n.t("In progress"),ready:HoiI18n.t("Needs verification"),waiting:HoiI18n.t("Waiting"),unknown:HoiI18n.t("Unverifiable")},origins={observed:HoiI18n.t("Observed"),calculated:HoiI18n.t("Calculated"),unknown:HoiI18n.t("Unavailable")};
  for(const item of result.items){
   const card=el('article','summary-card summary-'+item.tone);card.dataset.step=item.step_key;
   card.append(el('span','badge',tones[item.tone]),el('h3','',item.title),el('p','',item.message));
   card.append(el('p','summary-goal',HoiI18n.t("Guide objective · ")+item.guide_goal));
   if(item.links?.length){const list=el('ul','summary-dependencies');for(const target of item.links){const li=el('li');li.append(dependencyLink(target));list.append(li);}card.append(list);}
   if(item.comparisons?.length>1){const list=el('ul','summary-comparisons');for(const p of item.comparisons)list.append(el('li','',HoiI18n.t("{0}: target {1} · requested {2} · assigned {3}",p.name,p.target??HoiI18n.t("unverifiable"),p.requested??HoiI18n.t("unverifiable"),p.assigned??HoiI18n.t("unverifiable"))));card.append(list);}
   if(item.evidence?.length){const details=el('details','summary-evidence');details.append(el('summary','',HoiI18n.t("Game data")));
    for(const fact of item.evidence){const display=fact.status==='unknown'?HoiI18n.t("unavailable"):typeof fact.value==='boolean'?(fact.value?HoiI18n.t("yes"):'no'):Array.isArray(fact.value)?HoiI18n.t("{0} items observed",fact.value.length):String(fact.value);
     details.append(el('p','',fact.label+': '+display+' · '+(origins[fact.status]||HoiI18n.t("Runtime data"))));
    }card.append(details);
   }
   const link=el('a','',item.link_label);link.href=item.href.startsWith('#')?item.href:window.hoiGuide.withGuide(item.href);
   if(item.href.startsWith('#'))link.onclick=revealPlan;card.append(link);cards.append(card);
  }
 }
 function render(next){value=next;if(!value.guide){$('error').hidden=false;$('error').textContent=value.error||HoiI18n.t("Loading the guide…");return;}
  const {guide,view}=value;$('error').hidden=!value.error;$('error').textContent=value.error;
  const ordering=view.operation_order||{};unplacedOperations=new Set(ordering.unplaced||[]);
  const orderNotes=[];
  if(unplacedOperations.size)orderNotes.push(HoiI18n.t("{0} instructions have no comparable time reference. Their relative guide order is retained after timed instructions, unless a dependency requires otherwise.",unplacedOperations.size));
  if(ordering.conflict_count)orderNotes.push(HoiI18n.t("{0} temporal conflicts: declared dependencies take precedence over campaign dates, then source times. Review the source references.",ordering.conflict_count));
  $('operationOrderNotice').textContent=orderNotes.join(' ');$('operationOrderNotice').hidden=!orderNotes.length;
  dependencyRows=new Map(Object.values(view.model.tracks).flatMap(t=>t.rows).map(row=>[row.key,row]));
  if(lastGuide!==guide){cardCache.clear();blockCache.clear();filterKey='';summaryKey='';guideReferences?.setGuide(guide);if(lastGuide){for(const pending of frameCache.values())pending.then(url=>URL.revokeObjectURL(url)).catch(()=>{});frameCache.clear();mediaReferences.clear();if(lastGuide.id!==guide.id){expandedFrames.clear();expandedDependencies.clear();}if($('sourceMediaDialog').open)$('sourceMediaDialog').close();}visualReferenceSteps=new Set((guide.source_inventory?.items||[]).filter(item=>!item.excluded_reason&&Number.isFinite(item.timestamp_seconds)).flatMap(item=>item.step_keys||[]));lastGuide=guide;filter='all';const heading=HoiGuidePresentation.heading(guide.title);$('guideTitle').textContent=heading.title;$('guideTitle').title=guide.title;$('guideMetadata').textContent=heading.details;$('guideMetadata').hidden=!heading.details;$('subtitle').textContent=guide.subtitle;
   if(location.hash?.startsWith('#operation-')){$('operationsPlan').open=true;$('hideDone').checked=false;}
   $('guideSelect').replaceChildren();for(const item of value.catalog){const option=el('option','',HoiGuidePresentation.heading(item.title).title);option.title=item.title;option.value=item.id;option.selected=item.id===guide.id;if(typeof HoiGuideAvailability!=='undefined')HoiGuideAvailability.decorate(option,item.compatibility,HoiI18n.t);$('guideSelect').append(option);}
   const href=sourceLink();$('sourceLink').hidden=!href;if(href)$('sourceLink').href=href;$('sourceBasis').textContent=[guide.source?.author,guide.source?.published,guide.source?.basis].filter(Boolean).join(' · ');
   $('assumptions').replaceChildren(...(guide.assumptions||[]).map(text=>el('li','',text)));$('catalogWarnings').replaceChildren(...(guide.catalog_warnings||[]).map(text=>el('p','',text)));
   $('sourceDocuments').replaceChildren(...(guide.sources||[]).filter(src=>src.type==='file'&&typeof src.text==='string'&&src.text.trim()).map(src=>{const detail=el('details','source-document');detail.append(el('summary','',HoiI18n.t("Original document text")+' · '+(src.title||src.id)),el('p','section-note',HoiI18n.t("Original source text. Only the instructions shown in the plan are monitored.")),el('pre','source-document-text',src.text));return detail;}));
   $('chapters').replaceChildren(...(guide.source?.chapters||[]).map(chapter=>{const li=el('li'),a=el('a','',clock(chapter.start_time)+' · '+chapter.title);a.href=sourceLink(chapter.start_time);a.target='_blank';a.rel='noopener noreferrer';li.append(a);return li;}));
  }
  $('guideSelect').disabled=value.pending>0;$('reset').disabled=value.pending>0;links();
  renderSummary();
  $('mode').textContent={live:HoiI18n.t("Linked path · ")+(view.country||HoiI18n.t("current game")),preview:HoiI18n.t("Guide preview"),wrong_country:HoiI18n.t("Guide suspended · different country"),wrong_tree:HoiI18n.t("Guide suspended · different tree")}[view.mode];
  $('modeNote').textContent=view.mode==='live'?HoiI18n.t("Focus and research update in their respective trees. Operations with live criteria advance automatically; reminders remain available without confirmation. Confirm only criteria that cannot be observed."):HoiI18n.t("Instructions remain available. No progress is inferred without a compatible game.");
  const p=value.progress,storage=p.storage||{},campaign=p.campaign_id;
  $('campaignStatus').textContent=storage.error?HoiI18n.t("Progress not saved: ")+storage.error:
   ['wrong_campaign','campaign_unverified'].includes(view.progress_status)?HoiI18n.t("Progress suspended: the saved campaign does not match a verified live identity."):
   (campaign?HoiI18n.t("Campaign ")+p.country+' · '+campaign.split(':')[1].slice(0,8)+' · ':HoiI18n.t("Campaign identity not verified · "))+
   (storage.status==='persistent'?HoiI18n.t("Progress saved on this PC."):HoiI18n.t("Temporary progress."));
  const history=(p.history||[]).filter(row=>row.campaign_id===campaign&&row.country===p.country);
  $('runHistory').replaceChildren(...history.map((row,index)=>{const option=el('option','',`Checklist ${index+1} · ${row.run_id.slice(0,8)}${row.run_id===p.run_id?HoiI18n.t(" · current"):''}${row.branch_reason==='timeline_rewind'?HoiI18n.t(" · after loading an earlier save"):''}`);option.value=row.run_id;return option;}));
  $('runHistory').value=p.run_id;$('restoreHistory').disabled=value.pending>0||!history.length;$('refreshHistory').disabled=value.pending>0;
  const allRows=value.view.operations,rows=allRows.filter(r=>r.tracking!=='reminder'),done=rows.filter(r=>r.runtime_status==='completed').length,active=rows.filter(r=>r.runtime_status==='active').length;$('operationProgress').textContent=done+'/'+rows.length;$('operationActive').textContent=active?active+HoiI18n.t(" in progress"):'';
  const political=value.view.model.runtime.action_sequence?.political;
  const compact=HoiActionPlan.compact(allRows,political),upcoming=compact.next;
  const current=[];
  if(upcoming){
   const ordinal=upcoming.action.params?.source_action?.ordinal;
   current.push(el('p','eyebrow',Number.isInteger(ordinal)?HoiI18n.t('Current step {0}',ordinal):HoiI18n.t('NEXT INSTRUCTION TO CHECK')),cachedCard(upcoming,true));
  }else current.push(el('p','',rows.length?HoiI18n.t("No further instructions to check now. See upcoming activities."):HoiI18n.t("This guide does not contain operational instructions yet.")));
  sync($('nextOperation'),current);
  for(const [id,items]of [['ongoingOperations',compact.active],['waitingOperations',compact.waiting]]){
   $(id).hidden=!items.length;const list=$(id+'List');
   const signature=JSON.stringify(items.map(r=>[r.key,r.runtime_status,r.runtime_reason,r.title,r.name,r.construction,r.production]));
   if(list.dataset.signature!==signature){list.dataset.signature=signature;list.replaceChildren();for(const row of items){const item=el('li'),link=el('a','',name(row));link.href='#operation-'+row.key;link.onclick=revealPlan;item.append(link,el('span','', ' · '+badge(row)));list.append(item);}}
  }
  const filters=$('operationFilters'),available=Object.entries(categories).filter(([key])=>key==='all'||allRows.some(r=>r.category===key));
  const nextFilterKey=JSON.stringify([available,filter]);
  if(filterKey!==nextFilterKey){filterKey=nextFilterKey;filters.replaceChildren();for(const [key,label]of available){const button=el('button','operation-filter',label);button.type='button';button.setAttribute('aria-pressed',String(filter===key));button.onclick=()=>{filter=key;render(value);};filters.append(button);}}
  // A closed full plan has no rendering work. Existing nodes remain attached
  // while collapsed, preserving details and reference previews.
  if($('operationsPlan').open!==false){
   const cards=$('operationCards'),sections=[],occurrences=new Map();
   const visible=allRows.filter(row=>(filter==='all'||row.category===filter)&&(!$('hideDone').checked||row.runtime_status!=='completed'&&row.runtime_reason!=='production_superseded'));
   const visibleKeys=new Set(visible.map(row=>row.key));
   for(const block of HoiActionPlan.blocks(allRows)){
    const rows=block.rows.filter(row=>visibleKeys.has(row.key));if(!rows.length)continue;
    const count=occurrences.get(block.id)||0;occurrences.set(block.id,count+1);const id=block.id+':'+count;
    let cached=blockCache.get(id);
    if(!cached){const section=el('section','action-block'),header=el('div','action-block-heading'),title=el('h3'),badge=el('span','badge');header.append(title,badge);cached={section,header,title,badge,note:el('p','section-note',HoiI18n.t('Actions column · source order. Focus and research priorities remain in their trees.'))};blockCache.set(id,cached);}
    const statuses={completed:'Tracked actions completed',active:'In progress',ready:'Ready',waiting:'Waiting',not_applicable:'Not applicable',reminder:'Method reminder'};
    const title=HoiI18n.t('Step {0}',block.number),badge=HoiI18n.t(statuses[block.status]);
    if(cached.title.textContent!==title)cached.title.textContent=title;if(cached.badge.textContent!==badge)cached.badge.textContent=badge;
    sync(cached.section,[cached.header,...(block.source?[cached.note]:[]),...rows.map(row=>cachedCard(row))]);sections.push(cached.section);
   }
   if(!visible.length)sections.push(el('p','empty-operations',HoiI18n.t("No instructions to check in this category.")));
   sync(cards,sections);
  }
  guideReferences?.refresh();
 }
 $('guideSelect').onchange=()=>window.hoiGuide.select($('guideSelect').value);
 $('operationsPlan').ontoggle=()=>{if(value&&$('operationsPlan').open)render(value);};
 $('hideDone').onchange=()=>render(value);
 $('reset').onclick=async()=>{try{undo=await window.hoiGuide.newRun();const box=$('resetNotice');box.replaceChildren(document.createTextNode(HoiI18n.t("New checklist. ")));const b=el('button','',HoiI18n.t("Cancel"));b.type='button';b.onclick=async()=>{if(!undo||value.pending)return;await window.hoiGuide.restore(undo);undo=null;box.hidden=true;};box.append(b);box.hidden=false;}catch{}};
 $('refreshHistory').onclick=()=>window.hoiGuide.refreshProgress().catch(()=>{});
 $('restoreHistory').onclick=()=>window.hoiGuide.restore({run_id:$('runHistory').value}).catch(()=>{});
 window.addEventListener?.('hashchange',()=>{if(value?.guide&&location.hash?.startsWith('#operation-'))revealPlan();});
 window.hoiGuide.subscribe(render);
})();
