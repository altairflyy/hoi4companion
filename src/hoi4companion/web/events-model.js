'use strict';
// Events page model: filtering and wording only. The list comes from the
// backend (/api/events, installed game scripts); nothing here decides who
// receives an event.
(function(root,factory){const api=factory();if(typeof module==='object'&&module.exports)module.exports=api;else root.HoiEventsModel=api;})(typeof globalThis==='object'?globalThis:this,()=>{
 const ORIGINS=['focus','decision','event'];
 const norm=s=>String(s||'').toLocaleLowerCase('it').normalize('NFD').replace(/[̀-ͯ]/g,'');
 // Origin kinds of one event: what fires it, or 'none' when nothing traced does.
 function origins(event){const kinds=[...new Set((event.triggered_by||[]).map(s=>s.kind).filter(k=>ORIGINS.includes(k)))];return kinds.length?kinds:['none'];}
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
 return {origins,matches,visible,counts,ORIGINS};
});
