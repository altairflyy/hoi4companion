'use strict';
// Events the played nation can receive (installed game scripts), searchable.
(()=>{
 const t=HoiI18n.t,model=HoiEventsModel,$=id=>document.getElementById(id);
 let data=null,country='',loading=null;
 const el=(tag,cls,text)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
 for(const node of document.querySelectorAll('[data-copy]'))node.textContent=t(node.dataset.copy);
 $('eventSearch').placeholder=t('Search title, option or what triggers it');
 const originLabel={focus:'National focus',decision:'Decision',event:'Event'};
 let current=null;
 const read=text=>model.readable(text,{country,other:model.sender(current||{},country)||t('the other nation')});
 function card(event){
  current=event;
  const item=el('details','event-card'+(event.possible?' event-possible':''));
  const summary=el('summary');summary.append(el('span','event-title',read(event.title)||event.id));
  const tags=el('span','event-tags');
  const reach=model.scope(event,country),others=(event.receivers||[]).filter(tag=>tag!==country);
  if(reach==='own')tags.append(el('span','event-tag event-tag-own',t('Only {0}',country)));
  else if(reach==='shared')tags.append(el('span','event-tag event-tag-shared',t('Shared with {0} nations',others.length)));
  const from=model.sender(event,country);
  if(from)tags.append(el('span','event-tag event-tag-sender',t('Sent by {0}',from)));
  else if(model.fromOwn(event,country))tags.append(el('span','event-tag event-tag-own',t('From your focuses or decisions')));
  for(const kind of model.origins(event))if(kind!=='none')tags.append(el('span','event-tag event-tag-'+kind,t(originLabel[kind])));
  tags.append(el('span','event-tag',event.triggered_only?t('Fires only when called'):event.timed?t('Can fire on its own'):t('Timing not stated')));
  if(event.possible)tags.append(el('span','event-tag event-tag-possible',t('Possible')));
  summary.append(tags);item.append(summary);
  if(event.description)item.append(el('p','event-description',read(event.description)));
  if(reach==='shared'){
   const p=el('p','event-sources');
   p.append(el('strong','',t('Also received by: ')),others.slice(0,30).join(', ')+(others.length>30?' '+t('+{0} more',others.length-30):''));
   item.append(p);
  }
  const sources=event.triggered_by||[];
  if(sources.length){const p=el('p','event-sources');p.append(el('strong','',t('Triggered by: ')),sources.map(s=>`${t(originLabel[s.kind]||'Source')} · ${s.name}`).join(' · '));item.append(p);}
  else item.append(el('p','event-sources',event.possible?t('Its callers cannot be traced in the scripts; its trigger names this nation.'):t('No focus, decision or event of the scripts calls it directly (it may come from on_actions).')));
  if(event.options?.length){
   const list=el('ul','event-options');
   for(const option of event.options){const li=el('li','',read(option.name)||option.id);if(Number.isFinite(option.political_power_cost))li.append(el('span','event-cost',' · '+t('{0} political power',option.political_power_cost)));list.append(li);}
   item.append(el('p','event-options-title',t('Answers')),list);
  }
  item.append(el('p','event-id',event.id));
  return item;
 }
 function render(){
  $('eventsCountry').textContent=country||'—';
  const list=$('eventsList');list.replaceChildren();
  if(!data){$('eventsStatus').textContent=t('Waiting for the played nation');return;}
  const counts=model.counts(data);
  for(const option of $('eventsOrigin').options)option.textContent=t(option.dataset.copy)+` (${option.value==='all'?counts.all:counts[option.value]??0})`;
  for(const option of $('eventsReach').options)option.textContent=t(option.dataset.copy,country)+` (${counts[option.value]??0})`;
  $('eventsPossibleLabel').textContent=t('Also show possible events')+` (${counts.possible})`;
  const shown=model.visible(data,{query:$('eventSearch').value,origin:$('eventsOrigin').value,reach:$('eventsReach').value,possible:$('eventsPossible').checked});
  $('eventsStatus').textContent=data.reason==='country_unknown'?t('Waiting for the played nation'):t('{0} events shown',shown.length);
  for(const event of shown.slice(0,400))list.append(card(event));
  if(shown.length>400)list.append(el('p','event-more',t('Refine the search to see the other {0}',shown.length-400)));
 }
 async function load(tag){
  if(loading)return loading;
  loading=(async()=>{try{
   const url='/api/events'+(tag?'?country='+encodeURIComponent(tag):'');
   const response=await fetch(url),value=await response.json();if(!response.ok)throw Error(value.error||'HTTP '+response.status);
   data=value;country=value.country||tag||'';
  }catch(error){$('eventsStatus').textContent=t('Events unavailable: ')+error.message;}finally{loading=null;render();}})();
  return loading;
 }
 for(const id of ['eventSearch','eventsOrigin','eventsReach','eventsPossible'])$(id).addEventListener(id==='eventSearch'?'input':'change',render);
 // Follow the played nation: reload when the live country changes.
 if(window.hoiLive?.subscribe)window.hoiLive.subscribe(value=>{const tag=value?.sample?.country;if(value?.usable===true&&typeof tag==='string'&&tag!==country)load(tag);});
 load('');
})();
