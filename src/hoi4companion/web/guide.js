'use strict';
(()=>{
 const $=id=>document.getElementById(id),el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
 const categories={all:HoiI18n.t("All"),construction:HoiI18n.t("Factories and infrastructure"),production:HoiI18n.t("Production"),projects:HoiI18n.t("Special projects"),politics:HoiI18n.t("Politics and laws"),characters:HoiI18n.t("Characters"),army:HoiI18n.t("Armed forces"),templates:HoiI18n.t("Templates and vehicles"),diplomacy:HoiI18n.t("War and diplomacy"),trade:HoiI18n.t("Trade and resources"),checkpoints:HoiI18n.t("Other instructions")};
 let value=null,filter='all',undo=null,lastGuide=null,dependencyRows=new Map(),unplacedOperations=new Set(),actionReader=null;
 const cardCache=new Map(),blockCache=new Map();let filterKey='',summaryKey='',scrolledOperation='';
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
 // Two separate labels: when the step applies (status) and how its completion is
 // known (verification). Both only present the Rust evaluation and the prepared
 // step; neither decides status or progress.
 function whenLabel(row){if(row.runtime_reason==='production_superseded')return HoiI18n.t('Superseded');if(row.tracking==='reminder')return row.runtime_status==='waiting'?HoiI18n.t("Waiting"):row.runtime_status==='unverifiable'?HoiI18n.t("Source order"):null;return row.runtime_status==='completed'?(row.construction?.targets?.some(x=>Number.isInteger(x.target)&&Number.isInteger(x.current)&&x.current<x.target)?HoiI18n.t("Ordered · in the construction queue"):HoiI18n.t("Completed")):row.runtime_status==='not_applicable'?HoiI18n.t("Not applicable"):row.runtime_status==='waiting'?HoiI18n.t("Waiting"):row.runtime_status==='active'?HoiI18n.t("In progress"):row.started_when?(row.selection?.value===false?HoiI18n.t("No order detected"):HoiI18n.t("Start cannot be verified")):row.runtime_status==='ready'?HoiI18n.t("Now"):HoiI18n.t("Source order");}
 // The method comes from the prepared step's `verification` (set in Rust by guide_enrich).
 // Only a step prepared without it falls back to its completion reading game state,
 // not just the player's ticks. The label names the method; the status label says
 // whether the evaluation can currently apply it.
 const observable=node=>!!node&&(node.op==='fact'||(node.args||[]).some(observable)||observable(node.arg));
 function verification(row){
  if(row.tracking==='reminder')return 'reminder';
  if(row.verification?row.verification==='automatic'&&observable(row.completion_condition):observable(row.completion_condition))return 'automatic';
  // Unbound fallback lines of the Actions column: nothing in them was understood.
  const p=row.action?.params||{};
  if(p.source_action&&(!p.category||p.category==='checkpoints')&&!p.reference_resolution&&!['bound','not_observable'].includes(p.action_resolution?.status)&&!row.interpretation)return 'uninterpreted';
  return 'manual';
 }
 const verificationLabels={automatic:HoiI18n.t("Automatic"),manual:HoiI18n.t("Confirm yourself"),reminder:HoiI18n.t("Reminder"),uninterpreted:HoiI18n.t("Not interpreted")};
 function badges(row){const box=el('span','badge-pair'),when=whenLabel(row),how=verification(row);if(row.set_aside){const a=el('span','badge badge-set-aside',HoiI18n.t("Set aside"));a.title=HoiI18n.t("You set this step aside: it is not proposed as the next step until you resume it.");box.append(a);}if(when){const w=el('span','badge badge-when',when);if(when===HoiI18n.t("Source order"))w.title=HoiI18n.t("The guide does not say when: follow the source order.");box.append(w);}const v=el('span','badge badge-verification verification-'+how,verificationLabels[how]);v.title=HoiI18n.t("How completion is known");box.append(v);
  // UX R13: the player's own reading of the line, shown as theirs, never as a check.
  const own=row.interpretation;if(own){const c=el('span','badge badge-interpretation',HoiI18n.t("Your reading: {0}",[categories[own.category]||HoiI18n.t("Other instructions"),own.target?.name].filter(Boolean).join(' · ')));c.title=HoiI18n.t("Saved by you in Correct bindings. It changes how the line is shown, never when it counts as done.");box.append(c);}
  return box;}
 function badge(row){return [whenLabel(row),verificationLabels[verification(row)]].filter(Boolean).join(' · ');}
 function confirm(key,text,detail){
  const label=el('label','checkpoint-check'),input=el('input');
  input.type='checkbox';input.checked=value.progress.checks[key]===true;input.disabled=value.pending>0;
  input.setAttribute('data-check',key);
  input.addEventListener('change',()=>window.hoiGuide.confirm(key,input.checked).catch(()=>{}));
  label.title=HoiI18n.t("Tick it when you have done this in the game. The ✓ on the in-game card does the same.");
  const saved=detail?HoiI18n.t('Confirmed by you · {0}',detail):HoiI18n.t('Confirmed by you');
  label.append(input,el('span','',input.checked?saved:text));return label;
 }
 // Set aside / resume: stored with the checklist; Rust then stops proposing the
 // step and lets the following ones go on. Never completes anything by itself.
 function asideButton(row){if(row.tracking==='reminder'||['completed','not_applicable'].includes(row.runtime_status))return null;
  const b=el('button','set-aside-toggle',row.set_aside?HoiI18n.t("Resume"):HoiI18n.t("Set aside for later"));b.type='button';b.disabled=value.pending>0;b.setAttribute('data-set-aside',row.key);
  b.title=row.set_aside?HoiI18n.t("Bring this step back into the plan. Completing it still works as before."):HoiI18n.t("Put this step aside to go on with the next ones. It stays in the Set aside list until you resume or complete it.");
  b.onclick=()=>window.hoiGuide.setAside(row.key,!row.set_aside).catch(()=>{});return b;}
 function controls(row,box){
  if(row.superseded_by?.length){box.append(el('p','operation-copy',HoiI18n.t('Superseded targets are no longer requested; this does not prove the earlier setup was completed.')),dependencyItems(row.superseded_by,row));if(row.runtime_reason==='production_superseded')return;}
  if(row.tracking==='reminder'){box.append(el('p','operation-copy',HoiI18n.t("Method reminder: no confirmation required; it does not block other steps.")));return;}
  if(row.started_when&&row.runtime_status!=='completed'){
   box.append(el('p','operation-start-note',row.runtime_status==='active'?HoiI18n.t("Start detected using the guide's condition. Starting alone does not prove completion."):row.selection?.value===false?HoiI18n.t("Automatic start detection: waiting for the guide's condition to be met."):HoiI18n.t('Start cannot be verified')));
  }
  const done=HoiI18n.t("Done")+(row.confirmation?' · '+row.confirmation:'');
  if(['manual','special_project'].includes(row.action.kind)&&!row.completion_condition)box.append(confirm(row.key,done,row.confirmation));
  const phase=value.guide.phases?.find(p=>p.id===row.phase);
  const keys=new Set([row.when,row.priority_when,row.skip_when,row.started_when,phase?.when].flatMap(node=>HoiContracts.checkKeys(node)));
  if(row.completion_condition)for(const key of HoiContracts.checkKeys(row.completion_condition))if(key!==row.action.params?.legacy_key)keys.add(key);
  const finalKeys=new Set(HoiContracts.checkKeys(row.completion_condition));
  for(const key of keys){
   const condition=finalKeys.has(key)?HoiI18n.t("Final criterion verified in the game"):HoiI18n.t("Condition verified in the game");
   box.append(finalKeys.has(key)||key!==row.key?confirm(key,condition,condition):confirm(key,done,row.confirmation));
  }
  const aside=asideButton(row);if(aside)box.append(aside);
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
 const describe=row=>HoiActionPlan.describe(row,{reader:actionReader,usable:value.view.mode==='live',guide_id:value.guide.id,expected_guide_id:value.guide.id,overlay:value.state?.overlay,political_key:value.view.model.runtime.action_sequence?.political});
 // Abbreviated event answers are completed like LPF: full in-game answer
 // first, the author's shorthand in parentheses.
 function displayTitle(row,operational=describe(row)){
  const heading=operational.title||name(row),answer=row.action?.params?.event_option_name||row.action?.params?.reference_resolution?.option_text;
  return answer&&/(?:…|\.{3})/.test(heading)&&!heading.includes(answer)?`«${answer}» (${heading})`:heading;
 }
 // UX R12: "Not this?" opens the associations page on this step; only a saved user choice changes the binding.
 // UX R13: lines followed by hand open the same page to record the player's reading.
 function notThis(row){const how=verification(row),href=HoiActionPlan.correctionHref(row,how);if(!href)return null;const bound=HoiActionPlan.correctable(row);
  const a=el('a','not-this',bound?HoiI18n.t("Not this?"):row.interpretation?HoiI18n.t("Change your reading"):HoiI18n.t("What does this mean?"));a.href=window.hoiGuide.withGuide(href);
  a.title=bound?HoiI18n.t("Choose the right focus, technology or project for this step. Nothing changes until you save a correction."):HoiI18n.t("Say what this line is about. It changes how the line is shown, never when it counts as done.");return a;}
 function card(row,highlight=false){const node=el('article','operation-card'+(row.tracking==='reminder'?' reminder':'')+(row.event_observation?.visible?' event-visible':'')+(highlight?' highlighted':'')+(row.runtime_status==='completed'?' completed':'')+(row.runtime_status==='active'?' in-progress':'')+(row.set_aside?' set-aside':''));node.id=(highlight?'next-operation-':'operation-')+row.key;node.dataset.step=row.key;
  const operational=describe(row);
  const hint=row.action.params?.category_hint;
  const label=(categories[row.category]||HoiI18n.t("INSTRUCTION"))+(hint&&hint!==row.category?' · '+hint:'');
  const top=el('div','operation-top');top.append(el('span','eyebrow',label),badges(row));const fix=notThis(row);if(fix)top.append(fix);node.append(top);
  const repeatedTitle=row.tracking==='reminder'&&row.instructions?.some(text=>text.replace(/\s+/g,' ').startsWith(name(row).replace(/\s+/g,' ')));
  const completed=displayTitle(row,operational);
  if(!repeatedTitle)node.append(referenceText('h3','',completed,row));
  // A title composed from a fully understood line keeps the author's words visible.
  if(HoiActionPlan.composedTitle(row.reading)){
   const notes=(row.reading.qualifiers||[]).map(q=>q.kind==='asap'?HoiI18n.t('As soon as possible'):q.kind==='priority'?HoiI18n.t('Priority'):q.kind==='political_power'?HoiI18n.t('{0} PP',q.value):q.kind==='note'?HoiI18n.t('Guide note: {0}',q.text):null).filter(Boolean);
   if(notes.length)node.append(el('p','operation-qualifiers',notes.join(' · ')));
   node.append(el('p','operation-original-text',HoiI18n.t('Original text: {0}',row.reading.original)));
  }
  if(row.action.params?.timing||!row.action.params?.source_action)node.append(referenceText('p','operation-timing',HoiI18n.t("When: ")+row.timing,row));
  // The author's condition for a line split from a group ("At 24% paranoia")
  // sits under the title, highlighted: players missed it as a grey note.
  // Same rule as the HUD card (`hud_service::step_card`): params.timing, else this.
  const condition=!row.action.params?.timing&&row.action.params?.decomposition?.timing?.trim()||'';
  if(condition)node.append(referenceText('p','operation-timing operation-condition',HoiI18n.t("When: ")+condition,row));
  if(unplacedOperations.has(row.key)&&!row.action.params?.source_action)node.append(el('p','operation-copy',HoiI18n.t("Chronological position is unverified: consult this instruction's timing and conditions.")));
  if(row.game_date)node.append(el('p','operation-timing',HoiI18n.t("Date shown in the guide: ")+row.game_date+HoiI18n.t(" · chronological reference, not an automatic deadline")));
  if(operational.access)node.append(el('p','operation-access',operational.access));
  if(operational.instructions.length){const list=el('ol','operation-instructions');for(const line of operational.instructions)for(const part of line.split('\n').filter(s=>s.trim()&&(repeatedTitle||s.trim()!==completed.trim())))list.append(referenceText('li','',part.replace(/^\s*[-•]\s*/,''),row));if(list.children.length)node.append(list);}
  // Abbreviations are explained beside the instruction, never inside a partly understood sentence.
  if(operational.glossary?.length){const list=el('ul','operation-glossary');list.setAttribute('aria-label',HoiI18n.t('Abbreviations in this instruction'));for(const term of operational.glossary){const item=el('li'),code=el('abbr','',term.code);code.title=HoiI18n.t(term.meaning);item.append(code,' '+HoiI18n.t(term.meaning));list.append(item);}node.append(list);}
  if(operational.observations?.length){const list=el('ul','operation-observations production-status');for(const text of operational.observations)list.append(el('li','',text));node.append(list);}
  const related=guideReferences?.related(row,[operational.title||name(row),...operational.instructions,row.timing]);if(related)node.append(related);
  const sourceText=(row.instructions||[]).join(' '),topics=new Set();
  if(/\bMIOs?\b/i.test(sourceText))topics.add('mios');
  // Reference topics follow the bound category or an explicit source prefix, not the display topic guessed from wording.
  const bound=row.action.params?.category;
  if(bound==='production'||bound==='checkpoints'&&/^(?:MIL|NAV|PRODUCTION)\s*:/i.test(sourceText.trim()))topics.add('production');
  if(bound==='projects'||row.action.kind==='special_project')topics.add('projects');
  if(/\b(?:support|regimental) compan|\b(?:Eng|Log|Hos|Rec)\b/.test(sourceText))topics.add('companies');
  if(/\b(?:doctrines?|spirits?)\b/i.test(sourceText))topics.add('doctrines');
  const references=(value.guide.advanced_references||[]).filter(d=>topics.has(d.category)&&/^[a-f0-9]{64}$/.test(d.id));
  if(references.length){const details=el('details','operation-advanced-references');details.append(el('summary','',HoiI18n.t('Advanced guide references')));for(const ref of references){const a=el('a','',ref.name);a.href='/importer#advanced-'+ref.id;details.append(a,el('br'));}node.append(details);}
  for(const note of operational.reading_notes)node.append(el('p','operation-reading-note',note));
  if(row.tracking!=='reminder'){
   const details=el('details','operation-diagnostics');details.append(el('summary','',HoiI18n.t('Tracking and HUD details')));
   details.append(el('p','operation-monitoring',operational.monitoring),el('p','operation-hud operation-hud-'+operational.hud.status,operational.hud.message));node.append(details);
  }
  if(row.evidence?.some(e=>e.quote)||row.instructions?.length){
   const original=el('details','operation-original');original.append(el('summary','',HoiI18n.t('Original action and references')));
   for(const quote of new Set([...(row.instructions||[]),...(row.evidence||[]).map(e=>e.quote)].filter(Boolean)))original.append(referenceText('blockquote','',quote,row));node.append(original);
  }
  const refs=[row.action.params?.design_reference,...(row.action.params?.production_targets||[]).map(p=>p.design_reference)].filter(Boolean);
  for(const ref of row.action.params?.reference_images||[]){const details=el('details','design-reference'),image=el('img');details.append(el('summary','',HoiI18n.t('Design reference')+' · '+ref.name));image.src='/api/reference-library/image?id='+encodeURIComponent(ref.id);image.alt=ref.name;image.loading='lazy';image.style.maxWidth='100%';details.append(image);node.append(details);}
  for(const ref of refs){const details=el('details','design-reference');details.append(el('summary','',HoiI18n.t('Design reference')+' · '+ref.name));if(ref.image_id){const image=el('img');image.src='/api/reference-library/image?id='+encodeURIComponent(ref.image_id);image.alt=ref.name;image.loading='lazy';image.style.maxWidth='100%';details.append(image);}details.append(el('pre','',JSON.stringify({modules:ref.modules,upgrades:ref.upgrades},null,2)));node.append(details);}
  if(row.event_observation){const e=row.event_observation,p=row.action.params;node.append(el('p','operation-observations',e.visible===true?HoiI18n.t("EVENT OPEN · ")+p.event_id:e.visible===false?HoiI18n.t("Event is not currently open."):HoiI18n.t("Event visibility cannot be verified.")));if(p.event_option_id)node.append(el('p','event-choice',HoiI18n.t("Recommended choice: ")+(p.event_option_name||p.event_option||p.event_option_id)+(e.option_available===true?HoiI18n.t(" · available in the event window"):e.option_available===false?HoiI18n.t(" · not among the current choices"):HoiI18n.t(" · availability cannot be verified"))));}
  if(row.action_readiness&&row.runtime_status!=='completed'){
   const r=row.action_readiness,notes=HoiGuidePresentation.readinessNotes(r);
   for(const text of notes.visible)node.append(el('p','operation-copy',text));
   const resourceNames={'projects.points.specialization_land':HoiI18n.t("Land breakthrough points"),'projects.points.specialization_air':HoiI18n.t("Air breakthrough points"),'projects.points.specialization_naval':HoiI18n.t("Naval breakthrough points"),'projects.points.specialization_nuclear':HoiI18n.t("Physics breakthrough points")};
   for(const resource of r.resources||[])node.append(el('p','operation-observations',`${resourceNames[resource.key]||resource.key}: ${resource.available??HoiI18n.t("unverifiable")} / ${resource.cost}${resource.affordable===false?HoiI18n.t(" · insufficient: HUD waiting"):''}`));
   if(notes.why.length){const why=el('details','readiness-why');why.append(el('summary','',HoiI18n.t("Why?")),...notes.why.map(text=>el('p','operation-copy',text)));node.append(why);}
  }
  // The condition is already shown under the title; the rest of the note stays here.
  // The parent line a bullet sits under is labelled, so it never reads as this step's own instruction.
  const parent=row.action.params?.decomposition?.context?.trim();
  const note=condition||parent?(row.note||'').split('\n').filter(line=>line.trim()!==condition).map(line=>line.trim()===parent?HoiI18n.t('Under the guide line: “{0}”',parent):line).join('\n').trim():row.note;
  if(note&&!row.evidence?.some(e=>e.quote===row.note)&&!/^Abbreviations \(/.test(note))node.append(el('p','operation-copy',note));
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
  if(row.construction){const c=row.construction;if(Array.isArray(c.targets)){const list=el('ul','operation-observations');for(const target of c.targets){const location=(target.state_name||'STATE_'+target.state_id)+(target.province_id?' / '+target.province_id:'');list.append(el('li','',`${target.building_name||target.building_id} · ${location}: ${target.current??HoiI18n.t('unverifiable')} / ${target.target??HoiI18n.t('unverifiable')}${target.queued===true?' · '+HoiI18n.t('In progress'):''}`));}node.append(list);}else node.append(el('p','operation-observations',c.target===null?HoiI18n.t("Baseline not recorded yet: open the guide before issuing the order."):HoiI18n.t("Buildings in the state: {0} / {1} · baseline: {2}",c.current??HoiI18n.t("unverifiable"),c.target,c.baseline)));}
  // Grounding of abbreviated references: what the text was matched to, or the
  // real candidates when no single nation-compatible meaning exists.
  const ref=row.action?.params||{},kindLabel=k=>HoiI18n.t(({event_option:'event answer',event:'event',decision:'decision',focus:'focus',law:'law',idea:'national spirit',technology:'technology',advisor:'advisor'})[k]||k);
  const isAnswer=r=>!r.kind||r.kind==='event_option';
  if(ref.reference_resolution){const r=ref.reference_resolution;node.append(el('p','operation-copy',isAnswer(r)?HoiI18n.t("Recognized: event answer “{0}” ({1}).",r.option_text||r.option_id,r.event_id):HoiI18n.t("Recognized: {0} “{1}” ({2}).",kindLabel(r.kind),r.name||r.id,r.id)));
   // A block's lead line: recognized only, the block holds other work too.
   if(r.tracked===false)node.append(el('p','operation-copy',HoiI18n.t(r.scope==='line'?"Not tracked automatically: confirm every action of this line yourself.":"Not tracked automatically: this block also holds other instructions.")));}
  for(const more of Array.isArray(ref.reference_resolution?.also)?ref.reference_resolution.also:[])node.append(el('p','operation-copy',isAnswer(more)?HoiI18n.t("Also recognized: “{0}” → event answer “{1}” ({2}).",more.text||'',more.option_text||more.option_id,more.event_id):HoiI18n.t("Also recognized: “{0}” → {1} “{2}” ({3}).",more.text||'',kindLabel(more.kind),more.name||more.id,more.id)));
  for(const open of Array.isArray(ref.reference_resolution?.unresolved)?ref.reference_resolution.unresolved:[])node.append(el('p','operation-copy',HoiI18n.t("Not recognized: “{0}”. Possible references: {1}",open.text||'',(open.candidates||[]).map(c=>`${c.name||c.id} (${kindLabel(c.kind)})`).join(' · ')||'—')));
  // The author's trigger ("On X…: …"): the live condition it became, or why
  // it stays unknown.
  const trig=ref.trigger_resolution;
  if(trig?.status==='compiled'&&trig.entity)node.append(el('p','operation-copy',HoiI18n.t(trig.relation==='completed'?"Applies when {0} “{1}” is completed.":trig.relation==='before'?"Applies until {0} “{1}” is started.":"Applies when {0} “{1}” is started or completed.",kindLabel(trig.entity.kind),trig.entity.name||trig.entity.id)));
  else if(trig)node.append(el('p','operation-copy',HoiI18n.t("Condition “{0}” is not recognized automatically: check it yourself.",trig.text||'')));
  // A written-out action ("REL UK, FRA", "Justify HUN"): what was understood,
  // and whether the game state shows it.
  const act=ref.action_resolution,actionLabel=a=>HoiI18n.t(({improve_relations:'improve relations',stop_improving_relations:'stop improving relations',non_aggression_pact:'non-aggression pact',declare_war:'declare war',guarantee_independence:'guarantee independence',cancel_guarantee:'cancel guarantee',embargo:'embargo',cancel_embargo:'cancel embargo',send_attache:'send attaché',recall_attache:'recall attaché',lend_lease:'lend-lease',stop_lend_lease:'stop lend-lease',diplomatic_pressure:'diplomatic pressure',build_intel_network:'build intel network',justify_war_goal:'justify a war goal',annex:'annex',integrate:'integrate',invade:'invade',collaboration_government:'collaboration government'})[a]||a);
  if(act?.status==='bound'&&Array.isArray(act.actions))node.append(el('p','operation-copy',HoiI18n.t("Recognized: {0}.",act.actions.map(a=>`${actionLabel(a.action)} ${a.target}`).join(' · '))));
  else if(act?.status==='not_observable')node.append(el('p','operation-copy',HoiI18n.t("Recognized: {0} {1}. The game does not show this live: confirm it yourself.",actionLabel(act.action),(act.targets||[]).join(', '))));
  if(ref.binding_withdrawn)node.append(el('p','operation-copy',HoiI18n.t("Link removed: {0} ({1}) is not compatible with this nation. Track it manually.",ref.binding_withdrawn.name||ref.binding_withdrawn.id,ref.binding_withdrawn.id)));
  // The player's own answer for an ambiguous line (stored by the backend, applied by Rust).
  const confirmedAnswer=ref.reference_candidates?.confirmed;
  if(confirmedAnswer?.basis==='user_confirmed'){
   const listed=[...(ref.reference_candidates.candidates||[]),...(ref.reference_candidates.event_options||[])].find(c=>c.id===confirmedAnswer.option_id&&(!c.event_id||c.event_id===confirmedAnswer.event_id));
   const line=el('p','operation-copy',HoiI18n.t("Confirmed by you: {0}",listed?.name||confirmedAnswer.option_id)),remove=el('button','remove-confirmation',HoiI18n.t("Remove confirmation"));remove.type='button';
   remove.onclick=async()=>{remove.disabled=true;try{const response=await fetch('/api/guide/answers',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({guide_id:value.guide.id,key:row.key,quote:ref.reference_candidates.text,op:'remove'})});if(!response.ok)throw Error();await window.hoiGuide.select(value.guide.id,false);}catch{remove.disabled=false;}};
   line.append(' ',remove);node.append(line);
  }
  if(Array.isArray(ref.reference_candidates?.candidates)&&ref.reference_candidates.candidates.length)node.append(el('p','operation-copy',HoiI18n.t(ref.reference_candidates.status==='no_compatible_candidate'?"No match for this nation. Similar names in the game: {0}":"Possible references: {0}",ref.reference_candidates.candidates.map(c=>`${c.name||c.id} (${kindLabel(c.kind)})`).join(' · '))));
  // Items listed without a quantity stay in this step; never a completion target.
  const unquantified=Array.isArray(row.action?.params?.production_unquantified)?row.action.params.production_unquantified:[];
  if(unquantified.length&&!row.production?.layout)node.append(el('p','operation-copy',HoiI18n.t("Also listed without a quantity: {0}. Check them in the game.",unquantified.map(u=>u.variant_name||u.equipment_name||u.source_text).join(', '))));
  if(row.production?.layout){const table=el('table','production-targets'),head=el('tr');for(const label of [HoiI18n.t("Product"),HoiI18n.t("Target"),HoiI18n.t("Requested live"),HoiI18n.t("Available live")])head.append(el('th','',label));table.append(head);row.production.layout.forEach((p,i)=>{const target=row.action.params.production_targets[i],line=el('tr');const naval=target.production_selector?.category==='naval';for(const text of [target.display_name||target.variant_name||target.equipment_name||target.equipment_id,p.superseded_by?.length?HoiI18n.t('Superseded'):target.quantity_mode==='remaining'?HoiI18n.t(naval?'ALL remaining dockyards':'ALL remaining factories'):p.target??'—',p.requested??'—',p.assigned??'—'])line.append(el('td','',String(text)));table.append(line);});for(const u of unquantified){const line=el('tr','unquantified');for(const text of [u.variant_name||u.equipment_name||u.source_text,HoiI18n.t("Quantity not given in the guide"),'—','—'])line.append(el('td','',String(text)));table.append(line);}node.append(table,el('p','operation-copy',HoiI18n.t("Factory targets include requested factories that are not available yet. The objective is not based only on active factories.")));}
  else if(row.production){const p=row.production,target=p.target===null?HoiI18n.t("target not determined"):p.target;
   node.append(el('p','operation-observations',HoiI18n.t("Production · {0}: {1} / {2} · requested: {3} · available: {4} · active: {5} · damaged: {6}",p.basis==='requested'?HoiI18n.t('requested factories'):HoiI18n.t('assigned factories'),p.basis==='requested'?(p.requested??HoiI18n.t("unverifiable")):(p.assigned??HoiI18n.t("unverifiable")),target,p.requested??'—',p.assigned??'—',p.active??'—',p.damaged??'—')));
   if(p.baseline!==null)node.append(el('p','operation-copy',HoiI18n.t("Initially assigned: ")+p.baseline));
   if(p.reason==='baseline_missing')node.append(el('p','operation-copy',HoiI18n.t("The baseline must be recorded before increasing the requested factories.")));
   if(p.lines.length){const list=el('ul','operation-observations');for(const line of p.lines)list.append(el('li','',`${line.name} · ${line.equipment_name||line.equipment} · ${line.category==='naval'?HoiI18n.t("dockyards"):HoiI18n.t("military factories")}: ${line.assigned_factories} assegnate`));node.append(list);}
  }
  if(row.construction_observations?.length){const list=el('ul','operation-observations');const known=new Map((row.action.params?.construction_targets||[]).map(x=>[x.state_id+':'+x.building_id,x]));
   for(const line of row.construction_observations){const own=known.get(line.state_id+':'+line.building_id)||[...known.values()].find(x=>x.building_id===line.building_id);
    const building=own?.building_name||line.name||line.building_id,region=line.state_name||(own?.state_id===line.state_id?own.state_name:null)||HoiI18n.t('State {0}',line.state_id);
    list.append(el('li','',HoiI18n.t("{0} · {1}: {2} queued · {3} active factories",building,region,line.amount,line.active_factories)));}node.append(el('p','eyebrow',HoiI18n.t("LIVE ORDERS")),list);}
  dependencyList(row,node);
  controls(row,node);
  const reference=row.frame_reference;
  if(reference||visualReferenceSteps.has(row.key)||(row.evidence||[]).some(e=>Number.isFinite(e.timestamp_seconds)))node.append(sourceFrame(row));
  const href=sourceLink(reference?.seconds,reference?.url);if(href){const a=el('a','operation-source',HoiI18n.t("Source")+(reference?' · video '+clock(reference.seconds):'')+' ↗');a.href=href;a.target='_blank';a.rel='noopener noreferrer';node.append(a);}return node;
 }
 // Guided mode (UX R9): "Now / Then / Waiting" from the Rust statuses, the
 // complete plan stays below. The choice is a per-viewer convenience.
 const viewKey='hoi4companion.guideView';
 let guidedView=(()=>{try{return localStorage.getItem(viewKey)!=='advanced';}catch{return true;}})(),guidedKey='';
 function setGuidedView(on){guidedView=on;try{localStorage.setItem(viewKey,on?'guided':'advanced');}catch{}if(value?.guide)render(value);}
 function stepLink(row){const a=el('a','guided-title',displayTitle(row));a.href='#operation-'+row.key;a.onclick=revealPlan;return a;}
 function guidedItem(row){
  const item=el('li','guided-item'+(row.runtime_status==='active'?' in-progress':''));item.dataset.step=row.key;
  const top=el('div','guided-meta');top.append(el('span','eyebrow',categories[row.category]||HoiI18n.t("Other instructions")),badges(row));
  item.append(top,stepLink(row));const fix=notThis(row);if(fix)item.append(' ',fix);
  // One control: "Done" where the player confirms; none where the game shows it.
  if((row.completion_condition?HoiContracts.checkKeys(row.completion_condition).includes(row.key):['manual','special_project'].includes(row.action.kind))&&row.runtime_status!=='completed')item.append(confirm(row.key,HoiI18n.t("Done")));
  const aside=asideButton(row);if(aside)item.append(' ',aside);
  return item;
 }
 function laneItem(lane,label){
  const row=value.view.lanes[lane]?.next;if(value.view.mode!=='live'||row?.runtime_status!=='ready')return null;
  const item=el('li','guided-item guided-lane'),top=el('div','guided-meta'),a=el('a','guided-title',name(row));
  a.href=window.hoiGuide.withGuide(row.href||(lane==='focus'?'/':'/technologies'));
  const how=el('span','badge-pair');how.append(el('span','badge badge-when',HoiI18n.t("Now")),el('span','badge badge-verification verification-automatic',HoiI18n.t("Automatic")));
  top.append(el('span','eyebrow',label),how);item.append(top,a);const fix=notThis(row);if(fix)item.append(' ',fix);return item;
 }
 // The nearest wait in words (UX R10): prerequisite step, recognized trigger,
 // the author's unrecognized condition, or the Rust reason code through the
 // runtime_reasons table; an unlisted code shows nothing rather than a code.
 function guidedReason(r){
  if(!r)return null;
  if(r.kind==='step')return HoiI18n.t("after “{0}”",displayTitle(r.row));
  if(r.kind==='entity'){const kind=HoiI18n.t(({decision:'decision',focus:'focus',technology:'technology'})[r.entity]||r.entity||'focus');
   return HoiI18n.t(r.relation==='completed'?"after {0} “{1}” is completed":r.relation==='before'?"only before {0} “{1}” is started":"once {0} “{1}” is started",kind,r.name);}
  if(r.kind==='text')return HoiI18n.t("condition: “{0}”",r.text);
  return r.kind==='reason'?HoiI18n.reason(r.code):null;
 }
 function renderGuided(rows,political){
  const box=$('guidedPlan');box.hidden=!guidedView;if(!guidedView)return;
  const g=HoiActionPlan.guided(rows,political),lanes=[['focus',HoiI18n.t("National focuses")],['research',HoiI18n.t("Technology research")]];
  const signature=JSON.stringify([value.guide.id,value.view.mode,value.pending,[...g.now,g.then,g.waiting,g.reason?.row,...g.active].filter(Boolean).map(r=>[r.key,r.runtime_status,r.title,r.name,r.reading,value.progress.checks?.[r.key]]),lanes.map(([l])=>[value.view.lanes[l]?.next?.key,value.view.lanes[l]?.next?.runtime_status])]);
  if(signature===guidedKey)return;guidedKey=signature;
  const parts=[el('h2','guided-heading',HoiI18n.t("Now"))];
  const list=el('ul','guided-now');for(const row of g.now)list.append(guidedItem(row));
  for(const [lane,label]of lanes){if(list.children.length>=5)break;const item=laneItem(lane,label);if(item)list.append(item);}
  parts.push(list.children.length?list:el('p','section-note',rows.length?HoiI18n.t("Nothing to do now: the open steps wait for a condition."):HoiI18n.t("This guide does not contain operational instructions yet.")));
  if(g.active.length){const p=el('p','guided-line');p.append(el('strong','',HoiI18n.t("In progress")+': '));g.active.slice(0,3).forEach((row,i)=>{if(i)p.append(' · ');p.append(stepLink(row));});parts.push(p);}
  if(g.then){const p=el('p','guided-line');p.append(el('strong','',HoiI18n.t("Then")+': '),stepLink(g.then));parts.push(p);}
  if(g.waiting){const p=el('p','guided-line guided-waiting');p.append(el('strong','',HoiI18n.t("Waiting")+': '),stepLink(g.waiting));
   const text=guidedReason(g.reason);if(text)p.append(el('span','guided-reason',' · '+text));parts.push(p);}
  box.replaceChildren(...parts);
 }
 // Steps set aside, in both modes, so the player can come back at any time:
 // open the card, confirm it where it is confirmed by hand, or resume it.
 function renderSetAside(rows){
  const items=rows.filter(r=>r.set_aside),box=$('setAsideOperations'),list=$('setAsideOperationsList');box.hidden=!items.length;
  const signature=JSON.stringify([value.pending,items.map(r=>[r.key,r.runtime_status,r.runtime_reason,r.title,r.name,r.reading,value.progress.checks?.[r.key]])]);
  if(list.dataset.signature===signature)return;list.dataset.signature=signature;list.replaceChildren();
  for(const row of items){const item=el('li','set-aside-item');item.dataset.step=row.key;item.append(stepLink(row),el('span','',' · '+badge(row)+' '));
   if((row.completion_condition?HoiContracts.checkKeys(row.completion_condition).includes(row.key):['manual','special_project'].includes(row.action.kind))&&row.runtime_status!=='completed')item.append(confirm(row.key,HoiI18n.t("Done")));
   item.append(' ',asideButton(row));list.append(item);}
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
  if(lastGuide!==guide){actionReader=guide.action_language?HoiActionPlan.createReader(guide.action_language):null;cardCache.clear();blockCache.clear();filterKey='';summaryKey='';
   if(guideReferences?.setGuide(guide)){const nav=$('guideQuickReferences'),buttons=guideReferences.shortcuts();nav.replaceChildren(...buttons);nav.hidden=!buttons.length;}
   if(lastGuide){for(const pending of frameCache.values())pending.then(url=>URL.revokeObjectURL(url)).catch(()=>{});frameCache.clear();mediaReferences.clear();if(lastGuide.id!==guide.id){expandedFrames.clear();expandedDependencies.clear();}if($('sourceMediaDialog').open)$('sourceMediaDialog').close();}visualReferenceSteps=new Set((guide.source_inventory?.items||[]).filter(item=>!item.excluded_reason&&Number.isFinite(item.timestamp_seconds)).flatMap(item=>item.step_keys||[]));lastGuide=guide;filter='all';const heading=HoiGuidePresentation.heading(guide.title);$('guideTitle').textContent=heading.title;$('guideTitle').title=guide.title;$('guideMetadata').textContent=heading.details;$('guideMetadata').hidden=!heading.details;$('subtitle').textContent=guide.subtitle;
   if(location.hash?.startsWith('#operation-')){$('operationsPlan').open=true;$('hideDone').checked=false;}
   $('guideSelect').replaceChildren();for(const item of value.catalog){const option=el('option','',HoiGuidePresentation.heading(item.title).title);option.title=item.title;option.value=item.id;option.selected=item.id===guide.id;if(typeof HoiGuideAvailability!=='undefined')HoiGuideAvailability.decorate(option,item.compatibility,HoiI18n.t);$('guideSelect').append(option);}
   const href=sourceLink();$('sourceLink').hidden=!href;if(href)$('sourceLink').href=href;$('sourceBasis').textContent=[guide.source?.author,guide.source?.published,guide.source?.basis].filter(Boolean).join(' · ');
   $('assumptions').replaceChildren(...(guide.assumptions||[]).map(text=>el('li','',text)));$('catalogWarnings').replaceChildren(...(guide.catalog_warnings||[]).map(text=>el('p','',text)));
   $('sourceDocuments').replaceChildren(...(guide.sources||[]).filter(src=>src.type==='file'&&typeof src.text==='string'&&src.text.trim()).map(src=>{const detail=el('details','source-document');detail.append(el('summary','',HoiI18n.t("Original document text")+' · '+(src.title||src.id)),el('p','section-note',HoiI18n.t("Original source text. Only the instructions shown in the plan are monitored.")),el('pre','source-document-text',src.text));return detail;}));
   $('chapters').replaceChildren(...(guide.source?.chapters||[]).map(chapter=>{const li=el('li'),a=el('a','',clock(chapter.start_time)+' · '+chapter.title);a.href=sourceLink(chapter.start_time);a.target='_blank';a.rel='noopener noreferrer';li.append(a);return li;}));
  }
  $('guideSelect').disabled=value.pending>0;$('reset').disabled=value.pending>0;links();
  renderSummary();
  const game=HoiCampaignStatus.present(value.live,value.campaign,HoiI18n.t);
  $('mode').textContent=game.action==='walkthrough'?HoiI18n.t('Current campaign'):HoiI18n.t('Guide preview');
  $('modeNote').textContent=[game.message,game.hint].filter(Boolean).join(' ');
  const p=value.progress,storage=p.storage||{},campaign=p.campaign_id;
  $('campaignStatus').textContent=storage.error?HoiI18n.t("Progress not saved: ")+storage.error:
   (campaign?HoiI18n.t("Campaign ")+p.country+' · '+campaign.split(':')[1].slice(0,8)+' · ':HoiI18n.t("Campaign identity not verified · "))+
   (storage.status==='persistent'?HoiI18n.t("Progress saved on this PC."):HoiI18n.t("Temporary progress."));
  const history=(p.history||[]).filter(row=>row.campaign_id===campaign&&row.country===p.country);
  $('runHistory').replaceChildren(...history.map((row,index)=>{const option=el('option','',`Checklist ${index+1} · ${row.run_id.slice(0,8)}${row.run_id===p.run_id?HoiI18n.t(" · current"):''}${row.branch_reason==='timeline_rewind'?HoiI18n.t(" · after loading an earlier save"):''}`);option.value=row.run_id;return option;}));
  $('runHistory').value=p.run_id;$('restoreHistory').disabled=value.pending>0||!history.length;$('refreshHistory').disabled=value.pending>0;
  const allRows=value.view.operations,rows=allRows.filter(r=>r.tracking!=='reminder'),done=rows.filter(r=>r.runtime_status==='completed').length,active=rows.filter(r=>r.runtime_status==='active').length;
  const verified=value.verifiedOperations;
  const progress=Number.isInteger(verified)?HoiI18n.t('Verified in game: {0}/{1}',verified,rows.length):'';
  const total=done===verified?'':HoiI18n.t('Completed steps: {0}/{1}',done,rows.length);
  $('operationProgress').textContent=[progress,total].filter(Boolean).join(' · ');$('operationActive').textContent=active?active+HoiI18n.t(" in progress"):'';
  const political=value.view.model.runtime.action_sequence?.political;
  const compact=HoiActionPlan.compact(allRows,political),upcoming=compact.next;
  const current=[];
  if(upcoming){
   const ordinal=upcoming.action.params?.source_action?.ordinal;
   current.push(el('p','eyebrow',Number.isInteger(ordinal)?HoiI18n.t('Current step {0}',ordinal):HoiI18n.t('NEXT INSTRUCTION TO CHECK')),cachedCard(upcoming,true));
  }else current.push(el('p','',rows.length?HoiI18n.t("No further instructions to check now. See upcoming activities."):HoiI18n.t("This guide does not contain operational instructions yet.")));
  sync($('nextOperation'),current);
  for(const id of ['guidedModeButton','advancedModeButton'])$(id).setAttribute('aria-pressed',String((id==='guidedModeButton')===guidedView));
  renderGuided(allRows,political);$('nextOperation').hidden=guidedView;
  renderSetAside(allRows);
  for(const [id,items]of [['ongoingOperations',compact.active],['waitingOperations',compact.waiting]]){
   $(id).hidden=guidedView||!items.length;const list=$(id+'List');
   const signature=JSON.stringify(items.map(r=>[r.key,r.runtime_status,r.runtime_reason,r.title,r.name,r.construction,r.production]));
   if(list.dataset.signature!==signature){list.dataset.signature=signature;list.replaceChildren();for(const row of items){const item=el('li'),link=el('a','',name(row));link.href='#operation-'+row.key;link.onclick=revealPlan;item.append(link,el('span','', ' · '+badge(row)));list.append(item);}}
  }
  const filters=$('operationFilters'),available=Object.entries(categories).filter(([key])=>key==='all'||allRows.some(r=>r.category===key));
  const nextFilterKey=JSON.stringify([available,filter]);
  if(filterKey!==nextFilterKey){filterKey=nextFilterKey;filters.replaceChildren();for(const [key,label]of available){const button=el('button','operation-filter',label);button.type='button';button.setAttribute('aria-pressed',String(filter===key));button.onclick=()=>{filter=key;render(value);};filters.append(button);}}
  // A closed full plan has no rendering work. Existing nodes remain attached
  // while collapsed, preserving details and reference previews.
  if($('operationsPlan').open!==false){
   const cards=$('operationCards'),sections=[],occurrences=new Map();let noteShown=false;
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
    sync(cached.section,[cached.header,...(block.source&&!noteShown?(noteShown=true,[cached.note]):[]),...rows.map(row=>cachedCard(row))]);sections.push(cached.section);
   }
   if(!visible.length)sections.push(el('p','empty-operations',HoiI18n.t("No instructions to check in this category.")));
   sync(cards,sections);
  }
  guideReferences?.refresh();
  const hash=location.hash;
  if(!hash?.startsWith('#operation-'))scrolledOperation='';
  else if(scrolledOperation!==hash){
   let id=hash.slice(1);
   try{id=decodeURIComponent(id);}catch{/* Keep the original fragment if it cannot be decoded. */}
   const target=$(id);
   if(target){scrolledOperation=hash;target.scrollIntoView();}
  }
 }
 $('guidedModeButton').onclick=()=>setGuidedView(true);$('advancedModeButton').onclick=()=>setGuidedView(false);
 $('guideSelect').onchange=()=>window.hoiGuide.select($('guideSelect').value);
 $('operationsPlan').ontoggle=()=>{if(value&&$('operationsPlan').open)render(value);};
 $('hideDone').onchange=()=>render(value);
 $('reset').onclick=async()=>{try{undo=await window.hoiGuide.newRun();const box=$('resetNotice');box.replaceChildren(document.createTextNode(HoiI18n.t("New checklist. ")));const b=el('button','',HoiI18n.t("Cancel"));b.type='button';b.onclick=async()=>{if(!undo||value.pending)return;await window.hoiGuide.restore(undo);undo=null;box.hidden=true;};box.append(b);box.hidden=false;}catch{}};
 $('refreshHistory').onclick=()=>window.hoiGuide.refreshProgress().catch(()=>{});
 $('restoreHistory').onclick=()=>window.hoiGuide.restore({run_id:$('runHistory').value}).catch(()=>{});
 window.addEventListener?.('hashchange',()=>{scrolledOperation='';if(value?.guide&&location.hash?.startsWith('#operation-'))revealPlan();});
 window.hoiGuide.subscribe(render);
})();
