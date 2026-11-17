'use strict';
// Events page model: filtering and wording only. The list comes from the
// backend (/api/events, installed game scripts); nothing here decides who
// receives an event or which guide focuses lead to it.
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
 // True when a focus of the active guide leads to the event (backend-traced).
 const fromGuide=event=>(event.guide_focuses||[]).length>0;
 // The guide can meet the event: a guide focus leads to it, or it comes
 // anyway. Hidden only when just this nation's focuses outside the guide do.
 const withGuide=event=>event.guide_reach!=='other_branch';
 // The guide view applies only when a guide with focuses is active.
 const guideActive=data=>(data?.guide_focus_count||0)>0;
 function matches(event,query){
  const q=norm(query).trim();if(!q)return true;
  return [event.title,event.id,event.description,...(event.options||[]).map(o=>o.name),...(event.triggered_by||[]).map(s=>s.name)].some(text=>norm(text).includes(q));
 }
 // Events shown for the current filters. The guide view drops the events of
 // branches the guide does not take and lists first those its focuses lead
 // to, in guide order; the full view keeps the backend's (alphabetical) order.
 function visible(data,{query='',origin='all',possible=false,scope='guide'}={}){
  const guide=scope==='guide'&&guideActive(data);
  let list=[...(data?.events||[]),...(!guide&&possible?(data?.possible||[]).map(e=>({...e,possible:true})):[])];
  if(guide)list=list.filter(withGuide).map((e,i)=>[e,i]).sort((a,b)=>(a[0].guide_order??1e9)-(b[0].guide_order??1e9)||a[1]-b[1]).map(([e])=>e);
  return list.filter(event=>(origin==='all'||origins(event).includes(origin))&&matches(event,query));
 }
 function counts(data,{scope='guide'}={}){
  const all=data?.events||[],events=scope==='guide'&&guideActive(data)?all.filter(withGuide):all;
  const result={all:events.length,possible:(data?.possible||[]).length,guide:all.filter(withGuide).length,nation:all.length,
   fromGuide:all.filter(fromGuide).length,otherBranch:all.length-all.filter(withGuide).length};
  for(const kind of [...ORIGINS,'none'])result[kind]=events.filter(e=>origins(e).includes(kind)).length;
  return result;
 }
 return {origins,matches,visible,counts,readable,sender,fromGuide,withGuide,guideActive,ORIGINS};
});
