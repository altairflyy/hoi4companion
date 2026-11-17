'use strict';
// Events page model: filtering and wording only. The list comes from the
// backend (/api/events, installed game scripts); nothing here decides who
// receives an event.
(function(root,factory){const api=factory();if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiEventsModel=api;})(typeof globalThis==='object'?globalThis:this,()=>{
 const ORIGINS=['focus','decision','event'];
 const norm=s=>String(s||'').toLocaleLowerCase('it').normalize('NFD').replace(/[̀-ͯ]/g,'');
 // Origin kinds of one event: what fires it, or 'none' when nothing traced does.
 function origins(event){const kinds=[...new Set((event.triggered_by||[]).map(s=>s.kind).filter(k=>ORIGINS.includes(k)))];return kinds.length?kinds:['none'];}
 // Script placeholders read as words: ROOT is the played nation, FROM the
 // nation that sent the event; anything else is left out.
 function readable(text,{country='',other='the other nation'}={}){
  // An article the game writes before a name ("la [FROM.GetName]") is dropped.
  const article=/(?:\b(?:la|il|lo|le|gli|i|the)\s+|\bl['’])(?=\[(?:ROOT|THIS|FROM)\.)/gi;
  return String(text||'').replace(article,'').replace(/\[(?:ROOT|THIS)\.[A-Za-z]+\]/g,country||'…').replace(/\[FROM\.[A-Za-z]+\]/g,other).replace(/\[[^\]]*\]/g,'…').replace(/\s+/g,' ').trim()
   .replace(/^./,c=>c.toLocaleUpperCase());
 }
 // The nation that sends an event, when every traced trigger belongs to one
 // other nation (its focus or decision ids start with its tag).
 function sender(event,country){
  const tags=[...new Set((event.triggered_by||[]).filter(s=>s.kind!=='event').map(s=>/^([A-Z][A-Z0-9]{2})_/.exec(s.id||'')?.[1]).filter(Boolean))];
  return tags.length===1&&tags[0]!==country?tags[0]:null;
 }
 function matches(event,query){
  const q=norm(query).trim();if(!q)return true;
  return [event.title,event.id,event.description,...(event.options||[]).map(o=>o.name),...(event.triggered_by||[]).map(s=>s.name)].some(text=>norm(text).includes(q));
 }
 // Events shown for the current filters, in the backend's (alphabetical) order.
 function visible(data,{query='',origin='all',possible=false}={}){
  const list=[...(data?.events||[]),...(possible?(data?.possible||[]).map(e=>({...e,possible:true})):[])];
  return list.filter(event=>(origin==='all'||origins(event).includes(origin))&&matches(event,query));
 }
 function counts(data){
  const result={all:(data?.events||[]).length,possible:(data?.possible||[]).length};
  for(const kind of [...ORIGINS,'none'])result[kind]=(data?.events||[]).filter(e=>origins(e).includes(kind)).length;
  return result;
 }
 return {origins,matches,visible,counts,readable,sender,ORIGINS};
});
