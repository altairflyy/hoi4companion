'use strict';
(()=>{
 const $=id=>document.getElementById(id),model=window.HoiCountryModel;
 const countFormat=new Intl.NumberFormat('it-IT',{maximumFractionDigits:0});
 const decimalFormat=new Intl.NumberFormat('it-IT',{maximumFractionDigits:2});
 const signedFormat=new Intl.NumberFormat('it-IT',{maximumFractionDigits:2,signDisplay:'exceptZero'});
 const productionFormats={2:new Intl.NumberFormat('it-IT',{minimumFractionDigits:2,maximumFractionDigits:2}),4:new Intl.NumberFormat('it-IT',{minimumFractionDigits:4,maximumFractionDigits:4})};
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
  el.textContent={observed:'Rilevato',partial:'Parziale',unavailable:'Non disponibile'}[s.status];
  $(key+'Reason').textContent=text(s.reason,'');return s;
 }
 function rows(target,items,draw){const tbody=$(target);tbody.replaceChildren();for(const item of list(items)){const row=document.createElement('tr');draw(row,item);tbody.append(row);}return tbody.children.length;}
 function empty(key,count,sourceCount=count){
  const s=view.sections[key];
  $(key+'Empty').textContent=count?'':s.status==='unavailable'?'In attesa di dati correnti per questa sezione.':sourceCount?'Nessuna voce corrisponde alla ricerca.':s.status==='partial'?'Nessuna voce leggibile in questo campione.':'Nessuna voce nel campione rilevato.';
 }
 function renderStates(){
  const s=view.sections.states,all=list(s.items),query=$('stateSearch').value.trim().toLocaleLowerCase('it');
  const items=query?all.filter(item=>(text(item.name,'')+' '+String(item.id??'')).toLocaleLowerCase('it').includes(query)):all;
  const n=rows('stateRows',items,(r,x)=>{nameCell(r,x);cell(r,typeof x.owned==='boolean'&&typeof x.controlled==='boolean'?(x.owned&&x.controlled?'Posseduto e controllato':x.owned?'Posseduto':x.controlled?'Controllato':null):null);cell(r,x.civilian_factories,{numeric:true,integer:true});cell(r,x.military_factories,{numeric:true,integer:true});cell(r,x.dockyards,{numeric:true,integer:true});cell(r,x.infrastructure,{numeric:true,integer:true});});empty('states',n,all.length);
 }
 function render(value){
  view=model.evaluate(value,window.hoiLive?.enabled!==false);
  document.body.classList.toggle('country-current',view.mode==='current');
  $('countryTag').textContent=view.country||'—';
  const mode={current:['Campione corrente del gioco','Ogni sezione indica separatamente la propria copertura. I campi non rilevati restano vuoti.'],unavailable:['Nazione collegata · dati estesi non disponibili','Il lettore di focus e ricerche è collegato; i dati della nazione non sono ancora disponibili o non corrispondono al paese rilevato.'],disabled:['Lettura disattivata','Riattiva la lettura del gioco per vedere valori correnti.'],stale:['Stato non corrente · valori nascosti','La connessione non conferma più un campione utilizzabile. I dati precedenti non sono presentati come attuali.'],disconnected:['Gioco non collegato','Apri una partita e lascia il lettore collegato. Nessuna statistica viene stimata.']}[view.mode];
  $('dataStatus').textContent=mode[0];$('dataNote').textContent=mode[1];
  const date=typeof view.observed_at==='string'?new Date(view.observed_at):null;
  $('observationTime').hidden=!date||!Number.isFinite(date.getTime());
  $('observationTime').textContent=date&&Number.isFinite(date.getTime())?'Campione: '+date.toLocaleString('it-IT'):'';
  const divisions=header('divisions');$('divisionTotal').textContent=number(divisions.total,{integer:true});
  empty('divisions',rows('divisionRows',divisions.by_type,(r,x)=>{cell(r,x.name||x.type);cell(r,x.count,{numeric:true,integer:true});}));
  $('templateDetails').hidden=!rows('templateRows',divisions.by_template,(r,x)=>{nameCell(r,x);cell(r,x.type);cell(r,x.count,{numeric:true,integer:true});});
  const manpower=header('manpower');for(const field of ['available','fielded','training'])$('manpower'+field[0].toUpperCase()+field.slice(1)).textContent=number(manpower[field],{integer:true});
  header('states');renderStates();
  const production=header('production');
  for(const key of ['civilian','military','dockyards'])$('effective'+key[0].toUpperCase()+key.slice(1)).textContent=number(production.effective_factories?.[key],{integer:true});
  empty('production',rows('productionRows',production.lines,(r,x)=>{nameCell(r,x);cell(r,({military:'Militare',naval:'Navale'})[x.category]||x.category);cell(r,x.equipment);cell(r,x.requested_factories,{numeric:true,integer:true});cell(r,x.assigned_factories,{numeric:true,integer:true});const output=model.productionOutput(x.output_per_day,x.category);cell(r,output?productionFormats[output.digits].format(output.value):null);}));
  const equipment=header('equipment');empty('equipment',rows('equipmentRows',equipment.items,(r,x)=>{nameCell(r,x);cell(r,x.stockpile,{numeric:true});cell(r,x.needed,{numeric:true});cell(r,x.balance_per_day,{numeric:true,signed:true});}));
  const resources=header('resources');empty('resources',rows('resourceRows',resources.items,(r,x)=>{nameCell(r,x);cell(r,x.produced,{numeric:true});cell(r,x.imported,{numeric:true});cell(r,x.exported,{numeric:true});cell(r,x.available,{numeric:true,signed:true});cell(r,x.used,{numeric:true});cell(r,x.balance,{numeric:true,signed:true});}));
 }
 $('stateSearch').addEventListener('input',renderStates);
 render(null);window.hoiLive?.subscribe(render);
})();
