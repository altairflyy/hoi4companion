'use strict';
// Explicitly opened editor; guide definitions, corrections and run progress stay separate.
(()=>{
 const $=id=>document.getElementById(id),node=(tag,text)=>{const n=document.createElement(tag);if(text!==undefined)n.textContent=text;return n;};
 let data=null,catalog=[],allSteps=[],busy=true,loaded=false,missing=null,lines=null;
 const query=new URLSearchParams(location.search),fromCard=query.get('from')==='card',identity=ref=>JSON.stringify({kind:ref.kind,id:ref.id,...(ref.scope?{scope:ref.scope}:{})});
 const controls=['associationGuide','associationStep','stepSearch','associationSearch','associationMatches','interpretSearch','interpretStep','interpretCategory','interpretTargetSearch','interpretTargets'];
 // UX R13: the player's reading of a line followed by hand. Presentation only;
 // Rust validates the line and resolves the entity's name from the catalog.
 const categoryNames={construction:"Factories and infrastructure",production:"Production",army:"Armed forces",diplomacy:"War and diplomacy",trade:"Trade and resources",politics:"Politics and laws",projects:"Special projects",checkpoints:"Other instructions"};
 const targetKinds=new Set(['country','focus','technology','state','equipment','project','building']);
 const kindNames={country:"Country",focus:"National focus",technology:"Technology",state:"State",equipment:"Equipment",project:"Special project",building:"Building"};
 const line=()=>lines?.steps?.find(row=>row.key===$('interpretStep').value);
 function error(e){$('associationError').hidden=false;$('associationError').textContent=e.message||String(e);}
 async function request(path,body){const r=await fetch(path,{method:body===undefined?'GET':'POST',headers:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)}),v=await r.json();if(!r.ok)throw Error(v.error||HoiI18n.t("Request failed"));return v;}
 const current=()=>allSteps.find(row=>row.key===$('associationStep').value);
 function enabled(){
  for(const id of controls)$(id).disabled=busy||!loaded;
  const row=current(),chosen=$('associationMatches').value;
  $('saveAssociation').disabled=busy||!loaded||!row||!chosen||chosen===identity(row.action.entity);
  $('restoreAssociation').disabled=busy||!loaded||!row||!data.changes.some(item=>item.key===row.key);
  const own=line(),saved=own?.interpretation,target=$('interpretTargets').value||null,category=$('interpretCategory').value;
  $('saveInterpretation').disabled=busy||!loaded||!own||!category||!!saved&&saved.category===category&&(saved.target?identity(saved.target):'')===(target||'');
  $('removeInterpretation').disabled=busy||!loaded||!own||!saved;
 }
 function targets(){
  const own=line(),list=$('interpretTargets');list.replaceChildren();const none=node('option',HoiI18n.t("No game entity"));none.value='';list.append(none);
  const text=$('interpretTargetSearch').value.trim().toLocaleLowerCase(),scope=data?.guide?.applies_to?.tree_id,saved=own?.interpretation?.target;
  const matches=!text?[]:catalog.filter(item=>targetKinds.has(item.kind)&&(item.kind!=='focus'||!scope||item.scope===scope)&&[item.name,item.localized_name,item.id,...item.aliases||[]].some(name=>typeof name==='string'&&name.toLocaleLowerCase().includes(text)));
  const shown=matches.slice(0,100);if(saved&&!shown.some(item=>identity(item)===identity(saved)))shown.unshift({...saved,localized_name:saved.name});
  for(const item of shown){const option=node('option',(item.localized_name||item.name||item.id)+' · '+HoiI18n.t(kindNames[item.kind]||item.kind)+' · '+item.id);option.value=identity(item);list.append(option);}
  list.value=saved?identity(saved):'';$('interpretTargetCount').textContent=text?matches.length+HoiI18n.t(" matches")+(matches.length>100?HoiI18n.t(" · first 100 results: narrow your search."):'.'):'';enabled();
 }
 function interpretLine(){const own=line();$('interpretTargetSearch').value='';
  $('interpretLine').textContent=own?HoiI18n.t("Original instruction: ")+own.text:lines?.steps?.length?'':HoiI18n.t("No lines to interpret in this guide.");
  $('interpretCategory').value=own?.interpretation?.category||own?.category||'';targets();
 }
 function interpretSteps(preferred){
  const text=$('interpretSearch').value.trim().toLocaleLowerCase(),select=$('interpretStep');select.replaceChildren();
  for(const row of (lines?.steps||[]).filter(row=>!text||[row.key,row.text].some(v=>typeof v==='string'&&v.toLocaleLowerCase().includes(text)))){
   const option=node('option',(row.interpretation?'✓ ':'')+row.text);option.value=row.key;select.append(option);
  }
  select.value=[...select.options||[]].some(r=>r.value===preferred)?preferred:select.children[0]?.value||'';interpretLine();
 }
 async function loadLines(id,key){
  lines=await request('/api/guide/interpretations?id='+encodeURIComponent(id));
  $('interpretCategory').replaceChildren();for(const value of lines.categories||[]){const option=node('option',HoiI18n.t(categoryNames[value]||value));option.value=value;$('interpretCategory').append(option);}
  $('interpretSearch').value='';interpretSteps(key);
  $('interpretNotice').textContent=(lines.steps||[]).filter(row=>row.interpretation).length+HoiI18n.t(" lines with your reading.")+(lines.stale?.length?' '+HoiI18n.t("{0} earlier readings no longer match their line and are not shown.",lines.stale.length):'');
 }
 async function saveLine(remove=false){const own=line();if(!own)return;
  const chosen=$('interpretTargets').value,interpretation=remove?null:{category:$('interpretCategory').value,target:chosen?JSON.parse(chosen):null};
  await request('/api/guide/interpretations',{guide_id:data.guide.id,key:own.key,expected:own.text,interpretation});await loadLines(data.guide.id,own.key);
  $('interpretNotice').textContent=remove?HoiI18n.t("Your reading was removed. The line is shown as the source wrote it."):HoiI18n.t("Your reading is saved. It changes how the line is shown, never when it counts as done.");
 }
 async function run(fn){if(busy)return;busy=true;enabled();$('associationError').hidden=true;try{await fn();}catch(e){error(e);}finally{busy=false;enabled();}}
 function search(){
  const row=current(),list=$('associationMatches');list.replaceChildren();if(!row){enabled();return;}
  const ref=row.action.entity,country=data.guide.applies_to?.country,scope=data.guide.applies_to?.tree_id||ref.scope;
  const text=$('associationSearch').value.trim().toLocaleLowerCase();
  const matches=catalog.filter(item=>item.kind===ref.kind&&(ref.kind!=='focus'||(!scope||scope===item.scope)&&(!country||!item.countries?.length||item.countries.includes(country)))&&(!text||[item.name,item.localized_name,item.id,...item.aliases||[]].some(name=>typeof name==='string'&&name.toLocaleLowerCase().includes(text))));
  for(const item of matches.slice(0,100)){const option=node('option',(item.localized_name||item.name)+' · '+item.id+(item.scope?' · '+item.scope:''));option.value=identity(item);list.append(option);}
  list.value='';$('associationMatchCount').textContent=matches.length+HoiI18n.t(" matches")+(matches.length>100?HoiI18n.t(" · first 100 results: narrow your search."):'.');enabled();
 }
 function step(){const row=current();$('associationSearch').value='';
  $('currentAssociation').textContent=missing&&!row?(lines?.steps?.some(r=>r.key===missing)?HoiI18n.t("This line has no focus, technology or project binding. Say what it means under “What a line means” below."):HoiI18n.t("This step has no focus, technology or project binding to correct here. Its text stays as the source wrote it.")):row?HoiI18n.t("Current: ")+(row.name||row.title||row.action.entity.id)+' · '+row.action.entity.id+(row.action.entity.scope?' · '+row.action.entity.scope:''):HoiI18n.t("No steps with an entity binding.");
  $('associationSourceNote').textContent=row?.title&&row.title!==row.name?HoiI18n.t("Original instruction: ")+row.title:'';search();
 }
 function steps(preferred){
  const text=$('stepSearch').value.trim().toLocaleLowerCase(),select=$('associationStep');select.replaceChildren();
  for(const row of allSteps.filter(row=>!text||[row.key,row.name,row.title,row.action.entity.id].some(v=>typeof v==='string'&&v.toLocaleLowerCase().includes(text)))){
   const option=node('option',row.key+' · '+(row.name||row.title||row.action.entity.id));option.value=row.key;select.append(option);
  }
  select.value=allSteps.some(r=>r.key===preferred)&&[...select.options||[]].some(r=>r.value===preferred)?preferred:missing?'':select.children[0]?.value||'';step();
 }
 async function load(id,key){
  loaded=false;
  data=await request('/api/guide/associations'+(id?'?id='+encodeURIComponent(id):''));loaded=true;
  $('associationGuide').value=data.guide.id;
  allSteps=Object.values(data.guide.tracks).flat().filter(row=>row.action.entity);
  // A requested step that cannot be corrected here is named, never swapped for another step (UX R12).
  missing=key&&!allSteps.some(row=>row.key===key)?key:null;
  const back=fromCard&&key?'#operation-'+encodeURIComponent(key):'';$('returnGuide').href='/walkthrough?guide='+encodeURIComponent(data.guide.id)+back;
  await loadLines(data.guide.id,key);
  $('stepSearch').value='';steps(key);$('associationNotice').textContent=HoiI18n.t("Guide: ")+data.guide.title+' · '+data.changes.length+HoiI18n.t(" local corrections.")+(fromCard&&key&&!missing?' '+HoiI18n.t("Opened from the step card: pick the right entity below, then save. Nothing changes until you save."):'');
 }
 async function save(restore=false){const row=current();if(!row)return;
  const update={guide_id:data.guide.id,key:row.key,expected:row.action.entity,entity:restore?null:JSON.parse($('associationMatches').value)};
  await request('/api/guide/associations',update);await load(data.guide.id,row.key);
  $('associationNotice').textContent=restore?HoiI18n.t("Original binding restored."):HoiI18n.t("Correction saved. Return to the guide to use it in the trees; progress is preserved.");
 }
 $('associationGuide').onchange=()=>run(()=>load($('associationGuide').value));$('associationStep').onchange=()=>{missing=null;step();};
 $('stepSearch').oninput=()=>{missing=null;steps($('associationStep').value);};$('associationSearch').oninput=search;$('associationMatches').onchange=enabled;
 $('saveAssociation').onclick=()=>run(()=>save());$('restoreAssociation').onclick=()=>run(()=>save(true));
 $('interpretSearch').oninput=()=>interpretSteps($('interpretStep').value);$('interpretStep').onchange=interpretLine;$('interpretCategory').onchange=enabled;
 $('interpretTargetSearch').oninput=targets;$('interpretTargets').onchange=enabled;
 $('saveInterpretation').onclick=()=>run(()=>saveLine());$('removeInterpretation').onclick=()=>run(()=>saveLine(true));
 (async()=>{try{const [guides,entities]=await Promise.all([request('/api/guides'),request('/api/import/catalog')]);catalog=entities.entities;
  for(const item of guides.guides){const option=node('option',item.title);option.value=item.id;$('associationGuide').append(option);}
  await load(query.get('guide'),query.get('step'));
 }catch(e){error(e);}finally{busy=false;enabled();}})();
})();
