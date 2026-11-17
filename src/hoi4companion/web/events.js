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
  const guideFocuses=event.guide_focuses||[];
  if(guideFocuses.length)tags.append(el('span','event-tag event-tag-guide',t('Guide focus: {0}',guideFocuses[0].name)+(guideFocuses.length>1?' '+t('+{0} more',guideFocuses.length-1):'')));
  const branch=event.branch_focuses||[];
  if(event.guide_reach==='other_branch'&&branch.length)tags.append(el('span','event-tag event-tag-branch',t('Branch outside the guide: {0}',branch[0].name)));
  const from=model.sender(event,country);
  if(from)tags.append(el('span','event-tag event-tag-sender',t('Sent by {0}',from)));
  for(const kind of model.origins(event))if(kind!=='none')tags.append(el('span','event-tag event-tag-'+kind,t(originLabel[kind])));
  tags.append(el('span','event-tag',event.triggered_only?t('Fires only when called'):event.timed?t('Can fire on its own'):t('Timing not stated')));
  if(event.possible)tags.append(el('span','event-tag event-tag-possible',t('Possible')));
  summary.append(tags);item.append(summary);
  if(event.description)item.append(el('p','event-description',read(event.description)));
  if(guideFocuses.length>1){const p=el('p','event-sources');p.append(el('strong','',t('Guide focuses that lead to it: ')),guideFocuses.map(s=>s.name).join(' · '));item.append(p);}
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
  const scope=$('eventsScope').value,guide=model.guideActive(data);
  const guideOption=$('eventsScope').querySelector('[value=guide]');guideOption.disabled=!guide;
  if(!guide&&scope==='guide')$('eventsScope').value='nation';
  const counts=model.counts(data,{scope:$('eventsScope').value});
  $('eventsGuide').textContent=!data.guide?.title?t('No active guide: all events of the nation are shown.')
   :data.guide.applies===false?t('The active guide is for {0}: all events of the nation are shown.',data.guide.country)
   :t('Guide: {0}',data.guide.title);
  for(const option of $('eventsOrigin').options)option.textContent=t(option.dataset.copy)+` (${option.value==='all'?counts.all:counts[option.value]??0})`;
  for(const option of $('eventsScope').options)option.textContent=t(option.dataset.copy,country)+` (${counts[option.value]??0})`;
  $('eventsPossible').disabled=$('eventsScope').value==='guide';
  $('eventsPossibleLabel').textContent=t('Also show possible events')+` (${counts.possible})`;
  const shown=model.visible(data,{query:$('eventSearch').value,origin:$('eventsOrigin').value,scope:$('eventsScope').value,possible:$('eventsPossible').checked});
  let status=data.reason==='country_unknown'?t('Waiting for the played nation'):t('{0} events shown',shown.length);
  if(guide&&$('eventsScope').value==='guide'&&data.reason!=='country_unknown')
   status+=' · '+t('{0} from your guide focuses at the top',counts.fromGuide)+' · '+t('{0} hidden: only focuses outside the guide lead to them',counts.otherBranch);
  $('eventsStatus').textContent=status;
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
 for(const id of ['eventSearch','eventsOrigin','eventsScope','eventsPossible'])$(id).addEventListener(id==='eventSearch'?'input':'change',render);
 // Follow the played nation: reload when the live country changes.
 if(window.hoiLive?.subscribe)window.hoiLive.subscribe(value=>{const tag=value?.sample?.country;if(value?.usable===true&&typeof tag==='string'&&tag!==country)load(tag);});
 load('');
})();
