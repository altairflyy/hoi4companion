'use strict';
// Explanations of an existing runtime evaluation, never another rule evaluator.
((root)=>{
 const contracts=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const number=value=>typeof value==='number'&&Number.isFinite(value);
 const count=value=>Number.isSafeInteger(value)&&value>=0;
 const title=step=>step.name||step.title||step.key;
 function factKeys(node,result=new Set()){
  if(node?.op==='fact')result.add(node.key);
  for(const child of node?.args||[])factKeys(child,result);
  if(node?.arg)factKeys(node.arg,result);return result;
 }
 function evidence(state,key,label){const f=contracts.fact(state,key);return {key,label,value:f.value,status:f.status,source:f.source,reason:f.reason||''};}
 const label=key=>key.includes('political_power')?'Potere politico':key.endsWith('.military_factories')?'Fabbriche militari':key.endsWith('.civilian_factories')?'Fabbriche civili':key.endsWith('.infrastructure')?'Infrastrutture':key.split('.').at(-1).replaceAll('_',' ');
 function nativeHref(step){const href=step.href;
  if(typeof href==='string'&&href.startsWith('/')&&!href.startsWith('//')){
   try{const parsed=new URL(href,'http://local');if(parsed.origin==='http://local'&&['/','/technologies'].includes(parsed.pathname))return parsed.pathname+parsed.search+parsed.hash;}catch{}
  }
  return step.action.kind==='research'?'/technologies':'/';
 }
 function project(guide,runtime,value,options={}){
  const state=contracts.state(value),limit=Number.isInteger(options.limit)?Math.max(1,Math.min(8,options.limit)):6;
  const result={schema_version:1,mode:runtime?.mode||'preview',country:runtime?.country||null,notice:'',items:[],hidden_count:0};
  if(runtime?.guide_id!==guide?.id){result.notice='Riepilogo sospeso: valutazione riferita a un’altra guida.';return result;}
  if(['wrong_country','wrong_tree'].includes(runtime.mode)){result.notice='Guida sospesa: nazione o albero della partita non corrispondenti.';return result;}
  if(runtime.mode!=='live'||!state.usable){result.notice='Stato della partita non disponibile. Il piano resta consultabile; nessun dato precedente viene presentato come attuale.';return result;}
  if(runtime.country!==contracts.fact(state,'country').value){result.notice='Riepilogo sospeso: valutazione e dati si riferiscono a nazioni diverse.';return result;}
  guide=contracts.guide(guide); // Same legacy metadata adapter used by the existing presentation.
  const definitions=new Map(Object.values(guide.tracks||{}).flat().map(s=>[s.key,s])),rows=new Map((runtime.steps||[]).map(r=>[r.key,r]));
  const dependencyLinks=row=>(row.blocked_by||[]).map(key=>definitions.get(key)).filter(Boolean).slice(0,4).map(reference);
  const items=[];let order=0;
  const metric=(name,value)=>({label:name,value:count(value)?value:null,status:count(value)?'calculated':'unknown',source:count(value)?'runtime.production':''});
  for(const row of runtime.steps||[]){
   const step=definitions.get(row.key);if(!step||step.tracking==='reminder'||row.status==='not_applicable'||['select_focus','research','free'].includes(step.action.kind))continue;
   if(row.action_readiness?.domain==='political'&&runtime.action_sequence?.political!==row.key)continue;
   const p=step.action.params||{},category=p.category||(step.action.kind==='special_project'?'projects':'checkpoints');
   // Future blocked instructions stay in the full plan, not in the current summary.
   if(row.blocked_by?.length||row.reason==='skip_condition_unknown')continue;
   let item={key:row.key,step_key:row.key,category,title:title(step),tone:'ready',message:'',guide_goal:'',
    evidence:[],runtime_status:row.status,reason:row.reason,blocked_by:[...(row.blocked_by||[])],
    href:'#operation-'+encodeURIComponent(row.key),link_label:'Mostra istruzione',rank:40,priority:row.priority||0,order:order++};
   const knownKeys=new Set();for(const node of [step.when,step.completion,step.started_when,guide.phases?.find(p=>p.id===step.phase)?.when])factKeys(node,knownKeys);
   item.evidence=[...knownKeys].map(key=>evidence(state,key,label(key)));
   if(row.production){
    const comparisons=(row.production.layout||[row.production]).map((detail,i)=>({
     name:row.production.layout?(p.production_targets?.[i]?.variant_name||p.production_targets?.[i]?.equipment_name||p.production_targets?.[i]?.equipment_id||'Prodotto '+(i+1)):(p.variant_name||p.equipment_name||p.equipment_id||title(step)),
     target:count(detail.target)?detail.target:null,requested:count(detail.requested)?detail.requested:null,
     assigned:count(detail.assigned)?detail.assigned:null,active:count(detail.active)?detail.active:null,
     basis:detail.basis,reason:detail.reason||''}));
    const gap=comparisons.some(p=>p.requested!==null&&p.assigned!==null&&p.requested>p.assigned);
    if(row.status==='completed'&&!gap)continue;
    item.comparisons=comparisons;
    item.evidence=comparisons.flatMap(p=>[metric('Fabbriche richieste',p.requested),metric('Fabbriche assegnate',p.assigned)]);
    item.guide_goal=comparisons.map(p=>p.name+': '+(p.target===null?'totale non verificabile':p.target+' fabbriche '+(p.basis==='requested'?'richieste':'assegnate'))).join(' · ');
    if(gap){item.tone='attention';item.rank=10;item.title='Produzione: attenzione';item.reason='requested_exceeds_assigned';
     item.message=comparisons.filter(p=>p.requested!==null&&p.assigned!==null&&p.requested>p.assigned).map(p=>`${p.name}: ${p.requested} richieste, ${p.assigned} assegnate.`).join(' ');
     item.message+=row.status==='completed'?' Il criterio della guida è soddisfatto; richiesta impostata e capacità assegnata restano due dati distinti.':' Le fabbriche richieste non corrispondono ancora a quelle assegnate.';
    }else if(comparisons.some(p=>p.assigned===null||p.target===null)){
     item.tone='unknown';item.rank=50;item.title='Produzione non verificabile';item.message='Manca un conteggio qualificato o il totale da raggiungere. Non deduco che l’ordine sia stato eseguito.';
    }else{item.title=row.status==='active'?'Produzione in corso':'Produzione da impostare';item.tone=row.status==='active'?'active':'ready';
     item.rank=row.status==='active'?20:40;item.message='Confronta il totale della guida con fabbriche richieste e assegnate nella partita.';}
   }else{
    if(row.status==='completed')continue;
    const pp=item.evidence.find(e=>e.key.includes('political_power'));
    if(row.action_readiness?.domain==='political'&&!(row.status==='unverifiable'&&pp?.status==='unknown')){
     const p=row.action_readiness.political_power;item.title=p?.affordable===false?'Potere politico insufficiente':'Prossimo incarico o legge';item.tone=p?.affordable===false?'waiting':row.availability.value===null?'unknown':'ready';item.rank=30;
     item.message=`${title(step)}. Potere politico: ${p?.available??'non verificabile'}; costo indicato dalla guida: ${p?.cost??'non specificato'}.`;
     if(row.action_readiness.unknown.length)item.message+=' Restano requisiti non verificabili.';
     if(p?.affordable===false)item.message+=' Attendi le risorse richieste; la sequenza conserva questa azione.';
    }else if(row.status==='unverifiable'&&pp?.status==='unknown'){
     item.title='Potere politico non verificabile';item.tone='unknown';item.rank=30;
     item.message='La guida richiede un controllo del potere politico, ma il dato necessario non è disponibile. Non posso stabilire se la condizione sia rispettata.';
    }else if(row.status==='active'){
     item.tone='active';item.rank=20;
     if(row.selection?.source?.startsWith('manual_started:')){
      item.title='Avvio segnalato manualmente';item.evidence=[];item.message='Hai segnalato manualmente l’avvio. Il completamento non è ancora confermato.';
     }else if(category==='construction'){
      item.title='Costruzione avviata';item.message='Ordine di costruzione avviato. Il completamento non è ancora confermato.';
     }else if(category==='projects'||step.action.kind==='special_project'){
      item.title='Progetto in corso';item.message='Avvio rilevato dal criterio della guida. Il progetto non risulta ancora completato.';
     }else{item.title='Attività in corso';item.message='Il runtime rileva l’avvio previsto dalla guida; il criterio finale resta distinto.';}
    }else if(row.status==='waiting'){
     item.title='Condizione della guida in attesa';item.tone='waiting';item.rank=50;
     item.message='La condizione dichiarata dalla guida non è ancora soddisfatta. Questo non equivale a un errore o a un ordine eseguito.';
    }else if(row.status==='unverifiable'){
     item.title=row.reason==='catalog_unmatched'?'Associazione da verificare':'Attività da verificare';item.tone='unknown';item.rank=50;
     item.message=row.reason==='catalog_unmatched'?'L’identità richiesta non corrisponde al catalogo locale.':'Il runtime non dispone di una prova sufficiente del criterio finale. Nessun completamento viene dedotto.';
    }else{item.title='Istruzione pronta da verificare';item.message='Le dipendenze dichiarate risultano soddisfatte. La disponibilità effettiva nel gioco non è certificata.';}
    item.guide_goal=title(step);
    if(category==='construction'){
     const target=row.construction?.target??(step.completion?.op==='fact'&&number(step.completion.value)?step.completion.value:null);
     item.guide_goal=(count(target)?'Raggiungere '+target:count(p.quantity)?(p.quantity_mode==='target_total'?'Raggiungere ':'Aggiungere ')+p.quantity:'Verificare l’obiettivo di costruzione')+(p.building_name?' '+p.building_name:'')+(p.state_name?' · '+p.state_name:'');
     if(row.construction){item.evidence.push({label:'Conteggio iniziale',value:row.construction.baseline,status:row.construction.baseline===null?'unknown':'calculated',source:'runtime.construction'},
      {label:'Conteggio attuale',value:row.construction.current,status:row.construction.current===null?'unknown':'observed',source:'runtime.construction'});}
    }
   }
   items.push(item);
  }
  for(const [category,kind]of [['research','research'],['focus','select_focus']]){
   const candidates=(runtime.steps||[]).filter(r=>definitions.get(r.key)?.action.kind===kind&&!['completed','active','not_applicable'].includes(r.status));
   const parallel=runtime.parallel?.[category],ready=(parallel?.steps||[]).map(k=>rows.get(k)).filter(Boolean);
   const nextKeys=new Set(Object.values(runtime.tracks||{}).map(t=>t.next));
   const row=ready[0]||candidates.find(r=>nextKeys.has(r.key))||candidates[0];if(!row)continue;
   const step=definitions.get(row.key),research=category==='research';
   const item={key:category,step_key:row.key,category,tone:ready.length?'ready':row.status==='unverifiable'?'unknown':'waiting',
    title:research?'Ricerca successiva':'Focus nazionale successivo',guide_goal:research?'Seguire la ricerca nella scheda tecnologica.':'Seguire il focus nel suo albero.',
    runtime_status:row.status,reason:row.reason,blocked_by:[...(row.blocked_by||[])],evidence:[],
    href:nativeHref(step),link_label:research?'Apri tecnologie':'Apri focus',rank:ready.length?35:55,priority:row.priority||0,order:order++};
   item.message=ready.length?(research?`${parallel.capacity} slot liberi; ${ready.length} ricerche previste dalla guida sono pronte.`:'Il runtime propone il focus previsto dalla guida. Lo trovi evidenziato nel suo albero.'):
    parallel?.capacity===0?(research?'Gli slot rilevati sono occupati. Le ricerche successive restano consultabili nelle loro schede.':'Un focus risulta già attivo. Il prossimo previsto resta consultabile nel suo albero.'):
    row.reason==='dependency_not_completed'?(research?'La ricerca successiva dipende dal completamento di una ricerca precedente o di un altro passaggio della guida.':'Il focus successivo attende il completamento dei passaggi richiesti.'):
    row.reason==='dependency_unknown'?'Il completamento di un passaggio precedente non è verificabile.':'Il runtime attende una condizione o una verifica: consulta la scheda dedicata.';
   if(row.blocked_by?.length){
    item.links=dependencyLinks(row);const names=item.links.map(l=>l.label).join(', ');
    if(names)item.message=(row.reason==='dependency_unknown'?'Completamento non verificabile: ':'Attende il completamento di: ')+names+'.';
    if(row.blocked_by.length>4)item.message+=' Altri prerequisiti sono indicati nella scheda.';
   }
   if(research&&parallel?.capacity===0){
    const slots=contracts.fact(state,'research.slots');
    if(slots.status!=='unknown'&&Array.isArray(slots.value))item.message=`Gli slot rilevati sono occupati: ${slots.value.filter(s=>s.technology_id).length} su ${slots.value.length}. Nessuna nuova ricerca da avviare; consulta le schede per quelle in corso.`;
   }
   items.push(item);
  }
  items.sort((a,b)=>a.rank-b.rank||b.priority-a.priority||a.order-b.order);
  result.items=items.slice(0,limit).map(({rank,priority,order,...item})=>item);
  result.hidden_count=Math.max(0,items.length-result.items.length);
  if(!items.length)result.notice='Nessuna attività richiede una spiegazione aggiuntiva in questo momento. Il piano completo resta consultabile.';
  return result;
 }
 function reference(step){return {label:title(step),href:['research','select_focus'].includes(step.action.kind)?nativeHref(step):'#operation-'+encodeURIComponent(step.key)};}
 const api={project,reference};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiOperationalSummary=api;
})(typeof window!=='undefined'?window:globalThis);
