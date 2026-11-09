'use strict';
((root)=>{
 const uiText=typeof module!=='undefined'&&module.exports?require('./i18n.js').t:root.HoiI18n.t;
 const c=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const focusCriteria=typeof module!=='undefined'&&module.exports?require('./focus-criteria.js'):root.HoiFocusCriteria;
 const production=typeof module!=='undefined'&&module.exports?require('./production-progress.js'):root.HoiProductionProgress;
 const unknown=(reason='not_observed')=>({status:'unknown',value:null,source:'',reason});
 const calculated=(value,source)=>value===null?unknown():({status:'calculated',value,source});
 const settled=row=>['completed','not_applicable'].includes(row.status);
 function researchProgression(guide,state){
  const f=c.fact(state,'research.completed'),result=new Map();
  if(!state.usable||f.status!=='observed'||!f.source||!Array.isArray(f.value))return result;
  const done=new Set(f.value);
  for(const rows of Object.values(guide.tracks)){
   const previous=[];
   for(const step of rows){
    if(step.action.kind!=='research')continue;
    const chain=step.relative_research?.chain,target=step.action.entity.id;
    if(chain){
     if(chain.includes(target)){if(chain.slice(0,chain.indexOf(target)).some(x=>!done.has(x)))result.set(step.key,'research_previous_level_incomplete');}
     else if(chain.every(x=>done.has(x)))result.set(step.key,'research_line_complete');
     else if(previous.some(p=>chain.includes(p.action.entity.id)&&!done.has(p.action.entity.id)))result.set(step.key,'research_previous_level_incomplete');
    }
    previous.push(step);
   }
  }
  return result;
 }
 function researchChoices(guide,state,bindings,offered={}){
  const allRows=Object.entries(guide.tracks).sort(([a],[b])=>compareIds(a,b)).flatMap(([,rows])=>rows),rows=allRows.filter(s=>s.research_choice),result=new Map(rows.map(s=>[s.key,{options:[],branch:null,status:'unknown'}]));
  if(!rows.length||!state.usable)return result;
  const completed=c.fact(state,'research.completed'),active=c.fact(state,'research.active');
  if([completed,active].some(f=>f.status!=='observed'||!f.source||!Array.isArray(f.value)))return result;
  const done=new Set(completed.value),seen=new Set([...completed.value,...active.value]),groups=new Map();
  const signature=branches=>JSON.stringify(Object.entries(branches).sort(([a],[b])=>a.localeCompare(b)));
  for(const step of rows){const r=step.research_choice,prior=groups.get(r.group);if(prior&&signature(prior)!==signature(r.branches))return result;groups.set(r.group,r.branches);}
  const selected=new Map();
  for(const [group,branches]of groups){const used=new Set(rows.filter(s=>s.research_choice.group===group).map(s=>bindings[s.key])),options=Object.entries(branches).filter(([key,chain])=>chain.some(x=>seen.has(x)||used.has(x))).map(([key])=>key);selected.set(group,options.length===1?options[0]:options.length?false:null);}
  const previous=new Map();
  for(const step of rows){const r=step.research_choice,branch=selected.get(r.group),info=result.get(step.key),bound=bindings[step.key],earlier=previous.get(r.group)||[];info.branch=branch||null;
   if(Object.values(r.branches).some(chain=>chain.includes(bound)))Object.assign(info,{status:'bound',options:[bound]});
   else if(r.mode==='once'){
    const saved=offered?.[step.key],valid=Array.isArray(saved)&&saved.length>=1&&saved.length<=Object.keys(r.branches).length&&saved.every(x=>typeof x==='string'&&Object.values(r.branches).some(chain=>chain.includes(x)))&&new Set(saved).size===saved.length&&Object.values(r.branches).every(chain=>saved.filter(x=>chain.includes(x)).length<=1);
    const earlier=allRows.slice(0,allRows.indexOf(step)),options=[];
    for(const [,chain]of Object.entries(r.branches).sort(([a],[b])=>compareIds(a,b))){
     const frozen=valid?saved.find(x=>chain.includes(x)):null;
     const blocked=chain.length>1&&earlier.some(p=>p.action.kind==='research'&&
      (chain.includes(bindings[p.key]??p.action.entity?.id)&&!done.has(bindings[p.key]??p.action.entity?.id)||
       !bindings[p.key]&&(JSON.stringify(p.relative_research?.chain)===JSON.stringify(chain)||Object.values(p.research_choice?.branches||{}).some(c=>JSON.stringify(c)===JSON.stringify(chain)))));
     const target=frozen||(!blocked&&(chain.length===1?chain[0]:chain.find(x=>!done.has(x))));if(target)options.push(target);
    }
    const chosen=options.filter(x=>active.value.includes(x)||done.has(x)&&(valid||Object.values(r.branches).some(chain=>chain.length===1&&chain[0]===x)));
    if(chosen.length===1)Object.assign(info,{branch:Object.keys(r.branches).find(k=>r.branches[k].includes(chosen[0])),status:'selected',options:chosen});
    else if(chosen.length)Object.assign(info,{branch:null,status:'conflict',options:[]});
    else Object.assign(info,{branch:null,status:'open',options});
   }
   else if(branch===false)info.status='conflict';
   else if(earlier.some(s=>!done.has(bindings[s.key])))info.status='waiting';
   else{for(const [key,chain]of Object.entries(r.branches).sort(([a],[b])=>compareIds(a,b))){if(branch&&key!==branch)continue;const id=r.level?chain[r.level-1]:chain.find(x=>!done.has(x));if(id)info.options.push(id);}info.status=branch?'selected':'open';}
   previous.set(r.group,[...earlier,step]);
  }
  return result;
 }
 const buildingFacts={infrastructure:'infrastructure',arms_factory:'military_factories',industrial_complex:'civilian_factories',dockyard:'dockyards'};
 const integer=(value,minimum=0)=>Number.isSafeInteger(value)&&value>=minimum;
 function baselinesFor(input){
  const value=input&&Object.hasOwn(input,'baselines')?input.baselines:{};
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length>4096)throw Error(uiText("Invalid game baselines"));
  for(const [key,entry]of Object.entries(value))if(!key||!entry||typeof entry!=='object'||Array.isArray(entry)||Object.keys(entry).sort().join(',')!=='country,fact_key,quantity,source,value'||['country','fact_key','source'].some(field=>typeof entry[field]!=='string'||!entry[field])||!integer(entry.value)||!integer(entry.quantity,1))throw Error(uiText("Invalid game baseline"));
  return value;
 }
 function fortificationContext(guide,state){
  const facts={...state.facts},details=new Map();
  const value=(key,observed=false)=>{const f=c.fact(state,key);return state.usable&&f.source&&(observed?f.status==='observed':['observed','calculated'].includes(f.status))?f.value:null;};
  for(const rows of Object.values(guide.tracks))for(const step of rows){
   const p=step.action.params||{},prefix='guide.fortifications.'+step.key,completion=step.completion;
   if(!['bunker','coastal_bunker'].includes(p.building_id)||!completion||Object.keys(completion).length!==4||completion.op!=='fact'||completion.key!==prefix+'.completed'||completion.test!=='eq'||completion.value!==true)continue;
   const selector=p.province_selector&&typeof p.province_selector==='object'?p.province_selector:{};
   const ids=selector.kind==='border'?value('fortifications.borders.'+(selector.country||'')):(p.province_ids??(Object.hasOwn(p,'province_id')?[p.province_id]:null));
   const target=p.target_level,valid=Array.isArray(ids)&&ids.length>0&&ids.length<=512&&ids.every(n=>Number.isSafeInteger(n)&&n>0&&n<=100000)&&new Set(ids).size===ids.length&&Number.isSafeInteger(target)&&target>0&&target<=1000;
   const levels=valid?ids.map(n=>value('country.fortifications.items.'+n+'.'+p.building_id,true)):[],observed=valid&&levels.every(n=>Number.isSafeInteger(n)&&n>=0&&n<=1000);
   const done=observed?levels.every(n=>n>=target):null,queued=value('construction.queued'),queueKnown=Array.isArray(queued)&&queued.every(n=>typeof n==='string');
   const started=valid&&queueKnown?(done===true||ids.some(n=>queued.includes('province:'+n+':'+p.building_id))):(done===true?true:null);
   for(const [suffix,val]of [['started',started],['completed',done]])facts[prefix+'.'+suffix]=val===null?unknown('Provincial fortification evidence unavailable'):calculated(val,'fortification_goal:'+step.key);
   details.set(step.key,{target_level:Number.isSafeInteger(target)&&target>0&&target<=1000?target:null,province_ids:valid?[...ids]:[],completed_provinces:observed?levels.filter(n=>n>=target).length:null,total_provinces:valid?ids.length:null,building_id:p.building_id});
  }
  return {state:{...state,facts},details};
 }
 function constructionContext(guide,state,baselines){
  const facts={...state.facts},details=new Map(),country=c.fact(state,'country').value;
  for(const rows of Object.values(guide.tracks))for(const step of rows){
   const p=step.action.params||{},provincial=['bunker','coastal_bunker'].includes(p.building_id)&&integer(p.province_id,1),field=provincial?p.building_id:Object.hasOwn(buildingFacts,p.building_id)?buildingFacts[p.building_id]:null;
   if(!['manual','special_project'].includes(step.action.kind)||!field||p.category!=='construction'||p.quantity_mode!=='additional'||!provincial&&!integer(p.state_id,1)||!integer(p.quantity,1))continue;
   const key='guide.construction.'+step.key+'.added',condition=step.completion;
   if(!condition||condition.op!=='fact'||condition.key!==key||condition.test!=='gte'||condition.value!==p.quantity||Object.keys(condition).length!==4)continue;
   const factKey=provincial?'country.fortifications.items.'+p.province_id+'.'+field:'country.states.items.'+p.state_id+'.'+field,entry=baselines[step.key],current=c.fact(state,factKey);
   const valid=entry&&entry.country===country&&entry.fact_key===factKey&&entry.quantity===p.quantity&&integer(entry.value+p.quantity);
   const count=current.status==='observed'&&integer(current.value)?current.value:null;
   facts[key]=valid&&count!==null?calculated(count-entry.value,'run_baseline:'+step.key):unknown(uiText("Baseline cannot be verified"));
   details.set(step.key,{baseline:valid?entry.value:null,current:count,target:valid?entry.value+p.quantity:null});
   if(provincial){const target=details.get(step.key).target;details.set(step.key,{target_level:target,province_ids:[p.province_id],completed_provinces:count!==null&&target!==null?Number(count>=target):null,total_provinces:1,building_id:p.building_id});}
  }
  const fortified=fortificationContext(guide,{...state,facts});for(const [key,value]of fortified.details)details.set(key,value);
  return {state:fortified.state,details};
 }
 function actionDomain(step){const p=step.action.params||{};if(step.tracking==='reminder')return null;if(step.action.kind==='special_project')return 'project';if(p.commander_id||['corps_commander','navy_leader'].includes(p.role))return 'appointment';return Object.hasOwn(step,'political_power_cost')||['law_id','law_name','idea_id','idea_name','advisor_id','person_id','person_name','trait_name','role','decision_id','decision_name'].some(k=>p[k])?'political':null;}
 function actionReadiness(step,state,checks){
  const domain=actionDomain(step);if(!domain)return null;
  const missing=[],blocked=[],resources=[],affordability=[],value={domain,unknown:missing,blocked,resources,requirements_declared:Object.hasOwn(step,'requirements')};
  if(domain==='political'&&!step.action.params?.cost_currency||Object.hasOwn(step,'political_power_cost')){
   const cost=step.political_power_cost??null,f=c.fact(state,'country.politics.political_power'),amount=typeof f.value==='number'&&Number.isFinite(f.value)?f.value:null;
   const affordable=cost===null||amount===null?null:amount>=cost;
   value.political_power={cost,available:amount,affordable,cost_basis:cost===null?'unknown':'guide_declared'};
   if(cost===null)missing.push('political_cost_unknown');if(amount===null)missing.push('political_power_unknown');if(affordable===false)blocked.push('insufficient_political_power');
   affordability.push(affordable);
  }
  const costs=step.resource_costs;
  if(domain==='project'&&costs===undefined){missing.push('project_cost_unknown');affordability.push(null);}
  for(const [key,cost] of Object.entries(costs||{})){
   const f=c.fact(state,key),amount=typeof f.value==='number'&&Number.isFinite(f.value)?f.value:null,affordable=amount===null?null:amount>=cost;
   resources.push({key,cost,available:amount,affordable});affordability.push(affordable);
   if(affordable===false&&!blocked.includes('insufficient_resource'))blocked.push('insufficient_resource');
   else if(affordable===null&&!missing.includes('resource_balance_unknown'))missing.push('resource_balance_unknown');
  }
  value.resources_available=affordability.includes(false)?false:affordability.includes(null)?null:true;
  if(Object.hasOwn(step,'requirements')){const eligible=c.condition(step.requirements,state,checks);if(eligible===false)blocked.push('action_requirements_false');else if(eligible===null)missing.push('action_requirements_unknown');}
  else missing.push('action_requirements_unknown');
  if(domain==='appointment')missing.push('commander_assignment_unobserved');
  if(domain==='project')missing.push('facility_compatibility_unknown');
  value.value=blocked.length?false:missing.length?null:true;return value;
 }
 function compareIds(a,b){
  const left=Array.from(a,c=>c.codePointAt(0)),right=Array.from(b,c=>c.codePointAt(0));
  for(let i=0;i<Math.min(left.length,right.length);i++)if(left[i]!==right[i])return left[i]-right[i];
  return left.length-right.length;
 }
 function validateCampaignObservations(value){
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length&&(Object.keys(value).some(k=>!['country','war_seen','milestones'].includes(k))||typeof value.country!=='string'||!/^[A-Z0-9]{3}$/.test(value.country)||Object.hasOwn(value,'war_seen')&&value.war_seen!==true||!Object.hasOwn(value,'war_seen')&&(!value.milestones||Object.keys(value.milestones).length===0)||Object.hasOwn(value,'milestones')&&(!value.milestones||typeof value.milestones!=='object'||Array.isArray(value.milestones)||Object.keys(value.milestones).length<1||Object.keys(value.milestones).length>4096||Object.entries(value.milestones).some(([k,v])=>[...k].length<1||[...k].length>2048||v!==true))))throw Error('Osservazioni campagna non valide');
 }
 function campaignContext(state,observations){
  const facts={...state.facts,'guide.campaign.after_war':unknown('Guerra o avanzamento campagna non qualificati')};
  if(observations!==null){
   validateCampaignObservations(observations);
   const war=state.facts['country.diplomacy.at_war'],country=state.facts.country;
   if(state.usable&&['observed','calculated'].includes(country?.status)&&country.source&&observations.country===country.value&&observations.milestones)facts['guide.campaign.milestones']=calculated(Object.keys(observations.milestones).sort(),'campaign_milestone_adapter');
   if(state.usable&&war?.status==='observed'&&war.source&&typeof war.value==='boolean'&&['observed','calculated'].includes(country?.status)&&country.source&&typeof country.value==='string'&&/^[A-Z0-9]{3}$/.test(country.value))facts['guide.campaign.after_war']=calculated(!war.value&&observations.country===country.value&&observations.war_seen===true,'campaign_condition_adapter');
  }
  return {...state,facts};
 }
 function progressFor(guide,state,input){
  if(input===null||input===undefined)return {status:'none',run_id:null,checks:{},started:{}};
  if(typeof input!=='object'||Array.isArray(input)||input.schema_version!==1||typeof input.guide_id!=='string'||typeof input.run_id!=='string'||!input.run_id.trim()||[...input.run_id].length>160||input.session_id!==undefined&&input.session_id!==null&&(typeof input.session_id!=='string'||!input.session_id.trim()))throw Error(uiText("Invalid guide progress"));
  const provided=Object.hasOwn(input,'checks')?input.checks:{};
  if(!provided||typeof provided!=='object'||Array.isArray(provided)||Object.values(provided).some(x=>typeof x!=='boolean'))throw Error(uiText("Invalid game confirmations"));
  const started=Object.hasOwn(input,'started')?input.started:{};
  if(!started||typeof started!=='object'||Array.isArray(started)||Object.values(started).some(x=>typeof x!=='boolean'))throw Error(uiText("Invalid game start confirmations"));
  validateCampaignObservations(Object.hasOwn(input,'campaign_conditions')?input.campaign_conditions:{});
  if(input.guide_id!==(guide.id||''))return {status:'wrong_guide',run_id:null,checks:{},started:{}};
  if(input.campaign_required&&!input.campaign_id)return {status:'campaign_unverified',run_id:null,checks:{},started:{}};
  if(input.campaign_id!==undefined&&input.campaign_id!==null){
   if(typeof input.campaign_id!=='string'||!/^hoi4:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}:[0-9a-f]{8}$/.test(input.campaign_id)||input.campaign_id.split(':')[1]==='00000000-0000-0000-0000-000000000000')throw Error(uiText("Invalid game campaign"));
   const observed=state.facts['campaign.id'];
   if(!state.usable||observed?.status!=='observed'||!observed.source)return {status:'campaign_unverified',run_id:null,checks:{},started:{}};
   if(observed.value!==input.campaign_id||input.country!==c.fact(state,'country').value)return {status:'wrong_campaign',run_id:null,checks:{},started:{}};
  }
  if(input.session_id&&!state.usable)return {status:'session_unverified',run_id:null,checks:{},started:{}};
  if(input.session_id&&input.session_id!==state.session_id)return {status:'wrong_session',run_id:null,checks:{},started:{}};
  const operations=new Set(Object.values(guide.tracks).flat().filter(step=>['manual','special_project'].includes(step.action.kind)&&step.tracking!=='reminder').map(step=>step.key));
  const allowed=new Set((guide.milestones||[]).map(x=>x.key));
  for(const rows of Object.values(guide.tracks))for(const step of rows){
   if(['manual','special_project'].includes(step.action.kind)&&step.tracking!=='reminder')allowed.add(step.key);
   for(const field of ['when','priority_when','skip_when','completion','started_when'])for(const key of c.checkKeys(step[field]))allowed.add(key);
  }
  for(const phase of guide.phases||[])for(const key of c.checkKeys(phase.when))allowed.add(key);
  return {status:'accepted',run_id:input.run_id,checks:Object.fromEntries(Object.entries(provided).filter(([key])=>allowed.has(key))),started:Object.fromEntries(Object.entries(started).filter(([key])=>operations.has(key)))};
 }
 function evaluate(input,value,progress=null){
  let guide=c.guide(input);const state=c.state(value),scope=guide.applies_to;
  const country=c.fact(state,'country').value,tree=c.fact(state,'focus.tree').value;
  const mode=!state.usable?'preview':scope.country&&country!==null&&country!==scope.country?'wrong_country':scope.tree_id&&tree!==null&&tree!==scope.tree_id?'wrong_tree':scope.country&&country===null||scope.tree_id&&tree===null?'preview':'live';
  const run=progressFor(guide,state,progress),baselines=baselinesFor(progress);
  const bindings=run.status==='accepted'&&progress?.research_bindings&&typeof progress.research_bindings==='object'?progress.research_bindings:{};
  for(const rows of Object.values(guide.tracks))for(const step of rows)if(step.research_choice){const selected=bindings[step.key],valid=Object.values(step.research_choice.branches).some(chain=>chain.includes(selected));step.action.entity.id=valid?selected:'unbound-choice:'+step.key;step.target=step.action.entity.id;if(valid)delete step.catalog_match;else step.catalog_match=false;}
  for(const rows of Object.values(guide.tracks))for(const step of rows)if(step.relative_research||step.relative_research_unresolved){const selected=bindings[step.key],valid=step.relative_research?.chain.includes(selected);if(valid&&step.action.entity.id.startsWith('unbound-up:'))delete step.catalog_match;step.action.entity.id=valid?selected:'unbound-up:'+step.key;step.target=step.action.entity.id;if(!valid)step.catalog_match=false;}
  const acceptedBaselines=run.status==='accepted'?baselines:{},campaign=campaignContext(mode==='live'?state:{...state,usable:false},run.status==='accepted'?progress.campaign_conditions||{}:null),construction=constructionContext(guide,campaign,acceptedBaselines),assignments=production.context(guide,construction.state,acceptedBaselines),context=assignments.state;
  const choices=researchChoices(guide,run.status==='accepted'?context:{...context,usable:false},bindings,run.status==='accepted'?progress?.research_options||{}:{});
  const researchProgress=researchProgression(guide,context);
  const checks=['wrong_country','wrong_tree'].includes(mode)?{}:run.checks;
  const phases=new Map((guide.phases||[]).map(x=>[x.id,x]));
  const condition=node=>c.condition(node,context,checks);
  const membership=(key,target)=>{const f=c.fact(context,key);return f.status==='unknown'||!Array.isArray(f.value)?unknown():calculated(f.value.includes(target),'membership:'+key);};
  const definitions=new Map(),results=new Map(),indices=new Map(),barriers=new Map();let index=0;
  const trackEntries=Object.entries(guide.tracks).sort(([a],[b])=>compareIds(a,b));
  for(const [track,rows]of trackEntries){
   const manual=[];
   for(const step of rows){
    definitions.set(step.key,step);indices.set(step.key,index++);barriers.set(step.key,guide.dependency_policy==='explicit'||Object.hasOwn(step,'depends_on')||Object.hasOwn(step,'depends_on_started')?[]:[...manual]);
    const kind=step.action.kind,isNative=['select_focus','research'].includes(kind),isFocus=kind==='select_focus',isReminder=step.tracking==='reminder',continuous=isFocus&&step.action.params?.focus_mode==='continuous';
    let completion=unknown(),basis='free';
    if(isReminder){completion=unknown('not_tracked');basis='reminder';}
    else if(Object.hasOwn(step,'completion')){completion=calculated(condition(step.completion),'declared_completion');basis='condition';}
    else if(['manual','special_project'].includes(kind)){completion=calculated(Object.hasOwn(checks,step.key)?checks[step.key]:null,'check:'+step.key);basis='manual';}
    else if(isNative){completion=continuous?calculated(false,'continuous_focus_has_no_completion'):membership(isFocus?'focus.completed':'research.completed',step.target);basis='native_membership';}
    const selection=isReminder?unknown():isNative?membership(continuous?'focus.continuous':isFocus?'focus.active':'research.active',step.target):Object.hasOwn(step,'started_when')?calculated(condition(step.started_when),'declared_start'):['manual','special_project'].includes(kind)&&Object.hasOwn(run.started,step.key)?calculated(run.started[step.key],'manual_started:'+step.key):unknown();
    const priorityValue=step.priority_when?condition(step.priority_when):true;
    results.set(step.key,{key:step.key,track,status:'unverifiable',reason:'not_evaluated',blocked_by:[],completion,selection,completion_basis:basis,priority:priorityValue===true?(step.priority||0):0,priority_known:priorityValue!==null,availability:unknown('dynamic_availability_not_evaluated')});
    const choice=choices.get(step.key);if(choice||step.relative_research)results.get(step.key).research_target=step.target;if(choice&&step.target.startsWith('unbound-choice:')){const row=results.get(step.key);row.research_options=choice.options;row.research_choice_status=choice.status;if(choice.options.length){if(!Object.hasOwn(step,'completion'))row.completion=calculated(false,'unselected_research_choice');row.selection=calculated(false,'unselected_research_choice');}}
    const readiness=actionReadiness(step,context,checks);if(readiness){const row=results.get(step.key);row.action_readiness=readiness;row.availability=readiness.value===null?unknown(readiness.unknown.join(',')):calculated(readiness.value,'declared_action_requirements');}
    if(step.action.kind==='select_focus'&&step.focus_criteria){const row=results.get(step.key),details=focusCriteria.assess(step.focus_criteria,context);row.focus_readiness=details;row.availability=details.value===null?unknown(details.reason):calculated(details.value,'game_focus_criteria');}
    if(isReminder)results.get(step.key).tracking='reminder';
    if(construction.details.has(step.key)){const detail=construction.details.get(step.key);results.get(step.key)[Object.hasOwn(detail,'province_ids')?'fortifications':'construction']=detail;}
    if(assignments.details.has(step.key))results.get(step.key).production=assignments.details.get(step.key);
    if(guide.dependency_policy!=='explicit'&&kind==='manual'&&!isReminder)manual.push(step.key);
   }
  }
  const indegree=new Map(),successors=new Map([...definitions.keys()].map(key=>[key,[]]));
  for(const [key,step]of definitions){const deps=[...step.depends_on||[],...step.depends_on_started||[]];indegree.set(key,deps.length);for(const dep of deps)successors.get(dep).push(key);}
  const queue=[...definitions.keys()].filter(key=>indegree.get(key)===0);
  for(let cursor=0;cursor<queue.length;cursor++){
   const key=queue[cursor],step=definitions.get(key),row=results.get(key),kind=step.action.kind;
   const native=['select_focus','research'].includes(kind),entityScope=kind==='select_focus'?step.action.entity.scope:null;
   const set=(status,reason)=>{row.status=status;row.reason=reason;};
   const testGate=(node,falseReason,unknownReason)=>{const result=condition(node);if(result!==true){set(result===false?'waiting':'unverifiable',result===false?falseReason:unknownReason);return false;}return true;};
   if(['wrong_country','wrong_tree'].includes(mode)){row.completion=unknown(step.tracking==='reminder'?'not_tracked':'not_observed');row.selection=unknown();set('not_applicable','guide_scope_mismatch');}
   else if(entityScope&&tree!==null&&tree!==entityScope){row.completion=unknown();row.selection=unknown();set('not_applicable','entity_scope_mismatch');}
   else if(entityScope&&tree===null){row.completion=unknown();row.selection=unknown();set('unverifiable','entity_scope_unknown');}
   else if(step.research_choice&&step.target.startsWith('unbound-choice:')&&!row.research_options?.length)set(row.research_choice_status==='waiting'?'waiting':'unverifiable','research_choice_'+row.research_choice_status);
   else if((step.relative_research||step.relative_research_unresolved)&&step.target.startsWith('unbound-up:')){const reason=researchProgress.get(key)||'research_binding_unknown';set(reason==='research_line_complete'?'not_applicable':reason==='research_previous_level_incomplete'?'waiting':'unverifiable',reason);}
   else if(row.completion.value===true)set('completed','completion_true');
   else if(row.selection.value===true)set('active','selection_true');
   else if(native&&mode!=='live')set('unverifiable','live_state_unavailable');
   else if(step.skip_when&&condition(step.skip_when)!==false)set(condition(step.skip_when)===true?'not_applicable':'unverifiable',condition(step.skip_when)===true?'skip_condition_true':'skip_condition_unknown');
   else if(step.phase&&phases.get(step.phase).when&&!testGate(phases.get(step.phase).when,'phase_condition_false','phase_condition_unknown')){}
   else if(step.when&&!testGate(step.when,'condition_false','condition_unknown')){}
   else if(step.catalog_match===false&&!row.research_options?.length)set('unverifiable','catalog_unmatched');
   else{
    const deps=step.depends_on||[];
    const explicit=deps.filter(dep=>results.get(dep).status!=='completed');
    const started=(step.depends_on_started||[]).filter(dep=>results.get(dep).selection.value!==true&&results.get(dep).completion.value!==true);
    const implicit=barriers.get(key).filter(dep=>results.get(dep).completion.value!==true);
    row.blocked_by=[...new Set([...explicit,...started,...implicit])];
    if(row.blocked_by.length){
     const missing=started.some(dep=>results.get(dep).selection.value===null)||explicit.some(dep=>results.get(dep).status==='unverifiable')||implicit.some(dep=>results.get(dep).completion.value===null);
     set(missing?'unverifiable':'waiting',missing?'dependency_unknown':'dependency_not_completed');
    }else if(row.action_readiness?.blocked.length)set('waiting',row.action_readiness.blocked[0]);
    else if(row.focus_readiness?.value===false)set('waiting',row.focus_readiness.reason);
    else if(researchProgress.get(key)==='research_previous_level_incomplete')set('waiting','research_previous_level_incomplete');
    else if(native&&row.selection.value===null)set('unverifiable','selection_unknown');
    else if(step.tracking==='reminder')set('ready','reminder_available');
    else if(row.completion.value===null&&kind!=='free')set('unverifiable',['manual','special_project'].includes(kind)?'manual_confirmation_required':'completion_unknown');
    else set('ready',kind==='free'?'free_choice':'prerequisites_satisfied');
   }
   if(step.action.params?.category==='production'&&step.tracking!=='reminder'){row.production_admitted=mode==='live';if(mode==='live'){const denied=production.admission(step,phases,context,results,checks);if(denied)production.deny(row,denied);}}
   if(mode==='live'&&['improve_relations','stop_improving_relations','non_aggression_pact','assign_operative_mission'].includes(step.action.params?.operation_id)){
    const history=step.completion?.op==='any'?step.completion.args.find(n=>n.op==='fact'&&n.key==='guide.campaign.milestones'&&typeof n.value==='string'&&n.value.startsWith('foreign:')):null;
    if(!history||condition(history)!==true){const denied=production.admission(step,phases,context,results,checks);if(denied)Object.assign(row,denied,{completion:unknown('interaction_not_admitted'),selection:unknown('interaction_not_admitted')});}
   }
   for(const other of successors.get(key)){indegree.set(other,indegree.get(other)-1);if(indegree.get(other)===0)queue.push(other);}
  }
  production.applyRevisions(guide,context,results,checks);
  const compare=(a,b)=>results.get(b).priority-results.get(a).priority||indices.get(a)-indices.get(b);
  const steps=[...results.values()],tracks={},candidates=new Set();
  for(const [track,rows]of trackEntries){
   const keys=rows.map(x=>x.key),order=[...keys].sort(compare),next=order.find(key=>definitions.get(key).tracking!=='reminder'&&!definitions.get(key).action.params?.focus_optional&&!settled(results.get(key))&&results.get(key).status!=='active')||null;
   Object.defineProperty(tracks,track,{value:{keys,order,next},enumerable:true});
   for(const key of order){const row=results.get(key);if(settled(row)||row.status==='active')continue;if(definitions.get(key).tracking==='reminder'||definitions.get(key).action.params?.focus_optional||definitions.get(key).action.kind==='special_project')continue;if(row.status!=='ready'){if(['dependency_not_completed','dependency_unknown','research_previous_level_incomplete'].includes(row.reason))continue;break;}candidates.add(key);}
  }
  const focus=c.fact(context,'focus.active'),slots=c.fact(context,'research.slots');
  const capacity={focus:Array.isArray(focus.value)?(focus.value.length?0:1):null,research:Array.isArray(slots.value)?slots.value.filter(x=>!x.technology_id).length:null};
  const ready=steps.filter(x=>x.status==='ready'&&x.tracking!=='reminder').map(x=>x.key).sort(compare),parallel={};
  for(const [lane,kind]of [['focus','select_focus'],['research','research']]){
   const seen=new Set(),selected=[];
   for(const key of ready){
    if(capacity[lane]===null||selected.length>=capacity[lane])break;
    if(!candidates.has(key)||definitions.get(key).action.kind!==kind||definitions.get(key).action.params?.focus_optional)continue;
    const ref=definitions.get(key).action.entity,identity=JSON.stringify([ref.scope??null,ref.id]);
    if(!seen.has(identity)){seen.add(identity);selected.push(key);}
   }
   parallel[lane]={capacity:capacity[lane],steps:selected};
  }
  if([...definitions.values()].some(s=>s.action.params?.focus_optional))parallel.focus.optional_steps=ready.filter(key=>definitions.get(key).action.params?.focus_optional&&capacity.focus>0);
  const focusPlan=steps.some(s=>s.focus_readiness)?focusCriteria.alternatives(trackEntries.flatMap(([track,rows])=>rows.map(s=>[track,s])),results,tracks,capacity.focus):null;
  if(focusPlan)parallel.focus.alternative_steps=focusPlan.steps;
  return {schema_version:1,guide_id:guide.id||'',run_id:run.run_id,progress_status:run.status,mode,country,steps,tracks,ready,parallel,...(focusPlan?{focus_plan:focusPlan}:{}),action_context:{session_id:state.session_id??null,campaign_id:c.fact(state,'campaign.id').value},action_sequence:{political:[...definitions].find(([key,step])=>actionDomain(step)==='political'&&!settled(results.get(key)))?.[0]||null,projects:[...definitions].filter(([key,step])=>actionDomain(step)==='project'&&!settled(results.get(key))).map(([key])=>key)}};
 }
 const api={evaluate};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiGuideRuntime=api;
})(typeof window!=='undefined'?window:globalThis);
