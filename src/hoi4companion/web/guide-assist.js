'use strict';
// Shared contextual controls for the existing, game-shaped trees.
(()=>{
 const host=document.getElementById('guideAssist');if(!host)return;
 const lane=host.dataset.lane,labels={next:'PROSSIMO',active:'IN CORSO',upcoming:'POI',waiting:'IN ATTESA',preview:'ANTEPRIMA'};
 let value=null,navigate=null;
 const el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
 function marker(id,scope){return value?.view?.lanes[lane].markers.find(m=>m.id===id&&(!m.scope||m.scope===scope));}
 function paint(nodes,scope){for(const node of nodes){
  const id=node.dataset.focusId||node.dataset.techId,m=marker(id,scope);
  for(const tone of Object.keys(labels))node.classList.toggle('guide-'+tone,m?.tone===tone);
  node.querySelector('.guide-node-label')?.remove();
  if(m){node.dataset.guideStep=m.key;node.append(el('span','guide-node-label',labels[m.tone]));const original=node.getAttribute('aria-label')||'';node.setAttribute('aria-label',original.split(' · Guida:')[0]+' · Guida: '+labels[m.tone]);}
  else{delete node.dataset.guideStep;const original=node.getAttribute('aria-label')||'';node.setAttribute('aria-label',original.split(' · Guida:')[0]);}
 }}
 function details(id,scope){
  if(!value?.guide||!value.view||value.view.mode.startsWith('wrong_'))return null;
  const rows=Object.values(value.view.model.tracks).flatMap(t=>t.rows).filter(row=>row.target===id&&row.action.kind===(lane==='focus'?'select_focus':'research')&&(!row.action.entity.scope||row.action.entity.scope===scope));
  const row=rows.find(r=>!['completed','not_applicable'].includes(r.runtime_status))||rows[0];if(!row)return null;
  const box=el('section','guide-node-detail');box.append(el('strong','',marker(id,scope)?labels[marker(id,scope).tone]+' · '+value.guide.title:'Nella guida · '+value.guide.title));
  if(row.note)box.append(el('p','',row.note));
  const conditions=[row.when,row.priority_when,row.skip_when,row.completion_condition,(value.guide.phases||[]).find(p=>p.id===row.phase)?.when];
  for(const key of new Set(conditions.flatMap(node=>HoiContracts.checkKeys(node)))){
   const label=el('label','guide-check'),input=el('input');input.type='checkbox';input.checked=value.progress.checks[key]===true;input.disabled=value.pending>0;
   input.addEventListener('change',()=>window.hoiGuide.confirm(key,input.checked).catch(()=>{}));label.append(input,el('span','',row.confirmation||'Ho verificato questa condizione nel gioco'));box.append(label);
  }
  const a=el('a','','Istruzioni operative ↗');a.href=window.hoiGuide.withGuide('/walkthrough');box.append(a);return box;
 }
 function render(){host.replaceChildren();if(!value?.guide){host.append(el('p','',value?.error||'Caricamento guida…'));return;}
  const header=el('div','guide-assist-heading'),label=el('label','','Guida '),select=el('select');select.setAttribute('aria-label','Guida da evidenziare');
  for(const item of value.catalog){const opt=el('option','',item.title);opt.value=item.id;opt.selected=item.id===value.guide.id;select.append(opt);}select.disabled=value.pending>0;select.addEventListener('change',()=>window.hoiGuide.select(select.value));label.append(select);header.append(label);
  const a=el('a','','Istruzioni operative ↗');a.href=window.hoiGuide.withGuide('/walkthrough');header.append(a);host.append(header);
  for(const link of document.querySelectorAll('.view-tabs a, .side a.nav')){const href=link.getAttribute('href');if(href?.startsWith('/'))link.href=window.hoiGuide.withGuide(href);}
  const view=value.view,plan=view.lanes[lane],summary=el('div','guide-assist-summary'),status=el('strong');
  const marked=plan.markers.filter(m=>m.tone!=='active'),first=marked[0];
  if(view.mode.startsWith('wrong_'))status.textContent='Guida sospesa: la partita non corrisponde';
  else if(plan.blocker)status.textContent='Prima di continuare: '+plan.blocker.name;
  else if(first)status.textContent=labels[first.tone]+' · '+(first.row.name||first.id);
  else status.textContent=plan.markers.length?'Passaggio della guida in corso':'Nessun altro elemento prescritto nella guida';
  summary.append(status);
  if(first||plan.next){const button=el('button','line-button','Mostra '+(lane==='focus'?'focus':'ricerca'));button.type='button';button.addEventListener('click',()=>{const row=first?.row||plan.next;if(navigate)Promise.resolve(navigate(row)).catch(e=>{status.textContent=e.message;});else if(row.href)location.assign(window.hoiGuide.withGuide(row.href));});summary.append(button);}
  if(plan.blocker){const a=el('a','','Verifica il passaggio');a.href=window.hoiGuide.withGuide('/walkthrough#operation-'+plan.blocker.key);summary.append(a);}
  host.append(summary);
  const note=view.mode==='live'?'Oro: prossimo passo · azzurro: in corso · tratteggio: attesa o passo successivo. La disponibilità finale si verifica nel gioco.':'Anteprima del percorso dal suo inizio: nessuna selezione attuale verificata. Collega una partita valida per aggiornare le evidenziazioni.';
  host.append(el('p','guide-assist-note',note));if(value.error)host.append(el('p','guide-assist-error',value.error));
 }
 window.hoiGuideTree={paint,details,marker,connect(fn){navigate=fn;}};
 window.hoiGuide.subscribe(next=>{value=next;render();window.dispatchEvent(new CustomEvent('hoi-guide-change'));});
})();
