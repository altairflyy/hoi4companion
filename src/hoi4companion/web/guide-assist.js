'use strict';
// Shared contextual controls for the existing, game-shaped trees.
(()=>{
 const host=document.getElementById('guideAssist');if(!host)return;
 const lane=host.dataset.lane,labels={next:HoiI18n.t("NEXT"),choice:HoiI18n.t("CHOICE"),alternative:HoiI18n.t('ALTERNATIVE'),active:HoiI18n.t("IN PROGRESS"),upcoming:'POI',waiting:HoiI18n.t("WAITING"),preview:HoiI18n.t("PREVIEW")};
 let value=null,navigate=null;
 const el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
 function criteria(parent,row){
  const r=row?.focus_readiness;if(!r)return;
  for(const check of r.checks||[]){
   if(!['country.metrics.war_support','country.metrics.stability'].includes(check.fact))continue;
   const label=check.fact.endsWith('war_support')?HoiI18n.t('War support'):HoiI18n.t('Stability');
   const amount=typeof check.actual==='number'?(check.actual*100).toFixed(3)+'%':HoiI18n.t('Unavailable');
   parent.append(el('p','guide-assist-note',HoiI18n.t('{0}: {1} · required {2} {3}%',label,amount,check.test,(check.required*100).toFixed(3))));
  }
  if(r.value===null)parent.append(el('p','guide-assist-note',HoiI18n.t('Some focus requirements are not observable yet; this is the guide target, not verified availability.')));
 }
 function marker(id,scope){return value?.view?.lanes[lane].markers.find(m=>m.id===id&&(!m.scope||m.scope===scope));}
 function paint(nodes,scope){for(const node of nodes){
  const id=node.dataset.focusId||node.dataset.techId,m=marker(id,scope);
  for(const tone of Object.keys(labels))node.classList.toggle('guide-'+tone,m?.tone===tone);
  node.querySelector('.guide-node-label')?.remove();
  if(m){node.dataset.guideStep=m.key;node.append(el('span','guide-node-label',labels[m.tone]));const original=node.getAttribute('aria-label')||'';node.setAttribute('aria-label',original.split(HoiI18n.t(" · Guide:"))[0]+HoiI18n.t(" · Guide: ")+labels[m.tone]);}
  else{delete node.dataset.guideStep;const original=node.getAttribute('aria-label')||'';node.setAttribute('aria-label',original.split(HoiI18n.t(" · Guide:"))[0]);}
 }}
 function details(id,scope){
  if(!value?.guide||!value.view||value.view.mode.startsWith('wrong_'))return null;
  const rows=Object.values(value.view.model.tracks).flatMap(t=>t.rows).filter(row=>(row.target===id||row.research_options?.includes(id))&&row.action.kind===(lane==='focus'?'select_focus':'research')&&(!row.action.entity.scope||row.action.entity.scope===scope));
  const row=rows.find(r=>!['completed','not_applicable'].includes(r.runtime_status))||rows[0];if(!row)return null;
  const box=el('section','guide-node-detail');box.append(el('strong','',marker(id,scope)?labels[marker(id,scope).tone]+' · '+value.guide.title:HoiI18n.t("In guide · ")+value.guide.title));
  criteria(box,row);
  if(row.note)box.append(el('p','',row.note));
  const conditions=[row.when,row.priority_when,row.skip_when,row.completion_condition,(value.guide.phases||[]).find(p=>p.id===row.phase)?.when];
  for(const key of new Set(conditions.flatMap(node=>HoiContracts.checkKeys(node)))){
   const label=el('label','guide-check'),input=el('input');input.type='checkbox';input.checked=value.progress.checks[key]===true;input.disabled=value.pending>0;
   input.addEventListener('change',()=>window.hoiGuide.confirm(key,input.checked).catch(()=>{}));label.append(input,el('span','',row.confirmation||HoiI18n.t("I verified this condition in the game")));box.append(label);
  }
  const a=el('a','',HoiI18n.t("Operational instructions ↗"));a.href=window.hoiGuide.withGuide('/walkthrough');box.append(a);
  const edit=el('a','',HoiI18n.t("Correct this binding"));edit.href=window.hoiGuide.withGuide('/associations?step='+encodeURIComponent(row.key));box.append(edit);return box;
 }
 function render(){host.replaceChildren();if(!value?.guide){host.append(el('p','',value?.error||HoiI18n.t("Loading guide…")));return;}
  const header=el('div','guide-assist-heading'),label=el('label','',HoiI18n.t("Guide ")),select=el('select');select.setAttribute('aria-label',HoiI18n.t("Guide to highlight"));
  for(const item of value.catalog){const opt=el('option','',item.title);opt.value=item.id;opt.selected=item.id===value.guide.id;if(typeof HoiGuideAvailability!=='undefined')HoiGuideAvailability.decorate(opt,item.compatibility,HoiI18n.t);select.append(opt);}select.disabled=value.pending>0;select.addEventListener('change',()=>window.hoiGuide.select(select.value));label.append(select);header.append(label);
  const a=el('a','',HoiI18n.t("Operational instructions ↗"));a.href=window.hoiGuide.withGuide('/walkthrough');header.append(a);
  const edit=el('a','',HoiI18n.t("Correct bindings"));edit.href=window.hoiGuide.withGuide('/associations');header.append(edit);host.append(header);
  for(const link of document.querySelectorAll('.view-tabs a, .side a.nav')){const href=link.getAttribute('href');if(href?.startsWith('/'))link.href=window.hoiGuide.withGuide(href);}
  const view=value.view,plan=view.lanes[lane],summary=el('div','guide-assist-summary'),status=el('strong');
  const marked=plan.markers.filter(m=>m.tone!=='active'),first=marked[0];
  if(view.mode.startsWith('wrong_'))status.textContent=HoiI18n.t("Guide suspended: the game does not match");
  else if(plan.blocker)status.textContent=HoiI18n.t("Before continuing: ")+plan.blocker.name;
  else if(first)status.textContent=labels[first.tone]+' · '+(first.row.name||first.id);
  else status.textContent=plan.markers.length?HoiI18n.t("Guide step in progress"):HoiI18n.t("No further items prescribed by this guide");
  if(lane==='focus'&&plan.next?.focus_readiness?.value===false)status.textContent=HoiI18n.t('Main focus waiting: {0}',plan.next.name||plan.next.target);
  summary.append(status);
  if(first||plan.next){const button=el('button','line-button',first?.tone==='alternative'?HoiI18n.t('Show alternative'):HoiI18n.t("Show ")+(lane==='focus'?'focus':HoiI18n.t("research")));button.type='button';button.addEventListener('click',()=>{const row=first?.row||plan.next;if(navigate)Promise.resolve(navigate(row)).catch(e=>{status.textContent=e.message;});else if(row.href)location.assign(window.hoiGuide.withGuide(row.href));});summary.append(button);}
  if(plan.blocker){const a=el('a','',HoiI18n.t("Check this step"));a.href=window.hoiGuide.withGuide('/walkthrough#operation-'+plan.blocker.key);summary.append(a);}
  host.append(summary);
  if(lane==='focus'){criteria(host,plan.next);if(first?.tone==='alternative')host.append(el('p','guide-assist-note',HoiI18n.t('Optional alternative from this guide. The main focus keeps its priority and returns when its requirements are met.')));}
  const note=view.mode==='live'?HoiI18n.t("Gold: main step · purple: optional choice or available alternative · blue: in progress · dashed: waiting. Unknown requirements still need checking in the game."):HoiI18n.t("Preview from the start of the guide: no current selection verified. Connect a valid game to update highlights.");
  host.append(el('p','guide-assist-note',note));if(value.error)host.append(el('p','guide-assist-error',value.error));
 }
 window.hoiGuideTree={paint,details,marker,connect(fn){navigate=fn;}};
 window.hoiGuide.subscribe(next=>{value=next;render();window.dispatchEvent(new CustomEvent('hoi-guide-change'));});
})();
