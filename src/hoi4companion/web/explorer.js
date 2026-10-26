'use strict';
const $ = id => document.getElementById(id);
const state = {overview:null,tree:null,selected:null,goal:null,search:'',zoom:.78,manual:new Set(),poll:false,coords:new Map(),treeRequest:0};
const statusLabels={completed:HoiI18n.t("Completed"),active:HoiI18n.t("In progress"),ready:HoiI18n.t("Prerequisites met"),locked:HoiI18n.t("Missing prerequisites"),excluded:HoiI18n.t("Excluded path"),verify:HoiI18n.t("Conditions to verify"),unknown:HoiI18n.t("Unverified")};
let focusLabelContext;
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
      else{const item=el('span','unresolved-icon',match[2].replace(/^GFX_/,'').replaceAll('_',' '));item.title=HoiI18n.t("Game symbol unavailable: ")+token;root.append(item);}
      continue;
    }
    const item=el('span','dynamic-text',token.startsWith('$')?HoiI18n.t("Dynamic value"):token);
    item.title=HoiI18n.t("Depends on the current game; original reference: ")+token;root.append(item);
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
  $('modeText').textContent=v.demo?HoiI18n.t("Local demo"):HoiI18n.t("HOI4 data");$('modeHint').textContent=v.demo?HoiI18n.t("Game path not configured"):HoiI18n.t("Actual files, read-only");
  $('warnings').hidden=!v.warnings?.length;$('warnings').textContent=(v.warnings||[]).slice(0,5).join('\n');
  $('modsPanel').hidden=!v.mods;
  if(v.mods){
    const mods=v.mods,labels={observed:HoiI18n.t("Profile loaded"),partial:HoiI18n.t("Some mods could not be read"),unavailable:HoiI18n.t("Profile unavailable"),off:HoiI18n.t("Mods disabled in settings")};
    $('modsSummary').textContent=HoiI18n.t("Mods · {0} · {1} folders loaded",labels[mods.status]||HoiI18n.t("Needs verification"),(mods.roots||[]).length);
    $('modsSource').textContent=mods.source?HoiI18n.t("Loaded profile: ")+mods.source:HoiI18n.t("No mod profile loaded.");
    clear($('modsList'));for(const item of mods.items||[])$('modsList').append(el('li','',`${item.name} — ${item.status==='loaded'?HoiI18n.t("Loaded files: ")+item.path:item.reason}`));
    if(mods.status==='observed'&&!mods.items.length)$('modsList').append(el('li','',HoiI18n.t("No mods enabled in this profile.")));
  }
  $('dlcPanel').hidden=!v.dlcs;
  if(v.dlcs){
    const hidden=v.excluded_trees||[];
    $('dlcSummary').textContent=HoiI18n.t("DLC filter enabled · {0} DLCs available · {1} trees shown · {2} hidden",v.dlcs.enabled_count,v.trees.length,hidden.length);
    const labels={enabled:HoiI18n.t("Installed and not disabled"),integrated:HoiI18n.t("Integrated into the game"),disabled:HoiI18n.t("Disabled in the launcher"),missing:HoiI18n.t("Missing files"),unknown:HoiI18n.t("Activation needs verification")};
    clear($('dlcList'));for(const item of v.dlcs.items)$('dlcList').append(el('li','',`${item.name} — ${labels[item.status]||HoiI18n.t("Needs verification")}`));
    $('excludedSummary').textContent=hidden.length?HoiI18n.t("Hidden trees and reasons:"):HoiI18n.t("No trees excluded.");
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
  tip.append(el('div','tooltip-status',HoiI18n.t("{0} base days · {1}",Math.round(focus.cost*7),statusLabels[statuses()[focus.id]]||HoiI18n.t("Unverified"))));
  if(focus.icon_variants?.length)tip.append(el('p','tooltip-note',HoiI18n.t("Conditional icon: this is a preview.")));
  if(focus.description)tip.append(richText(focus.description.length>480?focus.description.slice(0,480)+'…':focus.description));
  const effects=focus.tooltip_sections?.find(x=>x.heading===HoiI18n.t("Effects"));
  if(effects){tip.append(el('strong','tooltip-subheading',HoiI18n.t("Effects from files")),richText(effects.text.length>220?effects.text.slice(0,220)+'…':effects.text));}
  const guidance=window.hoiGuideTree?.marker(focus.id,state.tree.id);if(guidance)tip.append(el('strong','tooltip-subheading',HoiI18n.t("Guide: ")+({next:HoiI18n.t("Next focus"),upcoming:HoiI18n.t("After the current focus"),active:HoiI18n.t("In progress"),waiting:HoiI18n.t("Waiting"),preview:HoiI18n.t("Next focus preview")}[guidance.tone])));
  tip.append(el('p','tooltip-note',HoiI18n.t("Click for full details. Game conditions and values have not been evaluated.")));
  tip.hidden=false;node.setAttribute('aria-describedby','focusTooltip');
  const rect=node.getBoundingClientRect(),width=tip.offsetWidth,height=tip.offsetHeight;
  const left=rect.right+12+width<innerWidth?rect.right+12:Math.max(10,rect.left-width-12);
  tip.style.left=Math.min(left,innerWidth-width-10)+'px';tip.style.top=Math.max(10,Math.min(rect.top,innerHeight-height-10))+'px';
}
function centerFocus(id){
  const position=state.coords.get(id);if(!position)return;
  const node=state.geometry.node,viewport=$('graphViewport');viewport.scrollLeft=(position.x+node.width/2)*state.zoom-viewport.clientWidth/2;viewport.scrollTop=(position.y+node.height/2)*state.zoom-viewport.clientHeight/2;
}
function selectFocus(id){const keepFocus=Boolean(document.activeElement?.dataset.focusId);state.selected=id;render();$('inspectorContent').scrollTop=0;if(keepFocus){const node=[...$('nodes').children].find(x=>x.dataset.focusId===id);node?.focus({preventScroll:true});}}
function render(){
  const tree=state.tree;if(!tree)return;hideTooltip();
  $('sideTree').textContent=tree.id;$('sideCount').textContent=tree.focuses.length+HoiI18n.t(" identified focuses");
  $('graphTitle').textContent=tree.id.toUpperCase();$('graphCounter').textContent=tree.focuses.length+HoiI18n.t(" NODES");
  $('assetInfo').textContent=tree.asset_coverage?HoiI18n.t("Original icons: {0}/{1}{2}",tree.asset_coverage.resolved,tree.asset_coverage.total,tree.asset_coverage.conditional?' · '+tree.asset_coverage.conditional+HoiI18n.t(" conditional"):''):HoiI18n.t("Demo icons");
  const byId=new Map(tree.focuses.map(f=>[f.id,f])),status=statuses();
  const geometry=HoiFocusLayout.layout(tree),{coords,width,height}=geometry;state.geometry=geometry;state.coords=coords;
  $('assetInfo').textContent+=' · '+(geometry.native?HoiI18n.t("Local GUI geometry"):HoiI18n.t("Fallback geometry"));
  const world=$('world');world.style.width=width+'px';world.style.height=height+'px';world.style.transform=`scale(${state.zoom})`;
  $('worldSizer').style.width=(width*state.zoom)+'px';$('worldSizer').style.height=(height*state.zoom)+'px';
  const wires=$('wires');clear(wires);wires.setAttribute('viewBox',`0 0 ${width} ${height}`);wires.setAttribute('width',width);wires.setAttribute('height',height);
  const route=state.goal?ancestors(state.goal):null,exclusive=new Set();
  const pathFor=(d,cls)=>{const path=document.createElementNS('http://www.w3.org/2000/svg','path');path.setAttribute('d',d);path.setAttribute('class',cls);wires.append(path);};
  const imageFor=(sprite,x,y,w,h,cls='')=>{if(!sprite?.url)return;const image=document.createElementNS('http://www.w3.org/2000/svg','image');for(const [k,v]of Object.entries({href:sprite.url,x,y,width:w,height:h,preserveAspectRatio:'none',class:cls}))image.setAttribute(k,v);wires.append(image);};
  for(const focus of tree.focuses){
    const dest=coords.get(focus.id);if(!dest)continue;
    for(const group of focus.prerequisites)for(const prev of group){
      if(!coords.has(prev))continue;
      const edge=HoiFocusLayout.edge(coords.get(prev),dest,geometry,tree.visuals),marked=route&&route.has(prev)&&route.has(focus.id);
      const completed=status[prev]==='completed'&&status[focus.id]==='completed',active=status[prev]==='completed'&&status[focus.id]==='active';
      const variant=completed?(group.length>1?'completed_alternative':'completed'):(group.length>1?'alternative':'neutral');
      const assets=tree.visuals?.link_states?.[variant]||tree.visuals?.links;
      const pieces=geometry.native?HoiFocusLayout.tiles(edge.points,assets,tree.visuals.positions.link_spacing):[];
      for(const p of pieces)imageFor(p.sprite,p.x,p.y,p.width,p.height,'focus-link-art'+(!completed&&!active?' focus-link-unverified':'')+(route&&!marked?' dim':''));
      if(!pieces.length||marked)pathFor(edge.path,'wire'+(marked?' goal':route?' dim':''));
    }
    for(const id of focus.mutually_exclusive){
      if(!coords.has(id))continue;const key=[focus.id,id].sort().join('|');if(exclusive.has(key))continue;exclusive.add(key);
      const link=HoiFocusLayout.exclusive(dest,coords.get(id),geometry,tree.visuals),{start,end}=link,c=link.children||{},mid=(start.x+end.x)/2,cls='focus-link-art'+(route?' dim':'');
      if(link.native&&c.mid?.sprite&&c.link1?.sprite&&c.link2?.sprite){
        const sprite=c.mid.sprite,w=sprite.width||24,h=sprite.height||24;
        for(const [key,left,right]of [['link1',start.x+(c.link1?.position.x||0),mid-w/2],['link2',mid+w/2,end.x]]){
          const part=c[key];if(right>left&&part?.sprite)imageFor(part.sprite,left,start.y+(part.position.y||0),right-left,part.sprite.height||4,cls);
        }
        imageFor(sprite,mid-w/2,start.y,w,h,cls);
        if(end.x-start.x>=w*3){for(const [key,x]of [['left',start.x],['right',end.x-w]])if(c[key]?.sprite)imageFor(c[key].sprite,x,start.y,w,h,cls);}
      }else{
        pathFor(`M ${start.x} ${start.y} L ${end.x} ${end.y}`,'wire mutex'+(route?' dim':''));
        if(tree.theme?.exclusive)imageFor(tree.theme.exclusive,mid-12,(start.y+end.y)/2-12,24,24,cls);
      }
    }
  }
  const nodes=$('nodes'),labels=[];clear(nodes);
  for(const focus of tree.focuses){
    const position=coords.get(focus.id),code=status[focus.id]||'unknown';if(!position)continue;
    const node=el('button','focus-node '+code);node.type='button';node.dataset.focusId=focus.id;node.style.left=position.x+'px';node.style.top=position.y+'px';
    node.style.width=geometry.node.width+'px';node.style.height=geometry.node.height+'px';node.style.minHeight=geometry.node.height+'px';
    const match=!state.search||focus.id.toLowerCase().includes(state.search)||plain(focus.name).toLowerCase().includes(state.search);
    if(!match||(route&&!route.has(focus.id)))node.classList.add('dim');if(route?.has(focus.id))node.classList.add('on-route');if(state.selected===focus.id)node.classList.add('selected');
    const emblem=el('div','focus-emblem');emblem.append(spriteImage(focus.sprite||focus.variant_sprites?.[0]));
    if(focus.icon_variants?.length)emblem.append(el('span','variant-badge','?'));
    if(code==='completed'||code==='active')emblem.append(el('span','completion-badge',code==='completed'?'✓':'◉'));
    const title=el('div','focus-nameplate'),frame=tree.visuals?.states?.[code]||tree.theme?.[code];if(frame)title.append(spriteImage(frame,'focus-titlebar'));
    const name=richText(focus.name,'focus-name');title.append(name);
    if(geometry.native){
      const children=tree.visuals.templates.national_focus_item.children;
      const symbol=HoiFocusLayout.component(children.symbol,geometry.node),bg=HoiFocusLayout.component(children.bg,geometry.node),text=HoiFocusLayout.component(children.name,geometry.node);
      if(symbol&&bg&&text){node.classList.add('native-focus');
        emblem.style.left=symbol.x+'px';emblem.style.top=symbol.y+'px';emblem.style.setProperty('--symbol-transform',`${symbol.center?'translate(-50%,-50%) ':''}scale(${symbol.scale})`);
        title.style.left=bg.x+'px';title.style.top=bg.y+'px';title.style.width=(frame?.width||geometry.node.width)+'px';title.style.height=(frame?.height||40)+'px';
        name.style.left=(text.x-bg.x)+'px';name.style.top=(text.y-bg.y)+'px';name.style.width=(children.name.width||147)+'px';name.style.height=(children.name.height||20)+'px';name.style.textAlign=children.name.align||'center';
        const fontSize=Number(children.name.font?.match(/_(\d{1,2})/)?.[1]);if(fontSize>=8&&fontSize<=32)name.style.fontSize=fontSize+'px';
        labels.push(name);
      }
    }
    node.append(emblem,title,el('span','focus-duration',`${Math.round(focus.cost*7)}d*`));
    node.setAttribute('aria-label',`${plain(focus.name)}: ${statusLabels[code]||HoiI18n.t("state not verified")}`);
    node.addEventListener('click',()=>selectFocus(focus.id));node.addEventListener('pointerenter',()=>showTooltip(focus,node));node.addEventListener('pointerleave',hideTooltip);node.addEventListener('focus',()=>showTooltip(focus,node));node.addEventListener('blur',hideTooltip);nodes.append(node);
  }
  // Browser glyphs differ from the game's font. Fit the declared text box.
  if(labels.length&&typeof getComputedStyle==='function'){
    focusLabelContext??=document.createElement('canvas').getContext('2d');
    if(focusLabelContext){const font=getComputedStyle(labels[0]);
      for(const name of labels){const nominal=parseFloat(name.style.fontSize)||13,width=parseFloat(name.style.width);
        focusLabelContext.font=`${font.fontWeight} ${nominal}px ${font.fontFamily}`;
        const measured=focusLabelContext.measureText(plain(name.textContent)).width;
        if(measured>width)name.style.fontSize=Math.max(10,Math.floor(nominal*width/measured*10)/10)+'px';
      }
    }
  }
  renderInspector(byId,status);
  window.hoiGuideTree?.paint(document.querySelectorAll('.focus-node'),tree.id);
  $('snapshotInfo').textContent=window.hoiLive?.enabled?HoiI18n.t("State from process memory. Dynamic availability is not inferred; manual marks are separate."):tree.snapshot?`Snapshot ${tree.snapshot.source} · ${tree.snapshot.tag}${tree.snapshot.note?' · '+tree.snapshot.note:''}`:tree.snapshot_error?HoiI18n.t("Snapshot not loaded: {0}",tree.snapshot_error):HoiI18n.t("Without a snapshot: manual progress is stored only in the browser, not in the game.");
}
function renderInspector(byId,status){
  const container=$('inspectorContent');clear(container);const focus=byId.get(state.selected);$('focusIndex').textContent=focus?`${[...byId.keys()].indexOf(focus.id)+1} / ${byId.size}`:'—';
  if(!focus){container.append(el('div','empty-icon','◈'),el('p','',HoiI18n.t("Select a focus to see its prerequisites and path.")));return;}
  container.append(spriteImage(focus.sprite||focus.variant_sprites?.[0],'inspector-icon'),el('div','eyebrow',HoiI18n.t("NATIONAL FOCUS")),richText(focus.name,'focus-title','h2'),el('div','focus-code',focus.id),el('div','status-chip '+status[focus.id],statusLabels[status[focus.id]]||HoiI18n.t("Unverified")));
  const guidance=window.hoiGuideTree?.details(focus.id,state.tree.id);if(guidance)container.append(guidance);
  if(focus.description)container.append(richText(focus.description));
  container.append(el('div','tooltip-note',HoiI18n.t("Text from game files; dynamic conditions and values have not been evaluated.")));
  if(focus.icon_variants?.length){container.append(el('div','minor-heading',HoiI18n.t("ICON VARIANTS")),el('p','muted',HoiI18n.t("The current variant depends on the game state.")));const gallery=el('div','variant-gallery');for(const sprite of focus.variant_sprites||[]){const image=spriteImage(sprite);image.title=sprite.name;gallery.append(image);}container.append(gallery);}
  const meta=el('div','meta-grid');const duration=el('div');duration.append(el('small','',HoiI18n.t("Base duration")),el('strong','',HoiI18n.t("{0} days*",Math.round(focus.cost*7))));meta.append(duration);container.append(el('div','minor-heading',HoiI18n.t("DATA AND REQUIREMENTS")),meta);
  const ul=el('ul','requirements');if(!focus.prerequisites.length)ul.append(el('li','',HoiI18n.t("No previous focus required.")));
  for(const group of focus.prerequisites){const item=el('li');item.append(el('b','',group.length>1?HoiI18n.t("AT LEAST ONE · "):HoiI18n.t("REQUIRED · ")));group.forEach((id,index)=>{if(index)item.append(document.createTextNode(HoiI18n.t(" or ")));const button=el('button','focus-reference',plain(byId.get(id)?.name||id));button.addEventListener('click',()=>{if(byId.has(id)){selectFocus(id);centerFocus(id);}});item.append(button);});ul.append(item);}
  if(focus.mutually_exclusive.length){const item=el('li');item.append(el('b','',HoiI18n.t("EXCLUSIVE · ")),richText(focus.mutually_exclusive.map(id=>byId.get(id)?.name||id).join(', ')));ul.append(item);}
  if(focus.has_conditions)ul.append(el('li','',HoiI18n.t("⚠ Additional conditions to check in the game.")));if(focus.has_bypass)ul.append(el('li','',HoiI18n.t("↷ Bypass conditions are defined.")));if(focus.position_warning)ul.append(el('li','',HoiI18n.t("⚠ Approximate position: missing or cyclic reference.")));container.append(ul);
  for(const section of focus.tooltip_sections||[])container.append(el('div','minor-heading',section.heading.toUpperCase()),richText(section.text));
  container.append(el('div','minor-heading',HoiI18n.t("ACTIONS")));
  const goal=el('button','action',HoiI18n.t("◈ Highlight all prerequisites"));goal.addEventListener('click',()=>{state.goal=focus.id;render();});container.append(goal);
  if(!state.tree.has_snapshot&&!window.hoiLive?.enabled){const done=state.manual.has(focus.id),button=el('button','action secondary',done?HoiI18n.t("↶ Undo manual completion"):HoiI18n.t("✓ Mark as manually completed"));button.addEventListener('click',()=>{if(done)state.manual.delete(focus.id);else state.manual.add(focus.id);syncManual();render();});container.append(button);}
  const source=el('details','asset-details');source.append(el('summary','',HoiI18n.t("Data source")),el('div','muted',focus.source),el('div','muted',focus.sprite?.source||HoiI18n.t("Unresolved icon")));
  if(focus.sprite?.override)source.append(el('div','muted',HoiI18n.t("Texture from a DLC or configured mod.")));container.append(source);
}
async function loadTree(id){
  const request=++state.treeRequest;
  if(!id){state.tree={id:HoiI18n.t("No compatible trees"),focuses:[]};state.selected=null;state.goal=null;render();$('snapshotInfo').textContent=HoiI18n.t("Check paths and exclusion reasons in the DLC panel.");return;}
  const old=state.tree?.id,tree=await getJson('/api/tree?id='+encodeURIComponent(id)+(window.hoiLive?.enabled?'&source=live':''));if(request!==state.treeRequest)return;if(tree.error)throw new Error(tree.error);state.tree=tree;setManual();if(state.selected&&!currentFocus(state.selected))state.selected=null;const requestedFocus=!old?new URLSearchParams(location.search).get('focus'):null;if(requestedFocus&&currentFocus(requestedFocus))state.selected=requestedFocus;if(!state.selected)state.selected=state.tree.focuses[0]?.id||null;render();if(old!==id)requestAnimationFrame(()=>centerFocus(state.selected));
}
async function reload(all=true){try{if(all){state.overview=await getJson('/api/config');updateOverview();if(!state.tree){const requested=new URLSearchParams(location.search).get('tree')||window.hoiGuide?.value?.guide?.applies_to?.tree_id;if(state.overview.trees.some(t=>t.id===requested))$('treeSelect').value=requested;}}await loadTree($('treeSelect').value||state.overview.settings.tree_id);$('connection').textContent=HoiI18n.t("● Local server");}catch(error){$('connection').textContent=HoiI18n.t("● Connection error");$('warnings').hidden=false;$('warnings').textContent=error.message;}}
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
  window.hoiGuideTree?.connect(async row=>{const id=row.action.entity.scope||window.hoiGuide.value.guide.applies_to.tree_id;if(!state.overview?.trees.some(t=>t.id===id))throw Error(HoiI18n.t("The guide's focus tree is not available in the catalog"));$('treeSelect').value=id;state.selected=row.target;state.search='';$('search').value='';state.goal=null;await loadTree(id);centerFocus(row.target);});
  let guideTreeInitialised=false;
  window.addEventListener('hoi-guide-change',()=>{
    const guided=window.hoiGuide?.value?.guide?.applies_to?.tree_id;
    if(!guideTreeInitialised&&state.overview&&guided){guideTreeInitialised=true;if(!new URLSearchParams(location.search).has('tree')&&state.overview.trees.some(t=>t.id===guided)&&state.tree?.id!==guided){$('treeSelect').value=guided;loadTree(guided).catch(error=>{$('snapshotInfo').textContent=error.message;});return;}}
    if(state.tree)render();
  });
  let lastLiveEnabled=window.hoiLive?.enabled;
  window.hoiLive?.subscribe(()=>{const switched=lastLiveEnabled!==window.hoiLive.enabled;lastLiveEnabled=window.hoiLive.enabled;if(state.tree){if(switched)loadTree(state.tree.id).catch(error=>{$('snapshotInfo').textContent=error.message;});else if(!window.hoiGuide)render();}});
  window.addEventListener('hoi-live-follow',async event=>{const id=event.detail.focus_tree_id;if(!state.overview?.trees.some(t=>t.id===id))return;$('treeSelect').value=id;state.selected=event.detail.current_focus||null;await loadTree(id);});
  setInterval(()=>{if(!window.hoiLive?.enabled&&state.poll&&state.overview?.settings.snapshot_path)reload(false);},15000);reload();
}
init();
