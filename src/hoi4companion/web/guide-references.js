/* Passive source references. This module never changes guide or campaign state. */
'use strict';
(function(root,factory){const api=factory();if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiGuideReferences=api;})(typeof globalThis==='object'?globalThis:this,()=>{
 const clean=s=>s.replace(/\s+/g,' ').trim(),norm=s=>clean(s).toLocaleLowerCase('en'),escape=s=>s.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
 const section=/\b(?:setup|builds|plans?|priority|outcomes|events|achievements|improvements|references|strategy)\b/i;
 const heading=s=>{const base=s.replace(/\s*\([^)]*\)\s*$/,'');return s.length<=120&&/\p{L}/u.test(base)&&!/^[-•]/.test(s)&&base===base.toLocaleUpperCase('en')&&!/[.!?]$/.test(s);};
 // Reconstruct presentation from the row/cell boundaries already retained in
 // imported sources. This does not parse actions or modify saved definitions.
 function documentBlocks(text){
  if(!text.includes('[Table row]'))return text.split('\n').map(text=>({type:'text',text}));
  const chunks=text.split('[Table row]'),out=chunks.shift().split('\n').filter(Boolean).map(text=>({type:'text',text}));
  for(const chunk of chunks){
   const cells=[...chunk.matchAll(/\[Column \d+\]\r?\n([\s\S]*?)(?=\[Column \d+\]|$)/g)].map(m=>m[1].trim());
   if(!cells.length)continue;
   const last=cells.at(-1).split('\n'),tail=last.findIndex((line,i)=>i>0&&heading(clean(line))&&(section.test(line)||/^PART\b/.test(line)));
   if(tail>=0)cells[cells.length-1]=last.slice(0,tail).join('\n').trim();
   out.push({type:'row',cells});
   if(tail>=0)out.push(...last.slice(tail).map(text=>({type:'text',text})));
  }
  return out;
 }
 function sections(source){
  const result=[],stack=[];let serial=0,priority=false;
  const append=block=>{for(const item of stack){item.blocks.push(block);item.parts.push(block.type==='row'?block.cells.join('\n'):block.text);}};
  const make=(title,level)=>({id:source.id+':section:'+serial++,source_id:source.id,source_title:source.title||source.id,title,context:stack.map(s=>s.title).join(' › '),level,parts:[],blocks:[]});
  const add=(title,level)=>{
   title=title.replace(/:\s*$/,'');while(stack.length&&stack.at(-1).level>=level)stack.pop();
   append({type:'heading',text:title});const item=make(title,level);result.push(item);stack.push(item);
  };
  const addHeading=title=>add(title,/^HQ\s+SETUP\b/i.test(title)&&stack.some(s=>/^ARMY$/i.test(s.title))?3:section.test(title)||/^PART\b/.test(title)?1:2);
  for(const block of documentBlocks(source.text)){
   if(block.type==='text'){
    const plain=clean(block.text),markdown=/^(#{1,6})\s+(.+?)\s*#*\s*$/.exec(block.text);
    if(markdown){priority=false;add(markdown[2],markdown[1].length);}
    else if(heading(plain)){priority=false;addHeading(plain);}
    else if(!priority)append(block);
    continue;
   }
   const cells=block.cells,first=cells[0],band=cells.slice(1).every(c=>!c);
   if(cells.length===3&&/^ACTIONS$/i.test(first)&&/^RESEARCH PRIORITY\*?$/i.test(cells[1])){priority=true;stack.length=0;continue;}
   if(priority){if(!band||!heading(first)||first.includes('\n'))continue;priority=false;}
   if(cells.every(c=>!c))continue;
   if(band&&!first.includes('\n')&&heading(first)){addHeading(first);continue;}
   const columnHeader=cells.length>1&&cells.every(c=>heading(clean(c)))&&cells.some(c=>/\b(?:GENERAL|ROLE|ORDERS|NAME|MEANING|FLEET|COMMAND|UNITS|CODE|ADJUSTMENTS)\b/.test(c));
   append({...block,header:columnHeader});
   // Keep label-level references (e.g. peace-conference countries) without
   // letting them steal following rows or split an army table into fragments.
   if(!columnHeader&&cells.length>1&&!first.includes('\n')&&heading(first)){
    const item=make(first,2);item.parts.push(cells.slice(1).join('\n'));item.blocks.push(block);result.push(item);
   }
  }
  return result.map(({parts,level,...item})=>({...item,text:parts.join('\n').trim()})).filter(item=>item.text);
 }
 function compile(entries){
  const aliases=new Map();
  const alias=(text,item)=>{
   text=clean(text);if(!text||text.length>160)return;
   const key=norm(text),items=aliases.get(key)||[];if(!items.some(x=>x.id===item.id))items.push(item);aliases.set(key,items);
  };
  for(const item of entries){
   alias(item.title,item);
   const base=clean(item.title.replace(/\s*\([^)]*\)\s*$/,''));alias(base,item);
   alias(base.replace(/\b(build|setup|plan)s$/i,'$1'),item);
   const invasion=/^(.+?) invasion$/i.exec(base);if(invasion)alias('Invade '+invasion[1],item);
   if(/\b(?:setup|plans?)\b/i.test(item.context)&&!section.test(base))alias(base+' Setup',item);
   if(/\bsetup$/i.test(base)||/\bsetup\b/i.test(item.context)&&/^(?:ARMY|NAVY|AIR FORCE)$/i.test(base))alias('Setup',item);
   if(/ [\x26/] /.test(base))for(const member of base.split(/ [\x26/] /)){
    alias(member,item);if(/\b(?:setup|plans?)\b/i.test(item.context)&&!section.test(member))alias(member+' Setup',item);
   }
  }
  const words=[...aliases.keys()].sort((a,b)=>b.length-a.length).map(x=>escape(x).replace(/ /g,'\\s+'));
  return {entries,aliases,pattern:words.length?new RegExp('(?<![\\p{L}\\p{N}_])(?:'+words.join('|')+')\\*?(?![\\p{L}\\p{N}_])','giu'):null};
 }
 function build(guide){
  const sources=(guide.sources||[]).filter(s=>typeof s.id==='string'&&typeof s.text==='string');
  return {...compile(sources.flatMap(sections)),guide_id:guide.id,sources};
 }
 function images(row){
  const p=row?.action?.params||{},all=[...(p.reference_images||[]),p.design_reference,...(p.production_targets||[]).map(t=>t.design_reference)],out=[];
  for(const ref of all){
   const id=ref?.image_id||ref?.id;
   if(!/^[a-f0-9]{64}$/.test(id||'')||typeof ref.name!=='string'||!ref.name.trim())continue;
   if(!out.some(r=>r.id===id&&r.title===ref.name))out.push({id,title:ref.name,image_id:id,text:'',context:'',source_title:'',source_id:''});
  }
  return out;
 }
 const cue=/(?:\b(?:see|refer to|reference|consult|follow|check)\s+(?:the\s+)?|\b(?:vedi|consulta|riferimento a)\s+)$/i;
 function setupCandidates(candidates,text){
  const army=/\b(?:army|armies|hq)\b/i.test(text),navy=/\b(?:navy|fleets?)\b/i.test(text);
  if(army===navy)return candidates;
  const scores=candidates.map(entry=>({entry,score:army?/^ARMY$/i.test(entry.title)?0:/^HQ SETUP$/i.test(entry.title)?1:/^ARMY SETUP$/i.test(entry.title)?2:Infinity:/^NAVY$/i.test(entry.title)?0:/^NAVY SETUP$/i.test(entry.title)?1:Infinity}));
  const best=Math.min(...scores.map(s=>s.score));return Number.isFinite(best)?scores.filter(s=>s.score===best).map(s=>s.entry):[];
 }
 function quickReferences(index){
  return [
   {label:'Army setup',candidates:setupCandidates(index.aliases.get('setup')||[],'Armies')},
   {label:'War plans',candidates:index.entries.filter(e=>/^WAR PLANS?$/i.test(e.title))},
   {label:'Peace conference',candidates:index.entries.filter(e=>/^PEACE CONFERENCE(?: PRIORITY)?$/i.test(e.title))},
   {label:'Relevant events',candidates:index.entries.filter(e=>/^RELEVANT EVENTS$/i.test(e.title))}
  ].filter(r=>r.candidates.length).map(r=>({...r,text:r.label,reference:true}));
 }
 function segments(text,index,extra=[],context=''){
  if(typeof text!=='string')return [];
  let local=index;
  if(extra.length){
   const key=JSON.stringify(extra.map(r=>[r.id,r.title])),cache=index.imageIndexes||(index.imageIndexes=new Map());
   if(!cache.has(key)){if(cache.size>=64)cache.delete(cache.keys().next().value);cache.set(key,compile([...index.entries,...extra]));}
   local=cache.get(key);
  }
  const spans=[];
  if(local.pattern){local.pattern.lastIndex=0;for(const match of text.matchAll(local.pattern)){
   let label=match[0],start=match.index;
   const before=text.slice(Math.max(0,match.index-160),match.index);
   if(/^setup$/i.test(label)&&!cue.test(before))continue;
   let candidates=local.aliases.get(norm(label.replace(/\*$/,'')))||[];
   if(/^(?:(?:army|armies|navy|hq)\s+)?setup\*?$/i.test(label)){
    const previous=before.match(/([\p{L}\p{N}_/-]+)\s+$/u),qualified=/^setup\*?$/i.test(label)&&previous&&(/^[A-Z\d_/-]{2,}$/.test(previous[1])||/^(?:Axis|Allies)$/.test(previous[1]));
    if(qualified){start-=previous[0].length;label=text.slice(start,match.index+match[0].length);candidates=[];}
    else {
     const clause=before.split(/[\n;•]/).at(-1),hints=[...clause.matchAll(/\b(?:army|armies|hq|navy|fleets?)\b/gi)];
     const headers=[...text.slice(0,match.index).matchAll(/(?:^|\n)\s*[-•]?\s*(ARMY|ARMIES|NAVY)\s*:/gi)];
     const hint=/\b(?:army|armies|hq|navy)\b/i.test(label)?label:hints.at(-1)?.[0]||headers.at(-1)?.[1]||context;
     candidates=setupCandidates(candidates,hint);
    }
   }
   if(!label.endsWith('*')&&!/\b(?:setup|build|plan)$/i.test(label)&&!cue.test(before)&&!candidates.some(c=>c.image_id))continue;
   spans.push({start,end:match.index+match[0].length,text:label,reference:true,candidates});
  }}
  // Missing explicit references remain inspectable. Never fabricate a section
  // or use spelling resemblance to silently select another source.
  const explicit=/(?:\b(?:see|refer to|consult)|\b(?:fai riferimento a|vedi|consulta))\s+(?:the\s+)?["“]?([^\n,;:.!?"”]{1,100})/gi;
  for(const match of text.matchAll(explicit)){
   const label=match[1].trim(),start=match.index+match[0].lastIndexOf(match[1]);
   if(!spans.some(s=>s.start<start+label.length&&s.end>start))spans.push({start,end:start+label.length,text:label,reference:true,candidates:[]});
  }
  for(const match of text.matchAll(/(?<![\p{L}\p{N}_*])[\p{L}\p{N}][\p{L}\p{N}_/-]*\*(?!\*)/gu)){
   if(!spans.some(s=>s.start<match.index+match[0].length&&s.end>match.index))spans.push({start:match.index,end:match.index+match[0].length,text:match[0],reference:true,candidates:[]});
  }
  spans.sort((a,b)=>a.start-b.start||b.end-a.end);
  const output=[];let cursor=0;
  for(const span of spans){if(span.start<cursor)continue;if(span.start>cursor)output.push({text:text.slice(cursor,span.start)});output.push(span);cursor=span.end;}
  if(cursor<text.length)output.push({text:text.slice(cursor)});return output;
 }
 function create({document,t=message=>message}){
  let index=build({}),panel=null,body=null,title=null,origin=null,timer=null,pinned=false,trigger=null,restoring=false;
  const el=(tag,cls,text)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const clear=()=>{if(timer!==null){clearTimeout(timer);timer=null;}};
  function close(restore=false){clear();if(panel)panel.hidden=true;const old=trigger;trigger=null;pinned=false;if(restore&&old?.isConnected){restoring=true;try{old.focus();}finally{restoring=false;}}}
  function later(){clear();if(!pinned)timer=setTimeout(()=>close(),220);}
  function mount(){
   if(panel)return;
   panel=el('section','guide-reference-preview');panel.id='guideReferencePreview';panel.hidden=true;panel.tabIndex=-1;
   panel.setAttribute('role','dialog');panel.setAttribute('aria-modal','false');panel.setAttribute('aria-labelledby','guideReferenceTitle');
   const head=el('div','guide-reference-heading');title=el('h2');title.id='guideReferenceTitle';const exit=el('button','',t('Close'));exit.type='button';exit.onclick=()=>close(true);head.append(title,exit);
   origin=el('p','section-note',t('Original source reference · not a live observation.'));
   body=el('div','guide-reference-body');panel.append(head,origin,body);document.body.append(panel);
   panel.onpointerenter=clear;panel.onpointerleave=later;
   document.addEventListener('keydown',e=>{if(e.key==='Escape'&&!panel.hidden){e.preventDefault();close(pinned);}});
   document.addEventListener('pointerdown',e=>{if(!panel.hidden&&!panel.contains(e.target)&&!trigger?.contains(e.target))close();});
   document.addEventListener('scroll',e=>{if(!pinned&&e.target!==panel&&!panel.contains(e.target))close();},true);
  }
  function position(anchor){
   const rect=anchor.getBoundingClientRect(),width=document.documentElement.clientWidth,height=document.documentElement.clientHeight;
   const box=panel.getBoundingClientRect(),left=Math.max(8,Math.min(rect.left,width-box.width-8));
   const below=rect.bottom+8,top=below+box.height<=height-8?below:Math.max(8,rect.top-box.height-8);
   panel.style.left=left+'px';panel.style.top=top+'px';
  }
  function content(part){
   title.textContent=part.text.replace(/\*$/,'');body.replaceChildren();
   if(!part.candidates.length){
    body.append(el('p','',t('This reference could not be located in the imported sources.')));
    const link=el('a','',t('Browse original document text'));link.href='#sourceDetails';link.onclick=()=>{const target=document.getElementById('sourceDetails');if(target)target.open=true;close();};body.append(link);
    for(const source of index.sources){
     const details=el('details','guide-reference-choice');details.append(el('summary','',source.title||source.id));
     let loaded=false;details.ontoggle=()=>{if(details.open&&!loaded){loaded=true;renderBlocks(details,documentBlocks(source.text));}};body.append(details);
    }
    return;
   }
   if(part.candidates.length>1)body.append(el('p','section-note',t('Several sections match. Select the relevant reference.')));
   for(const entry of part.candidates){
    const section=part.candidates.length>1?el('details','guide-reference-choice'):el('section'),container=el('div');
    if(part.candidates.length>1)section.append(el('summary','',[entry.source_title,entry.context,entry.title].filter(Boolean).join(' › ')));
    else container.append(el('p','guide-reference-origin',[entry.source_title,entry.context,entry.title].filter(Boolean).join(' › ')));
    if(entry.image_id){const image=el('img');image.src='/api/reference-library/image?id='+encodeURIComponent(entry.image_id);image.alt=entry.title;image.loading='lazy';container.append(image);}
    if(entry.blocks?.length)renderBlocks(container,entry.blocks);
    else if(entry.text)renderBlocks(container,documentBlocks(entry.text));section.append(container);body.append(section);
   }
  }
  function renderBlocks(container,blocks){
   let rows=[];
   const flush=()=>{
    if(!rows.length)return;
    const wrapper=el('div','guide-reference-table-scroll'),table=el('table','guide-reference-table'),body=el('tbody');
    wrapper.tabIndex=0;wrapper.setAttribute('role','region');wrapper.setAttribute('aria-label',t('Original source reference · not a live observation.'));
    const width=Math.max(...rows.map(r=>r.cells.length));
    for(const row of rows){
     const tr=el('tr',row.header?'reference-column-heading':'');
     const cells=row.cells;
     cells.forEach((text,i)=>{
      const cell=el(row.header||cells.length>1&&i===0?'th':'td','',text);
      if(cell.tagName==='TH'||row.header||cells.length>1&&i===0)cell.setAttribute('scope',row.header?'col':'row');
      if(i===cells.length-1&&width>cells.length)cell.setAttribute('colspan',String(width-cells.length+1));
      tr.append(cell);
     });body.append(tr);
    }
    table.append(body);wrapper.append(table);container.append(wrapper);rows=[];
   };
   for(const block of blocks){
    if(block.type==='row'){rows.push(block);continue;}flush();
    if(block.text?.trim())container.append(el(block.type==='heading'?'h3':'p',block.type==='heading'?'guide-reference-band':'guide-reference-text',block.text));
   }flush();
  }
  function show(part,anchor,pin){
   if(!pin&&pinned)return;clear();mount();trigger=anchor;pinned=pin;content(part);panel.hidden=false;position(anchor);
   if(pin)panel.focus({preventScroll:true});
  }
  function referenceButton(part,cls='guide-reference-link'){
   const button=el('button',cls,part.text);button.type='button';button.setAttribute('aria-haspopup','dialog');button.setAttribute('aria-controls','guideReferencePreview');button.setAttribute('aria-label',t('View reference: {0}').replace('{0}',part.text));
   // A click opens the preview and a second click closes it; hovering or
   // tabbing over a reference never pops a preview over the page.
   button.onclick=()=>{if(panel&&!panel.hidden&&trigger===button)close(true);else show(part,button,true);};return button;
  }
  function appendParts(node,parts){
   for(const part of parts){
    if(!part.reference){node.append(document.createTextNode(part.text));continue;}
    node.append(referenceButton(part));
   }
  }
  function append(node,text,row){appendParts(node,segments(text,index,images(row),row?.name||row?.title||''));}
  function related(row,displayed){
   const extra=images(row),key=p=>p.candidates.length?p.candidates.map(r=>r.id).sort().join('|'):norm(p.text),seen=new Set();
   for(const text of displayed)for(const part of segments(text,index,extra,row?.name||row?.title||''))if(part.reference)seen.add(key(part));
   const parts=[];
   for(const text of [...(row.instructions||[]),...(row.action?.params?.instructions||[]),...(row.evidence||[]).map(e=>e.quote)]){
    for(const part of segments(text,index,extra,row?.name||row?.title||'')){if(!part.reference||seen.has(key(part)))continue;seen.add(key(part));if(parts.length)parts.push({text:' · '});parts.push(part);}
   }
   if(!parts.length)return null;
   const node=el('p','guide-reference-related');node.append(document.createTextNode(t('Guide references')+': '));appendParts(node,parts);return node;
  }
  return {append,related,shortcuts(){return quickReferences(index).map(part=>referenceButton({...part,text:t(part.label)},'guide-reference-shortcut'));},setGuide(guide){
   const sources=(guide.sources||[]).filter(s=>typeof s.id==='string'&&typeof s.text==='string');
   if(guide.id===index.guide_id&&sources.length===index.sources.length&&sources.every((s,i)=>['id','title','text'].every(k=>s[k]===index.sources[i][k])))return false;
   close();index=build(guide);return true;
  },refresh(){if(trigger&&!trigger.isConnected&&!pinned)close();},close};
 }
 return {build,segments,images,create,quickReferences,documentBlocks};
});
