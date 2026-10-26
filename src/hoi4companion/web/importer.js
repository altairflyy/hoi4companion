'use strict';
(()=>{
 const $=id=>document.getElementById(id),m=HoiImporter;
 const node=(tag,text)=>{const n=document.createElement(tag);if(text!==undefined)n.textContent=text;return n;};
 let sources=[],report=null,busy=false,aiConfigured=false,generation=0,entities=[],eventExport={catalog:[]},sections=[];
 let aiCatalogPromise=null;
 const buttons=['acquire','packet','extract','validate','saveDraft','exportGuide','install','copyPrompt','downloadPrompt','downloadCatalog','downloadSection'];
 function exportedCatalog(section){return section?.id==='events'&&$('eventsMultiple').checked?{...section,...eventExport,catalog_scope:section.id}:section;}
 function selectedCatalog(){return exportedCatalog(sections.find(s=>s.id===$('catalogSection').value));}
 function missingCountryMetadata(sectionId){return sections.some(section=>(!sectionId||section.id===sectionId)&&section.country_filter?.metadata_available===false&&section.catalog.length>0);}
 function catalogLabel(section){if(section.id!=='events')return HoiI18n.t('{0} · {1} entries',section.title,section.catalog.length);let events=0,answers=0;for(const item of section.catalog){if(item.kind==='event')events++;else if(item.kind==='event_option')answers++;}return HoiI18n.t('{0} · events: {1} · answers: {2}',section.title,events,answers);}
 function catalogView(){
  const section=selectedCatalog(),events=$('catalogSection').value==='events';
  $('eventsFilter').hidden=!events;
  for(const option of $('catalogSection').children){const value=sections.find(s=>s.id===option.value);if(value)option.textContent=catalogLabel(exportedCatalog(value));}
  $('catalogSummary').textContent=section?catalogLabel(section)+'. '+(events&&$('eventsMultiple').checked?HoiI18n.t("Only events with at least two distinct scripted choices."):''):'';
  const missing=sections.filter(s=>missingCountryMetadata(s.id)).map(s=>s.title);
  $('catalogWarning').hidden=!missing.length;$('catalogWarning').textContent=missing.length?HoiI18n.t("Country filter not enabled for ")+missing.join('; ')+HoiI18n.t(": the service is still using the previous catalog. Restart HOI4 Companion, then reload this page. Downloads for affected sections and the full country catalog are suspended; select All countries to export the global catalog."):'';
  if(events&&section?.country_filter?.metadata_available){const filter=section.country_filter;$('catalogSummary').textContent+=HoiI18n.t(" Country: {0}. Excluded {1} incompatible events before filtering multiple-choice events. Partial static coverage: shared events and events with uncertain recipients are retained.",filter.countries.join(', '),filter.excluded_events);}
  else if(section?.id==='politics'&&section.country_filter?.metadata_available){const filter=section.country_filter;$('catalogSummary').textContent+=HoiI18n.t(" Country: {0}. Excluded {1} incompatible entries; retained {2} with uncertain applicability. The selected country supplies the original tag. Partial static coverage.",filter.countries.join(', '),filter.excluded_entries,filter.retained_unknown_entries);}
  else if(['events','politics'].includes(section?.id)&&!section.country_filter&&$('treeScope').value)$('catalogSummary').textContent+=HoiI18n.t(" Cannot determine the country from the selected tree: this section is not country-filtered.");
  controls();
 }
 function rebuildSections(){
  const selected=$('catalogSection').value;sections=m.catalogSections(entities,$('treeScope').value);$('catalogSection').replaceChildren();
  for(const section of sections){if(section.id==='other'&&!section.catalog.length)continue;const option=node('option',`${section.title} · ${section.catalog.length}`);option.value=section.id;$('catalogSection').append(option);}
  eventExport=m.eventCatalog(sections.find(s=>s.id==='events').catalog);
  $('catalogSection').value=sections.some(s=>s.id===selected)?selected:'focus';catalogView();
 }
 function controls(){for(const id of buttons)$(id).disabled=busy;
  $('packet').disabled=busy||!sources.length;$('extract').disabled=busy||!sources.length||!aiConfigured;
  $('copyPrompt').disabled=$('downloadPrompt').disabled=busy||!$('importPrompt').value;
  $('downloadCatalog').disabled=busy||!entities.length||missingCountryMetadata();
  $('downloadSection').disabled=busy||!selectedCatalog()?.catalog.length||missingCountryMetadata($('catalogSection').value);
  $('catalogSection').disabled=busy||!entities.length;$('eventsMultiple').disabled=busy;$('treeScope').disabled=busy||!entities.length;
  for(const id of ['exportGuide','install'])$(id).disabled=!m.canPublish(report,$('reviewed').checked,busy);
  $('draftFile').disabled=busy;
 }
 function invalidate(){generation++;report=null;$('reviewed').checked=false;$('validation').textContent=HoiI18n.t("Draft changed: resolve IDs and validate again.");$('openGuide').hidden=true;controls();}
 function error(e){$('error').hidden=false;$('error').textContent=e.message||String(e);}
 async function request(path,body){const response=await fetch(path,{method:body===undefined?'GET':'POST',headers:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});const value=await response.json();if(!response.ok)throw Error(value.error||HoiI18n.t("Request failed"));return value;}
 async function aiSections(){
  if(!aiCatalogPromise)aiCatalogPromise=request('/api/import/catalog?export=ai').then(value=>{
   if(value.localization_policy!=='local_lookup'||value.alias_policy!=='english_reference_only'||!Array.isArray(value.entities))throw Error(HoiI18n.t("The service is still using the old export: restart HOI4 Companion and reload the page."));return value.entities;
  }).catch(error=>{aiCatalogPromise=null;throw error;});
  return m.catalogSections(await aiCatalogPromise,$('treeScope').value).map(section=>({...section,localization_policy:'local_lookup',alias_policy:'english_reference_only'}));
 }
 let videoTimer=null;
 function videoView(job){
  $('videoStatus').textContent=job.message+(job.encoded_seconds!==undefined&&job.status==='running'?HoiI18n.t(" · {0} / {1} seconds encoded",Math.floor(job.encoded_seconds),Math.ceil(job.duration_seconds||0)):'');
  if(job.source_width&&job.source_height)$('videoStatus').textContent+=HoiI18n.t(" · Actual source: {0}×{1}",job.source_width,job.source_height);
  if(job.audio_status==='running')$('videoStatus').textContent+=HoiI18n.t(" · Preparing separate audio…");
  if(job.audio_warning)$('videoStatus').textContent+=' · '+job.audio_warning;
  $('prepareVideo').disabled=job.status==='running'||job.audio_status==='running';$('videoFiles').replaceChildren();
  const clock=n=>Math.floor(n/60)+':'+String(Math.floor(n%60)).padStart(2,'0');
  for(const part of job.parts||[]){const p=node('p'),a=node('a',`${part.name} · ${(part.bytes/1e6).toFixed(1)} MB · ${clock(part.start_seconds)}–${clock(part.end_seconds)}`);a.href=part.url;a.download=part.name;p.append(a);$('videoFiles').append(p);}
  if(job.audio){const p=node('p'),a=node('a',HoiI18n.t("Download complete audio track · {0} · {1} MB",job.audio.name,(job.audio.bytes/1e6).toFixed(1)));a.href=job.audio.url;a.download=job.audio.name;p.append(a);$('videoFiles').append(p);}
  for(const [field,label]of [['manifest_url',HoiI18n.t("Download timestamp manifest to attach to your AI")],['description_url',HoiI18n.t("Download video description")]])if(job[field]){const p=node('p'),a=node('a',label);a.href=job[field];p.append(a);$('videoFiles').append(p);}
 }
 async function watchVideo(id){try{const job=await request('/api/import/video/status?id='+encodeURIComponent(id));videoView(job);if(job.status==='running'||job.audio_status==='running')videoTimer=setTimeout(()=>watchVideo(id),2000);}catch(e){$('videoStatus').textContent=e.message;$('prepareVideo').disabled=false;}}
 $('prepareVideo').onclick=async()=>{if(videoTimer)clearTimeout(videoTimer);$('prepareVideo').disabled=true;try{const job=await request('/api/import/video',{url:$('videoUrl').value.trim(),...($('videoRefreshSource').checked?{refresh_source:true}:{})});try{localStorage.setItem('hoi4-video-job',job.id);}catch{}videoView(job);watchVideo(job.id);}catch(e){$('videoStatus').textContent=e.message;$('prepareVideo').disabled=false;}};
 try{const id=localStorage.getItem('hoi4-video-job');if(id)watchVideo(id);}catch{}
 async function run(task){if(busy)return;busy=true;$('error').hidden=true;controls();try{await task();}catch(e){error(e);}finally{busy=false;controls();}}
 function download(name,value,text=false){const url=URL.createObjectURL(new Blob([text?value:JSON.stringify(value,null,2)+'\n'],{type:text?'text/plain;charset=utf-8':'application/json'})),a=node('a');a.href=url;a.download=name;document.body.append(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);}
 function readGuide(){return m.readDraft($('guideJson').value).guide;}
 function sourceList(){
  $('sourceCount').textContent=sources.length+HoiI18n.t(" acquired sources");$('sources').replaceChildren();
  for(const src of sources){const details=node('details');details.className='source-card';details.append(node('summary',src.title+' · '+src.type+' · '+src.text.length+HoiI18n.t(" characters")));
   details.append(node('p',src.url||HoiI18n.t("Local text")),node('p',(src.warnings||[]).join(' ')),node('pre',src.text));const remove=node('button',HoiI18n.t("Remove source"));remove.type='button';remove.onclick=()=>{sources=sources.filter(item=>item.id!==src.id);invalidate();sourceList();};details.append(remove);$('sources').append(details);
  }
 }
 function writeGuide(value){$('guideJson').value=JSON.stringify(value,null,2);}
 function constructionEditor(row){
  const params=row.action.params,details=node('details');details.className='construction-editor';details.append(node('summary',HoiI18n.t("Correct construction")));
  details.append(node('p',HoiI18n.t("Correct the instruction in the uploaded copy. Quotations and the original file remain unchanged. Live bindings are recalculated after applying the correction.")));
  function field(text,control){const label=node('label',text);control.setAttribute?.('aria-label',text+' · '+row.key);label.append(control);details.append(label);return control;}
  const stateSearch=field(HoiI18n.t("Find a state"),node('input'));stateSearch.type='search';stateSearch.placeholder=HoiI18n.t("English name, installed translation or ID");
  const stateSelect=field(HoiI18n.t("State"),node('select')),buildingSelect=field(HoiI18n.t("Building"),node('select'));
  const states=entities.filter(item=>item.kind==='state'),buildings=entities.filter(item=>item.kind==='building');
  const label=item=>(item.localized_name||item.name)+' · '+item.id;
  function stateOptions(){const current=stateSelect.value||String(params.state_id??''),query=stateSearch.value.trim().toLocaleLowerCase();stateSelect.replaceChildren();
   const empty=node('option',HoiI18n.t("Choose a state…"));empty.value='';stateSelect.append(empty);
   for(const item of states.filter(item=>[item.id,item.name,item.localized_name,...item.aliases||[]].some(text=>typeof text==='string'&&text.toLocaleLowerCase().includes(query)))){const option=node('option',label(item));option.value=item.id;stateSelect.append(option);}
   stateSelect.value=current;
  }
  stateOptions();stateSearch.oninput=stateOptions;
  const empty=node('option',HoiI18n.t("Choose a building…"));empty.value='';buildingSelect.append(empty);
  for(const item of buildings){const option=node('option',label(item));option.value=item.id;buildingSelect.append(option);}buildingSelect.value=params.building_id||'';
  const province=field(HoiI18n.t("Province ID (for province buildings)"),node('input'));province.type='number';province.min='1';province.step='1';province.value=params.province_id??'';
  const mode=field(HoiI18n.t("Target"),node('select'));
  for(const [value,text]of [['additional',HoiI18n.t("Add to the initial count")],['target_total',HoiI18n.t("Reach the total / level")]]){const option=node('option',text);option.value=value;mode.append(option);}
  mode.value=params.target_level||params.target_quantity?'target_total':params.quantity_mode||'additional';
  const quantity=field(HoiI18n.t("Quantity / level"),node('input'));quantity.type='number';quantity.min='1';quantity.step='1';quantity.value=params.target_level??params.target_quantity??params.quantity??'';
  const instruction=field(HoiI18n.t("Corrected instruction"),node('textarea'));instruction.rows=3;instruction.value=(params.instructions||[]).join('\n');
  const apply=node('button',HoiI18n.t("Apply correction and verify"));apply.type='button';apply.onclick=()=>run(async()=>{
   const next=m.editConstruction(readGuide(),row.key,{state:states.find(item=>item.id===stateSelect.value),building:buildings.find(item=>item.id===buildingSelect.value),province_id:String(province.value).trim()?Number(province.value):null,quantity:Number(quantity.value),quantity_mode:mode.value,instruction:instruction.value});
   writeGuide(next);invalidate();await validate();
  });details.append(apply);return details;
 }
 function sourceEvidence(parent,item){
  const source=sources.find(row=>row.id===item.source_id);
  parent.append(node('blockquote',item.quote||HoiI18n.t("Quotation missing")));
  parent.append(node('p',(source?.title||item.source_id)+(item.timestamp_seconds!==undefined?' · '+item.timestamp_seconds+'s':'')));
  const href=m.sourceHref(source,item.timestamp_seconds);
  if(href){const link=node('a',HoiI18n.t("Open original source")+(item.timestamp_seconds!==undefined?HoiI18n.t(" at the specified point"):''));link.href=href;link.target='_blank';link.rel='noopener noreferrer';parent.append(link);}
  if(source?.type==='youtube'&&href&&Number.isFinite(item.timestamp_seconds)&&item.timestamp_seconds>=0){
   const show=node('button',HoiI18n.t("Show local frame"));show.type='button';
   show.onclick=()=>{show.disabled=true;const img=node('img'),caption=node('p',HoiI18n.t("Loading frame…"));img.alt=HoiI18n.t("Video source at ")+item.timestamp_seconds+HoiI18n.t(" seconds");img.loading='lazy';
    img.onload=()=>{caption.textContent=HoiI18n.t("Source video frame, not the current game.");};
    img.onerror=()=>{img.remove();caption.textContent=HoiI18n.t("Frame unavailable: prepare the video in Import guide or open the original source.");show.disabled=false;};
    img.src='/api/import/frame?url='+encodeURIComponent(source.url)+'&seconds='+encodeURIComponent(item.timestamp_seconds);parent.append(caption,img);};parent.append(show);
  }
  if(source?.text){const context=node('details');context.append(node('summary',HoiI18n.t("Attached context (does not verify the original source)")));
   const at=source.text.indexOf(item.quote),start=Math.max(0,at-300);context.append(node('pre',source.text.slice(start,start+Math.min(5000,(item.quote?.length||0)+600))));parent.append(context);}
 }
 function coverageView(value){
  const coverage=value.coverage,guide=value.guide||value.draft,container=$('coverageReview');container.replaceChildren();
  if(!coverage||coverage.status==='unavailable'){$('coverageSummary').textContent=HoiI18n.t("Coverage unavailable: this guide has no inventory of extracted instructions. It remains compatible; completeness against the source has not been verified.");return;}
  $('coverageSummary').textContent=HoiI18n.t("{0} of {1} instructions linked · {2} excluded with a reason · {3} issues. This checks the declared inventory; it does not certify that the AI reviewed the entire source.",coverage.linked,coverage.total,coverage.excluded,coverage.issues.length);
  const issues=coverage.issues||[],resolved=node('details');resolved.append(node('summary',HoiI18n.t("Links and exclusions already checked · editing is optional")));
  for(const issue of issues.filter(row=>!row.item_id))container.append(node('p',issue.message));
  const choices=node('details');choices.append(node('summary',HoiI18n.t("Step keys and titles for linking")));
  for(const row of Object.values(guide?.tracks||{}).flat())choices.append(node('p',row.key+' · '+(row.name||row.title||row.key)));container.append(choices);
  for(const item of guide?.source_inventory?.items||[]){
   const problems=issues.filter(row=>row.item_id===item.id),card=node('article');card.className='coverage-card';
   const evidence=node('div'),editor=node('div');sourceEvidence(evidence,item);editor.append(node('h4',HoiI18n.t("Instruction ")+item.id));
   for(const problem of problems)editor.append(node('p',problem.message));
   const label=node('label',HoiI18n.t("Linked steps (comma-separated keys)")),input=node('input');input.value=(item.step_keys||[]).join(', ');label.append(input);editor.append(label);
   const apply=node('button',HoiI18n.t("Apply link"));apply.type='button';apply.onclick=()=>{try{writeGuide(m.editCoverage(readGuide(),item.id,input.value.split(',').map(s=>s.trim()).filter(Boolean)));invalidate();}catch(e){error(e);}};editor.append(apply);
   const why=node('label',HoiI18n.t("Reason for exclusion (e.g. repeated content from the same clip)")),reason=node('textarea');reason.value=item.excluded_reason||'';why.append(reason);editor.append(why);
   const exclude=node('button',HoiI18n.t("Exclude with a reason"));exclude.type='button';exclude.onclick=()=>{try{writeGuide(m.editCoverage(readGuide(),item.id,[],reason.value));invalidate();}catch(e){error(e);}};editor.append(exclude);
   card.append(evidence,editor);(problems.length?container:resolved).append(card);
  }
  if(resolved.children.length>1)container.append(resolved);
 }
 function showReport(value){report=value;coverageView(value);$('issues').replaceChildren();$('sourceWarnings').replaceChildren();$('preview').replaceChildren();
  $('validation').textContent=value.valid?HoiI18n.t("Validation passed: the guide is ready to add. Check the source details shown below."):HoiI18n.t("Review required: ")+value.errors.length+HoiI18n.t(" blocking issues.");
  for(const message of value.errors)$('issues').append(node('li',message));
  for(const message of new Set(value.warnings))$('sourceWarnings').append(node('li',message));
  const operations=value.operations||[],count=mode=>operations.filter(row=>row.monitoring===mode).length;
  $('monitoringSummary').textContent=operations.length?count('automatic')+HoiI18n.t(" automatic · ")+count('start_only')+HoiI18n.t(" with automatic start detection only · ")+count('manual')+HoiI18n.t(" requiring confirmation · ")+count('reminder')+HoiI18n.t(" reminders. Monitoring starts in the walkthrough when live state is available."):'';
  const visual=value.warnings.filter(text=>text.includes(HoiI18n.t("declared visual observation"))).length;
  $('reviewSummary').textContent=visual?visual+HoiI18n.t(" instructions include details extracted from video images: check quantities, states and compositions. Expand warnings for details."):HoiI18n.t("Review concerns the guide's content; the application checks IDs and live bindings.");
  if(!value.guide){if(value.draft)writeGuide(value.draft);return;}
  writeGuide(value.guide);
  const resolvedGroup=node('details'),resolvedItems=node('div');resolvedGroup.className='resolved-associations';resolvedItems.className='preview-cards';let resolvedCount=0;
  for(const [lane,rows]of Object.entries(value.guide.tracks))for(const row of rows){
   const resolution=value.resolutions.find(item=>item.key===row.key),operation=operations.find(item=>item.key===row.key),card=node('article');card.className='preview-card'+(resolution&&resolution.status!=='resolved'?' unresolved':'');
   const monitoring={automatic:HoiI18n.t("automatic completion"),start_only:HoiI18n.t("automatic start · outcome needs verification"),manual:HoiI18n.t("criterion still manual"),reminder:HoiI18n.t("reminder · no confirmation")};
   card.append(node('p',lane+' · '+row.key+' · '+(operation?monitoring[operation.monitoring]:resolution?.status==='unresolved_route'?HoiI18n.t("path cannot be verified"):resolution?.status||HoiI18n.t("manual verification"))));
   const reasons={additional_baseline_missing:HoiI18n.t("Additional construction requires the pre-order building count or a target total."),building_completion_not_observed:HoiI18n.t("The reader detects the queue; the final level of this building is not exposed yet."),target_not_defined:HoiI18n.t("The guide does not specify a target total or level."),building_or_state_not_resolved:HoiI18n.t("Building or state not uniquely identified."),declared_manual_completion:HoiI18n.t("The criterion declared in the guide requires confirmation.")};
   Object.assign(reasons,{location_unresolved:HoiI18n.t("City, province or state not found in the local catalog. An unknown explicit ID is never replaced with another location."),location_ambiguous:HoiI18n.t("This location matches multiple destinations: specify the state and, for a province building, the province."),location_conflict:HoiI18n.t("The specified city, province and state are inconsistent. Correct the destination."),province_required:HoiI18n.t("This building requires a precise province: specify a unique city or province ID as well as the state.")});
   if(operation?.location){const p=operation.location;card.append(node('p',HoiI18n.t("Destination: ")+p.state_name+HoiI18n.t(" · state ")+p.state_id+(p.province_id?HoiI18n.t(" · province ")+p.province_id:'')+(p.via==='city'?HoiI18n.t(" · resolved from the specified city"):'')));}
   Object.assign(reasons,{production_equipment_unresolved:HoiI18n.t("Equipment type not bound to a game ID. Specify equipment_id or a unique English/localized name."),production_equipment_ambiguous:HoiI18n.t("The name matches multiple equipment types: a unique ID is required."),production_variant_ambiguous:HoiI18n.t("Ambiguous model or multiple models in one instruction: split the assignments."),production_variant_incompatible:HoiI18n.t("The specified model belongs to a different type than the required equipment."),production_target_missing:HoiI18n.t("Line identified: automatic start detection is available, but the factory allocation target is missing."),production_category_invalid:HoiI18n.t("Specify military or naval for the production line type."),production_mio_not_observed:HoiI18n.t("The production reader does not expose MIO assignment.")});
   if(operation?.reason==='production_live_assignment')card.append(node('p',HoiI18n.t("Live check of factories actually assigned to the required type/model; the requested count alone does not complete the instruction.")));
   if(operation?.reason==='production_relative_assignment')card.append(node('p',HoiI18n.t("The application records existing factory assignments when the instruction becomes available and verifies the required increase.")));
   if(operation?.reason==='baseline_from_live')card.append(node('p',HoiI18n.t("The baseline is recorded automatically from the game when the instruction becomes available, before the order. Completion requires that baseline plus the requested quantity.")));
   if(operation?.monitoring!=='automatic'&&reasons[operation?.reason])card.append(node('p',reasons[operation.reason]));
   if(resolution?.supporting_focus_ids?.length)card.append(node('p',HoiI18n.t("Guide path: ")+resolution.supporting_focus_ids.join(', ')));
   for(const excluded of resolution?.excluded_candidates||[]){const entry=excluded.entity;card.append(node('p',HoiI18n.t("Excluded from the path: ")+(entry.localized_name||entry.name||entry.id)+' ('+entry.id+HoiI18n.t(") · incompatible with ")+excluded.conflicting_focus_ids.join(', ')));}
   const contextExclusions=[...(resolution?.context_exclusions||[]),...(operation?.bindings||[]).flatMap(binding=>binding.excluded_candidates||[])];
   if(contextExclusions.length){const details=node('details');details.className='resolution-context';details.append(node('summary',HoiI18n.t("Why this binding?")));
    for(const excluded of contextExclusions){const entry=excluded.entity;details.append(node('p',(entry.name||entry.id)+' ('+entry.id+') · '+(excluded.reason==='declared_parent_or_role_conflict'?HoiI18n.t("event, group or role differs from the specified one"):HoiI18n.t("country or tree incompatible with the declared context"))));}
    card.append(details);
   }
   const upgrade=row.research_upgrade;
   if(upgrade)card.append(node('p',HoiI18n.t("Next level in the branch: ")+(upgrade.from_id||HoiI18n.t("start of the branch"))+' → '+row.action.entity.id));
   const label=node('label',HoiI18n.t("Title")),title=node('input');title.value=row.title||row.name||row.key;label.append(title);card.append(label);
   title.onchange=()=>{writeGuide(m.editStep(readGuide(),row.key,{title:title.value}));invalidate();};
   if(row.action.kind==='manual'){
    const label=node('label',HoiI18n.t("Tracking")),select=node('select');select.setAttribute?.('aria-label',HoiI18n.t("Tracking ")+row.key);
    for(const [key,text] of [['action',HoiI18n.t("Action to verify")],['reminder',HoiI18n.t("Method reminder · no completion tracking")]]){const option=node('option',text);option.value=key;select.append(option);}
    select.value=row.tracking||'action';select.onchange=()=>{try{writeGuide(m.editStep(readGuide(),row.key,{tracking:select.value}));invalidate();}catch(e){error(e);}};label.append(select);card.append(label);
    if(row.action.params?.category==='construction')card.append(constructionEditor(row));
   }
   const politicalReasons={commander_assignment_unobserved:HoiI18n.t("Commander and role bound to the catalog. Army/fleet assignment is not exposed by the reader: roster membership does not prove completion."),politics_identity_unresolved:HoiI18n.t("Political entity, law or appointment not uniquely bound to the local catalog. Specify an English name, trait or ID."),politics_identity_ambiguous:HoiI18n.t("Multiple identities match: specify the exact character and role."),event_identity_unresolved:HoiI18n.t("Event/option not identified: specify event_name and the English choice text."),event_identity_ambiguous:HoiI18n.t("Ambiguous event or option: event_id and a unique choice are required."),event_visible_option:HoiI18n.t("The event window and choice are detected live. Closing the event does not prove which answer was selected."),decision_identity_resolved:HoiI18n.t("Decision bound to the catalog. Availability, start and outcome are not qualified by the reader yet: no automatic progress."),method_reminder:HoiI18n.t("Method reminder: remains available without confirmations or blocking the sequence.")};
   if(politicalReasons[operation?.reason])card.append(node('p',politicalReasons[operation.reason]));
   if(row.action.entity){card.append(node('p',row.action.entity.kind+': '+row.action.entity.id+(row.action.entity.scope?' · '+row.action.entity.scope:'')));
    const selectLabel=node('label',HoiI18n.t("Catalog match")),select=node('select'),placeholder=node('option',HoiI18n.t("Find another binding…"));placeholder.value='';select.append(placeholder);
    function options(candidates){select.replaceChildren(placeholder);for(const candidate of candidates){const option=node('option',(candidate.localized_name||candidate.name)+' · '+candidate.id+(candidate.scope?' · '+candidate.scope:''));option.value=JSON.stringify({kind:candidate.kind,id:candidate.id,...(candidate.scope?{scope:candidate.scope}:{})});select.append(option);}}
    options(resolution?.candidates||[]);
    if(resolution?.status==='resolved'){const ref=row.action.entity;select.value=JSON.stringify({kind:ref.kind,id:ref.id,...(ref.scope?{scope:ref.scope}:{})});}
    const searchLabel=node('label',HoiI18n.t("Find a name or ID in the catalog")),search=node('input');search.type='search';searchLabel.append(search);card.append(searchLabel);
    search.oninput=()=>{const query=search.value.toLocaleLowerCase();options(entities.filter(item=>item.kind===row.action.entity.kind&&(!row.action.entity.scope||item.scope===row.action.entity.scope)&&[item.id,item.name,item.localized_name,...item.aliases||[]].some(text=>typeof text==='string'&&text.toLocaleLowerCase().includes(query))).slice(0,50));};
    select.onchange=()=>{if(select.value){writeGuide(m.editStep(readGuide(),row.key,{entity:JSON.parse(select.value)}));invalidate();}};selectLabel.append(select);card.append(selectLabel);
   }
   const construction=row.action.kind==='manual'&&row.action.params?.category==='construction';
   const sourceDetails=construction?node('details'):card;
   if(construction){sourceDetails.append(node('summary',HoiI18n.t("Original notes and quotations (unchanged by corrections)")));for(const text of row.action.params.instructions||[])card.append(node('p',text));}
   if(row.note)sourceDetails.append(node('p',row.note));
   if(row.depends_on?.length)card.append(node('p',HoiI18n.t("Depends on: ")+row.depends_on.join(', ')));
   for(const evidence of row.evidence||[])sourceDetails.append(node('blockquote',(evidence.quote||HoiI18n.t("Quotation missing"))+'\n— '+evidence.source_id+(evidence.timestamp_seconds!==undefined?' · '+evidence.timestamp_seconds+'s':'')));
   if(construction)card.append(sourceDetails);
   if(row.action.params){const p=node('pre',JSON.stringify(row.action.params,null,2));p.className='params';if(construction){const technical=node('details');technical.append(node('summary',HoiI18n.t("Technical parameters")),p);card.append(technical);}else card.append(p);}
   if(resolution?.status==='resolved'){
    resolvedCount++;const compact=node('article'),editor=node('details');compact.className='preview-card resolved';editor.className='association-editor';editor.append(node('summary',HoiI18n.t("Correct binding")),card);
    const match=resolution.entity||row.action.entity,candidate=resolution.candidates?.find(item=>item.id===match.id&&(!match.scope||item.scope===match.scope));
    compact.append(node('h3',candidate?.localized_name||candidate?.name||row.name||row.title||row.key),node('p',HoiI18n.t("Binding found automatically")),editor);resolvedItems.append(compact);
   }else $('preview').append(card);
  }
  if(resolvedCount){resolvedGroup.append(node('summary',resolvedCount+HoiI18n.t(" bindings resolved · no action required")),resolvedItems);$('preview').replaceChildren(resolvedGroup,...$('preview').children);}
 }
 async function loadDocument(text){const parsed=m.readDraft(text);if(parsed.sources)return request('/api/import/document',{document:JSON.parse(text)});return parsed;}
 async function validate(){const version=generation;const parsed=await loadDocument($('guideJson').value);if(version!==generation)return;
  const guide=parsed.guide;if(parsed.sources){sources=parsed.sources;sourceList();}m.normalise(guide);
  const value=await request('/api/import/preview',{guide,sources});if(version===generation)showReport(value);}
 $('copyPrompt').onclick=()=>run(async()=>{if(!navigator.clipboard?.writeText)throw Error(HoiI18n.t("Clipboard access unavailable: use Download prompt .txt"));await navigator.clipboard.writeText($('importPrompt').value);$('notice').textContent=HoiI18n.t("Prompt copied: paste it into your AI and attach the source guide.");});
 $('downloadPrompt').onclick=()=>download('hoi4-companion-prompt.txt',$('importPrompt').value,true);
 $('downloadCatalog').onclick=()=>{if(missingCountryMetadata())return;return run(async()=>{const groups=await aiSections();download('hoi4-companion-catalogo-id.json',{catalog_schema_version:1,reference_language:'english',catalog_scope:'all',localization_policy:'local_lookup',alias_policy:'english_reference_only',
  ...($('treeScope').value?{focus_tree_id:$('treeScope').value}:{}),...(groups.find(s=>s.id==='events')?.country_filter?{country_filter:groups.find(s=>s.id==='events').country_filter,
   section_country_filters:Object.fromEntries(groups.filter(s=>s.country_filter).map(s=>[s.id,s.country_filter]))}:{}),catalog:groups.flatMap(s=>s.catalog)});});};
 $('eventsMultiple').checked=true;
 $('catalogSection').onchange=catalogView;$('eventsMultiple').onchange=catalogView;$('treeScope').onchange=rebuildSections;
 $('downloadSection').onclick=()=>{const selected=selectedCatalog();if(!selected?.catalog.length||missingCountryMetadata(selected.id))return;return run(async()=>{let section=(await aiSections()).find(s=>s.id===selected.id);if(section.id==='events'&&$('eventsMultiple').checked)section={...section,...m.eventCatalog(section.catalog),catalog_scope:section.id};download(`hoi4-companion-catalogo-${section.id}.json`,section);});};
 $('sourceText').maxLength=180000;
 $('acquire').onclick=()=>run(async()=>{const source=await request('/api/import/source',{url:$('sourceUrl').value.trim(),text:$('sourceText').value});if(sources.some(src=>src.id===source.id))throw Error(HoiI18n.t("This source has already been acquired"));if(sources.length>=8)throw Error(HoiI18n.t("Maximum 8 sources"));sources.push(source);invalidate();sourceList();$('sourceText').value='';$('sourceUrl').value='';});
 $('packet').onclick=()=>run(async()=>{download('guide-import.packet.json',await request('/api/import/packet',{sources,tree_id:$('treeScope').value||null}));$('notice').textContent=HoiI18n.t("Package ready: give it to your AI and upload the returned guide JSON here.");});
 $('extract').onclick=()=>run(async()=>{const version=generation;const value=await request('/api/import/extract',{sources,tree_id:$('treeScope').value||null});if(version===generation){$('reviewed').checked=false;showReport(value);}});
 $('guideJson').oninput=invalidate;$('reviewed').onchange=controls;
 $('validate').onclick=()=>run(validate);
 $('saveDraft').onclick=()=>run(async()=>{const parsed=$('guideJson').value.trim()?m.readDraft($('guideJson').value):null;download('guide-import.draft.json',parsed?.authoring?parsed.document:{import_schema_version:1,guide:parsed?.guide||null,sources});});
 $('draftFile').onchange=()=>run(async()=>{const file=$('draftFile').files[0];if(!file)return;if(file.size>2000000)throw Error(HoiI18n.t("File exceeds 2 MB"));invalidate();const text=await file.text();$('guideJson').value=text;const version=generation,draft=await loadDocument(text);if(version!==generation)return;if(draft.sources)sources=draft.sources;if(draft.guide)writeGuide(draft.guide);sourceList();if(draft.guide)await validate();});
 $('exportGuide').onclick=()=>{if(m.canPublish(report,$('reviewed').checked,busy))download(report.guide.id+'.json',report.guide);};
 $('install').onclick=()=>run(async()=>{if(!report?.valid||!$('reviewed').checked)throw Error(HoiI18n.t("Validation and review required"));const result=await request('/api/import/install',{guide:report.guide,sources,reviewed:true});$('notice').textContent=HoiI18n.t("Guide added to local guides: ")+result.id;$('openGuide').href=result.href;$('openGuide').hidden=false;});
 (async()=>{try{const [status,catalog,prompt]=await Promise.all([request('/api/import/status'),request('/api/import/catalog'),request('/api/import/prompt')]);aiConfigured=status.ai_configured;
  $('importPrompt').value=prompt.prompt||'';
  entities=catalog.entities;$('notice').textContent=catalog.demo?HoiI18n.t("Demo catalog: configure HOI4 to resolve real IDs."):HoiI18n.t("Local catalog loaded · ")+catalog.entities.length+HoiI18n.t(" entities available.");
  $('aiStatus').textContent=aiConfigured?HoiI18n.t("OpenAI API configured · model ")+status.model:HoiI18n.t("API not configured. The external package workflow is available. To use the API, set OPENAI_API_KEY and HOI4_IMPORT_MODEL in the server's local environment.");
  for(const scope of [...new Set(catalog.entities.map(item=>item.scope).filter(Boolean))].sort()){const option=node('option',scope);option.value=scope;$('treeScope').append(option);}
  rebuildSections();
 }catch(e){error(e);}finally{controls();}})();
})();
