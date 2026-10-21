'use strict';
const $=id=>document.getElementById(id), state={tree:null,selected:null,zoom:1,coords:new Map(),search:'',section:'all',catalog:null,autoFit:true};
const W=180,H=72,DX=220,DY=110;
const el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
const plain=s=>(s||'').replace(/§./g,'');
async function json(url,options){const r=await fetch(url,options);if(!r.ok)throw Error(`Errore HTTP ${r.status}`);const d=await r.json();if(d.error)throw Error(d.error);return d;}
function fail(e){$('error').hidden=false;$('error').textContent=e.message||String(e);}
function rich(text){const box=el('div','research-copy');let color='',pos=0;const re=/§(.)|£([A-Za-z0-9_]+)(?:\|(\d+))?/g;let m;
 const add=s=>{if(s)box.append(el('span',color?`research-color-${color}`:'',s));};
 while((m=re.exec(text||''))){add(text.slice(pos,m.index));if(m[1])color=m[1]==='!'?'':m[1];else{const token=m[0],sprite=state.tree?.inline_sprites?.[token];if(sprite){const img=el('img','inline-sprite');img.src=sprite.url;img.alt=token;box.append(img);}else add(token);}pos=re.lastIndex;}add((text||'').slice(pos));return box;}
function portrait(t,cls){if(!t.sprite)return el('div','tech-fallback','⚙');const img=el('img',cls);img.src=t.sprite.url;img.alt='';img.loading='lazy';img.addEventListener('error',()=>img.replaceWith(el('div','tech-fallback','⚙')));return img;}
function meta(t){return `${t.start_year??'Anno —'} · costo ×${t.research_cost??'—'}`;}
function all(){return state.tree?.technologies||[];}
function tech(id){return all().find(t=>t.id===id);}
function tooltip(t,button){const box=$('tooltip');box.replaceChildren(el('strong','',plain(t.name)),el('p','',stateLabel(t.id)),el('p','',meta(t)),rich(t.description||'Nessuna descrizione nei file di localizzazione.'));if(t.has_conditions||t.special_project)box.append(el('p','tech-note','Requisiti della partita o progetto speciale: da verificare nel gioco.'));box.hidden=false;const r=button.getBoundingClientRect();box.style.left=`${Math.max(8,Math.min(r.right+12,innerWidth-box.offsetWidth-12))}px`;box.style.top=`${Math.max(8,Math.min(r.top,innerHeight-box.offsetHeight-12))}px`;}
function hideTip(){$('tooltip').hidden=true;}
function center(id){const p=state.coords.get(id);if(!p)return;const v=$('graphViewport');v.scrollTo({left:(p.x+(p.width||W)/2)*state.zoom-v.clientWidth/2,top:(p.y+(p.height||H)/2)*state.zoom-v.clientHeight/2,behavior:'smooth'});}
function section(title,values,container){if(!values.length)return;const box=el('section','tech-section');box.append(el('h3','',title));for(const item of values)box.append(item);container.append(box);}
function link(id,note=''){const t=tech(id),ref=state.tree.references?.[id],label=plain(t?.name||ref?.name||id)+(note?' '+note:'');if(!t&&!ref?.folder)return el('p','',label+' · fuori dalla categoria');const b=el('button','tech-link',label+(!t?' ↗':''));b.type='button';b.addEventListener('click',async()=>{try{if(!t){$('folder').value=ref.folder;await loadFolder();}select(id);center(id);}catch(e){fail(e);}});return b;}
function inspect(t){const box=$('inspector');box.replaceChildren(portrait(t,'tech-portrait'),el('h2','focus-title',plain(t.name)),el('div','research-id',t.id));
 const grid=el('div','meta-grid');for(const [name,value]of [['Anno',t.start_year??'—'],['Coefficiente costo','×'+(t.research_cost??'—')]]){const cell=el('div');cell.append(el('small','',name),el('strong','',String(value)));grid.append(cell);}box.append(grid,rich(t.description||'Nessuna descrizione localizzata disponibile.'));
 if(t.has_conditions||t.special_project)box.append(el('p','tech-note',t.special_project?'Collegata a un progetto speciale. Il completamento del progetto e le altre condizioni vanno verificati nella partita.':'Questa tecnologia ha condizioni di ricerca aggiuntive, da verificare nella partita.'));
 const incoming=all().filter(x=>x.paths.some(p=>p.target===t.id));section('PERCORSI IN ENTRATA',incoming.map(x=>link(x.id)),box);
 section('PERCORSI SUCCESSIVI',t.paths.map(p=>link(p.target,p.coefficient!==null&&p.coefficient!==1?`(coefficiente percorso ×${p.coefficient})`:'')),box);
 section('DIPENDENZE DICHIARATE',t.dependencies.map(d=>link(d.id,`(valore ${d.value})`)),box);
 section('ESCLUSIONE RECIPROCA',t.exclusions.map(id=>link(id)),box);
 section('SOTTOTECNOLOGIE',t.sub_technologies.map(id=>link(id)),box);
 section('SBLOCCHI',t.unlocks.map(u=>{const n=el('div','tech-unlock',plain(u.name));n.append(el('small','',u.kind));return n;}),box);
 const source=el('details','tech-section');source.append(el('summary','','Sorgente locale'),el('p','source-path',t.source));if(t.sprite)source.append(el('p','source-path',t.sprite.name));box.append(source);
}
function select(id){const t=tech(id);if(!t){state.selected=null;$('inspector').replaceChildren(el('p','','Nessuna tecnologia in questo ramo.'));return;}if(t.in_folder&&!state.coords.has(id)){state.section=native(t)?'all':'unmapped';renderSections();render();}state.selected=id;document.querySelectorAll('.tech-node').forEach(n=>n.classList.toggle('selected',n.dataset.techId===id));inspect(t);if(t.layout_hidden_reason)$('inspector').prepend(el('p','tech-note',t.layout_hidden_reason));paintLive();}
function observedState(id){return window.hoiLive?.enabled?HoiLiveModel.technologyState(window.hoiLive.value,id):'unknown';}
function stateLabel(id){return {completed:'Ricercata',active:'In ricerca',unresearched:'Non ricercata · disponibilità da verificare',unknown:'Stato non verificato'}[observedState(id)];}
function paintLive(){
 document.querySelectorAll('.tech-node').forEach(n=>{const id=n.dataset.techId,t=tech(id),code=observedState(id);n.classList.toggle('live-completed',code==='completed');n.classList.toggle('live-active',code==='active');n.setAttribute('aria-label',`${plain(t.name)}, ${meta(t)} · ${stateLabel(id)}`);const key=code==='completed'?'researched':code==='active'?'currently_researching':'unavailable',frame=t.visual?.states?.[key];if(frame?.url)n.style.backgroundImage=`url("${frame.url}")`;else n.style.backgroundImage='';const special=n.querySelector('.special-frame'),outer=t.visual?.special_project_states?.[key];if(special&&outer?.url)special.src=outer.url;});
 document.querySelectorAll('.tech-wire[data-target]').forEach(n=>{const a=observedState(n.dataset.source),b=observedState(n.dataset.target);n.classList.toggle('wire-completed',a==='completed'&&b==='completed');n.classList.toggle('wire-active',a==='completed'&&b==='active');});
 $('inspector').querySelector('.tech-inspector-state')?.remove();if(state.selected&&window.hoiLive?.enabled)$('inspector').prepend(el('p','tech-note tech-inspector-state',stateLabel(state.selected)+' · memoria del processo'));
}
function fallbackLayout(nodes){const ids=new Set(nodes.map(t=>t.id)),adj=new Map(nodes.map(t=>[t.id,new Set()]));
 for(const t of nodes)for(const p of t.paths)if(ids.has(p.target)){adj.get(t.id).add(p.target);adj.get(p.target).add(t.id);}
 const seen=new Set(),components=[];for(const t of nodes){if(seen.has(t.id))continue;const queue=[t.id],part=[];seen.add(t.id);while(queue.length){const id=queue.pop();part.push(tech(id));for(const neighbor of adj.get(id))if(!seen.has(neighbor)){seen.add(neighbor);queue.push(neighbor);}}components.push(part);}
 state.coords.clear();let top=50,width=800;const captions=[];
 for(const part of components){const years=[...new Set(part.map(t=>t.start_year??0))].sort((a,b)=>a-b);const rowKeys=[...new Set(part.map(t=>t.folders.find(f=>f.id===state.tree.id)?.x??0))].sort((a,b)=>a-b);const occupied=new Set();let maxRow=0;
 part.sort((a,b)=>(a.start_year??0)-(b.start_year??0)||a.id.localeCompare(b.id));
 for(const t of part){const x=40+years.indexOf(t.start_year??0)*DX;let row=rowKeys.indexOf(t.folders.find(f=>f.id===state.tree.id)?.x??0);while(occupied.has(`${x}:${row}`))row++;occupied.add(`${x}:${row}`);maxRow=Math.max(row,maxRow);state.coords.set(t.id,{x,y:top+row*DY});width=Math.max(width,x+W+50);}
 captions.push({text:plain(part[0].name),y:top-25});top+=(maxRow+1)*DY+60;}
 return {width,height:Math.max(500,top),captions};
}
function native(t){return t?.folders.find(f=>f.id===state.tree.id)?.layout;}
function layout(nodes){
 const mapped=nodes.filter(t=>native(t));if(!mapped.length)return fallbackLayout(nodes);
 const coords=new Map(),captions=[];let maxX=300,maxY=250;
 for(const t of mapped){const p=native(t),point={x:p.x,y:p.y,width:p.width||72,height:p.height||72};coords.set(t.id,point);maxX=Math.max(maxX,point.x+point.width+24);maxY=Math.max(maxY,point.y+point.height+24);}
 for(const c of state.tree.layout?.captions||[]){const p={...c,text:plain(c.text)};captions.push(p);maxX=Math.max(maxX,p.x+(p.width||160));maxY=Math.max(maxY,p.y+(p.height||32));}
 state.coords=coords;return {width:maxX,height:maxY,captions};
}
function sectionNodes(){return all().filter(t=>t.in_folder&&(state.section==='unmapped'?!native(t):state.section==='all'?(!state.tree.layout||!!native(t)):native(t)?.section_id===state.section));}
function renderSections(){const box=$('technologySections');box.replaceChildren();const sections=state.tree.sections||[],unmapped=all().filter(t=>t.in_folder&&!native(t)).length;if(!sections.length){box.hidden=true;return;}box.hidden=false;
 const add=(id,label)=>{const b=el('button','technology-section',label);b.type='button';b.setAttribute('aria-pressed',String(state.section===id));b.addEventListener('click',()=>{state.section=id;renderSections();render();$('graphViewport').scrollTo(0,0);});box.append(b);};
 add('all','Scheda del gioco');for(const s of sections)add(s.id,`${plain(s.name)} · ${s.count}`);if(unmapped)add('unmapped',`Altri dati del catalogo · ${unmapped}`);
}
function renderTabs(){const box=$('technologyTabs');box.replaceChildren();for(const f of state.catalog?.folders||[]){const b=el('button','technology-tab');b.type='button';b.setAttribute('aria-pressed',String(f.id===$('folder').value));b.setAttribute('aria-label',`${plain(f.name)}, ${f.layout?.mapped??f.count} tecnologie`);b.title=plain(f.name);if(f.tab?.sprite){const img=el('img');img.src=f.tab.sprite.url;img.alt='';img.addEventListener('error',()=>img.remove());b.append(img);}else b.append(el('span','tab-symbol','⚙'));b.append(el('span','',plain(f.name)));b.addEventListener('click',()=>{if($('folder').value===f.id)return;$('folder').value=f.id;loadFolder().catch(fail);});b.addEventListener('keydown',event=>{if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;event.preventDefault();const buttons=[...box.children],i=buttons.indexOf(b),next=event.key==='Home'?0:event.key==='End'?buttons.length-1:(i+(event.key==='ArrowRight'?1:-1)+buttons.length)%buttons.length;buttons[next].focus();});box.append(b);}}
function applyZoom(){$('world').style.transform=`scale(${state.zoom})`;$('worldSizer').style.width=`${state.width*state.zoom}px`;$('worldSizer').style.height=`${state.height*state.zoom}px`;$('zoomValue').textContent=`${Math.round(state.zoom*100)}%`;hideTip();}
function fitSheet(){const v=$('graphViewport');if(!state.width||!v.clientWidth)return;state.zoom=Math.max(.25,Math.min(1,(v.clientWidth-18)/state.width,(v.clientHeight-18)/state.height));$('zoom').value=Math.round(state.zoom*100);applyZoom();v.scrollTo(0,0);}
function wirePath(a,b,direction){const aw=a.width||72,ah=a.height||72,bw=b.width||72,bh=b.height||72;
 if(direction==='LEFT'||Math.abs(b.y-a.y)<ah/2){const right=b.x>=a.x,ax=right?a.x+aw:a.x,ay=a.y+ah/2,bx=right?b.x:b.x+bw,by=b.y+bh/2,mid=(ax+bx)/2;return `M${ax},${ay} H${mid} V${by} H${bx}`;}
 const down=b.y>=a.y,ax=a.x+aw/2,ay=down?a.y+ah:a.y,bx=b.x+bw/2,by=down?b.y:b.y+bh,mid=(ay+by)/2;return `M${ax},${ay} V${mid} H${bx} V${by}`;
}
function renderBackdrop(){
 const box=$('backdrop');box.replaceChildren();
 for(const layer of state.tree.visuals?.layers||[]){const sprite=layer.sprite;if(!sprite?.url)continue;const n=el('div','technology-background');n.style.left=`${layer.x||0}px`;n.style.top=`${layer.y||0}px`;n.style.width=`${layer.width||state.width}px`;n.style.height=`${layer.height||state.height}px`;
  const bx=sprite.border?.x||0,by=sprite.border?.y||0;
  if(sprite.kind==='corneredtilespritetype'&&(bx||by)){
   // A cornered GUI sprite expands one frame around the entire region.
   // Repeating its complete texture would draw an unintended grid of frames.
   n.style.boxSizing='border-box';n.style.borderStyle='solid';n.style.borderColor='transparent';n.style.borderWidth=`${by}px ${bx}px`;n.style.borderImageSource=`url("${sprite.url}")`;n.style.borderImageSlice=`${by} ${bx} fill`;n.style.borderImageRepeat='stretch';
  }else{n.style.backgroundImage=`url("${sprite.url}")`;n.style.backgroundRepeat=layer.repeat?'repeat':'no-repeat';}
  box.append(n);
 }
}
function toggleInspector(open=!document.body.classList.contains('show-inspector')){document.body.classList.toggle('show-inspector',open);$('inspectorToggle').setAttribute('aria-expanded',String(open));hideTip();requestAnimationFrame(()=>{if(state.autoFit)fitSheet();});}
function filter(){const q=state.search.toLocaleLowerCase();document.querySelectorAll('.tech-node').forEach(n=>{const t=tech(n.dataset.techId);n.classList.toggle('dim',!!q&&!`${plain(t.name)} ${t.id}`.toLocaleLowerCase().includes(q));});}
function render(){
 hideTip();const nodes=sectionNodes(),dimensions=layout(nodes);state.width=dimensions.width;state.height=dimensions.height;
 $('world').style.width=`${state.width}px`;$('world').style.height=`${state.height}px`;
 $('wires').setAttribute('width',state.width);$('wires').setAttribute('height',state.height);$('wires').replaceChildren();$('nodes').replaceChildren();renderBackdrop();
 const ns='http://www.w3.org/2000/svg',drawn=new Set();
 function wire(from,to,kind){const a=state.coords.get(from),b=state.coords.get(to);if(!a||!b)return;const key=kind==='xor'?[from,to].sort().join(':'):from+':'+to+':'+kind;if(drawn.has(key))return;drawn.add(key);
  const d=wirePath(a,b,native(tech(from))?.direction),outline=document.createElementNS(ns,'path'),line=document.createElementNS(ns,'path');
  outline.setAttribute('d',d);outline.setAttribute('class','tech-wire-outline '+kind);line.setAttribute('d',d);line.setAttribute('class','tech-wire '+kind);line.dataset.source=from;line.dataset.target=to;$('wires').append(outline,line);
 }
 for(const t of nodes){for(const p of t.paths)if(!(t.layout_ignored_paths||[]).includes(p.target))wire(t.id,p.target,'');for(const d of t.dependencies)wire(d.id,t.id,'dep');for(const id of t.exclusions)wire(t.id,id,'xor');}
 for(const c of dimensions.captions){const n=el('span','branch-caption'+(c.kind?' native-'+c.kind:''),c.text);n.style.left=`${c.x||0}px`;n.style.top=`${c.y||0}px`;if(c.width)n.style.width=`${c.width}px`;if(c.align)n.style.textAlign=c.align;$('nodes').append(n);}
 for(const t of nodes){const p=state.coords.get(t.id),variant=native(t)?.variant||'equipment',b=el('button','tech-node tech-'+variant+(t.special_project?' special':''));b.type='button';b.classList.toggle('layout-fallback',!native(t));b.dataset.techId=t.id;b.style.left=`${p.x}px`;b.style.top=`${p.y}px`;b.style.width=`${p.width||W}px`;b.style.height=`${p.height||H}px`;b.title=plain(t.name);b.setAttribute('aria-label',plain(t.name)+', '+meta(t));
  const image=portrait(t,'tech-icon');if(t.special_project&&t.visual?.special_project_states?.unavailable?.url){const frame=el('img','special-frame');frame.src=t.visual.special_project_states.unavailable.url;frame.alt='';b.append(frame);}b.append(image);if(variant==='equipment')b.append(el('strong','tech-name',plain(t.name)));
  b.addEventListener('click',()=>{select(t.id);if(innerWidth<700)toggleInspector(true);});b.addEventListener('dblclick',()=>toggleInspector(true));b.addEventListener('pointerenter',()=>tooltip(t,b));b.addEventListener('pointerleave',hideTip);b.addEventListener('focus',()=>tooltip(t,b));b.addEventListener('blur',hideTip);$('nodes').append(b);
 }
 $('heading').textContent=plain(state.tree.name).toLocaleUpperCase();$('sheetTitle').textContent=plain(state.tree.name);$('count').textContent=`${nodes.length} tecnologie`;$('sideTree').textContent=plain(state.tree.name);$('sideCount').textContent=`${all().filter(t=>t.in_folder).length} tecnologie nel catalogo`;$('coverage').textContent=`Icone ${state.tree.asset_coverage.resolved}/${state.tree.asset_coverage.total}`;
 const hidden=state.tree.layout?.hidden||[];$('layoutSummary').textContent=state.section==='unmapped'?'Dati fuori dalla scheda mostrata dal gioco. '+(hidden.length?'Alcuni rami dipendono da DLC o radici non attive.':'La posizione non è stata verificata.'):'';
 $('layoutSummary').hidden=state.section!=='unmapped';applyZoom();filter();const first=[...nodes].sort((a,b)=>state.coords.get(a.id).y-state.coords.get(b.id).y||state.coords.get(a.id).x-state.coords.get(b.id).x)[0];select(nodes.some(t=>t.id===state.selected)?state.selected:first?.id);paintLive();
}
let request=0;async function loadFolder(){const id=$('folder').value,seq=++request;if(!id){$('error').hidden=false;$('error').textContent='Nessuna categoria di ricerca disponibile. Configura la cartella di HOI4 nel Focus Explorer.';return;}const tree=await json('/api/technology-tree?id='+encodeURIComponent(id));if(seq!==request)return;state.tree=tree;state.section='all';state.autoFit=true;state.search='';$('search').value='';renderTabs();renderSections();render();sessionStorage.setItem('hoi4-technology-folder',id);requestAnimationFrame(fitSheet);}
async function start(refresh=false){$('error').hidden=true;hideTip();const old=$('folder').value||new URLSearchParams(location.search).get('folder')||sessionStorage.getItem('hoi4-technology-folder');if(refresh)await json('/api/refresh',{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'});const catalog=await json('/api/technologies');catalog.folders.sort((a,b)=>(a.tab?.order??10000)-(b.tab?.order??10000));state.catalog=catalog;$('country').textContent=`Icone nazionali: ${catalog.country}`;$('folder').replaceChildren();for(const f of catalog.folders){const opt=el('option','',`${plain(f.name)} (${f.count})`);opt.value=f.id;$('folder').append(opt);}if(catalog.folders.some(f=>f.id===old))$('folder').value=old;$('dlcSummary').textContent=`${catalog.folders.length} schede di ricerca compatibili con i DLC`;$('excluded').replaceChildren(...catalog.excluded_folders.map(f=>el('li','',`${f.id}: ${f.reason}`)));await loadFolder();const requestedTech=new URLSearchParams(location.search).get('tech');if(requestedTech&&tech(requestedTech)){select(requestedTech);toggleInspector(true);requestAnimationFrame(()=>center(requestedTech));}}
$('folder').addEventListener('change',()=>loadFolder().catch(fail));$('reload').addEventListener('click',async()=>{const b=$('reload');b.disabled=true;try{await start(true);}catch(e){fail(e);}finally{b.disabled=false;}});let searchTimer;$('search').addEventListener('input',()=>{state.search=$('search').value;filter();clearTimeout(searchTimer);searchTimer=setTimeout(()=>{const match=all().find(t=>t.in_folder&&`${plain(t.name)} ${t.id}`.toLowerCase().includes(state.search.toLowerCase()));if(state.search&&match){select(match.id);center(match.id);}},180);});$('center').addEventListener('click',()=>center(state.selected));$('zoom').addEventListener('input',()=>{state.autoFit=false;state.zoom=Number($('zoom').value)/100;applyZoom();});
window.hoiLive?.subscribe(()=>{if(state.tree)paintLive();});window.addEventListener('hoi-live-follow',event=>location.assign('/?tree='+encodeURIComponent(event.detail.focus_tree_id)));
const viewport=$('graphViewport');let drag=null;viewport.addEventListener('pointerdown',e=>{if(e.button!==0||e.target.closest('button'))return;drag={x:e.clientX,y:e.clientY,left:viewport.scrollLeft,top:viewport.scrollTop};viewport.setPointerCapture(e.pointerId);});viewport.addEventListener('pointermove',e=>{if(!drag)return;viewport.scrollLeft=drag.left-(e.clientX-drag.x);viewport.scrollTop=drag.top-(e.clientY-drag.y);});viewport.addEventListener('pointerup',()=>drag=null);viewport.addEventListener('pointercancel',()=>drag=null);viewport.addEventListener('scroll',hideTip);window.addEventListener('resize',()=>{hideTip();if(state.autoFit)fitSheet();});document.addEventListener('keydown',e=>{if(e.key==='Escape')hideTip();});start().catch(fail);

$('fit').addEventListener('click',()=>{state.autoFit=true;fitSheet();});
$('inspectorToggle').addEventListener('click',()=>toggleInspector());
$('branchToggle').addEventListener('click',()=>{const open=document.body.classList.toggle('show-branches');$('branchToggle').setAttribute('aria-expanded',String(open));if(state.autoFit)requestAnimationFrame(fitSheet);});
$('liveDetails').addEventListener('click',()=>{const open=document.body.classList.toggle('show-live-details');$('liveDetails').setAttribute('aria-expanded',String(open));if(state.autoFit)requestAnimationFrame(fitSheet);});
if(typeof ResizeObserver!=='undefined')new ResizeObserver(()=>{if(state.autoFit)fitSheet();}).observe(viewport);
