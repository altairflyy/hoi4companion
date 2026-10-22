'use strict';
(()=>{
 const $=id=>document.getElementById(id),el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
 const categories={all:'Tutte',construction:'Fabbriche e infrastrutture',production:'Produzione',projects:'Progetti speciali',politics:'Politica e leggi',characters:'Personaggi',army:'Forze armate',templates:'Template e mezzi',checkpoints:'Eventi e condizioni'};
 let value=null,filter='all',undo=null,lastGuide=null;
 if(location.pathname==='/hud'){document.body.classList.add('hud-view');const token=new URLSearchParams(location.search).get('instance')||'';document.title='HOI4 Companion HUD '+(/^[a-f0-9]{32}$/.test(token)?token:'');}
 const sourceLink=seconds=>{try{const url=new URL(value.guide.source?.url||value.guide.sources?.[0]?.url);if(!['http:','https:'].includes(url.protocol))return null;if(Number.isFinite(seconds))url.searchParams.set('t',String(Math.floor(seconds)));return url.href;}catch{return null;}};
 const clock=seconds=>Math.floor(seconds/60)+':'+String(Math.floor(seconds%60)).padStart(2,'0');
 const name=row=>row.name||row.title||row.key;
 function badge(row){if(row.tracking==='reminder')return row.runtime_status==='waiting'?'Promemoria · più avanti':row.runtime_status==='unverifiable'?'Promemoria · momento da verificare':'Promemoria';return row.runtime_status==='completed'?'Completata':row.runtime_status==='not_applicable'?'Non applicabile':row.runtime_status==='waiting'?'Più avanti':row.runtime_status==='active'?(row.start_basis==='live'?'Avviata · rilevata live':row.start_basis==='condition'?'Avviata · criterio verificato':'Avviata · in corso (manuale)'):row.started_when?(row.selection?.value===false?'Nessun ordine rilevato':'Avvio non verificabile'):row.runtime_reason==='manual_confirmation_required'||row.runtime_status==='ready'?'Da verificare nel gioco':'Momento da verificare';}
 function confirm(key,text){const label=el('label','checkpoint-check'),input=el('input');input.type='checkbox';input.checked=value.progress.checks[key]===true;input.disabled=value.pending>0;input.setAttribute('data-check',key);input.addEventListener('change',()=>window.hoiGuide.confirm(key,input.checked).catch(()=>{}));label.append(input,el('span','',text));return label;}
 function controls(row,box){
  if(row.tracking==='reminder'){box.append(el('p','operation-copy','Indicazione sul metodo: nessuna conferma richiesta, non blocca gli altri passaggi.'));return;}
  if(row.started_when&&row.runtime_status!=='completed'){
   box.append(el('p','operation-start-note',row.runtime_status==='active'?'Avvio rilevato dalla condizione della guida. Il solo avvio non prova il completamento.':'Avvio automatico: attendo che la condizione della guida risulti verificata.'));
  }
  if(['manual','special_project'].includes(row.action.kind)&&!row.started_when&&row.selection&&row.runtime_status!=='completed'){
   const started=value.progress.started?.[row.key]===true,button=el('button','operation-start',started?'Annulla avvio segnalato':'Ho impartito l’ordine');
   button.type='button';button.disabled=value.pending>0;button.setAttribute('data-start',row.key);button.setAttribute('aria-pressed',String(started));button.onclick=()=>window.hoiGuide.start(row.key,!started).catch(()=>{});box.append(button);
   if(row.runtime_status==='active')box.append(el('p','operation-start-note','Ordine impartito manualmente; completamento ancora da verificare. Nessuna dipendenza è sbloccata dal solo avvio.'));
  }
  if(['manual','special_project'].includes(row.action.kind)&&!row.completion_condition)box.append(confirm(row.key,'Completata · '+(row.confirmation||'criterio finale verificato nel gioco')));
  const phase=value.guide.phases?.find(p=>p.id===row.phase);
  const keys=new Set([row.when,row.priority_when,row.skip_when,row.started_when,phase?.when].flatMap(node=>HoiContracts.checkKeys(node)));
  if(row.completion_condition)for(const key of HoiContracts.checkKeys(row.completion_condition))if(key!==row.action.params?.legacy_key)keys.add(key);
  const finalKeys=new Set(HoiContracts.checkKeys(row.completion_condition));
  for(const key of keys)box.append(confirm(key,finalKeys.has(key)?'Criterio finale verificato nel gioco':key===row.key?'Completata · '+(row.confirmation||'criterio finale verificato nel gioco'):'Condizione verificata nel gioco'));
 }
 function card(row,highlight=false){const node=el('article','operation-card'+(row.tracking==='reminder'?' reminder':'')+(row.event_observation?.visible?' event-visible':'')+(highlight?' highlighted':'')+(row.runtime_status==='completed'?' completed':'')+(row.runtime_status==='active'?' in-progress':''));node.id=(highlight?'next-operation-':'operation-')+row.key;node.dataset.step=row.key;
  const top=el('div','operation-top');top.append(el('span','eyebrow',categories[row.category]||'ISTRUZIONE'),el('span','badge',badge(row)));node.append(top,el('h3','',name(row)));
  node.append(el('p','operation-timing','Quando: '+row.timing));
  if(row.instructions?.length){const list=el('ol','operation-instructions');for(const line of row.instructions)list.append(el('li','',line));node.append(list);}
  if(row.event_observation){const e=row.event_observation,p=row.action.params;node.append(el('p','operation-observations',e.visible===true?'EVENTO APERTO · '+p.event_id:e.visible===false?'Evento non aperto al momento.':'Visibilità evento non verificabile.'));if(p.event_option_id)node.append(el('p','event-choice','Scelta consigliata: '+(p.event_option_name||p.event_option||p.event_option_id)+(e.option_available===true?' · disponibile nella finestra':e.option_available===false?' · non presente tra le scelte correnti':' · disponibilità non verificabile')));}
  if(row.note)node.append(el('p','operation-copy',row.note));
  if(row.construction){const c=row.construction;node.append(el('p','operation-observations',c.target===null?'Conteggio iniziale non ancora registrato: apri la guida prima di impartire l’ordine.':`Edifici nella regione: ${c.current??'non verificabile'} / ${c.target} · iniziali: ${c.baseline}`));}
  if(row.production){const p=row.production,target=p.target===null?'obiettivo non determinato':p.target;
   node.append(el('p','operation-observations',`Produzione · fabbriche assegnate: ${p.assigned??'non verificabile'} / ${target} · richieste: ${p.requested??'—'} · attive: ${p.active??'—'} · danneggiate: ${p.damaged??'—'}`));
   if(p.baseline!==null)node.append(el('p','operation-copy','Assegnate inizialmente: '+p.baseline));
   if(p.reason==='baseline_missing')node.append(el('p','operation-copy','Il conteggio iniziale deve essere registrato prima di aumentare la richiesta di fabbriche.'));
   if(p.lines.length){const list=el('ul','operation-observations');for(const line of p.lines)list.append(el('li','',`${line.name} · ${line.equipment_name||line.equipment} · ${line.category==='naval'?'cantieri navali':'fabbriche militari'}: ${line.assigned_factories} assegnate`));node.append(list);}
  }
  if(row.construction_observations?.length){const list=el('ul','operation-observations');for(const line of row.construction_observations)list.append(el('li','',`${line.name||line.building_id} · ${line.state_name||'regione '+line.state_id}: ${line.amount} in coda · ${line.active_factories} fabbriche attive`));node.append(el('p','eyebrow','ORDINI LETTI LIVE'),list);}
  if(row.blocked_by?.length){const rows=value.view.operations;node.append(el('p','operation-wait','Prima: '+row.blocked_by.map(key=>name(rows.find(r=>r.key===key)||{key})).join(', ')));}
  controls(row,node);const href=sourceLink(row.source_seconds);if(href){const a=el('a','operation-source','Fonte'+(Number.isFinite(row.source_seconds)?' · video '+clock(row.source_seconds):'')+' ↗');a.href=href;a.target='_blank';a.rel='noopener noreferrer';node.append(a);}return node;
 }
 function links(){const {guide,view}=value;const focus=view.lanes.focus.next,tech=view.lanes.research.next;
  const scope=guide.applies_to?.tree_id;const baseFocus=scope?'/?tree='+encodeURIComponent(scope):'/';
  $('focusTreeLink').href=window.hoiGuide.withGuide(focus?.href||baseFocus);$('researchTreeLink').href=window.hoiGuide.withGuide(tech?.href||'/technologies');$('hudLink').href=window.hoiGuide.withGuide('/walkthrough');
  for(const a of document.querySelectorAll('.topbar a'))a.href=window.hoiGuide.withGuide(a.getAttribute('href'));
 }
 function render(next){value=next;if(!value.guide){$('error').hidden=false;$('error').textContent=value.error||'Caricamento della guida…';return;}
  const {guide,view}=value;$('error').hidden=!value.error;$('error').textContent=value.error;
  if(lastGuide!==guide){lastGuide=guide;filter='all';$('guideTitle').textContent=guide.title;$('subtitle').textContent=guide.subtitle;
   $('guideSelect').replaceChildren();for(const item of value.catalog){const option=el('option','',item.title);option.value=item.id;option.selected=item.id===guide.id;$('guideSelect').append(option);}
   const href=sourceLink();$('sourceLink').hidden=!href;if(href)$('sourceLink').href=href;$('sourceBasis').textContent=[guide.source?.author,guide.source?.published,guide.source?.basis].filter(Boolean).join(' · ');
   $('assumptions').replaceChildren(...(guide.assumptions||[]).map(text=>el('li','',text)));$('catalogWarnings').replaceChildren(...(guide.catalog_warnings||[]).map(text=>el('p','',text)));
   $('chapters').replaceChildren(...(guide.source?.chapters||[]).map(chapter=>{const li=el('li'),a=el('a','',clock(chapter.start_time)+' · '+chapter.title);a.href=sourceLink(chapter.start_time);a.target='_blank';a.rel='noopener noreferrer';li.append(a);return li;}));
  }
  $('guideSelect').disabled=value.pending>0;$('reset').disabled=value.pending>0;links();
  $('mode').textContent={live:'Percorso collegato · '+(view.country||'partita corrente'),preview:'Anteprima della guida',wrong_country:'Guida sospesa · nazione diversa',wrong_tree:'Guida sospesa · albero diverso'}[view.mode];
  $('modeNote').textContent=view.mode==='live'?'Focus e ricerche si aggiornano nei rispettivi alberi. Le operazioni con criteri live avanzano automaticamente; i promemoria restano consultabili senza conferme. Conferma solo i criteri non osservabili.':'Le istruzioni restano consultabili. Senza una partita compatibile non viene dedotto alcun avanzamento.';
  const allRows=value.view.operations,rows=allRows.filter(r=>r.tracking!=='reminder'),done=rows.filter(r=>r.runtime_status==='completed').length,active=rows.filter(r=>r.runtime_status==='active').length;$('operationProgress').textContent=done+'/'+rows.length;$('operationActive').textContent=active?active+' in corso':'';
  const eligible=rows.filter(row=>!['completed','not_applicable','waiting','active'].includes(row.runtime_status));
  const upcoming=rows.find(row=>row.event_observation?.visible===true&&!['completed','not_applicable'].includes(row.runtime_status))||eligible.find(row=>!['dependency_unknown','phase_condition_unknown','condition_unknown','live_state_unavailable'].includes(row.runtime_reason))||eligible[0];
  $('nextOperation').replaceChildren();if(upcoming){
   $('nextOperation').append(el('p','eyebrow','PROSSIMA ISTRUZIONE DA VERIFICARE'));
   if(location.pathname==='/hud')$('nextOperation').append(card(upcoming,true));
   else{const summary=el('div','operation-summary'),copy=el('div');copy.append(el('strong','',name(upcoming)),el('p','',upcoming.timing));const a=el('a','','Mostra istruzione ↓');a.href='#operation-'+upcoming.key;a.onclick=()=>{filter=upcoming.category;render(value);};summary.append(copy,a);$('nextOperation').append(summary);}
  }else $('nextOperation').append(el('p','',rows.length?'Non ci sono altre istruzioni da verificare ora. Consulta le attività successive.':'Questa guida non contiene ancora istruzioni operative.'));
  const filters=$('operationFilters');filters.replaceChildren();for(const [key,label]of Object.entries(categories))if(key==='all'||allRows.some(r=>r.category===key)){const b=el('button','operation-filter',label);b.type='button';b.setAttribute('aria-pressed',String(filter===key));b.onclick=()=>{filter=key;render(value);};filters.append(b);}
  const cards=$('operationCards');cards.replaceChildren();const visible=allRows.filter(row=>(filter==='all'||row.category===filter)&&(!$('hideDone').checked||row.runtime_status!=='completed'));
  for(const row of visible)cards.append(card(row));if(!visible.length)cards.append(el('p','empty-operations','Nessuna istruzione da verificare in questa categoria.'));
 }
 $('guideSelect').onchange=()=>window.hoiGuide.select($('guideSelect').value);
 $('hideDone').onchange=()=>render(value);
 $('reset').onclick=async()=>{try{undo=await window.hoiGuide.newRun();const box=$('resetNotice');box.replaceChildren(document.createTextNode('Nuova checklist. '));const b=el('button','','Annulla');b.type='button';b.onclick=async()=>{if(!undo||value.pending)return;await window.hoiGuide.restore(undo);undo=null;box.hidden=true;};box.append(b);box.hidden=false;}catch{}};
 window.hoiGuide.subscribe(render);
})();
