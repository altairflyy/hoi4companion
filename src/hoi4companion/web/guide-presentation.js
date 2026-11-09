'use strict';
// Pure projection shared by the two trees, operational walkthrough and HUD.
((root)=>{
 const uiText=typeof module!=='undefined'&&module.exports?require('./i18n.js').t:root.HoiI18n.t;
 const model=typeof module!=='undefined'&&module.exports?require('./guide-model.js'):root.HoiGuideModel;
 const contracts=typeof module!=='undefined'&&module.exports?require('./contracts.js'):root.HoiContracts;
 const native=step=>['select_focus','research'].includes(step.action.kind);
 function folder(step){try{return new URL(step?.href||'', 'http://local').searchParams.get('folder');}catch{return null;}}
 function frameResolver(guide){
  // Same selection policy as guide_frames.frame_reference. Media metadata only.
  const validTime=value=>Number.isFinite(value)&&value>=0&&value<14400;
  function videoUrl(source){try{
   const u=new URL(source?.url||'');
   if(u.protocol!=='https:'||u.username||u.password||u.port||/^https:\/\/[^/?#]*:/i.test(source?.url||'')||!['youtube.com','www.youtube.com','m.youtube.com','youtu.be'].includes(u.hostname))return null;
   let id=u.hostname==='youtu.be'?u.pathname.replace(/^\/+|\/+$/g,''):u.searchParams.get('v');
   if(u.pathname.startsWith('/shorts/')||u.pathname.startsWith('/live/'))id=u.pathname.split('/')[2];
   return /^[A-Za-z0-9_-]{11}$/.test(id||'')?'https://www.youtube.com/watch?v='+id:null;
  }catch{return null;}}
  const sources=new Map((guide.sources||[]).map(s=>[s.id,s]));
  const fallback=new Set([...sources.values(),guide.source].map(videoUrl).filter(Boolean)),linked=new Map();
  for(const item of guide.source_inventory?.items||[]){
   if(item.excluded_reason||!validTime(item.timestamp_seconds))continue;
   for(const key of item.step_keys||[]){if(!linked.has(key))linked.set(key,[]);linked.get(key).push(item);}
  }
  function select(items,seconds=null){
   let urls=new Set();
   for(const item of items){const source=sources.get(item.source_id),url=videoUrl(source);if(source?.url&&!url)return null;if(url)urls.add(url);}
   if(!urls.size)urls=fallback;
   return urls.size===1&&items.length?{url:[...urls][0],seconds:seconds??items[0].timestamp_seconds}:null;
  }
  return step=>{
   const direct=(step.evidence||[]).filter(e=>validTime(e.timestamp_seconds)),inventory=linked.get(step.key)||[];
   const params=step.action?.params||{},all=[...direct,...inventory];
   if(Object.hasOwn(params,'source_seconds')){
    const seconds=params.source_seconds;if(!validTime(seconds))return null;
    const matching=all.filter(e=>e.timestamp_seconds===seconds);
    return select(matching.length?matching:all.length?all:[{timestamp_seconds:seconds}],seconds);
   }
   for(const items of [direct.filter(e=>e.basis==='visual'),inventory.filter(e=>e.basis==='visual'),direct,inventory])if(items.length)return select(items);
   return null;
  };
 }
 const frameReference=(guide,step)=>frameResolver(guide)(step);
 function chronologicalOperations(operations,tracks,guide){
  // Display order only. Never turn a date/frame into a runtime condition.
  const sources=new Map((guide.sources||[]).map(s=>[s.id,s]));
  function sourceClock(source){
   if(!source?.url)return null;
   try{const u=new URL(source.url),video=u.searchParams.get('v')||(u.hostname==='youtu.be'?u.pathname.slice(1):null);
    return video&&['youtube.com','www.youtube.com','m.youtube.com','youtu.be'].includes(u.hostname)?'youtube:'+video:source.url;
   }catch{return source.url;}
  }
  const videos=new Set([sourceClock(guide.source),...(guide.sources||[]).filter(s=>s.type==='youtube').map(sourceClock)].filter(Boolean));
  function clock(row){
   const evidence=row.evidence||[],matching=evidence.filter(e=>e.timestamp_seconds===row.source_seconds);
   const clocks=new Set((matching.length?matching:evidence).map(e=>sourceClock(sources.get(e.source_id))||
    (videos.size===1?[...videos][0]:e.source_id)).filter(Boolean));
   if(clocks.size===1)return [...clocks][0];
   if(clocks.size>1)return null;
   return videos.size===1?[...videos][0]:videos.size===0&&sources.size<=1?'guide':null;
  }
  const date=row=>row.game_date?row.game_date.split('.').reduce((n,v)=>n*100+Number(v),0):null;
  const clocks=new Map(operations.filter(r=>Number.isFinite(r.source_seconds)).map(r=>[r.key,clock(r)]));
  const word=row=>{const p=row.action?.params?.source_action;return p&&sources.get(p.source_id)?.type==='file'&&Number.isInteger(p.ordinal)&&p.ordinal>0&&p.ordinal<=4096?p:null;};
  const placed=row=>date(row)!==null||clocks.get(row.key)!=null||word(row)!==null;
  const visible=new Map(operations.map((r,i)=>[r.key,{row:r,index:i,rank:placed(r)?0:1}]));
  const graph=new Map(),indegree=new Map(),nodes=new Map();
  function node(key){if(!nodes.has(key)){nodes.set(key,visible.get(key)||{index:nodes.size,rank:-1});graph.set(key,new Set());indegree.set(key,0);}}
  for(const track of Object.values(tracks))for(const row of track.rows)node(row.key);
  for(const row of operations)node(row.key);
  const details={unplaced:operations.filter(r=>!placed(r)).map(r=>r.key),conflicts:[],conflict_count:0};
  // Positive reachability remains true as edges are added; keep a bounded
  // memo so long reversed dependency chains do not require repeated scans.
  const reachable=new Map();let reachableCount=0;
  function reaches(start,target){
   const stack=[start],seen=new Set();
   while(stack.length){const key=stack.pop();if(key===target||reachable.get(target)?.has(key)){
    if(reachableCount<16384){if(!reachable.has(target))reachable.set(target,new Set());const entries=reachable.get(target);if(!entries.has(start)){entries.add(start);reachableCount++;}}
    return true;
   }if(seen.has(key))continue;seen.add(key);for(const child of graph.get(key))stack.push(child);}
   return false;
  }
  function edge(before,after,basis=null,anchors=null,report=true){
   if(!nodes.has(before)||!nodes.has(after))return false;
   if(graph.get(before).has(after))return true;
   if(basis){
    if(reaches(after,before)){if(report){details.conflict_count++;if(details.conflicts.length<32)details.conflicts.push({basis,before:anchors||[before],after});}return false;}
   }
   graph.get(before).add(after);indegree.set(after,indegree.get(after)+1);
   return true;
  }
  for(const track of Object.values(tracks))for(const row of track.rows)
   for(const key of [...(row.depends_on||[]),...(row.depends_on_started||[])])edge(key,row.key);
  function temporal(rows,value,basis){
   const groups=[];
   for(const row of [...rows].sort((a,b)=>value(a)-value(b)||visible.get(a.key).index-visible.get(b.key).index)){
    if(!groups.length||groups.at(-1).value!==value(row))groups.push({value:value(row),keys:[]});groups.at(-1).keys.push(row.key);
   }
   // A barrier between equal-time groups avoids quadratic all-to-all edges.
   let carried=new Set();
   for(let i=1;i<groups.length;i++){
    const previous=groups[i-1].keys,barrier=Symbol(basis);node(barrier);
    for(const key of previous)edge(key,barrier);
    const remaining=new Set();
    for(const key of groups[i].keys){
     // Keep unaffected members of a timestamp group when one conflicts with
     // a declared dependency or higher-precedence campaign date.
     if(!edge(barrier,key,basis,previous))for(const earlier of previous)
      if(!edge(earlier,key,basis,null,false))remaining.add(earlier);
     // A rejected edge must not erase the source ordering against later
     // groups: try each deferred predecessor again until it can be placed.
     for(const earlier of carried)if(!edge(earlier,key,basis,null,false))remaining.add(earlier);
    }
    // Keep only maximal deferred nodes: if X already precedes Y, carrying Y
    // preserves X's order too. This prevents reversed dependency chains from
    // accumulating a quadratic backlog of redundant rejected edges.
    const reduced=new Set();
    for(const earlier of remaining){let redundant=false;for(const other of reduced){
     if(reaches(earlier,other)){redundant=true;break;}
     if(reaches(other,earlier))reduced.delete(other);
    }if(!redundant)reduced.add(earlier);}
    carried=reduced;
   }
  }
  temporal(operations.filter(r=>date(r)!==null),date,'game_date');
  const timelines=new Map();
  for(const row of operations){const id=clocks.get(row.key);if(id!=null){if(!timelines.has(id))timelines.set(id,[]);timelines.get(id).push(row);}}
  for(const rows of timelines.values())temporal(rows,r=>r.source_seconds,'source_time');
  const documents=new Map();
  for(const row of operations){const p=word(row);if(p){if(!documents.has(p.source_id))documents.set(p.source_id,[]);documents.get(p.source_id).push(row);}}
  for(const rows of documents.values())temporal(rows,r=>word(r).ordinal,'source_action');
  const heap=[],compare=(a,b)=>nodes.get(a).rank-nodes.get(b).rank||nodes.get(a).index-nodes.get(b).index;
  function push(key){let i=heap.length;heap.push(key);while(i){const parent=(i-1)>>1;if(compare(heap[parent],key)<=0)break;heap[i]=heap[parent];i=parent;}heap[i]=key;}
  function pop(){const first=heap[0],last=heap.pop();if(heap.length){let i=0;while(2*i+1<heap.length){let c=2*i+1;if(c+1<heap.length&&compare(heap[c+1],heap[c])<0)c++;if(compare(last,heap[c])<=0)break;heap[i]=heap[c];i=c;}heap[i]=last;}return first;}
  for(const key of nodes.keys())if(indegree.get(key)===0)push(key);
  const ordered=[],emitted=new Set();let visited=0;
  while(heap.length){const key=pop();visited++;if(visible.has(key)){ordered.push(visible.get(key).row);emitted.add(key);}
   for(const child of graph.get(key)){indegree.set(child,indegree.get(child)-1);if(indegree.get(child)===0)push(child);}
  }
  if(visited!==nodes.size){details.conflict_count++;details.conflicts.push({basis:'dependency',before:[],after:null});
   // Legacy implicit cycles remain visibly unresolved, never silently dropped.
   ordered.push(...operations.filter(r=>!emitted.has(r.key)));
  }
  return {rows:ordered,details};
 }
 function evaluate(guide,state=null,progress={},evaluation=null){
  const resolveFrame=frameResolver(guide);
  const result=model.evaluate(guide,state,{...progress,evaluation}),all=Object.values(result.tracks).flatMap(t=>t.rows),lanes={};
  const scopeOK=!result.mode.startsWith('wrong_'),normalised=contracts.state(state);
  for(const [lane,kind]of [['focus','select_focus'],['research','research']]){
   const track=result[lane],next=track.next,markers=[];
   if(scopeOK){
    const ready=new Set(result.runtime.parallel[lane].steps);
    const add=(row,tone)=>{if(!row||!native(row))return;
     if(row.research_options?.length){for(const id of row.research_options){if(markers.some(m=>m.id===id))continue;const option={...row,...row.choice_entities?.[id],target:id,action:{...row.action,entity:{...row.action.entity,id}}};markers.push({id,key:row.key,scope:null,tone:row.research_options.length>1||row.research_choice?.mode==='once'?'choice':tone,row:option});}return;}
     if(row.research_choice&&row.target.startsWith('unbound-choice:')||markers.some(m=>m.id===row.target))return;
     markers.push({id:row.target,key:row.key,scope:row.action.entity.scope||null,tone,row});};
    for(const row of all.filter(r=>r.action.kind===kind&&r.runtime_status==='active'))add(row,'active');
    for(const key of ready)add(all.find(r=>r.key===key),'next');
    for(const key of result.runtime.parallel[lane].optional_steps||[])add(all.find(r=>r.key===key),'choice');
    for(const key of result.runtime.parallel[lane].alternative_steps||[])add(all.find(r=>r.key===key),'alternative');
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
   const actionPlan=typeof module!=='undefined'&&module.exports?require('./action-plan.js'):root.HoiActionPlan;
   return {...row,frame_reference:resolveFrame(row),category:actionPlan.category(row),
    ...(staticConstruction?{construction:staticConstruction}:{}),
    ...(params.event_id?{event_observation:{visible:membership('events.visible',params.event_id),option_available:params.event_option_id?membership('events.visible_options',params.event_id+':'+params.event_option_id):null}}:{}),
    start_basis:row.started_when?(contracts.checkKeys(row.started_when).length?'condition':'live'):'manual',construction_observations:construction,
    game_date:params.game_date||null,
    timing:params.timing||uiText("At the point specified by the guide; check the conditions in the game."),
    instructions:Array.isArray(params.instructions)?params.instructions.filter(x=>typeof x==='string'):[],
    source_seconds:params.source_seconds??(row.evidence||[]).find(e=>e.basis==='visual'&&Number.isFinite(e.timestamp_seconds))?.timestamp_seconds??(row.evidence||[]).find(e=>Number.isFinite(e.timestamp_seconds))?.timestamp_seconds??null};
  });
  const existing=new Set(operations.map(row=>row.key));
  for(const row of operations)if(row.action.params?.legacy_key)existing.add(row.action.params.legacy_key);
  for(const item of guide.milestones||[])if(!existing.has(item.key))operations.push({...item,
   name:item.title,action:{kind:'manual'},category:item.category||'checkpoints',
   frame_reference:resolveFrame({key:item.key,action:{params:{source_seconds:item.seconds}}}),
   timing:item.timing||uiText("Check the timing in the source; timestamps refer to the video."),instructions:[],
   source_seconds:item.seconds??null,completion_basis:'manual',runtime_status:scopeOK&&result.runtime.progress_status==='accepted'&&progress.checks?.[item.key]===true?'completed':'unverifiable',runtime_reason:'manual_confirmation_required'});
  const ordering=chronologicalOperations(operations,result.tracks,guide);
  return {mode:result.mode,country:result.country,progress_status:result.runtime.progress_status,lanes,
   operations:ordering.rows,operation_order:ordering.details,model:result};
 }
 const api={evaluate,folder,frameReference};if(typeof module!=='undefined'&&module.exports)module.exports=api;else root.HoiGuidePresentation=api;
})(typeof window!=='undefined'?window:globalThis);
