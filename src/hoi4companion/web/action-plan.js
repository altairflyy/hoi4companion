'use strict';
// Operational copy from bound data. No new completion, strategy or HUD decisions.
((root)=>{
 const t=(text,...args)=>root.HoiI18n?root.HoiI18n.t(text,...args):text.replace(/\{(\d+)\}/g,(_,i)=>String(args[i]));
 const label=(p,...keys)=>keys.map(k=>p[k]).find(v=>typeof v==='string'&&v.trim())||null;
 function category(row){
  const p=row.action?.params||{};if(p.category&&p.category!=='checkpoints')return p.category;
  const text=(Array.isArray(p.instructions)?p.instructions:[]).filter(s=>typeof s==='string').join('\n').trim();
  if(/^(?:BUILD|CONSTRUCT)(?:\s*:|\s)/i.test(text))return 'construction';
  if(/^(?:ARMY|NAVY|AIR\s*FORCE|RECRUIT|TRAIN)(?:\s*:|\s)/i.test(text))return 'army';
  if(/^(?:MIL|NAV|PRODUCTION)\s*:/i.test(text))return 'production';
  return p.category||(row.action?.kind==='special_project'?'projects':'checkpoints');
 }
 function describe(row,context={}){
  const p=row.action?.params||{},instructions=[];let panel=null,access=null,target=null,bound=false;
  if(p.category==='production'&&row.tracking!=='reminder'){
   panel='production';access=t('Open Production, then the equipment category.');
   const source=Array.isArray(p.production_targets)?p.production_targets:[p],lines=source.filter(x=>x&&typeof x==='object'&&!Array.isArray(x));
   bound=lines.length===source.length&&lines.length>0&&lines.every(x=>x.production_selector);
   for(const line of lines){
    const product=label(line,'variant_name','equipment_name','equipment_id')||t('Equipment specified by the guide');
    const amount=line.requested_factories??line.assigned_factories;
    const basis=line.assigned_factories!=null?t('assigned factories'):t('requested factories');
    instructions.push(line.quantity_mode==='remaining'?t('{0}: allocate all remaining military factories; keep this line last.',product):
     Number.isInteger(amount)?t('{0}: set {1} {2}{3}.',product,amount,basis,line.quantity_mode==='additional'?t(' more than the initial allocation'):''):
     t('{0}: the guide does not specify a verifiable factory quota.',product));
   }
   target=instructions.join(' ');
  }else if(p.category==='construction'&&row.tracking!=='reminder'){
   access=t('Open Construction and select the indicated region.');
   const building=label(p,'building_name','building_id')||t('Building specified by the guide');
   const place=p.province_selector?.kind==='border'?t('Every land province bordering {0}',p.province_selector.country):Array.isArray(p.province_ids)?t('Provinces {0}',p.province_ids.join(', ')):label(p,'state_name')||(p.state_id!=null?t('State {0}',p.state_id):p.province_id!=null?t('Province {0}',p.province_id):t('Location not resolved'));
   const count=p.quantity??p.target_level??p.target_quantity;
   instructions.push(['bunker','coastal_bunker'].includes(p.building_id)&&Number.isInteger(p.target_level)?t('{0} · level {1} in each province · {2}',building,p.target_level,place):Number.isInteger(count)?t('{0} {1} {2} · {3}',count,p.quantity_mode==='additional'?t('additional'):t('total'),building,place):t('{0} · {1}',building,place));
  }else if(row.action?.kind==='special_project'){
   panel='projects';access=t('Open Special projects and select a compatible facility.');
   target=p.project_name||row.name||row.action.entity?.id;bound=!!p.project_id||!!row.action.entity?.id&&row.catalog_match!==false;
   if(target)instructions.push(t('Start {0}; check facility specialization and breakthrough points.',target));
  }else if(p.event_id||p.event_name){
   panel='event';access=t('When this event is open, select the indicated response.');
   target=label(p,'event_option_name','event_option','event_option_id');bound=!!p.event_id&&!!p.event_option_id;
   if(target)instructions.push(t('{0} → {1}',label(p,'event_name','event_id')||t('Event'),target));
  }else if(['politics','characters'].includes(p.category)){
   access=t('Open Politics and the indicated law or appointment slot.');
   target=label(p,'law_name','person_name','idea_name','trait_name','law_id','advisor_id','idea_id','commander_id');
   bound=!!(p.law_id||p.advisor_id||p.idea_id);if(bound)panel='politics';
   if(target)instructions.push(t('Select {0}{1}.',target,(p.law_group||p.role||p.idea_group)?' · '+(p.law_group||p.role||p.idea_group):''));
  }else if(p.operation_id==='create_intelligence_agency'){
   access=t('Open Intelligence and create the agency.');instructions.push(access);
  }
  if(!instructions.length)instructions.push(...(Array.isArray(p.instructions)?p.instructions:[]).filter(s=>typeof s==='string'&&s.trim()));
  // Instructions are source copy unless a supported structured field replaces
  // them. The source quote is always retained separately by the caller.
  const readiness=row.action_readiness,resourcesReady=readiness?.resources_available===true&&!readiness.blocked?.length&&
   !(readiness.requirements_declared&&readiness.unknown?.includes('action_requirements_unknown'));
  let status=!panel?'unsupported':!bound?'unresolved':row.runtime_status==='completed'?'completed':
   row.tracking==='reminder'||['waiting','not_applicable'].includes(row.runtime_status)||row.runtime_status==='unverifiable'&&row.runtime_reason!=='manual_confirmation_required'?
    'waiting':readiness&&!resourcesReady?'waiting':'candidate';
  if(status==='candidate'&&(context.usable===false||panel==='politics'&&context.political_key!==undefined&&context.political_key!==row.key||row.runtime_status==='active'&&panel==='projects'))status='waiting';
  const messages={unsupported:t('No button highlighting for this action; follow the instruction in the game.'),
   unresolved:t('HUD target unresolved: correct the association before expecting a border.'),
   completed:panel==='production'?t('Production quota satisfied; the HUD can keep the matching line blue.'):t('Criterion satisfied; no new selection is requested.'),waiting:t('HUD waiting for order, conditions or qualified resources.'),
   candidate:t('HUD target supported. A border requires a qualified live state and a recognized visible control.')};
  let message=messages[status];
  const hud=context.overlay;
  if(context.usable&&hud?.status==='observed'&&hud.guide_id===context.guide_id&&context.guide_id===context.expected_guide_id){
   const diagnostic=(hud.items||[]).find(item=>item.step_keys?.includes(row.key));
   if(diagnostic?.message)message=diagnostic.message;
  }
  const completion=row.completion_condition||row.completion;
  const monitoring=row.tracking==='reminder'?t('Method reminder: no automatic completion.'):
   row.started_when&&completion?t('Start and completion are separate: a queued or active order starts the step; only its completion criterion finishes it.'):
   row.started_when?t('Start can be observed; final completion has no automatic criterion.'):
   completion?t('Completion follows the declared criterion; no separate start observation is available.'):
   t('Execution cannot be verified automatically from the available data.');
  const title=row.tracking!=='reminder'&&p.category==='production'?t('Set up production'):row.tracking!=='reminder'&&p.category==='construction'?instructions[0]:
   target&&['politics','event','projects'].includes(panel)?(panel==='politics'&&!label(p,'law_name','person_name','idea_name','trait_name')?(row.name||row.title||target):target):p.operation_id==='create_intelligence_agency'?t('Create intelligence agency'):null;
  return {title,instructions,access,target,monitoring,hud:{panel,status,message}};
 }
 function blocks(rows){
  const result=[];
  for(const row of rows){
   const p=row.action?.params?.source_action,valid=p&&typeof p.source_id==='string'&&typeof p.key==='string'&&Number.isInteger(p.ordinal)&&p.ordinal>0;
   const id=valid?JSON.stringify([p.source_id,p.key]):JSON.stringify(['step',row.key]);
   let group=result.at(-1);
   // Keep the runtime/presentation order even if a dependency splits a block.
   if(!group||group.id!==id){group={id,number:valid?p.ordinal:result.length+1,source:valid,rows:[]};result.push(group);}
   group.rows.push(row);
  }
  for(const group of result){
   const actions=group.rows.filter(r=>r.tracking!=='reminder');
   group.status=!actions.length?'reminder':actions.every(r=>r.runtime_status==='completed')?'completed':
    actions.some(r=>r.runtime_status==='active')?'active':actions.some(r=>r.runtime_status==='ready')?'ready':
    actions.every(r=>r.runtime_status==='not_applicable')?'not_applicable':'waiting';
  }
  return result;
 }
 function compact(rows,political){
  const actions=rows.filter(r=>r.tracking!=='reminder');
  const eligible=actions.filter(row=>(!row.action_readiness||row.action_readiness.domain!=='political'||row.key===political)&&(!['completed','not_applicable','waiting','active'].includes(row.runtime_status)||row.key===political&&row.runtime_status==='waiting'));
  const next=actions.find(row=>row.event_observation?.visible===true&&!['completed','not_applicable'].includes(row.runtime_status))||eligible.find(row=>!['dependency_unknown','phase_condition_unknown','condition_unknown','live_state_unavailable'].includes(row.runtime_reason))||eligible[0]||null;
  return {next,active:actions.filter(r=>r.runtime_status==='active'),waiting:actions.filter(r=>r.key!==next?.key&&['waiting','unverifiable'].includes(r.runtime_status)).slice(0,3)};
 }
 const api={describe,blocks,category,compact};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiActionPlan=api;
})(typeof window!=='undefined'?window:globalThis);
