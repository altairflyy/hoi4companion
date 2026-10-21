'use strict';
const $ = id => document.getElementById(id);
const state = {overview:null,tree:null,selected:null,goal:null,search:'',zoom:.78,manual:new Set(),poll:false,coords:new Map()};
const NODE_W=174, NODE_H=154, UNIT_X=100, UNIT_Y=190, GAP=74;
const statusLabels={completed:'Completato',active:'In corso',ready:'Prerequisiti soddisfatti',locked:'Prerequisiti mancanti',excluded:'Percorso escluso',verify:'Condizioni da verificare',unknown:'Non verificato'};
async function getJson(url,options){const response=await fetch(url,options),data=await response.json();if(!response.ok)throw new Error(data.error||response.statusText);return data;}
function clear(node){node.replaceChildren();}
function el(tag,cls,text){const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=String(text??'');return node;}
function plain(text){return String(text??'').replace(/§./g,'');}
function spriteImage(sprite,cls='game-sprite'){
  if(!sprite)return el('span','sprite-placeholder','◈');
  const image=el('img',cls);image.src=sprite.url;image.alt='';image.draggable=false;
  image.addEventListener('error',()=>image.replaceWith(el('span','sprite-placeholder','◈')),{once:true});
  return image;
}
function richText(text,cls='game-text',tag='div'){
  const root=el(tag,cls),source=String(text??'');
  const colors={Y:'gold',H:'gold',G:'green',R:'red',W:'white',g:'gray',L:'gray',B:'blue',b:'blue',C:'blue',O:'orange',r:'red',M:'purple'};
  const pattern=/§(.)|£([A-Za-z0-9_]+)(?:\|(\d+))?£?|\[[^\]\n]{1,200}\]|\$[A-Za-z0-9_.:@-]+(?:\|[^$\n]*)?\$/g;
  let start=0,color='';
  const append=value=>{if(value)root.append(el('span',color?'loc-'+color:'',value));};
  for(const match of source.matchAll(pattern)){
    append(source.slice(start,match.index));const token=match[0];start=match.index+token.length;
    if(match[1]){color=match[1]==='!'?'':colors[match[1]]||'';continue;}
    if(match[2]){
      const asset=state.tree?.inline_sprites?.[token];
      if(asset){const image=spriteImage(asset,'inline-sprite');image.title=match[2];root.append(image);}
      else{const item=el('span','unresolved-icon',match[2].replace(/^GFX_/,'').replaceAll('_',' '));item.title='Simbolo del gioco non disponibile: '+token;root.append(item);}
      continue;
    }
    const item=el('span','dynamic-text',token.startsWith('$')?'Valore dinamico':token);
    item.title='Dipende dalla partita; riferimento originale: '+token;root.append(item);
  }
  append(source.slice(start));return root;
}
function manualKey(){return `hoi4companion-manual:${state.tree?.id||'none'}`;}
function setManual(){try{state.manual=new Set(JSON.parse(localStorage.getItem(manualKey())||'[]'));}catch{state.manual=new Set();}}
function syncManual(){localStorage.setItem(manualKey(),JSON.stringify([...state.manual]));}
function statuses(){
  const tree=state.tree;if(!tree)return {};if(window.hoiLive?.enabled)return HoiLiveModel.focusStates(window.hoiLive.value,tree.id,tree.focuses.map(f=>f.id));if(tree.has_snapshot)return tree.status;
  const all={};for(const focus of tree.focuses){
    if(state.manual.has(focus.id))all[focus.id]='completed';
    else if(focus.mutually_exclusive.some(id=>state.manual.has(id)))all[focus.id]='excluded';
    else if(!focus.prerequisites.every(group=>group.some(id=>state.manual.has(id))))all[focus.id]='locked';
    else all[focus.id]=focus.has_conditions?'verify':'ready';
  }return all;
}
function ancestors(target){const result=new Set(),byId=new Map(state.tree.focuses.map(f=>[f.id,f])),stack=[target];while(stack.length){const id=stack.pop();if(!byId.has(id)||result.has(id))continue;result.add(id);for(const group of byId.get(id).prerequisites)stack.push(...group);}return result;}
function currentFocus(id){return state.tree.focuses.find(f=>f.id===id);}
function updateOverview(){
  const v=state.overview;if(!v)return;
  $('demoBadge').classList.toggle('hidden',!v.demo);$('modeDot').classList.toggle('gold',v.demo);
  $('modeText').textContent=v.demo?'Demo locale':'Dati HOI4';$('modeHint').textContent=v.demo?'Nessun percorso del gioco':'File reali in sola lettura';
  $('warnings').hidden=!v.warnings?.length;$('warnings').textContent=(v.warnings||[]).slice(0,5).join('\n');
  $('dlcPanel').hidden=!v.dlcs;
  if(v.dlcs){
    const hidden=v.excluded_trees||[];
    $('dlcSummary').textContent=`Filtro DLC attivo · ${v.dlcs.enabled_count} DLC disponibili · ${v.trees.length} alberi mostrati · ${hidden.length} non mostrati`;
    const labels={enabled:'Installato, non disattivato',integrated:'Integrato nel gioco',disabled:'Disattivato nel launcher',missing:'File mancanti',unknown:'Attivazione da verificare'};
    clear($('dlcList'));for(const item of v.dlcs.items)$('dlcList').append(el('li','',`${item.name} — ${labels[item.status]||'Da verificare'}`));
    $('excludedSummary').textContent=hidden.length?'Alberi non mostrati e motivi:':'Nessun albero escluso.';
    clear($('excludedTrees'));for(const item of hidden)$('excludedTrees').append(el('li','',`${item.id}: ${item.reason}`));
  }
  const selector=$('treeSelect'),current=selector.value;clear(selector);
  const trees=[...v.trees].sort((a,b)=>a.id.localeCompare(b.id));
  for(const tree of trees){const option=el('option','',`${tree.id} (${tree.count})`);option.value=tree.id;selector.append(option);}
  selector.value=trees.some(t=>t.id===v.settings.tree_id)?v.settings.tree_id:(trees.some(t=>t.id===current)?current:trees[0]?.id||'');
  selector.disabled=!trees.length;
}
function hideTooltip(){$('focusTooltip').hidden=true;}
function showTooltip(focus,node){
  const tip=$('focusTooltip');clear(tip);
  const header=el('div','tooltip-heading');header.append(spriteImage(focus.sprite||focus.variant_sprites?.[0],'tooltip-icon'),richText(focus.name,'tooltip-name'));tip.append(header);
  tip.append(el('div','tooltip-status',`${Math.round(focus.cost*7)} giorni base · ${statusLabels[statuses()[focus.id]]||'Non verificato'}`));
  if(focus.icon_variants?.length)tip.append(el('p','tooltip-note','Icona variabile: questa è un’anteprima.'));
  if(focus.description)tip.append(richText(focus.description.length>480?focus.description.slice(0,480)+'…':focus.description));
  const effects=focus.tooltip_sections?.find(x=>x.heading==='Effetti');
  if(effects){tip.append(el('strong','tooltip-subheading','Effetti dai file'),richText(effects.text.length>220?effects.text.slice(0,220)+'…':effects.text));}
  tip.append(el('p','tooltip-note','Clicca per tutti i dettagli. Condizioni e valori della partita non valutati.'));
  tip.hidden=false;node.setAttribute('aria-describedby','focusTooltip');
  const rect=node.getBoundingClientRect(),width=tip.offsetWidth,height=tip.offsetHeight;
  const left=rect.right+12+width<innerWidth?rect.right+12:Math.max(10,rect.left-width-12);
  tip.style.left=Math.min(left,innerWidth-width-10)+'px';tip.style.top=Math.max(10,Math.min(rect.top,innerHeight-height-10))+'px';
}
function centerFocus(id){
  const position=state.coords.get(id);if(!position)return;
  const viewport=$('graphViewport');viewport.scrollLeft=(position.x+NODE_W/2)*state.zoom-viewport.clientWidth/2;viewport.scrollTop=(position.y+NODE_H/2)*state.zoom-viewport.clientHeight/2;
}
function selectFocus(id){const keepFocus=Boolean(document.activeElement?.dataset.focusId);state.selected=id;render();$('inspectorContent').scrollTop=0;if(keepFocus){const node=[...$('nodes').children].find(x=>x.dataset.focusId===id);node?.focus({preventScroll:true});}}
function render(){
  const tree=state.tree;if(!tree)return;hideTooltip();
  $('sideTree').textContent=tree.id;$('sideCount').textContent=tree.focuses.length+' focus identificati';
  $('graphTitle').textContent=tree.id.toUpperCase();$('graphCounter').textContent=tree.focuses.length+' NODI';
  $('assetInfo').textContent=tree.asset_coverage?`Icone originali: ${tree.asset_coverage.resolved}/${tree.asset_coverage.total}${tree.asset_coverage.conditional?' · '+tree.asset_coverage.conditional+' variabili':''}`:'Icone dimostrative';
  const byId=new Map(tree.focuses.map(f=>[f.id,f])),status=statuses();
  const minX=Math.min(...tree.focuses.map(f=>f.x),0),minY=Math.min(...tree.focuses.map(f=>f.y),0);
  const coords=new Map(tree.focuses.map(f=>[f.id,{x:(f.x-minX)*UNIT_X+GAP,y:(f.y-minY)*UNIT_Y+GAP}]));state.coords=coords;
  const width=Math.max(...[...coords.values()].map(p=>p.x+NODE_W+GAP),500),height=Math.max(...[...coords.values()].map(p=>p.y+NODE_H+GAP),410);
  const world=$('world');world.style.width=width+'px';world.style.height=height+'px';world.style.transform=`scale(${state.zoom})`;
  $('worldSizer').style.width=(width*state.zoom)+'px';$('worldSizer').style.height=(height*state.zoom)+'px';
  const wires=$('wires');clear(wires);wires.setAttribute('viewBox',`0 0 ${width} ${height}`);wires.setAttribute('width',width);wires.setAttribute('height',height);
  const route=state.goal?ancestors(state.goal):null,exclusive=new Set();
  const pathFor=(d,cls)=>{const path=document.createElementNS('http://www.w3.org/2000/svg','path');path.setAttribute('d',d);path.setAttribute('class',cls);wires.append(path);};
  for(const focus of tree.focuses){
    const dest=coords.get(focus.id);
    for(const group of focus.prerequisites)for(const prev of group){
      if(!coords.has(prev))continue;const src=coords.get(prev),x1=src.x+NODE_W/2,y1=src.y+NODE_H,x2=dest.x+NODE_W/2,y2=dest.y;
      const midway=y2>y1?(y1+y2)/2:y1+18;
      pathFor(`M ${x1} ${y1} V ${midway} H ${x2} V ${y2}`,'wire'+(route&&route.has(prev)&&route.has(focus.id)?' goal':route?' dim':''));
    }
    for(const id of focus.mutually_exclusive){
      if(!coords.has(id))continue;const key=[focus.id,id].sort().join('|');if(exclusive.has(key))continue;exclusive.add(key);
      const other=coords.get(id),x1=dest.x+NODE_W/2,y1=dest.y+NODE_H/2,x2=other.x+NODE_W/2,y2=other.y+NODE_H/2;
      pathFor(`M ${x1} ${y1} L ${x2} ${y2}`,'wire mutex'+(route?' dim':''));
      if(tree.theme?.exclusive){const image=document.createElementNS('http://www.w3.org/2000/svg','image');image.setAttribute('href',tree.theme.exclusive.url);image.setAttribute('x',(x1+x2)/2-12);image.setAttribute('y',(y1+y2)/2-12);image.setAttribute('width',24);image.setAttribute('height',24);wires.append(image);}
    }
  }
  const nodes=$('nodes');clear(nodes);
  for(const focus of tree.focuses){
    const position=coords.get(focus.id),code=status[focus.id]||'unknown';
    const node=el('button','focus-node '+code);node.type='button';node.dataset.focusId=focus.id;node.style.left=position.x+'px';node.style.top=position.y+'px';
    const match=!state.search||focus.id.toLowerCase().includes(state.search)||plain(focus.name).toLowerCase().includes(state.search);
    if(!match||(route&&!route.has(focus.id)))node.classList.add('dim');if(route?.has(focus.id))node.classList.add('on-route');if(state.selected===focus.id)node.classList.add('selected');
    const emblem=el('div','focus-emblem');emblem.append(spriteImage(focus.sprite||focus.variant_sprites?.[0]));
    if(focus.icon_variants?.length)emblem.append(el('span','variant-badge','?'));
    if(code==='completed'||code==='active')emblem.append(el('span','completion-badge',code==='completed'?'✓':'◉'));
    const title=el('div','focus-nameplate');if(tree.theme?.[code])title.append(spriteImage(tree.theme[code],'focus-titlebar'));
    title.append(richText(focus.name,'focus-name'));
    node.append(emblem,title,el('span','focus-duration',`${Math.round(focus.cost*7)}d*`));
    node.setAttribute('aria-label',`${plain(focus.name)}: ${statusLabels[code]||'stato non verificato'}`);
    node.addEventListener('click',()=>selectFocus(focus.id));node.addEventListener('pointerenter',()=>showTooltip(focus,node));node.addEventListener('pointerleave',hideTooltip);node.addEventListener('focus',()=>showTooltip(focus,node));node.addEventListener('blur',hideTooltip);nodes.append(node);
  }
  renderInspector(byId,status);
  $('snapshotInfo').textContent=window.hoiLive?.enabled?'Stati dalla memoria del processo. Nessuna disponibilità dinamica dedotta; i segni manuali sono separati.':tree.snapshot?`Snapshot ${tree.snapshot.source} · ${tree.snapshot.tag}${tree.snapshot.note?' · '+tree.snapshot.note:''}`:tree.snapshot_error?`Snapshot non caricato: ${tree.snapshot_error}`:'Senza snapshot: progressi manuali salvati soltanto nel browser (non nel gioco).';
}
function renderInspector(byId,status){
  const container=$('inspectorContent');clear(container);const focus=byId.get(state.selected);$('focusIndex').textContent=focus?`${[...byId.keys()].indexOf(focus.id)+1} / ${byId.size}`:'—';
  if(!focus){container.append(el('div','empty-icon','◈'),el('p','','Seleziona un focus per visualizzarne prerequisiti e percorso.'));return;}
  container.append(spriteImage(focus.sprite||focus.variant_sprites?.[0],'inspector-icon'),el('div','eyebrow','FOCUS NAZIONALE'),richText(focus.name,'focus-title','h2'),el('div','focus-code',focus.id),el('div','status-chip '+status[focus.id],statusLabels[status[focus.id]]||'Non verificato'));
  if(focus.description)container.append(richText(focus.description));
  container.append(el('div','tooltip-note','Testi dai file del gioco; condizioni e valori dinamici non valutati.'));
  if(focus.icon_variants?.length){container.append(el('div','minor-heading','VARIANTI DELL’ICONA'),el('p','muted','La variante corrente dipende dalla partita.'));const gallery=el('div','variant-gallery');for(const sprite of focus.variant_sprites||[]){const image=spriteImage(sprite);image.title=sprite.name;gallery.append(image);}container.append(gallery);}
  const meta=el('div','meta-grid');const duration=el('div');duration.append(el('small','','Durata base'),el('strong','',`${Math.round(focus.cost*7)} giorni*`));meta.append(duration);container.append(el('div','minor-heading','DATI E REQUISITI'),meta);
  const ul=el('ul','requirements');if(!focus.prerequisites.length)ul.append(el('li','','Nessun focus precedente richiesto.'));
  for(const group of focus.prerequisites){const item=el('li');item.append(el('b','',group.length>1?'ALMENO UNO · ':'OBBLIGATORIO · '));group.forEach((id,index)=>{if(index)item.append(document.createTextNode(' oppure '));const button=el('button','focus-reference',plain(byId.get(id)?.name||id));button.addEventListener('click',()=>{if(byId.has(id)){selectFocus(id);centerFocus(id);}});item.append(button);});ul.append(item);}
  if(focus.mutually_exclusive.length){const item=el('li');item.append(el('b','','ESCLUSIVO · '),richText(focus.mutually_exclusive.map(id=>byId.get(id)?.name||id).join(', ')));ul.append(item);}
  if(focus.has_conditions)ul.append(el('li','','⚠ Condizioni aggiuntive da verificare nel gioco.'));if(focus.has_bypass)ul.append(el('li','','↷ Sono previste condizioni di bypass.'));if(focus.position_warning)ul.append(el('li','','⚠ Posizione approssimata: riferimento mancante o ciclico.'));container.append(ul);
  for(const section of focus.tooltip_sections||[])container.append(el('div','minor-heading',section.heading.toUpperCase()),richText(section.text));
  container.append(el('div','minor-heading','AZIONI'));
  const goal=el('button','action','◈ Evidenzia tutti i prerequisiti');goal.addEventListener('click',()=>{state.goal=focus.id;render();});container.append(goal);
  if(!state.tree.has_snapshot&&!window.hoiLive?.enabled){const done=state.manual.has(focus.id),button=el('button','action secondary',done?'↶ Annulla completamento manuale':'✓ Contrassegna manualmente come completato');button.addEventListener('click',()=>{if(done)state.manual.delete(focus.id);else state.manual.add(focus.id);syncManual();render();});container.append(button);}
  const source=el('details','asset-details');source.append(el('summary','','Origine dei dati'),el('div','muted',focus.source),el('div','muted',focus.sprite?.source||'Icona non risolta'));
  if(focus.sprite?.override)source.append(el('div','muted','Texture da DLC o mod configurata.'));container.append(source);
}
async function loadTree(id){
  if(!id){state.tree={id:'Nessun albero compatibile',focuses:[]};state.selected=null;state.goal=null;render();$('snapshotInfo').textContent='Controlla i percorsi e i motivi di esclusione nel pannello DLC.';return;}
  const old=state.tree?.id;state.tree=await getJson('/api/tree?id='+encodeURIComponent(id)+(window.hoiLive?.enabled?'&source=live':''));if(state.tree.error)throw new Error(state.tree.error);setManual();if(state.selected&&!currentFocus(state.selected))state.selected=null;const requestedFocus=!old?new URLSearchParams(location.search).get('focus'):null;if(requestedFocus&&currentFocus(requestedFocus))state.selected=requestedFocus;if(!state.selected)state.selected=state.tree.focuses[0]?.id||null;render();if(old!==id)requestAnimationFrame(()=>centerFocus(state.selected));
}
async function reload(all=true){try{if(all){state.overview=await getJson('/api/config');updateOverview();if(!state.tree){const requested=new URLSearchParams(location.search).get('tree');if(state.overview.trees.some(t=>t.id===requested))$('treeSelect').value=requested;}}await loadTree($('treeSelect').value||state.overview.settings.tree_id);$('connection').textContent='● Server locale';}catch(error){$('connection').textContent='● Errore di connessione';$('warnings').hidden=false;$('warnings').textContent=error.message;}}
function modal(show){hideTooltip();$('settingsModal').classList.toggle('hidden',!show);if(show){for(const [key,value] of Object.entries(state.overview.settings)){const field=$('settingsForm').elements.namedItem(key);if(field)field.value=value;}$('formError').hidden=true;}}
function init(){
  $('navSettings').addEventListener('click',()=>modal(true));$('closeSettings').addEventListener('click',()=>modal(false));$('cancelSettings').addEventListener('click',()=>modal(false));$('settingsModal').addEventListener('click',event=>{if(event.target===$('settingsModal'))modal(false);});
  document.addEventListener('keydown',event=>{if(event.key==='Escape'){modal(false);hideTooltip();}});
  $('settingsForm').addEventListener('submit',async event=>{event.preventDefault();try{state.overview=await getJson('/api/config',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(Object.fromEntries(new FormData(event.target)))});state.selected=null;state.goal=null;updateOverview();await loadTree(state.overview.settings.tree_id);modal(false);}catch(error){$('formError').hidden=false;$('formError').textContent=error.message;}});
  $('treeSelect').addEventListener('change',async event=>{state.selected=null;state.goal=null;await loadTree(event.target.value);});
  let searchTimer;$('search').addEventListener('input',event=>{state.search=event.target.value.trim().toLowerCase();render();clearTimeout(searchTimer);searchTimer=setTimeout(()=>{if(state.search){const focus=state.tree.focuses.find(f=>f.id.toLowerCase().includes(state.search)||plain(f.name).toLowerCase().includes(state.search));if(focus)centerFocus(focus.id);}},100);});
  $('resetGoal').addEventListener('click',()=>{state.goal=null;render();});$('centerFocus').addEventListener('click',()=>centerFocus(state.selected));
  $('reload').addEventListener('click',async()=>{const button=$('reload');button.disabled=true;try{await getJson('/api/refresh',{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'});await reload(true);}catch(error){$('warnings').hidden=false;$('warnings').textContent=error.message;}finally{button.disabled=false;}});
  $('zoom').addEventListener('input',event=>{state.zoom=Number(event.target.value)/100;$('zoomValue').textContent=event.target.value+'%';render();centerFocus(state.selected);});$('poll').addEventListener('change',event=>state.poll=event.target.checked);
  const viewport=$('graphViewport');viewport.addEventListener('scroll',()=>{hideTooltip();const node=document.activeElement;if(node?.dataset.focusId)requestAnimationFrame(()=>{const rect=node.getBoundingClientRect(),box=viewport.getBoundingClientRect(),focus=currentFocus(node.dataset.focusId);if(focus&&rect.bottom>box.top&&rect.top<box.bottom&&rect.right>box.left&&rect.left<box.right)showTooltip(focus,node);});});addEventListener('resize',hideTooltip);let drag=null;
  viewport.addEventListener('pointerdown',event=>{if(event.button!==0||event.target.closest('button'))return;drag={x:event.clientX,y:event.clientY,left:viewport.scrollLeft,top:viewport.scrollTop};viewport.setPointerCapture(event.pointerId);viewport.classList.add('dragging');});
  viewport.addEventListener('pointermove',event=>{if(drag){viewport.scrollLeft=drag.left-(event.clientX-drag.x);viewport.scrollTop=drag.top-(event.clientY-drag.y);}});
  const endDrag=()=>{drag=null;viewport.classList.remove('dragging');};viewport.addEventListener('pointerup',endDrag);viewport.addEventListener('pointercancel',endDrag);
  let lastLiveEnabled=window.hoiLive?.enabled;
  window.hoiLive?.subscribe(()=>{const switched=lastLiveEnabled!==window.hoiLive.enabled;lastLiveEnabled=window.hoiLive.enabled;if(state.tree){if(switched)loadTree(state.tree.id).catch(error=>{$('snapshotInfo').textContent=error.message;});else render();}});
  window.addEventListener('hoi-live-follow',async event=>{const id=event.detail.focus_tree_id;if(!state.overview?.trees.some(t=>t.id===id))return;$('treeSelect').value=id;state.selected=event.detail.current_focus||null;await loadTree(id);});
  setInterval(()=>{if(!window.hoiLive?.enabled&&state.poll&&state.overview?.settings.snapshot_path)reload(false);},15000);reload();
}
init();
