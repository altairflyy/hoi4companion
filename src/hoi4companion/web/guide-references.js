/* Passive source references. This module never changes guide or campaign state. */
'use strict';
(function(root,factory){const api=factory();if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiGuideReferences=api;})(typeof globalThis==='object'?globalThis:this,()=>{
 const clean=s=>s.replace(/\s+/g,' ').trim(),norm=s=>clean(s).toLocaleLowerCase('en'),escape=s=>s.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
 const section=/\b(?:setup|builds|plans?|priority|outcomes|events|achievements|improvements|references|strategy)\b/i;
 const heading=s=>{const base=s.replace(/\s*\([^)]*\)\s*$/,'');return s.length<=120&&/\p{L}/u.test(base)&&!/^[-•]/.test(s)&&base===base.toLocaleUpperCase('en')&&!/[.!?]$/.test(s);};
 function sections(source){
  const result=[],stack=[];let serial=0;
  const addText=text=>{for(const item of stack)item.parts.push(text);};
  const add=(title,level)=>{
   title=title.replace(/:\s*$/,'');
   while(stack.length&&stack.at(-1).level>=level)stack.pop();
   addText(title);
   const item={id:source.id+':section:'+serial++,source_id:source.id,source_title:source.title||source.id,title,context:stack.map(s=>s.title).join(' › '),level,parts:[]};
   result.push(item);stack.push(item);
  };
  const appendLines=text=>{for(const line of text.split('\n')){const plain=clean(line);if(heading(plain))add(plain,section.test(plain)||/^PART\b/.test(plain)?1:2);else addText(line);}};
  if(source.text.includes('[Table row]')){
   const blocks=source.text.split('[Table row]');let priority=false;
   for(const block of blocks){
    const columns=[...block.matchAll(/\[Column \d+\]\n([\s\S]*?)(?=\[Column \d+\]|$)/g)].map(m=>m[1].trim());
    if(!columns.length)continue;
    if(columns.length===3&&/^ACTIONS$/i.test(columns[0])&&/^RESEARCH PRIORITY\*?$/i.test(columns[1])){priority=true;stack.length=0;continue;}
    const first=columns[0],single=columns.length===1;
    if(priority){if(!single||!heading(first)||first.includes('\n'))continue;priority=false;}
    if(!first&&columns.every(c=>!c))continue;
    if(!first.includes('\n')&&heading(first)){
     add(first,single&&section.test(first)?1:2);
     if(!single)appendLines(columns.slice(1).join('\n'));
    }else appendLines(columns.filter(Boolean).join('\n'));
   }
  }else{
   for(const line of source.text.split('\n')){
    const markdown=/^(#{1,6})\s+(.+?)\s*#*\s*$/.exec(line),plain=clean(line);
    if(markdown)add(markdown[2],markdown[1].length);
    else if(heading(plain))add(plain,section.test(plain)?1:2);
    else addText(line);
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
   if(item.title.includes(' / '))alias(item.title.split(' / ')[0],item);
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
 function segments(text,index,extra=[]){
  if(typeof text!=='string')return [];
  let local=index;
  if(extra.length){
   const key=JSON.stringify(extra.map(r=>[r.id,r.title])),cache=index.imageIndexes||(index.imageIndexes=new Map());
   if(!cache.has(key)){if(cache.size>=64)cache.delete(cache.keys().next().value);cache.set(key,compile([...index.entries,...extra]));}
   local=cache.get(key);
  }
  const spans=[];
  if(local.pattern){local.pattern.lastIndex=0;for(const match of text.matchAll(local.pattern)){
   const label=match[0],candidates=local.aliases.get(norm(label.replace(/\*$/,'')))||[],before=text.slice(Math.max(0,match.index-60),match.index);
   if(!label.endsWith('*')&&!/\b(?:setup|build|plan)$/i.test(label)&&!cue.test(before)&&!candidates.some(c=>c.image_id))continue;
   spans.push({start:match.index,end:match.index+label.length,text:label,reference:true,candidates});
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
   document.addEventListener('scroll',()=>{if(!pinned)close();},true);
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
     let loaded=false;details.ontoggle=()=>{if(details.open&&!loaded){loaded=true;details.append(el('pre','guide-reference-text',source.text));}};body.append(details);
    }
    return;
   }
   if(part.candidates.length>1)body.append(el('p','section-note',t('Several sections match. Select the relevant reference.')));
   for(const entry of part.candidates){
    const section=part.candidates.length>1?el('details','guide-reference-choice'):el('section'),container=el('div');
    if(part.candidates.length>1)section.append(el('summary','',[entry.source_title,entry.context,entry.title].filter(Boolean).join(' › ')));
    else container.append(el('p','guide-reference-origin',[entry.source_title,entry.context,entry.title].filter(Boolean).join(' › ')));
    if(entry.image_id){const image=el('img');image.src='/api/reference-library/image?id='+encodeURIComponent(entry.image_id);image.alt=entry.title;image.loading='lazy';container.append(image);}
    if(entry.text)container.append(el('pre','guide-reference-text',entry.text));section.append(container);body.append(section);
   }
  }
  function show(part,anchor,pin){
   if(!pin&&pinned)return;clear();mount();trigger=anchor;pinned=pin;content(part);panel.hidden=false;position(anchor);
   if(pin)panel.focus({preventScroll:true});
  }
  function appendParts(node,parts){
   for(const part of parts){
    if(!part.reference){node.append(document.createTextNode(part.text));continue;}
    const button=el('button','guide-reference-link',part.text);button.type='button';button.setAttribute('aria-haspopup','dialog');button.setAttribute('aria-controls','guideReferencePreview');button.setAttribute('aria-label',t('View reference: {0}').replace('{0}',part.text));
    button.onpointerenter=()=>show(part,button,false);button.onpointerleave=later;button.onfocus=()=>{if(!restoring)show(part,button,false);};button.onblur=later;button.onclick=()=>show(part,button,true);node.append(button);
   }
  }
  function append(node,text,row){appendParts(node,segments(text,index,images(row)));}
  function related(row,displayed){
   const extra=images(row),key=p=>p.candidates.length?p.candidates.map(r=>r.id).sort().join('|'):norm(p.text),seen=new Set();
   for(const text of displayed)for(const part of segments(text,index,extra))if(part.reference)seen.add(key(part));
   const parts=[];
   for(const text of [...(row.instructions||[]),...(row.action?.params?.instructions||[]),...(row.evidence||[]).map(e=>e.quote)]){
    for(const part of segments(text,index,extra)){if(!part.reference||seen.has(key(part)))continue;seen.add(key(part));if(parts.length)parts.push({text:' · '});parts.push(part);}
   }
   if(!parts.length)return null;
   const node=el('p','guide-reference-related');node.append(document.createTextNode(t('Guide references')+': '));appendParts(node,parts);return node;
  }
  return {append,related,setGuide(guide){close();index=build(guide);},refresh(){if(trigger&&!trigger.isConnected&&!pinned)close();},close};
 }
 return {build,segments,images,create};
});
