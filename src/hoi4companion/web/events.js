'use strict';
// Events the played nation can receive (installed game scripts), searchable.
(()=>{
 const t=HoiI18n.t,model=HoiEventsModel,$=id=>document.getElementById(id);
 let data=null,country='',requestTag='',packet=null,loadError='';
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
  if(guideFocuses.length)tags.append(el('span','event-tag event-tag-guide',t('Guide focus: {0}',model.focusName(guideFocuses[0]))+(guideFocuses.length>1?' '+t('+{0} more',guideFocuses.length-1):'')));
  const branch=event.branch_focuses||[];
  if(model.guideActive(data)&&event.guide_reach==='other_branch'&&branch.length)tags.append(el('span','event-tag event-tag-branch',t('Branch outside the guide: {0}',model.focusName(branch[0]))));
  const historical=event.historical_focuses||[];
  if(event.guide_reach==='historical'&&historical.length)tags.append(el('span','event-tag event-tag-historical',t('Historical: {0}',model.focusName(historical[0]))));
  if(model.guideActive(data)&&event.guide_reach==='anyway')tags.append(el('span','event-tag',t('Comes without focuses')));
  const from=model.sender(event,country);
  if(from)tags.append(el('span','event-tag event-tag-sender',t('Sent by {0}',from)));
  for(const kind of model.origins(event))if(kind!=='none')tags.append(el('span','event-tag event-tag-'+kind,t(originLabel[kind])));
  tags.append(el('span','event-tag',event.triggered_only?t('Fires only when called'):event.timed?t('Can fire on its own'):t('Timing not stated')));
  if(event.possible)tags.append(el('span','event-tag event-tag-possible',t('Possible')));
  summary.append(tags);item.append(summary);
  if(event.description)item.append(el('p','event-description',read(event.description)));
  if(guideFocuses.length>1){const p=el('p','event-sources');p.append(el('strong','',t('Guide focuses that lead to it: ')),guideFocuses.map(model.focusName).join(' · '));item.append(p);}
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
  $('eventsCampaign').textContent=packet?.usable!==true&&packet?.reader_status==='game_absent'?HoiCampaignStatus.present(packet,null,t).message:'';
  const list=$('eventsList');list.replaceChildren();
  const available=Array.isArray(data?.events)&&data.reason!=='country_unknown';
  const scope=$('eventsScope').value,guide=model.guideActive(data);
  const guideOption=$('eventsScope').querySelector('[value=guide]');guideOption.disabled=!guide;
  if(!guide&&scope==='guide')$('eventsScope').value='nation';
  const counts=available?model.counts(data,{scope:$('eventsScope').value}):null;
  $('eventsGuide').textContent=!available?'':!data.guide?.title?t('No active guide: all events of the nation are shown.')
   :data.guide.applies===false?t('The active guide is for {0}: all events of the nation are shown.',data.guide.country)
   :t('Guide: {0}',data.guide.title);
  for(const option of $('eventsOrigin').options)option.textContent=t(option.dataset.copy)+` (${counts?.[option.value]??'—'})`;
  for(const option of $('eventsScope').options){
   const label=option.value==='nation'&&!country?t('All national events'):t(option.dataset.copy,country);
   option.textContent=label+(option.disabled?'':` (${counts?.[option.value]??'—'})`);
  }
  $('eventsPossible').disabled=$('eventsScope').value==='guide';
  $('eventsPossibleLabel').textContent=t('Also show possible events')+` (${counts?.possible??'—'})`;
  if(!available){$('eventsStatus').textContent=loadError||t('Waiting for the played nation');return;}
  const shown=model.visible(data,{query:$('eventSearch').value,origin:$('eventsOrigin').value,scope:$('eventsScope').value,possible:$('eventsPossible').checked});
  let status=t('{0} events shown',shown.length);
  if(guide&&$('eventsScope').value==='guide')
   status+=' · '+t('{0} from your guide focuses at the top',counts.fromGuide)+' · '+t('{0} hidden: only focuses outside the guide lead to them',counts.otherBranch);
  if(counts.historical)status+=' · '+t('{0} from other nations’ historical focuses',counts.historical);
  if(counts.foreign)status+=' · '+t('{0} hidden: other nations’ focuses or decisions send them',counts.foreign);
  $('eventsStatus').textContent=status;
  for(const event of shown.slice(0,400))list.append(card(event));
  if(shown.length>400)list.append(el('p','event-more',t('Refine the search to see the other {0}',shown.length-400)));
 }
 let generation=0;
 async function update(value){
  packet=value;
  const tag=value?.usable===true?value.sample?.country||'':'';
  if(tag===requestTag&&(!tag||!loadError)){render();return;}
  const current=++generation;requestTag=tag;country=tag;data=null;loadError='';render();
  if(!tag)return;
  try{
   const response=await fetch('/api/events?country='+encodeURIComponent(tag)),result=await response.json();
   if(!response.ok)throw Error(result.error||'HTTP '+response.status);
   if(current===generation&&result.country===tag)data=result;
  }catch(error){
   if(current===generation)loadError=t('Events unavailable: ')+error.message;
  }
  if(current===generation)render();
 }
 for(const id of ['eventSearch','eventsOrigin','eventsScope','eventsPossible'])$(id).addEventListener(id==='eventSearch'?'input':'change',render);
 // Follow the played nation: reload when the live country changes.
 render();window.hoiLive?.subscribe(update);
})();
