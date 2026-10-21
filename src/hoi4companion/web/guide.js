'use strict';
(()=>{
 if(location.pathname==='/hud'){document.body.classList.add('hud-view');const token=new URLSearchParams(location.search).get('instance')||'';document.title='HOI4 Companion HUD '+(/^[a-f0-9]{32}$/.test(token)?token:'');}
 const $=id=>document.getElementById(id),el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
 const labels={completed:'Confermato dal gioco',active:'In corso',pending:'Da fare',planned:'In programma',waiting:'Attendi conferma',manual:'Verifica manuale',confirmed:'Confermato da te',skipped:'Saltato nella guida',free:'Scelta libera',unavailable:'Non nel catalogo'};
 let guide=null,previousSample=null,lastState=null,local={checks:{}},undo=null,seenRevision=-1,pending=0,saveQueue=Promise.resolve();
 function save(update){const payload=update||{checks:{...local.checks}};pending++;saveQueue=saveQueue.then(async()=>{try{const response=await fetch('/api/guide/checks',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(payload)});if(!response.ok)throw Error('HTTP '+response.status);const saved=await response.json();seenRevision=saved.revision;local={checks:saved.checks};}catch(e){$('error').hidden=false;$('error').textContent='Conferme non sincronizzate con l’HUD: '+e.message;}finally{pending--;render();}});}
 function reset(message){undo=local;local={checks:{}};save({reset:true});const box=$('resetNotice');box.replaceChildren(document.createTextNode(message+' '));const b=el('button','','Annulla');b.type='button';b.onclick=()=>{local=undo||{checks:{}};undo=null;save();box.hidden=true;render();};box.append(b);box.hidden=false;}
 function checkbox(key,text){const label=el('label','checkpoint-check'),input=el('input');input.type='checkbox';input.checked=local.checks[key]===true;input.setAttribute('data-check',key);input.addEventListener('change',()=>{local.checks[key]=input.checked;save({key,checked:input.checked});render();});label.append(input,el('span','',text));return label;}
 function link(step){if(!step.href)return null;const a=el('a','open',step.kind==='focus'?'Apri nell’albero dei focus ↗':'Mostra nella scheda tecnologica ↗');a.href=step.href;return a;}
 function action(step,container){container.replaceChildren();if(!step)return;const a=link(step);if(a)container.append(a);if(['gate','project'].includes(step.kind))container.append(checkbox(step.key,step.confirmation||'Confermo dopo verifica nel gioco'));else if(step.gate&&step.status==='waiting')container.append(checkbox(step.gate,step.confirmation||'Condizione verificata nel gioco'));}
 function current(rows,container,kind){container.replaceChildren();if(!rows.length)return;const n=el('div','current');n.append(el('strong','',kind==='focus'?'In corso:':'Ricerche in corso:'));for(const row of rows)n.append(el('span','',row.name));container.append(n);}
 function card(track,kind,mode){const prefix=kind==='focus'?'focus':'research',next=track.next;current(track.active,$(prefix+'Current'),kind);$(prefix+'Next').textContent=next?next.name:'Priorità della guida completate';action(next,$(prefix+'Action'));
  $(prefix+'Note').textContent=next?(next.status==='waiting'?'Attesa richiesta dalla guida. ':'')+(mode!=='live'?'Anteprima: ':'')+next.note:'Non sono prescritti altri passaggi obbligatori.';
  if(kind==='focus'&&track.outside)$(prefix+'Current').append(el('p','current','Focus attuale fuori percorso: '+track.outside));
 }
 function renderRows(track,id,next){const box=$(id);box.replaceChildren();for(const step of track.rows){if($('hideDone').checked&&['completed','confirmed','skipped'].includes(step.status))continue;
  const li=el('li','step '+step.status+(step.key===next?.key?' next':''));li.id='step-'+step.key;li.dataset.step=step.key;li.append(el('span','ordinal',String(step.order)));
  const top=el('div','step-top');top.append(el('h3','',step.name),el('span','badge',labels[step.status]));li.append(top);
  if(step.title!==step.name)li.append(el('p','source-title',step.title));
  const metadata=[];if(step.folder_name)metadata.push(step.folder_name);if(step.year)metadata.push(String(step.year));if(step.estimated_days)metadata.push('durata base stimata '+step.estimated_days+' giorni');
  if(metadata.length)li.append(el('p','meta',metadata.join(' · ')));
  if(step.mapping_basis==='interpreted_upgrade')li.append(el('p','interpretation','UP → livello interpretato dai file locali'));
  const a=link(step);if(a)li.append(a);
  if(['gate','project'].includes(step.kind))li.append(checkbox(step.key,step.confirmation||'Verificato nel gioco'));
  if(step.gate)li.append(checkbox(step.gate,step.confirmation||'Condizione verificata'));
  if(step.available_gate)li.append(checkbox(step.available_gate,'Ho verificato nel gioco: questo focus è disponibile'));
  const details=el('details'),summary=el('summary','','Istruzioni e riferimento');details.append(summary,el('p','',step.note));
  if(step.target)details.append(el('p','meta',step.target));details.append(el('p','meta','Priorità '+step.source_order+' della descrizione originale'));li.append(details);box.append(li);
 }}
 function renderMilestones(){const box=$('milestones');box.replaceChildren();for(const item of guide.milestones){const node=el('article','milestone'+(local.checks[item.key]?' confirmed':''));node.append(el('h3','',item.title));const a=el('a','','Video · '+clock(item.seconds)+' ↗');a.href=guide.source.url+'&t='+item.seconds;a.target='_blank';a.rel='noopener noreferrer';node.append(a,el('p','',item.note),checkbox(item.key,item.confirmation));box.append(node);}}
 function clock(seconds){return Math.floor(seconds/60)+':'+String(Math.floor(seconds%60)).padStart(2,'0');}
 function render(){if(!guide)return;const value=window.hoiLive?.enabled?lastState:null,r=HoiGuideModel.evaluate(guide,value,local);
  const modeText={live:'Percorso collegato alla Polonia',preview:'Anteprima dal 1936',wrong_country:'Partita diversa: '+r.country,wrong_tree:'Albero della partita diverso dalla guida'};
  $('mode').textContent=modeText[r.mode];$('modeNote').textContent=r.mode==='live'?'Gli step si ricalcolano a ogni cambio dello stato letto. Completato significa confermato dal gioco; le checkbox sono verifiche manuali.':r.mode==='preview'?'Piano consultabile senza gioco. Al caricamento di una partita polacca verificata, focus e tecnologie si aggiorneranno automaticamente. Nessun vecchio campione viene usato.':'Non applico i completamenti di questa partita al percorso Romanov. La sequenza mostrata resta un’anteprima.';
  card(r.focus,'focus',r.mode);card(r.research,'research',r.mode);
  $('focusProgress').textContent=r.focus.completed+'/'+r.focus.rows.length;$('researchProgress').textContent=r.research.completed+'/'+r.research.rows.length;
  const parallel=$('parallel');parallel.replaceChildren();if(r.mode==='live'){parallel.append(el('p','current',r.research.free_slots+' slot liberi di '+r.research.slots.length));if(r.research.outside.length)parallel.append(el('p','current','Ricerche fuori percorso: '+r.research.outside.join(', ')));}
  if(r.research.suggestions.length>1){const list=el('ol','other-picks');for(const s of r.research.suggestions.slice(1,Math.max(1,r.research.free_slots??3)))list.append(el('li','',s.name));if(list.children.length){parallel.append(el('p','current','Poi, negli altri slot liberi:'),list);}}
  const projects=$('projects');projects.replaceChildren();for(const project of r.research.projects){const node=el('div','project-callout');node.append(el('strong','',project.name),el('p','',project.note),checkbox(project.key,project.confirmation));projects.append(node);}
  const unity=guide.tracks.focus.find(s=>s.available_gate),eligible=!!unity&&r.mode==='live'&&value.sample.completed_focuses.includes(unity.priority_after)&&!value.sample.completed_focuses.includes(unity.target);$('unityControl').hidden=!eligible;$('unityCheck').replaceChildren();if(unity)$('unityCheck').append(checkbox(unity.available_gate,'Ho verificato nel gioco: Proclamare l’unità slava è disponibile'));
  renderRows(r.focus,'focusSteps',r.focus.next);renderRows(r.research,'researchSteps',r.research.next);renderMilestones();
 }
 function onLive(value){lastState=value;if(value?.guide_revision!==undefined&&value.guide_revision>=seenRevision&&!pending){seenRevision=value.guide_revision;local={checks:value.guide_checks||{}};}const sample=value?.usable?value.sample:null;if(sample&&previousSample){const old=previousSample,done=new Set(sample.completed_focuses||[]),researched=new Set(sample.researched_technologies||[]);const changedCountry=old.country!==sample.country,rollback=old.country===sample.country&&((old.completed_focuses||[]).some(id=>!done.has(id))||(old.researched_technologies||[]).some(id=>!researched.has(id)));if((changedCountry||rollback)&&Object.values(local.checks).some(Boolean))reset('Le conferme manuali sono state azzerate dopo un cambio di nazione o il ritorno a uno stato precedente.');}if(sample)previousSample=sample;render();}
 async function start(){try{const response=await fetch('/api/guide');if(!response.ok)throw Error('Guida non disponibile: HTTP '+response.status);guide=await response.json();const checkResponse=await fetch('/api/guide/checks');if(!checkResponse.ok)throw Error('Checklist condivisa non disponibile');const saved=await checkResponse.json();local={checks:saved.checks};seenRevision=saved.revision;
   $('guideTitle').textContent=guide.title;$('subtitle').textContent=guide.subtitle;$('sourceLink').href=guide.source.url;$('sourceBasis').textContent=guide.source.author+' · '+guide.source.published+'. '+guide.source.basis;
   $('assumptions').replaceChildren(...guide.assumptions.map(t=>el('li','',t)));$('catalogWarnings').replaceChildren(...guide.catalog_warnings.map(t=>el('p','',t)));
   $('chapters').replaceChildren(...guide.source.chapters.map(c=>{const li=el('li'),a=el('a','',clock(c.start_time)+' · '+c.title);a.href=guide.source.url+'&t='+Math.floor(c.start_time);a.target='_blank';a.rel='noopener noreferrer';li.append(a);return li;}));
   window.hoiLive?.subscribe(onLive);render();
  }catch(e){$('error').hidden=false;$('error').textContent=e.message;}}
 $('reset').onclick=()=>{reset('Nuova checklist locale: nessun dato del gioco è stato modificato.');render();};$('hideDone').onchange=render;start();
})();
