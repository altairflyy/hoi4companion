'use strict';
// Operational copy from bound data. No new completion, strategy or HUD decisions.
((root)=>{
 const reading=typeof module!=='undefined'&&module.exports?require('./action-reading.js'):root.HoiActionReading;
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
  const p=row.action?.params||{},instructions=[],observations=[];let panel=null,access=null,target=null,bound=false,constructionTitle=null;
  if(p.category==='production'&&row.tracking!=='reminder'){
   const source=Array.isArray(p.production_targets)?p.production_targets:[p],lines=source.filter(x=>x&&typeof x==='object'&&!Array.isArray(x));
   const naval=line=>line.production_selector?.category==='naval';
   panel='production';access=lines.length&&lines.every(naval)?t('Open Production, then the naval category.'):t('Open Production, then the equipment category.');
   bound=lines.length===source.length&&lines.length>0&&lines.every(x=>x.production_selector);
   const name=line=>label(line,'display_name','variant_name','equipment_name','equipment_id')||t('Equipment specified by the guide');
   // One line per product: what to set, replaced by the live count once the
   // Rust evaluation has read it (a met target gets ✓), or by why it is not
   // detected. Never the order and its status as two separate lists.
   const layout=Array.isArray(row.production?.layout)?row.production.layout:row.production?[row.production]:[];
   lines.forEach((line,i)=>{
    const product=name(line),amount=line.requested_factories??line.assigned_factories,dock=naval(line);
    const basis=line.assigned_factories!=null?(dock?(amount===1?t('assigned dockyard'):t('assigned dockyards')):(amount===1?t('assigned factory'):t('assigned factories')))
     :(dock?(amount===1?t('requested dockyard'):t('requested dockyards')):(amount===1?t('requested factory'):t('requested factories')));
    let text=line.quantity_mode==='remaining'?t(dock?'{0}: allocate all remaining dockyards; keep this line last.':'{0}: allocate all remaining military factories; keep this line last.',product):
     Number.isInteger(amount)?t('{0}: set {1} {2}{3}.',product,amount,basis,line.quantity_mode==='additional'?t(' more than the initial allocation'):''):
     t('{0}: the guide does not specify a verifiable factory quota.',product);
    const d=layout[i];
    if(d&&typeof d==='object'&&!d.superseded_by?.length){
     const count=d.basis==='assigned'?d.assigned:d.requested;
     if(Array.isArray(d.lines)&&d.lines.length&&Number.isInteger(count)&&!d.reason&&!d.design_check){
      const met=line.quantity_mode==='remaining'?Number.isInteger(d.target)&&count>=d.target:Number.isInteger(d.target)&&count>=d.target;
      text=(met?'✓ ':'')+(line.quantity_mode==='remaining'
       ?t(dock?'{0}: all remaining dockyards · {1} requested (at least {2})':'{0}: all remaining factories · {1} requested (at least {2})',product,count,d.target??'—')
       :t(dock?(d.target===1?'{0}: {1} of {2} dockyard':'{0}: {1} of {2} dockyards'):(d.target===1?'{0}: {1} of {2} factory':'{0}: {1} of {2} factories'),product,count,d.target??'—'));
     }
    }
    instructions.push(text);
   });
   target=instructions.join(' ');
   // An unmatched target says why instead of staying silently at zero.
   lines.forEach((line,i)=>{const d=layout[i];if(!d||typeof d!=='object'||d.superseded_by?.length)return;
    const product=name(line);
    const near=(Array.isArray(d.near_lines)?d.near_lines:[]).filter(n=>n.why===d.reason);
    const names=[...new Set(near.map(n=>n.name||n.equipment))].join(', ');
    const text=d.reason==='production_unavailable'?t('{0}: live production lines cannot be verified now.',product):
     d.reason==='line_missing'||d.reason==='remaining_line_missing'?t('{0}: not detected — no matching production line is observed.',product):
     d.reason==='design_name_mismatch'?t('{0}: not detected — the line with this equipment has another name ({1}).',product,names):
     d.reason==='category_mismatch'?t('{0}: not detected — the equipment is in another production category ({1}).',product,[...new Set(near.map(n=>n.category))].join(', ')):
     d.reason==='equipment_mismatch'?t('{0}: not detected — a line with this name uses other equipment ({1}).',product,[...new Set(near.map(n=>n.equipment))].join(', ')):
     d.reason==='remaining_line_not_last'?t(naval(line)?'{0}: detected, but it is not the last naval line.':'{0}: detected, but it is not the last military line.',product):
     d.reason==='remaining_capacity_unknown'?t(naval(line)?'{0}: total dockyards cannot be verified now.':'{0}: total military factories cannot be verified now.',product):
     d.reason==='remaining_line_ambiguous'?t('{0}: more than one matching line is observed.',product):
     null;
    if(text)observations.push(text);});
  }else if(p.category==='construction'&&row.tracking!=='reminder'){
   access=t('Open Construction and select the indicated region.');
   const compound=Array.isArray(p.construction_targets)?p.construction_targets.filter(x=>x&&typeof x==='object'):[];
   if(compound.length){
    const buildings=[...new Set(compound.map(x=>x.building_name||x.building_id).filter(Boolean))];
    const regions=[...new Set(compound.map(x=>x.state_name||x.state_id).filter(x=>x!=null))];
    constructionTitle=t('Build: {0}',buildings.join(t(' and ')));
    instructions.push(regions.length===1?t('In {0}.',regions[0]):t('In {0} regions: {1}.',regions.length,regions.join(', ')));
   }else{
   const building=label(p,'building_name','building_id')||t('Building specified by the guide');
   const place=p.province_selector?.kind==='border'?t('Every land province bordering {0}',p.province_selector.country):Array.isArray(p.province_ids)?t('Provinces {0}',p.province_ids.join(', ')):label(p,'state_name')||(p.state_id!=null?t('State {0}',p.state_id):p.province_id!=null?t('Province {0}',p.province_id):t('Location not resolved'));
   const count=p.quantity??p.target_level??p.target_quantity;
   instructions.push(['bunker','coastal_bunker'].includes(p.building_id)&&Number.isInteger(p.target_level)?t('{0} · level {1} in each province · {2}',building,p.target_level,place):Number.isInteger(count)?t('{0} {1} {2} · {3}',count,p.quantity_mode==='additional'?t('additional'):t('total'),building,place):t('{0} · {1}',building,place));
   }
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
  const source=(Array.isArray(p.instructions)?p.instructions:[]).filter(s=>typeof s==='string'&&s.trim());
  const readings=context.reader?source.map(s=>context.reader.read(s,row)):[];
  if(!instructions.length)instructions.push(...(context.reader?readings.map(r=>r.text):source));
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
  const title=row.tracking!=='reminder'&&p.category==='production'?t('Set up production'):row.tracking!=='reminder'&&p.category==='construction'?(constructionTitle||instructions[0]):
   target&&['politics','event','projects'].includes(panel)?(panel==='politics'&&!label(p,'law_name','person_name','idea_name','trait_name')?(row.name||row.title||target):target):p.operation_id==='create_intelligence_agency'?t('Create intelligence agency'):null;
  const displayTitle=context.reader?.read(title||row.name||row.title||'',row).text||title;
  return {title:displayTitle,instructions,observations,access,target,monitoring,hud:{panel,status,message},
   glossary:[...new Map(readings.flatMap(r=>r.terms).map(term=>[term.code,term])).values()],
   reading_notes:[...new Set(readings.flatMap(r=>r.notes))]};
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
 const api={describe,blocks,category,compact,createReader:language=>reading.create(language)};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiActionPlan=api;
})(typeof window!=='undefined'?window:globalThis);
