'use strict';
(()=>{
 const $=id=>document.getElementById(id),el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
 const categories={all:HoiI18n.t("All"),construction:HoiI18n.t("Factories and infrastructure"),production:HoiI18n.t("Production"),projects:HoiI18n.t("Special projects"),politics:HoiI18n.t("Politics and laws"),characters:HoiI18n.t("Characters"),army:HoiI18n.t("Armed forces"),templates:HoiI18n.t("Templates and vehicles"),checkpoints:HoiI18n.t("Events and conditions")};
 let value=null,filter='all',undo=null,lastGuide=null,dependencyRows=new Map();
 function dependencyLink(target){const a=el('a','',target.label);a.href=target.href.startsWith('#')?target.href:window.hoiGuide.withGuide(target.href);if(target.href.startsWith('#'))a.onclick=()=>{filter='all';$('hideDone').checked=false;render(value);};return a;}
 const expandedFrames=new Set(),frameCache=new Map();
 function sourceFrame(row){
  const id=value.guide.id,key=id+':'+row.key+':'+row.source_seconds,details=el('details','operation-frame');
  details.append(el('summary','',HoiI18n.t("Show guide frame · {0}",clock(row.source_seconds))));let displayed=false;
  details.ontoggle=async()=>{
   if(!details.open){expandedFrames.delete(key);return;}expandedFrames.add(key);if(displayed)return;displayed=true;
   const caption=el('p','',HoiI18n.t("Source frame, not the current game."));details.append(caption);
   try{
    if(!frameCache.has(key))frameCache.set(key,(async()=>{const response=await fetch('/api/guide/frame?id='+encodeURIComponent(id)+'&key='+encodeURIComponent(row.key));if(!response.ok){const error=await response.json();throw Error(error.error);}return URL.createObjectURL(await response.blob());})());
    const url=await frameCache.get(key),img=el('img');img.alt=HoiI18n.t("Source visual reference for ")+name(row);img.src=url;details.append(img);
   }catch(e){caption.textContent=e.message;frameCache.delete(key);displayed=false;}
  };
  details.open=expandedFrames.has(key);return details;
 }
 if(location.pathname==='/hud'){document.body.classList.add('hud-view');const token=new URLSearchParams(location.search).get('instance')||'';document.title='HOI4 Companion HUD '+(/^[a-f0-9]{32}$/.test(token)?token:'');}
 const sourceLink=seconds=>{try{const url=new URL(value.guide.source?.url||value.guide.sources?.[0]?.url);if(!['http:','https:'].includes(url.protocol))return null;if(Number.isFinite(seconds))url.searchParams.set('t',String(Math.floor(seconds)));return url.href;}catch{return null;}};
 const clock=seconds=>Math.floor(seconds/60)+':'+String(Math.floor(seconds%60)).padStart(2,'0');
 const name=row=>row.name||row.title||row.key;
 function badge(row){if(row.tracking==='reminder')return row.runtime_status==='waiting'?HoiI18n.t("Reminder · upcoming"):row.runtime_status==='unverifiable'?HoiI18n.t("Reminder · timing needs verification"):HoiI18n.t("Reminder");return row.runtime_status==='completed'?HoiI18n.t("Completed"):row.runtime_status==='not_applicable'?HoiI18n.t("Not applicable"):row.runtime_status==='waiting'?HoiI18n.t("Upcoming"):row.runtime_status==='active'?(row.start_basis==='live'?HoiI18n.t("In progress · start detected live"):row.start_basis==='condition'?HoiI18n.t("In progress · criterion verified"):HoiI18n.t("Started · in progress (manual)")):row.started_when?(row.selection?.value===false?HoiI18n.t("No order detected"):HoiI18n.t("Start cannot be verified")):row.runtime_reason==='manual_confirmation_required'||row.runtime_status==='ready'?HoiI18n.t("Check in the game"):HoiI18n.t("Timing needs verification");}
 function confirm(key,text){const label=el('label','checkpoint-check'),input=el('input');input.type='checkbox';input.checked=value.progress.checks[key]===true;input.disabled=value.pending>0;input.setAttribute('data-check',key);input.addEventListener('change',()=>window.hoiGuide.confirm(key,input.checked).catch(()=>{}));label.append(input,el('span','',text));return label;}
 function controls(row,box){
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
 function card(row,highlight=false){const node=el('article','operation-card'+(row.tracking==='reminder'?' reminder':'')+(row.event_observation?.visible?' event-visible':'')+(highlight?' highlighted':'')+(row.runtime_status==='completed'?' completed':'')+(row.runtime_status==='active'?' in-progress':''));node.id=(highlight?'next-operation-':'operation-')+row.key;node.dataset.step=row.key;
  const top=el('div','operation-top');top.append(el('span','eyebrow',categories[row.category]||HoiI18n.t("INSTRUCTION")),el('span','badge',badge(row)));node.append(top,el('h3','',name(row)));
  node.append(el('p','operation-timing',HoiI18n.t("When: ")+row.timing));
  if(row.game_date)node.append(el('p','operation-timing',HoiI18n.t("Date shown in the guide: ")+row.game_date+HoiI18n.t(" · chronological reference, not an automatic deadline")));
  if(row.instructions?.length){const list=el('ol','operation-instructions');for(const line of row.instructions)list.append(el('li','',line));node.append(list);}
  if(row.event_observation){const e=row.event_observation,p=row.action.params;node.append(el('p','operation-observations',e.visible===true?HoiI18n.t("EVENT OPEN · ")+p.event_id:e.visible===false?HoiI18n.t("Event is not currently open."):HoiI18n.t("Event visibility cannot be verified.")));if(p.event_option_id)node.append(el('p','event-choice',HoiI18n.t("Recommended choice: ")+(p.event_option_name||p.event_option||p.event_option_id)+(e.option_available===true?HoiI18n.t(" · available in the event window"):e.option_available===false?HoiI18n.t(" · not among the current choices"):HoiI18n.t(" · availability cannot be verified"))));}
  if(row.action_readiness&&row.runtime_status!=='completed'){
   const r=row.action_readiness,p=r.political_power;
   if(p)node.append(el('p','operation-observations',HoiI18n.t("Political power: {0} · cost declared by the guide: {1}{2}",p.available??HoiI18n.t("unverifiable"),p.cost??HoiI18n.t("not specified"),p.affordable===false?' · insufficiente: mantieni questo ordine nella sequenza':'')));
   const resourceNames={'projects.points.specialization_land':HoiI18n.t("Land breakthrough points"),'projects.points.specialization_air':HoiI18n.t("Air breakthrough points"),'projects.points.specialization_naval':HoiI18n.t("Naval breakthrough points"),'projects.points.specialization_nuclear':HoiI18n.t("Physics breakthrough points")};
   for(const resource of r.resources||[])node.append(el('p','operation-observations',`${resourceNames[resource.key]||resource.key}: ${resource.available??HoiI18n.t("unverifiable")} / ${resource.cost}${resource.affordable===false?HoiI18n.t(" · insufficient: HUD waiting"):''}`));
   const reasons={project_cost_unknown:HoiI18n.t("Project point cost is not qualified."),resource_balance_unknown:HoiI18n.t("Resource balance is not qualified: HUD waiting."),insufficient_resource:HoiI18n.t("Insufficient points: the HUD border remains off."),political_power_unknown:HoiI18n.t("Current political power is not qualified."),political_cost_unknown:HoiI18n.t("Current cost cannot be verified: the guide does not specify a threshold."),action_requirements_unknown:HoiI18n.t("Dynamic requirements cannot be verified."),commander_assignment_unobserved:HoiI18n.t("The live reader does not expose commander assignment to units."),facility_compatibility_unknown:HoiI18n.t("Facility compatibility cannot be verified yet."),action_requirements_false:HoiI18n.t("Declared requirements are not met.")};
   for(const reason of [...r.blocked,...r.unknown])if(reasons[reason])node.append(el('p','operation-copy',reasons[reason]));
  }
  if(row.note)node.append(el('p','operation-copy',row.note));
  if(row.construction){const c=row.construction;node.append(el('p','operation-observations',c.target===null?HoiI18n.t("Baseline not recorded yet: open the guide before issuing the order."):HoiI18n.t("Buildings in the state: {0} / {1} · baseline: {2}",c.current??HoiI18n.t("unverifiable"),c.target,c.baseline)));}
  if(row.production?.layout){const table=el('table','production-targets'),head=el('tr');for(const label of [HoiI18n.t("Product"),HoiI18n.t("Target"),HoiI18n.t("Requested live"),HoiI18n.t("Available live")])head.append(el('th','',label));table.append(head);row.production.layout.forEach((p,i)=>{const target=row.action.params.production_targets[i],line=el('tr');for(const text of [target.variant_name||target.equipment_name||target.equipment_id,p.target??'—',p.requested??'—',p.assigned??'—'])line.append(el('td','',String(text)));table.append(line);});node.append(table,el('p','operation-copy',HoiI18n.t("Factory targets include requested factories that are not available yet. The objective is not based only on active factories.")));}
  else if(row.production){const p=row.production,target=p.target===null?HoiI18n.t("target not determined"):p.target;
   node.append(el('p','operation-observations',HoiI18n.t("Production · {0}: {1} / {2} · requested: {3} · available: {4} · active: {5} · damaged: {6}",p.basis==='requested'?HoiI18n.t('requested factories'):HoiI18n.t('assigned factories'),p.basis==='requested'?(p.requested??HoiI18n.t("unverifiable")):(p.assigned??HoiI18n.t("unverifiable")),target,p.requested??'—',p.assigned??'—',p.active??'—',p.damaged??'—')));
   if(p.baseline!==null)node.append(el('p','operation-copy',HoiI18n.t("Initially assigned: ")+p.baseline));
   if(p.reason==='baseline_missing')node.append(el('p','operation-copy',HoiI18n.t("The baseline must be recorded before increasing the requested factories.")));
   if(p.lines.length){const list=el('ul','operation-observations');for(const line of p.lines)list.append(el('li','',`${line.name} · ${line.equipment_name||line.equipment} · ${line.category==='naval'?HoiI18n.t("dockyards"):HoiI18n.t("military factories")}: ${line.assigned_factories} assegnate`));node.append(list);}
  }
  if(row.construction_observations?.length){const list=el('ul','operation-observations');for(const line of row.construction_observations)list.append(el('li','',HoiI18n.t("{0} · {1}: {2} queued · {3} active factories",line.name||line.building_id,line.state_name||'regione '+line.state_id,line.amount,line.active_factories)));node.append(el('p','eyebrow',HoiI18n.t("LIVE ORDERS")),list);}
  if(row.blocked_by?.length){node.append(el('p','operation-wait',row.runtime_reason==='dependency_unknown'?HoiI18n.t("Prerequisites to verify:"):HoiI18n.t("Waiting for prerequisites:")));const list=el('ul','summary-dependencies');for(const key of row.blocked_by){const target=dependencyRows.get(key),li=el('li');if(target){const ref=HoiOperationalSummary.reference(target);ref.label+=(row.depends_on_started||[]).includes(key)?HoiI18n.t(" (start)"):HoiI18n.t(" (completion)");li.append(dependencyLink(ref));}else li.append(el('span','',HoiI18n.t("Unresolved step · ")+key));list.append(li);}node.append(list);}
  controls(row,node);
  if(Number.isFinite(row.source_seconds))node.append(sourceFrame(row));
  const href=sourceLink(row.source_seconds);if(href){const a=el('a','operation-source',HoiI18n.t("Source")+(Number.isFinite(row.source_seconds)?' · video '+clock(row.source_seconds):'')+' ↗');a.href=href;a.target='_blank';a.rel='noopener noreferrer';node.append(a);}return node;
 }
 function links(){const {guide,view}=value;const focus=view.lanes.focus.next,tech=view.lanes.research.next;
  const scope=guide.applies_to?.tree_id;const baseFocus=scope?'/?tree='+encodeURIComponent(scope):'/';
  $('editAssociations').href=window.hoiGuide.withGuide('/associations');
  $('focusTreeLink').href=window.hoiGuide.withGuide(focus?.href||baseFocus);$('researchTreeLink').href=window.hoiGuide.withGuide(tech?.href||'/technologies');$('hudLink').href=window.hoiGuide.withGuide('/walkthrough');
  for(const a of document.querySelectorAll('.topbar a'))a.href=window.hoiGuide.withGuide(a.getAttribute('href'));
 }
 function renderSummary(){
  const result=HoiOperationalSummary.project(value.guide,value.view.model.runtime,value.state),cards=$('summaryCards');cards.replaceChildren();
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
   if(item.href.startsWith('#'))link.onclick=()=>{filter='all';$('hideDone').checked=false;render(value);};card.append(link);cards.append(card);
  }
 }
 function render(next){value=next;if(!value.guide){$('error').hidden=false;$('error').textContent=value.error||HoiI18n.t("Loading the guide…");return;}
  const {guide,view}=value;$('error').hidden=!value.error;$('error').textContent=value.error;
  dependencyRows=new Map(Object.values(view.model.tracks).flatMap(t=>t.rows).map(row=>[row.key,row]));
  if(lastGuide!==guide){if(lastGuide?.id!==guide.id){for(const pending of frameCache.values())pending.then(url=>URL.revokeObjectURL(url)).catch(()=>{});frameCache.clear();expandedFrames.clear();}lastGuide=guide;filter='all';$('guideTitle').textContent=guide.title;$('subtitle').textContent=guide.subtitle;
   $('guideSelect').replaceChildren();for(const item of value.catalog){const option=el('option','',item.title);option.value=item.id;option.selected=item.id===guide.id;$('guideSelect').append(option);}
   const href=sourceLink();$('sourceLink').hidden=!href;if(href)$('sourceLink').href=href;$('sourceBasis').textContent=[guide.source?.author,guide.source?.published,guide.source?.basis].filter(Boolean).join(' · ');
   $('assumptions').replaceChildren(...(guide.assumptions||[]).map(text=>el('li','',text)));$('catalogWarnings').replaceChildren(...(guide.catalog_warnings||[]).map(text=>el('p','',text)));
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
  const eligible=rows.filter(row=>(!row.action_readiness||row.action_readiness.domain!=='political'||row.key===political)&&(!['completed','not_applicable','waiting','active'].includes(row.runtime_status)||row.key===political&&row.runtime_status==='waiting'));
  const upcoming=rows.find(row=>row.event_observation?.visible===true&&!['completed','not_applicable'].includes(row.runtime_status))||eligible.find(row=>!['dependency_unknown','phase_condition_unknown','condition_unknown','live_state_unavailable'].includes(row.runtime_reason))||eligible[0];
  $('nextOperation').replaceChildren();if(upcoming){
   $('nextOperation').append(el('p','eyebrow',HoiI18n.t("NEXT INSTRUCTION TO CHECK")));
   if(location.pathname==='/hud')$('nextOperation').append(card(upcoming,true));
   else{const summary=el('div','operation-summary'),copy=el('div');copy.append(el('strong','',name(upcoming)),el('p','',upcoming.timing));const a=el('a','',HoiI18n.t("Show instruction ↓"));a.href='#operation-'+upcoming.key;a.onclick=()=>{filter=upcoming.category;render(value);};summary.append(copy,a);$('nextOperation').append(summary);}
  }else $('nextOperation').append(el('p','',rows.length?HoiI18n.t("No further instructions to check now. See upcoming activities."):HoiI18n.t("This guide does not contain operational instructions yet.")));
  const filters=$('operationFilters');filters.replaceChildren();for(const [key,label]of Object.entries(categories))if(key==='all'||allRows.some(r=>r.category===key)){const b=el('button','operation-filter',label);b.type='button';b.setAttribute('aria-pressed',String(filter===key));b.onclick=()=>{filter=key;render(value);};filters.append(b);}
  const cards=$('operationCards');cards.replaceChildren();const visible=allRows.filter(row=>(filter==='all'||row.category===filter)&&(!$('hideDone').checked||row.runtime_status!=='completed'));
  for(const row of visible)cards.append(card(row));if(!visible.length)cards.append(el('p','empty-operations',HoiI18n.t("No instructions to check in this category.")));
 }
 $('guideSelect').onchange=()=>window.hoiGuide.select($('guideSelect').value);
 $('hideDone').onchange=()=>render(value);
 $('reset').onclick=async()=>{try{undo=await window.hoiGuide.newRun();const box=$('resetNotice');box.replaceChildren(document.createTextNode(HoiI18n.t("New checklist. ")));const b=el('button','',HoiI18n.t("Cancel"));b.type='button';b.onclick=async()=>{if(!undo||value.pending)return;await window.hoiGuide.restore(undo);undo=null;box.hidden=true;};box.append(b);box.hidden=false;}catch{}};
 $('refreshHistory').onclick=()=>window.hoiGuide.refreshProgress().catch(()=>{});
 $('restoreHistory').onclick=()=>window.hoiGuide.restore({run_id:$('runHistory').value}).catch(()=>{});
 window.hoiGuide.subscribe(render);
})();
