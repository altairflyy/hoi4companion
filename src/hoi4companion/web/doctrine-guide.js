'use strict';
/* Structured reading of the advanced "doctrines and spirits" documents for the
   Templates & references page. Presentation only: it reshapes the authored text
   (ratings, buffs, spirits, comments, quick-reference table) without judging,
   reordering or completing anything, and returns null for any document whose
   layout it does not recognize, so the page falls back to the original text. */
(function(root){
 const URL_LINE=/^https?:\/\/\S+$/i;
 const NOISE=/^(?:\[document links\]|(?:related video|builds)(?:\s*•\s*(?:related video|builds))*)$/i;
 const clean=line=>String(line).replace(/\s+/g,' ').trim();
 // "GRAND BATTLEPLAN" reads as "Grand Battleplan"; mixed-case text is kept.
 function display(text){
  const value=clean(text);
  if(!/[A-Z]/.test(value)||/[a-z]/.test(value))return value;
  return value.toLowerCase().replace(/(^|[\s(/&-])(\p{L})/gu,(m,a,b)=>a+b.toUpperCase());
 }
 // "NAME (7/10)" with the rating as written; null otherwise.
 function rated(line){
  const m=clean(line).match(/^(.+?)\s*\((\d{1,2})\s*\/\s*10\)$/);
  if(!m||Number(m[2])>10)return null;
  return {name:display(m[1]),score:Number(m[2])};
 }
 // A table cell such as "A (10/10) / B (8/10)": each pick with its rating.
 function picks(cell){
  return clean(cell).split(/\s+\/\s+/).map(part=>rated(part)||{name:display(part),score:null}).filter(p=>p.name);
 }
 function quickTable(lines){
  const rows=[];let row=null,expect=false;
  for(const raw of lines){
   const line=clean(raw);
   if(line==='[Table row]'){row=[];rows.push(row);expect=false;continue;}
   if(/^\[Column \d+\]$/.test(line)){expect=!!row;continue;}
   if(expect&&row){row.push(line);expect=false;}
  }
  if(rows.length<2||rows.some(r=>r.length<2))return null;
  const [header,...body]=rows;
  return {header:header.map(display),rows:body.map(cells=>({category:display(cells[0]),columns:cells.slice(1).map(picks)}))};
 }
 function doctrines(text){
  const lines=String(text||'').split(/\r?\n/).map(clean).filter(Boolean);
  const first=lines.findIndex(l=>/^PART\s+[IVX]+\b/i.test(l));
  if(first<0)return null;
  const intro=[],links=[],groups=[];
  let part=null,group=null,item=null,mode=null,table=[];
  const startGroup=title=>{group={title,part,items:[]};groups.push(group);item=null;mode=null;};
  lines.forEach((line,i)=>{
   if(URL_LINE.test(line)){links.push(line);return;}
   if(NOISE.test(line))return;
   if(i<first){if(i>0&&!/•/.test(line))intro.push(line.replace(/^READING THE GUIDE\s*[—-]\s*/i,''));return;}
   const heading=line.match(/^PART\s+[IVX]+\s*[—:-]\s*(.+)$/i);
   if(heading){part=display(heading[1]);startGroup(part);return;}
   if(/^\[(?:Table row|Column \d+)\]$/.test(line)||table.length&&/^\[Column \d+\]$/.test(lines[i-1]||'')){table.push(line);return;}
   const r=rated(line);
   if(r){if(!group)startGroup(part||'');item={...r,buffs:[],spirits:[],comment:'',notes:[]};group.items.push(item);mode=null;return;}
   if(/^notable buffs:?$/i.test(line)){mode='buffs';return;}
   if(/^spirits:?$/i.test(line)){mode='spirits';return;}
   const comment=line.match(/^comment:\s*(.*)$/i);
   if(comment&&item){item.comment=comment[1];mode='comment';return;}
   // An upper-case line outside an item's lists is a section ("INFANTRY").
   if(!/[a-z]/.test(line)&&/[A-Z]{3}/.test(line)&&(mode===null||mode==='comment')){startGroup(display(line));return;}
   if(!item)return;
   if(mode==='buffs')item.buffs.push(line);
   else if(mode==='spirits')item.spirits.push(line.split(/\s*,\s*/).map(display).filter(Boolean));
   else if(mode==='comment')item.comment+=(item.comment?' ':'')+line;
   else item.notes.push(line);
  });
  const kept=groups.filter(g=>g.items.length);
  if(kept.reduce((n,g)=>n+g.items.length,0)<3)return null;
  for(const g of kept){const best=Math.max(...g.items.map(x=>x.score));if(g.items.length>1)for(const x of g.items)x.best=x.score===best;}
  return {kind:'doctrines',title:display(lines[0]),intro,quick:quickTable(table),groups:kept,links};
 }
 // "Name (Qualifier) – text": the name, its qualifiers and the author's text.
 function entry(line){
  const m=line.match(/^(.+?)\s+[–—]\s+(.+)$/)||line.match(/^(.+?)\s+-\s+(.+)$/);
  if(!m)return null;
  const tags=[];
  const name=clean(m[1].replace(/\(([^()]+)\)/g,(all,tag)=>{tags.push(clean(tag));return ' ';}));
  return name?{name,tags,text:m[2],notes:[]}:null;
 }
 function spirits(text){
  const lines=String(text||'').split(/\r?\n/).map(clean).filter(Boolean);
  if(lines.filter(l=>/^spirit of\b/i.test(l)).length<2)return null;
  const branches=[],takeaways=[],links=[];
  let branch=null,slot=null,last=null,summary=false;
  for(const line of lines){
   if(URL_LINE.test(line)){links.push(line);continue;}
   if(NOISE.test(line))continue;
   const best=line.match(/^(\P{L}*)\s*(best\b.*\bspirits?)$/iu);
   if(best){branch={title:best[2],icon:best[1].trim(),slots:[]};branches.push(branch);slot=null;last=null;summary=false;continue;}
   if(/^key takeaways?$/i.test(line)){summary=true;continue;}
   if(summary){const m=line.match(/^([^:]{1,40}):\s*(.+)$/);if(m)takeaways.push({label:m[1],text:m[2]});continue;}
   if(/^spirit of\b/i.test(line)){if(!branch){branch={title:'',icon:'',slots:[]};branches.push(branch);}slot={title:line,entries:[]};branch.slots.push(slot);last=null;continue;}
   if(!slot)continue;
   if(/^why not\b/i.test(line)&&last){last.notes.push(line);continue;}
   const e=entry(line);
   if(e){slot.entries.push(e);last=e;}
   else if(last)last.notes.push(line);
  }
  if(branches.reduce((n,b)=>n+b.slots.reduce((k,s)=>k+s.entries.length,0),0)<3)return null;
  return {kind:'spirits',branches:branches.filter(b=>b.slots.length),takeaways,links};
 }
 function read(doc){
  if(!doc||doc.category!=='doctrines'||typeof doc.text!=='string')return null;
  return doctrines(doc.text)||spirits(doc.text);
 }
 const api={read,display,rated};
 if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiDoctrineGuide=api;
})(typeof window!=='undefined'?window:globalThis);
