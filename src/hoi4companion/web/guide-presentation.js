'use strict';
// Pure projection shared by the two trees, operational walkthrough and HUD.
((root)=>{
 const model=typeof module!=='undefined'&&module.exports?require('./guide-model.js'):root.HoiGuideModel;
 const contracts=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const native=step=>['select_focus','research'].includes(step.action.kind);
 function folder(step){try{return new URL(step?.href||'', 'http://local').searchParams.get('folder');}catch{return null;}}
 function evaluate(guide,state=null,progress={}){
  const result=model.evaluate(guide,state,progress),all=Object.values(result.tracks).flatMap(t=>t.rows),lanes={};
  const scopeOK=!result.mode.startsWith('wrong_'),normalised=contracts.state(state);
  for(const [lane,kind]of [['focus','select_focus'],['research','research']]){
   const track=result[lane],next=track.next,markers=[];
   if(scopeOK){
    const ready=new Set(result.runtime.parallel[lane].steps);
    const add=(row,tone)=>{if(!row||!native(row)||markers.some(m=>m.id===row.target))return;markers.push({id:row.target,key:row.key,scope:row.action.entity.scope||null,tone,row});};
    for(const row of all.filter(r=>r.action.kind===kind&&r.runtime_status==='active'))add(row,'active');
    for(const key of ready)add(all.find(r=>r.key===key),'next');
    if(next?.action.kind===kind&&!['completed','not_applicable'].includes(next.runtime_status))add(next,
     result.mode!=='live'?'preview':next.runtime_status==='ready'?'upcoming':'waiting');
   }
   lanes[lane]={next:next?.action.kind===kind?next:null,blocker:next&&['manual','special_project'].includes(next.action.kind)?next:null,
    markers,folder:folder(next),capacity:result.runtime.parallel[lane].capacity};
  }
  const operations=all.filter(row=>!native(row)&&row.action.kind!=='free').map(row=>{
   const params=row.action.params||{};
   const queued=contracts.fact(normalised,'construction.queued'),targets=new Set();
   function visit(node){if(!node)return;if(node.op==='fact'&&node.key==='construction.queued'&&node.test==='contains')targets.add(node.value);for(const child of node.args||[])visit(child);if(node.arg)visit(node.arg);}
   visit(row.started_when);
   const construction=scopeOK&&result.mode==='live'&&queued.status!=='unknown'?(state?.sample?.country_overview?.construction?.lines||[]).filter(line=>line.amount>0&&!line.repair_levels&&!line.conversion&&(targets.has('building:'+line.building_id)||targets.has('state:'+line.state_id+':'+line.building_id))):[];
   const membership=(key,value)=>{const item=contracts.fact(scopeOK&&result.mode==='live'?normalised:null,key);return item.status==='unknown'?null:item.value.includes(value);};
   const goal=row.completion_condition,staticConstruction=params.category==='construction'&&Number.isInteger(params.baseline_level)&&goal?.op==='fact'&&goal.key.startsWith('country.states.items.')&&Number.isInteger(goal.value)?{baseline:params.baseline_level,target:goal.value,current:null}:null;
   if(staticConstruction){const counter=contracts.fact(scopeOK&&result.mode==='live'?normalised:null,goal.key);staticConstruction.current=counter.status==='unknown'?null:counter.value;}
   return {...row,category:params.category||(row.action.kind==='special_project'?'projects':'checkpoints'),
    ...(staticConstruction?{construction:staticConstruction}:{}),
    ...(params.event_id?{event_observation:{visible:membership('events.visible',params.event_id),option_available:params.event_option_id?membership('events.visible_options',params.event_id+':'+params.event_option_id):null}}:{}),
    start_basis:row.started_when?(contracts.checkKeys(row.started_when).length?'condition':'live'):'manual',construction_observations:construction,
    game_date:params.game_date||null,
    timing:params.timing||'Al punto indicato dalla guida; verifica le condizioni nel gioco.',
    instructions:Array.isArray(params.instructions)?params.instructions.filter(x=>typeof x==='string'):[],
    source_seconds:params.source_seconds??(row.evidence||[]).find(e=>e.basis==='visual'&&Number.isFinite(e.timestamp_seconds))?.timestamp_seconds??(row.evidence||[]).find(e=>Number.isFinite(e.timestamp_seconds))?.timestamp_seconds??null};
  });
  const existing=new Set(operations.map(row=>row.key));
  for(const row of operations)if(row.action.params?.legacy_key)existing.add(row.action.params.legacy_key);
  for(const item of guide.milestones||[])if(!existing.has(item.key))operations.push({...item,
   name:item.title,action:{kind:'manual'},category:item.category||'checkpoints',
   timing:item.timing||'Verifica il momento nella fonte; il minutaggio riguarda il video.',instructions:[],
   source_seconds:item.seconds??null,completion_basis:'manual',runtime_status:scopeOK&&result.runtime.progress_status==='accepted'&&progress.checks?.[item.key]===true?'completed':'unverifiable',runtime_reason:'manual_confirmation_required'});
  // Timestamps order source instructions, never estimate in-game completion.
  const dateKey=row=>row.game_date?row.game_date.split('.').reduce((n,v)=>n*100+Number(v),0):Infinity;
  operations.sort((a,b)=>dateKey(a)-dateKey(b)||(a.source_seconds??Infinity)-(b.source_seconds??Infinity));
  return {mode:result.mode,country:result.country,progress_status:result.runtime.progress_status,lanes,operations,model:result};
 }
 const api={evaluate,folder};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiGuidePresentation=api;
})(typeof window!=='undefined'?window:globalThis);
