'use strict';
(()=>{
 const $=id=>document.getElementById(id),model=window.HoiCountryModel;
 const countFormat=new Intl.NumberFormat(HoiI18n.language==='italian'?'it-IT':'en-US',{maximumFractionDigits:0});
 const decimalFormat=new Intl.NumberFormat(HoiI18n.language==='italian'?'it-IT':'en-US',{maximumFractionDigits:2});
 const signedFormat=new Intl.NumberFormat(HoiI18n.language==='italian'?'it-IT':'en-US',{maximumFractionDigits:2,signDisplay:'exceptZero'});
 const productionFormats={2:new Intl.NumberFormat(HoiI18n.language==='italian'?'it-IT':'en-US',{minimumFractionDigits:2,maximumFractionDigits:2}),4:new Intl.NumberFormat(HoiI18n.language==='italian'?'it-IT':'en-US',{minimumFractionDigits:4,maximumFractionDigits:4})};
 let view=model.evaluate(null);
 const list=x=>Array.isArray(x)?x.filter(y=>y&&typeof y==='object'&&!Array.isArray(y)):[];
 function text(value,fallback='—'){return typeof value==='string'&&value.trim()?value:fallback;}
 function number(value,options={}){const n=model.number(value,options);return n===null?'—':options.signed?signedFormat.format(n):options.integer?countFormat.format(n):decimalFormat.format(n);}
 function cell(row,value,{numeric=false,integer=false,signed=false}={}){
  const td=document.createElement('td');
  td.textContent=numeric?number(value,{integer,signed}):text(value);
  if(td.textContent==='—')td.className='unknown';
  else if(signed&&value!==0)td.className=value<0?'negative':'positive';
  row.append(td);return td;
 }
 function nameCell(row,item,fallback){
  const td=cell(row,item.name||fallback||item.id);
  if(item.id!==undefined&&item.id!==null&&String(item.id)!==td.textContent){const id=document.createElement('span');id.className='identifier';id.textContent=String(item.id);td.append(id);}
 }
 function header(key){
  const s=view.sections[key],el=$(key+'Status');el.className='status '+s.status;
  el.textContent={observed:HoiI18n.t("Observed"),partial:HoiI18n.t("Partial"),unavailable:HoiI18n.t("Unavailable")}[s.status];
  $(key+'Reason').textContent=text(s.reason,'');return s;
 }
 function rows(target,items,draw){const tbody=$(target);tbody.replaceChildren();for(const item of list(items)){const row=document.createElement('tr');draw(row,item);tbody.append(row);}return tbody.children.length;}
 function empty(key,count,sourceCount=count){
  const s=view.sections[key];
  $(key+'Empty').textContent=count?'':s.status==='unavailable'?HoiI18n.t("Waiting for current data for this section."):sourceCount?HoiI18n.t("No entries match your search."):s.status==='partial'?HoiI18n.t("No readable entries in this sample."):HoiI18n.t("No entries in the observed sample.");
 }
 function renderStates(){
  const s=view.sections.states,all=list(s.items),query=$('stateSearch').value.trim().toLocaleLowerCase('it');
  const items=query?all.filter(item=>(text(item.name,'')+' '+String(item.id??'')).toLocaleLowerCase('it').includes(query)):all;
  const n=rows('stateRows',items,(r,x)=>{nameCell(r,x);cell(r,typeof x.owned==='boolean'&&typeof x.controlled==='boolean'?(x.owned&&x.controlled?HoiI18n.t("Owned and controlled"):x.owned?HoiI18n.t("Owned"):x.controlled?HoiI18n.t("Controlled"):null):null);cell(r,x.civilian_factories,{numeric:true,integer:true});cell(r,x.military_factories,{numeric:true,integer:true});cell(r,x.dockyards,{numeric:true,integer:true});cell(r,x.infrastructure,{numeric:true,integer:true});cell(r,x.buildings&&typeof x.buildings==='object'?Object.entries(x.buildings).filter(([key])=>!['infrastructure','arms_factory','industrial_complex','dockyard'].includes(key)).map(([key,level])=>key.replaceAll('_',' ')+': '+number(level,{integer:true})).join(' · '):null);});empty('states',n,all.length);
 }
 function render(value){
  view=model.evaluate(value,window.hoiLive?.enabled!==false);
  document.body.classList.toggle('country-current',view.mode==='current');
  $('countryTag').textContent=view.country||'—';
  const mode={current:[HoiI18n.t("Current game sample"),HoiI18n.t("Each section reports its coverage separately. Unobserved fields remain empty.")],unavailable:[HoiI18n.t("Country connected · extended data unavailable"),HoiI18n.t("The focus and research reader is connected; country data is not available yet or does not match the observed country.")],disabled:[HoiI18n.t("Reading disabled"),HoiI18n.t("Enable game reading to see current values.")],stale:[HoiI18n.t("State is not current · values hidden"),HoiI18n.t("The connection no longer confirms a usable sample. Previous data is not presented as current.")],disconnected:[HoiI18n.t("Game not connected"),HoiI18n.t("Load a game and keep the reader connected. Statistics are never estimated.")]}[view.mode];
  $('dataStatus').textContent=mode[0];$('dataNote').textContent=mode[1];
  const date=typeof view.observed_at==='string'?new Date(view.observed_at):null;
  $('observationTime').hidden=!date||!Number.isFinite(date.getTime());
  $('observationTime').textContent=date&&Number.isFinite(date.getTime())?HoiI18n.t("Sample: ")+date.toLocaleString('it-IT'):'';
  const divisions=header('divisions');$('divisionTotal').textContent=number(divisions.total,{integer:true});
  empty('divisions',rows('divisionRows',divisions.by_type,(r,x)=>{cell(r,x.name||x.type);cell(r,x.count,{numeric:true,integer:true});}));
  $('templateDetails').hidden=!rows('templateRows',divisions.by_template,(r,x)=>{nameCell(r,x);cell(r,x.type);cell(r,x.count,{numeric:true,integer:true});});
  const manpower=header('manpower');for(const field of ['available','fielded','training'])$('manpower'+field[0].toUpperCase()+field.slice(1)).textContent=number(manpower[field],{integer:true});
  const experience=header('experience');for(const branch of ['army','navy','air'])$('experience'+branch[0].toUpperCase()+branch.slice(1)).textContent=number(experience[branch]);
  const diplomacy=header('diplomacy');$('diplomacyWar').textContent=typeof diplomacy.at_war==='boolean'?HoiI18n.t(diplomacy.at_war?'At war':'At peace'):'—';
  const relations=diplomacy.interactions?.relations;
  $('foreignCoverage').textContent=relations?.status==='observed'?'':HoiI18n.t('Relation details unavailable.');
  rows('foreignRows',relations?.status==='observed'?relations.items:[],(r,x)=>{cell(r,x.kind.replaceAll('_',' '));cell(r,HoiI18n.t(x.direction==='outgoing'?'Outgoing':'Incoming'));cell(r,x.direction==='outgoing'?x.to_country:x.from_country);});
  header('states');renderStates();
  const production=header('production');
  for(const key of ['civilian','military','dockyards'])$('effective'+key[0].toUpperCase()+key.slice(1)).textContent=number(production.effective_factories?.[key],{integer:true});
  empty('production',rows('productionRows',production.lines,(r,x)=>{nameCell(r,x);cell(r,({military:HoiI18n.t("Military"),naval:HoiI18n.t("Naval")})[x.category]||x.category);nameCell(r,{id:x.equipment,name:x.equipment_name});cell(r,x.requested_factories,{numeric:true,integer:true});cell(r,x.assigned_factories,{numeric:true,integer:true});cell(r,x.active_factories,{numeric:true,integer:true});cell(r,x.damaged_factories,{numeric:true,integer:true});const output=model.productionOutput(x.output_per_day,x.category);cell(r,output?productionFormats[output.digits].format(output.value):null);}));
  const equipment=header('equipment'),logistics=equipment.logistics;
  empty('equipment',rows('equipmentRows',equipment.items,(r,x)=>{nameCell(r,x);cell(r,x.stockpile,{numeric:true});}));
  $('logisticsCoverage').textContent=['observed','partial'].includes(logistics?.status)?HoiI18n.t('Native logistics formula · {0} day window. Negative availability indicates a shortage.',logistics.interval_days):HoiI18n.t('National logistics formula not qualified');
  rows('logisticsRows',['observed','partial'].includes(logistics?.status)?logistics.items:[],(r,x)=>{nameCell(r,x);cell(r,x.gross_stockpile,{numeric:true});cell(r,x.needed,{numeric:true,signed:true});cell(r,x.available,{numeric:true,signed:true});cell(r,x.balance_per_day,{numeric:true,signed:true});r.title=HoiI18n.t('Available stock after native supply reservations: {0}. Net availability subtracts equipment demand.',number(x.logistics_stockpile))+ (x.reason?' '+x.reason:'');});
  const politics=header('politics'),ideas=politics.ideas,advisors=politics.advisors;
  const characters=politics.characters;
  $('characterCoverage').textContent=characters?.status==='observed'?HoiI18n.t('Current native role status; appointments are listed separately.'):HoiI18n.t('Character roles unavailable.');
  rows('characterRows',characters?.status==='observed'?characters.items:[],(r,x)=>{nameCell(r,x);cell(r,[['country_leader','Country leader'],['advisor','Advisor'],['unit_leader','Unit leader'],['scientist','Scientist']].filter(([k])=>x[k]===true).map(([,name])=>HoiI18n.t(name)).join(', '));});
  $('politicsIdeasCoverage').textContent=ideas?.status==='observed'?HoiI18n.t("Complete membership in the current sample."):HoiI18n.t("Ideas and laws unavailable.");
  $('politicsAdvisorsCoverage').textContent=advisors?.status==='observed'?(advisors.items.length?HoiI18n.t("Currently active appointments."):HoiI18n.t("No active appointments observed.")):HoiI18n.t("Appointments unavailable.");
  rows('politicsIdeaRows',ideas?.status==='observed'?ideas.items:[],(r,x)=>{nameCell(r,x);cell(r,x.group_name||x.group);cell(r,x.is_law?HoiI18n.t("Law"):'Idea');});
  rows('politicsAdvisorRows',advisors?.status==='observed'?advisors.items:[],(r,x)=>{nameCell(r,x);cell(r,x.character_name??x.character_id);});
  const events=header('events');$('visibleEvents').replaceChildren();
  for(const event of list(events.visible)){const article=document.createElement('article'),title=document.createElement('h3');title.textContent=event.name||event.event_id;article.append(title);const id=document.createElement('p');id.textContent=event.event_id;article.append(id);for(const option of list(event.options)){const p=document.createElement('p');p.textContent=(option.name||option.id)+' · '+option.id;article.append(p);}$('visibleEvents').append(article);}empty('events',list(events.visible).length);
  const decisions=header('decisions'),history=decisions.execution_history;
  $('decisionMembership').textContent=history?.status==='observed'?HoiI18n.t('Native completed/taken records: {0}. Active timed decisions: {1}.',history.taken_ids.length,history.active_ids.length):Array.isArray(decisions.listed_ids)?decisions.listed_ids.length+HoiI18n.t(" entries in the native manager."):HoiI18n.t("List unavailable.");
  const resources=header('resources');empty('resources',rows('resourceRows',resources.items,(r,x)=>{nameCell(r,x);cell(r,x.produced,{numeric:true});cell(r,x.imported,{numeric:true});cell(r,x.exported,{numeric:true});cell(r,x.available,{numeric:true,signed:true});cell(r,x.used,{numeric:true});cell(r,x.balance,{numeric:true,signed:true});}));
 }
 $('stateSearch').addEventListener('input',renderStates);
 render(null);window.hoiLive?.subscribe(render);
})();
