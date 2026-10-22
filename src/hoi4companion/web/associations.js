'use strict';
// Explicitly opened editor; guide definitions, corrections and run progress stay separate.
(()=>{
 const $=id=>document.getElementById(id),node=(tag,text)=>{const n=document.createElement(tag);if(text!==undefined)n.textContent=text;return n;};
 let data=null,catalog=[],allSteps=[],busy=true,loaded=false;
 const query=new URLSearchParams(location.search),identity=ref=>JSON.stringify({kind:ref.kind,id:ref.id,...(ref.scope?{scope:ref.scope}:{})});
 const controls=['associationGuide','associationStep','stepSearch','associationSearch','associationMatches'];
 function error(e){$('associationError').hidden=false;$('associationError').textContent=e.message||String(e);}
 async function request(path,body){const r=await fetch(path,{method:body===undefined?'GET':'POST',headers:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)}),v=await r.json();if(!r.ok)throw Error(v.error||'Richiesta fallita');return v;}
 const current=()=>allSteps.find(row=>row.key===$('associationStep').value);
 function enabled(){
  for(const id of controls)$(id).disabled=busy||!loaded;
  const row=current(),chosen=$('associationMatches').value;
  $('saveAssociation').disabled=busy||!loaded||!row||!chosen||chosen===identity(row.action.entity);
  $('restoreAssociation').disabled=busy||!loaded||!row||!data.changes.some(item=>item.key===row.key);
 }
 async function run(fn){if(busy)return;busy=true;enabled();$('associationError').hidden=true;try{await fn();}catch(e){error(e);}finally{busy=false;enabled();}}
 function search(){
  const row=current(),list=$('associationMatches');list.replaceChildren();if(!row){enabled();return;}
  const ref=row.action.entity,country=data.guide.applies_to?.country,scope=data.guide.applies_to?.tree_id||ref.scope;
  const text=$('associationSearch').value.trim().toLocaleLowerCase();
  const matches=catalog.filter(item=>item.kind===ref.kind&&(ref.kind!=='focus'||(!scope||scope===item.scope)&&(!country||!item.countries?.length||item.countries.includes(country)))&&(!text||[item.name,item.localized_name,item.id,...item.aliases||[]].some(name=>typeof name==='string'&&name.toLocaleLowerCase().includes(text))));
  for(const item of matches.slice(0,100)){const option=node('option',(item.localized_name||item.name)+' · '+item.id+(item.scope?' · '+item.scope:''));option.value=identity(item);list.append(option);}
  list.value='';$('associationMatchCount').textContent=matches.length+' corrispondenze'+(matches.length>100?' · primi 100 risultati: restringi la ricerca.':'.');enabled();
 }
 function step(){const row=current();$('associationSearch').value='';
  $('currentAssociation').textContent=row?'Attuale: '+(row.name||row.title||row.action.entity.id)+' · '+row.action.entity.id+(row.action.entity.scope?' · '+row.action.entity.scope:''):'Nessun passaggio con associazione.';
  $('associationSourceNote').textContent=row?.title&&row.title!==row.name?'Istruzione originale: '+row.title:'';search();
 }
 function steps(preferred){
  const text=$('stepSearch').value.trim().toLocaleLowerCase(),select=$('associationStep');select.replaceChildren();
  for(const row of allSteps.filter(row=>!text||[row.key,row.name,row.title,row.action.entity.id].some(v=>typeof v==='string'&&v.toLocaleLowerCase().includes(text)))){
   const option=node('option',row.key+' · '+(row.name||row.title||row.action.entity.id));option.value=row.key;select.append(option);
  }
  select.value=allSteps.some(r=>r.key===preferred)&&[...select.options||[]].some(r=>r.value===preferred)?preferred:select.children[0]?.value||'';step();
 }
 async function load(id,key){
  loaded=false;
  data=await request('/api/guide/associations'+(id?'?id='+encodeURIComponent(id):''));loaded=true;
  $('associationGuide').value=data.guide.id;$('returnGuide').href='/walkthrough?guide='+encodeURIComponent(data.guide.id);
  allSteps=Object.values(data.guide.tracks).flat().filter(row=>row.action.entity);
  $('stepSearch').value='';steps(key);$('associationNotice').textContent='Guida: '+data.guide.title+' · '+data.changes.length+' correzioni locali.';
 }
 async function save(restore=false){const row=current();if(!row)return;
  const update={guide_id:data.guide.id,key:row.key,expected:row.action.entity,entity:restore?null:JSON.parse($('associationMatches').value)};
  await request('/api/guide/associations',update);await load(data.guide.id,row.key);
  $('associationNotice').textContent=restore?'Associazione originale ripristinata.':'Correzione salvata. Torna alla guida per usarla negli alberi; l’avanzamento è conservato.';
 }
 $('associationGuide').onchange=()=>run(()=>load($('associationGuide').value));$('associationStep').onchange=step;
 $('stepSearch').oninput=()=>steps($('associationStep').value);$('associationSearch').oninput=search;$('associationMatches').onchange=enabled;
 $('saveAssociation').onclick=()=>run(()=>save());$('restoreAssociation').onclick=()=>run(()=>save(true));
 (async()=>{try{const [guides,entities]=await Promise.all([request('/api/guides'),request('/api/import/catalog')]);catalog=entities.entities;
  for(const item of guides.guides){const option=node('option',item.title);option.value=item.id;$('associationGuide').append(option);}
  await load(query.get('guide'),query.get('step'));
 }catch(e){error(e);}finally{busy=false;enabled();}})();
})();
