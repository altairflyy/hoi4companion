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
 function catalogLabel(section){if(section.id!=='events')return `${section.title} · ${section.catalog.length} voci`;let events=0,answers=0;for(const item of section.catalog){if(item.kind==='event')events++;else if(item.kind==='event_option')answers++;}return `${section.title} · eventi: ${events} · risposte: ${answers}`;}
 function catalogView(){
  const section=selectedCatalog(),events=$('catalogSection').value==='events';
  $('eventsFilter').hidden=!events;
  for(const option of $('catalogSection').children){const value=sections.find(s=>s.id===option.value);if(value)option.textContent=catalogLabel(exportedCatalog(value));}
  $('catalogSummary').textContent=section?catalogLabel(section)+'. '+(events&&$('eventsMultiple').checked?'Solo eventi con almeno due risposte distinte definite negli script.':''):'';
  const missing=sections.filter(s=>missingCountryMetadata(s.id)).map(s=>s.title);
  $('catalogWarning').hidden=!missing.length;$('catalogWarning').textContent=missing.length?'Filtro nazionale non attivo per '+missing.join('; ')+': il servizio usa ancora il catalogo precedente. Chiudi e riapri HOI4 Companion, poi ricarica questa pagina. I download delle sezioni interessate e del catalogo completo per nazione sono sospesi; puoi selezionare Tutte le nazioni per esportare il catalogo globale.':'';
  if(events&&section?.country_filter?.metadata_available){const filter=section.country_filter;$('catalogSummary').textContent+=` Nazione: ${filter.countries.join(', ')}. Esclusi ${filter.excluded_events} eventi incompatibili prima del filtro sulle risposte multiple. Copertura statica parziale: conservati gli eventi condivisi e con destinatario incerto.`;}
  else if(section?.id==='politics'&&section.country_filter?.metadata_available){const filter=section.country_filter;$('catalogSummary').textContent+=` Nazione: ${filter.countries.join(', ')}. Escluse ${filter.excluded_entries} voci incompatibili; conservate ${filter.retained_unknown_entries} con applicabilità incerta. Il tag originale è assunto dalla nazione selezionata. Copertura statica parziale.`;}
  else if(['events','politics'].includes(section?.id)&&!section.country_filter&&$('treeScope').value)$('catalogSummary').textContent+=' Nazione non determinabile dall’albero selezionato: sezione non filtrata per paese.';
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
 function invalidate(){generation++;report=null;$('reviewed').checked=false;$('validation').textContent='Bozza modificata: risolvi gli ID e valida di nuovo.';$('openGuide').hidden=true;controls();}
 function error(e){$('error').hidden=false;$('error').textContent=e.message||String(e);}
 async function request(path,body){const response=await fetch(path,{method:body===undefined?'GET':'POST',headers:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});const value=await response.json();if(!response.ok)throw Error(value.error||'Richiesta fallita');return value;}
 async function aiSections(){
  if(!aiCatalogPromise)aiCatalogPromise=request('/api/import/catalog?export=ai').then(value=>{
   if(value.localization_policy!=='local_lookup'||value.alias_policy!=='english_reference_only'||!Array.isArray(value.entities))throw Error('Il servizio usa ancora il vecchio export: riavvia HOI4 Companion e ricarica la pagina.');return value.entities;
  }).catch(error=>{aiCatalogPromise=null;throw error;});
  return m.catalogSections(await aiCatalogPromise,$('treeScope').value).map(section=>({...section,localization_policy:'local_lookup',alias_policy:'english_reference_only'}));
 }
 let videoTimer=null;
 function videoView(job){
  $('videoStatus').textContent=job.message+(job.encoded_seconds!==undefined&&job.status==='running'?` · ${Math.floor(job.encoded_seconds)} / ${Math.ceil(job.duration_seconds||0)} secondi codificati`:'');
  if(job.source_width&&job.source_height)$('videoStatus').textContent+=` · Sorgente reale: ${job.source_width}×${job.source_height}`;
  if(job.audio_status==='running')$('videoStatus').textContent+=' · Preparazione audio separato…';
  if(job.audio_warning)$('videoStatus').textContent+=' · '+job.audio_warning;
  $('prepareVideo').disabled=job.status==='running'||job.audio_status==='running';$('videoFiles').replaceChildren();
  const clock=n=>Math.floor(n/60)+':'+String(Math.floor(n%60)).padStart(2,'0');
  for(const part of job.parts||[]){const p=node('p'),a=node('a',`${part.name} · ${(part.bytes/1e6).toFixed(1)} MB · ${clock(part.start_seconds)}–${clock(part.end_seconds)}`);a.href=part.url;a.download=part.name;p.append(a);$('videoFiles').append(p);}
  if(job.audio){const p=node('p'),a=node('a',`Scarica traccia audio completa · ${job.audio.name} · ${(job.audio.bytes/1e6).toFixed(1)} MB`);a.href=job.audio.url;a.download=job.audio.name;p.append(a);$('videoFiles').append(p);}
  for(const [field,label]of [['manifest_url','Scarica indice dei tempi da allegare all’IA'],['description_url','Scarica descrizione del video']])if(job[field]){const p=node('p'),a=node('a',label);a.href=job[field];p.append(a);$('videoFiles').append(p);}
 }
 async function watchVideo(id){try{const job=await request('/api/import/video/status?id='+encodeURIComponent(id));videoView(job);if(job.status==='running'||job.audio_status==='running')videoTimer=setTimeout(()=>watchVideo(id),2000);}catch(e){$('videoStatus').textContent=e.message;$('prepareVideo').disabled=false;}}
 $('prepareVideo').onclick=async()=>{if(videoTimer)clearTimeout(videoTimer);$('prepareVideo').disabled=true;try{const job=await request('/api/import/video',{url:$('videoUrl').value.trim(),...($('videoRefreshSource').checked?{refresh_source:true}:{})});try{localStorage.setItem('hoi4-video-job',job.id);}catch{}videoView(job);watchVideo(job.id);}catch(e){$('videoStatus').textContent=e.message;$('prepareVideo').disabled=false;}};
 try{const id=localStorage.getItem('hoi4-video-job');if(id)watchVideo(id);}catch{}
 async function run(task){if(busy)return;busy=true;$('error').hidden=true;controls();try{await task();}catch(e){error(e);}finally{busy=false;controls();}}
 function download(name,value,text=false){const url=URL.createObjectURL(new Blob([text?value:JSON.stringify(value,null,2)+'\n'],{type:text?'text/plain;charset=utf-8':'application/json'})),a=node('a');a.href=url;a.download=name;document.body.append(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);}
 function readGuide(){return m.readDraft($('guideJson').value).guide;}
 function sourceList(){
  $('sourceCount').textContent=sources.length+' fonti acquisite';$('sources').replaceChildren();
  for(const src of sources){const details=node('details');details.className='source-card';details.append(node('summary',src.title+' · '+src.type+' · '+src.text.length+' caratteri'));
   details.append(node('p',src.url||'Testo locale'),node('p',(src.warnings||[]).join(' ')),node('pre',src.text));const remove=node('button','Rimuovi fonte');remove.type='button';remove.onclick=()=>{sources=sources.filter(item=>item.id!==src.id);invalidate();sourceList();};details.append(remove);$('sources').append(details);
  }
 }
 function writeGuide(value){$('guideJson').value=JSON.stringify(value,null,2);}
 function constructionEditor(row){
  const params=row.action.params,details=node('details');details.className='construction-editor';details.append(node('summary','Correggi costruzione'));
  details.append(node('p','Correggi l’istruzione nella copia caricata. Le citazioni e il file originale restano invariati. Dopo l’applicazione i collegamenti live vengono ricalcolati.'));
  function field(text,control){const label=node('label',text);control.setAttribute?.('aria-label',text+' · '+row.key);label.append(control);details.append(label);return control;}
  const stateSearch=field('Cerca regione',node('input'));stateSearch.type='search';stateSearch.placeholder='Nome inglese, italiano oppure ID';
  const stateSelect=field('Regione',node('select')),buildingSelect=field('Edificio',node('select'));
  const states=entities.filter(item=>item.kind==='state'),buildings=entities.filter(item=>item.kind==='building');
  const label=item=>(item.localized_name||item.name)+' · '+item.id;
  function stateOptions(){const current=stateSelect.value||String(params.state_id??''),query=stateSearch.value.trim().toLocaleLowerCase();stateSelect.replaceChildren();
   const empty=node('option','Scegli una regione…');empty.value='';stateSelect.append(empty);
   for(const item of states.filter(item=>[item.id,item.name,item.localized_name,...item.aliases||[]].some(text=>typeof text==='string'&&text.toLocaleLowerCase().includes(query)))){const option=node('option',label(item));option.value=item.id;stateSelect.append(option);}
   stateSelect.value=current;
  }
  stateOptions();stateSearch.oninput=stateOptions;
  const empty=node('option','Scegli un edificio…');empty.value='';buildingSelect.append(empty);
  for(const item of buildings){const option=node('option',label(item));option.value=item.id;buildingSelect.append(option);}buildingSelect.value=params.building_id||'';
  const mode=field('Obiettivo',node('select'));
  for(const [value,text]of [['additional','Aggiungi rispetto al conteggio iniziale'],['target_total','Raggiungi il totale / livello']]){const option=node('option',text);option.value=value;mode.append(option);}
  mode.value=params.target_level||params.target_quantity?'target_total':params.quantity_mode||'additional';
  const quantity=field('Quantità / livello',node('input'));quantity.type='number';quantity.min='1';quantity.step='1';quantity.value=params.target_level??params.target_quantity??params.quantity??'';
  const instruction=field('Istruzione corretta',node('textarea'));instruction.rows=3;instruction.value=(params.instructions||[]).join('\n');
  const apply=node('button','Applica correzione e verifica');apply.type='button';apply.onclick=()=>run(async()=>{
   const next=m.editConstruction(readGuide(),row.key,{state:states.find(item=>item.id===stateSelect.value),building:buildings.find(item=>item.id===buildingSelect.value),quantity:Number(quantity.value),quantity_mode:mode.value,instruction:instruction.value});
   writeGuide(next);invalidate();await validate();
  });details.append(apply);return details;
 }
 function showReport(value){report=value;$('issues').replaceChildren();$('sourceWarnings').replaceChildren();$('preview').replaceChildren();
  $('validation').textContent=value.valid?'Validazione superata: guida pronta per essere aggiunta. Controlla i dettagli estratti dalla fonte indicati sotto.':'Revisione necessaria: '+value.errors.length+' problemi bloccanti.';
  for(const message of value.errors)$('issues').append(node('li',message));
  for(const message of new Set(value.warnings))$('sourceWarnings').append(node('li',message));
  const operations=value.operations||[],count=mode=>operations.filter(row=>row.monitoring===mode).length;
  $('monitoringSummary').textContent=operations.length?count('automatic')+' automatiche · '+count('start_only')+' con solo avvio automatico · '+count('manual')+' da confermare · '+count('reminder')+' promemoria. Il monitoraggio si attiva nel walkthrough con lo stato live disponibile.':'';
  const visual=value.warnings.filter(text=>text.includes('osservazione visiva dichiarata')).length;
  $('reviewSummary').textContent=visual?visual+' istruzioni contengono dettagli ricavati dalle immagini del video: controlla quantità, regioni e composizioni. I dettagli degli avvisi sono espandibili.':'La revisione riguarda il contenuto della guida; gli ID e i collegamenti live vengono controllati dal programma.';
  if(!value.guide){if(value.draft)writeGuide(value.draft);return;}
  writeGuide(value.guide);
  const resolvedGroup=node('details'),resolvedItems=node('div');resolvedGroup.className='resolved-associations';resolvedItems.className='preview-cards';let resolvedCount=0;
  for(const [lane,rows]of Object.entries(value.guide.tracks))for(const row of rows){
   const resolution=value.resolutions.find(item=>item.key===row.key),operation=operations.find(item=>item.key===row.key),card=node('article');card.className='preview-card'+(resolution&&resolution.status!=='resolved'?' unresolved':'');
   const monitoring={automatic:'completamento automatico',start_only:'avvio automatico · esito da verificare',manual:'criterio ancora manuale',reminder:'promemoria · nessuna conferma'};
   card.append(node('p',lane+' · '+row.key+' · '+(operation?monitoring[operation.monitoring]:resolution?.status==='unresolved_route'?'percorso non verificabile':resolution?.status||'verifica manuale')));
   const reasons={additional_baseline_missing:'Per contare le costruzioni aggiuntive serve il numero di edifici prima dell’ordine oppure il totale da raggiungere.',building_completion_not_observed:'Il lettore rileva la coda; il livello finale di questo edificio non è ancora esposto.',target_not_defined:'La guida non specifica un totale o un livello da raggiungere.',building_or_state_not_resolved:'Edificio o regione non identificati univocamente.',declared_manual_completion:'Il criterio dichiarato nella guida richiede una conferma.'};
   Object.assign(reasons,{production_equipment_unresolved:'Tipo di equipaggiamento non associato a un ID del gioco. Specifica equipment_id o un nome inglese/localizzato univoco.',production_equipment_ambiguous:'Il nome corrisponde a più tipi di equipaggiamento: serve un ID univoco.',production_variant_ambiguous:'Modello ambiguo o più modelli nella stessa istruzione: separa le assegnazioni.',production_variant_incompatible:'Il modello indicato appartiene a un tipo diverso dall’equipaggiamento richiesto.',production_target_missing:'Linea identificata: avvio automatico disponibile, ma manca il numero di fabbriche da assegnare.',production_category_invalid:'Specifica military oppure naval per il tipo di linea.',production_mio_not_observed:'L’assegnazione del MIO non è esposta dal lettore di produzione.'});
   if(operation?.reason==='production_live_assignment')card.append(node('p','Controllo live sulle fabbriche effettivamente assegnate al tipo/modello richiesto; il solo numero richiesto non completa l’istruzione.'));
   if(operation?.reason==='production_relative_assignment')card.append(node('p','Il programma registra le fabbriche già assegnate quando l’istruzione diventa disponibile e verifica l’aumento richiesto.'));
   if(operation?.reason==='baseline_from_live')card.append(node('p','Il conteggio iniziale viene registrato automaticamente dalla partita quando l’istruzione è disponibile, prima dell’ordine. Il completamento richiede quel conteggio più la quantità richiesta.'));
   if(operation?.monitoring!=='automatic'&&reasons[operation?.reason])card.append(node('p',reasons[operation.reason]));
   if(resolution?.supporting_focus_ids?.length)card.append(node('p','Percorso della guida: '+resolution.supporting_focus_ids.join(', ')));
   for(const excluded of resolution?.excluded_candidates||[]){const entry=excluded.entity;card.append(node('p','Escluso dal percorso: '+(entry.localized_name||entry.name||entry.id)+' ('+entry.id+') · incompatibile con '+excluded.conflicting_focus_ids.join(', ')));}
   const upgrade=row.research_upgrade;
   if(upgrade)card.append(node('p','Prossimo livello del ramo: '+(upgrade.from_id||'inizio del ramo')+' → '+row.action.entity.id));
   const label=node('label','Titolo'),title=node('input');title.value=row.title||row.name||row.key;label.append(title);card.append(label);
   title.onchange=()=>{writeGuide(m.editStep(readGuide(),row.key,{title:title.value}));invalidate();};
   if(row.action.kind==='manual'){
    const label=node('label','Monitoraggio'),select=node('select');select.setAttribute?.('aria-label','Monitoraggio '+row.key);
    for(const [key,text] of [['action','Azione da verificare'],['reminder','Promemoria sul metodo · senza completamento']]){const option=node('option',text);option.value=key;select.append(option);}
    select.value=row.tracking||'action';select.onchange=()=>{try{writeGuide(m.editStep(readGuide(),row.key,{tracking:select.value}));invalidate();}catch(e){error(e);}};label.append(select);card.append(label);
    if(row.action.params?.category==='construction')card.append(constructionEditor(row));
   }
   const politicalReasons={politics_identity_unresolved:'Nome politico, legge o incarico non associato univocamente al catalogo locale. Specifica nome inglese, tratto o ID.',politics_identity_ambiguous:'Più identità corrispondono: indica personaggio e ruolo precisi.',event_identity_unresolved:'Evento/opzione non identificati: specifica event_name e testo inglese della scelta.',event_identity_ambiguous:'Evento o opzione ambigui: serve event_id e una scelta univoca.',event_visible_option:'La finestra e la scelta vengono riconosciute live. La chiusura dell’evento non prova la risposta scelta.',decision_identity_resolved:'Decisione associata al catalogo. Disponibilità, avvio ed esito non ancora qualificati dal lettore: nessun avanzamento automatico.',method_reminder:'Indicazione sul metodo: resta consultabile, senza conferme e senza bloccare la sequenza.'};
   if(politicalReasons[operation?.reason])card.append(node('p',politicalReasons[operation.reason]));
   if(row.action.entity){card.append(node('p',row.action.entity.kind+': '+row.action.entity.id+(row.action.entity.scope?' · '+row.action.entity.scope:'')));
    const selectLabel=node('label','Corrispondenza nel catalogo'),select=node('select'),placeholder=node('option','Cerca un’altra associazione…');placeholder.value='';select.append(placeholder);
    function options(candidates){select.replaceChildren(placeholder);for(const candidate of candidates){const option=node('option',(candidate.localized_name||candidate.name)+' · '+candidate.id+(candidate.scope?' · '+candidate.scope:''));option.value=JSON.stringify({kind:candidate.kind,id:candidate.id,...(candidate.scope?{scope:candidate.scope}:{})});select.append(option);}}
    options(resolution?.candidates||[]);
    if(resolution?.status==='resolved'){const ref=row.action.entity;select.value=JSON.stringify({kind:ref.kind,id:ref.id,...(ref.scope?{scope:ref.scope}:{})});}
    const searchLabel=node('label','Cerca nome o ID nel catalogo'),search=node('input');search.type='search';searchLabel.append(search);card.append(searchLabel);
    search.oninput=()=>{const query=search.value.toLocaleLowerCase();options(entities.filter(item=>item.kind===row.action.entity.kind&&(!row.action.entity.scope||item.scope===row.action.entity.scope)&&[item.id,item.name,item.localized_name,...item.aliases||[]].some(text=>typeof text==='string'&&text.toLocaleLowerCase().includes(query))).slice(0,50));};
    select.onchange=()=>{if(select.value){writeGuide(m.editStep(readGuide(),row.key,{entity:JSON.parse(select.value)}));invalidate();}};selectLabel.append(select);card.append(selectLabel);
   }
   const construction=row.action.kind==='manual'&&row.action.params?.category==='construction';
   const sourceDetails=construction?node('details'):card;
   if(construction){sourceDetails.append(node('summary','Note e citazioni originali (non cambiano con la correzione)'));for(const text of row.action.params.instructions||[])card.append(node('p',text));}
   if(row.note)sourceDetails.append(node('p',row.note));
   if(row.depends_on?.length)card.append(node('p','Dipende da: '+row.depends_on.join(', ')));
   for(const evidence of row.evidence||[])sourceDetails.append(node('blockquote',(evidence.quote||'Citazione assente')+'\n— '+evidence.source_id+(evidence.timestamp_seconds!==undefined?' · '+evidence.timestamp_seconds+'s':'')));
   if(construction)card.append(sourceDetails);
   if(row.action.params){const p=node('pre',JSON.stringify(row.action.params,null,2));p.className='params';if(construction){const technical=node('details');technical.append(node('summary','Parametri tecnici'),p);card.append(technical);}else card.append(p);}
   if(resolution?.status==='resolved'){
    resolvedCount++;const compact=node('article'),editor=node('details');compact.className='preview-card resolved';editor.className='association-editor';editor.append(node('summary','Correggi associazione'),card);
    const match=resolution.entity||row.action.entity,candidate=resolution.candidates?.find(item=>item.id===match.id&&(!match.scope||item.scope===match.scope));
    compact.append(node('h3',candidate?.localized_name||candidate?.name||row.name||row.title||row.key),node('p','Associazione trovata automaticamente'),editor);resolvedItems.append(compact);
   }else $('preview').append(card);
  }
  if(resolvedCount){resolvedGroup.append(node('summary',resolvedCount+' associazioni risolte · nessuna azione richiesta'),resolvedItems);$('preview').replaceChildren(resolvedGroup,...$('preview').children);}
 }
 async function loadDocument(text){const parsed=m.readDraft(text);if(parsed.sources)return request('/api/import/document',{document:JSON.parse(text)});return parsed;}
 async function validate(){const version=generation;const parsed=await loadDocument($('guideJson').value);if(version!==generation)return;
  const guide=parsed.guide;if(parsed.sources){sources=parsed.sources;sourceList();}m.normalise(guide);
  const value=await request('/api/import/preview',{guide,sources});if(version===generation)showReport(value);}
 $('copyPrompt').onclick=()=>run(async()=>{if(!navigator.clipboard?.writeText)throw Error('Copia dagli appunti non disponibile: usa Scarica prompt .txt');await navigator.clipboard.writeText($('importPrompt').value);$('notice').textContent='Prompt copiato: incollalo nella tua IA e aggiungi la guida da convertire.';});
 $('downloadPrompt').onclick=()=>download('hoi4-companion-prompt.txt',$('importPrompt').value,true);
 $('downloadCatalog').onclick=()=>{if(missingCountryMetadata())return;return run(async()=>{const groups=await aiSections();download('hoi4-companion-catalogo-id.json',{catalog_schema_version:1,reference_language:'english',catalog_scope:'all',localization_policy:'local_lookup',alias_policy:'english_reference_only',
  ...($('treeScope').value?{focus_tree_id:$('treeScope').value}:{}),...(groups.find(s=>s.id==='events')?.country_filter?{country_filter:groups.find(s=>s.id==='events').country_filter,
   section_country_filters:Object.fromEntries(groups.filter(s=>s.country_filter).map(s=>[s.id,s.country_filter]))}:{}),catalog:groups.flatMap(s=>s.catalog)});});};
 $('eventsMultiple').checked=true;
 $('catalogSection').onchange=catalogView;$('eventsMultiple').onchange=catalogView;$('treeScope').onchange=rebuildSections;
 $('downloadSection').onclick=()=>{const selected=selectedCatalog();if(!selected?.catalog.length||missingCountryMetadata(selected.id))return;return run(async()=>{let section=(await aiSections()).find(s=>s.id===selected.id);if(section.id==='events'&&$('eventsMultiple').checked)section={...section,...m.eventCatalog(section.catalog),catalog_scope:section.id};download(`hoi4-companion-catalogo-${section.id}.json`,section);});};
 $('sourceText').maxLength=180000;
 $('acquire').onclick=()=>run(async()=>{const source=await request('/api/import/source',{url:$('sourceUrl').value.trim(),text:$('sourceText').value});if(sources.some(src=>src.id===source.id))throw Error('Questa fonte è già acquisita');if(sources.length>=8)throw Error('Massimo 8 fonti');sources.push(source);invalidate();sourceList();$('sourceText').value='';$('sourceUrl').value='';});
 $('packet').onclick=()=>run(async()=>{download('guide-import.packet.json',await request('/api/import/packet',{sources,tree_id:$('treeScope').value||null}));$('notice').textContent='Pacchetto pronto: forniscilo alla tua IA e carica qui il JSON della guida restituito.';});
 $('extract').onclick=()=>run(async()=>{const version=generation;const value=await request('/api/import/extract',{sources,tree_id:$('treeScope').value||null});if(version===generation){$('reviewed').checked=false;showReport(value);}});
 $('guideJson').oninput=invalidate;$('reviewed').onchange=controls;
 $('validate').onclick=()=>run(validate);
 $('saveDraft').onclick=()=>run(async()=>{const parsed=$('guideJson').value.trim()?m.readDraft($('guideJson').value):null;download('guide-import.draft.json',parsed?.authoring?parsed.document:{import_schema_version:1,guide:parsed?.guide||null,sources});});
 $('draftFile').onchange=()=>run(async()=>{const file=$('draftFile').files[0];if(!file)return;if(file.size>2000000)throw Error('File oltre 2 MB');invalidate();const text=await file.text();$('guideJson').value=text;const version=generation,draft=await loadDocument(text);if(version!==generation)return;if(draft.sources)sources=draft.sources;if(draft.guide)writeGuide(draft.guide);sourceList();if(draft.guide)await validate();});
 $('exportGuide').onclick=()=>{if(m.canPublish(report,$('reviewed').checked,busy))download(report.guide.id+'.json',report.guide);};
 $('install').onclick=()=>run(async()=>{if(!report?.valid||!$('reviewed').checked)throw Error('Validazione e revisione necessarie');const result=await request('/api/import/install',{guide:report.guide,sources,reviewed:true});$('notice').textContent='Guida aggiunta alle guide locali: '+result.id;$('openGuide').href=result.href;$('openGuide').hidden=false;});
 (async()=>{try{const [status,catalog,prompt]=await Promise.all([request('/api/import/status'),request('/api/import/catalog'),request('/api/import/prompt')]);aiConfigured=status.ai_configured;
  $('importPrompt').value=prompt.prompt||'';
  entities=catalog.entities;$('notice').textContent=catalog.demo?'Catalogo dimostrativo: configura HOI4 per risolvere gli ID reali.':'Catalogo locale caricato · '+catalog.entities.length+' entità disponibili.';
  $('aiStatus').textContent=aiConfigured?'API OpenAI configurata · modello '+status.model:'API non configurata. Il percorso con pacchetto esterno è disponibile. Per l’API imposta OPENAI_API_KEY e HOI4_IMPORT_MODEL nell’ambiente locale del server.';
  for(const scope of [...new Set(catalog.entities.map(item=>item.scope).filter(Boolean))].sort()){const option=node('option',scope);option.value=scope;$('treeScope').append(option);}
  rebuildSections();
 }catch(e){error(e);}finally{controls();}})();
})();
