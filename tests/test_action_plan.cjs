'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const plan=require('../src/hoi4companion/web/action-plan.js');
const fs=require('node:fs');
const row=(key,params={},extra={})=>({key,title:'Original informal wording',runtime_status:'ready',action:{kind:'manual',params},...extra});

test('compact view keeps the political sequence, active orders and upcoming waits separate',()=>{
 const rows=[row('building',{}, {runtime_status:'active'}),
  row('expensive',{}, {runtime_status:'waiting',runtime_reason:'insufficient_political_power',action_readiness:{domain:'political'}}),
  row('cheap',{}, {action_readiness:{domain:'political'}}),
  row('future',{}, {runtime_status:'waiting',runtime_reason:'dependency_not_completed'}),
  row('note',{}, {tracking:'reminder'})];
 const view=plan.compact(rows,'expensive');assert.equal(view.next.key,'expensive');
 assert.deepEqual(view.active.map(r=>r.key),['building']);assert.deepEqual(view.waiting.map(r=>r.key),['future']);
 rows.push(row('visible-event',{}, {event_observation:{visible:true}}));
 assert.equal(plan.compact(rows,'expensive').next.key,'visible-event');
 assert.equal(plan.compact([row('note',{}, {tracking:'reminder'})],null).next,null);
});

test('all guide entry pages ship the shared operational presentation dependency',()=>{
 for(const file of ['guide.html','index.html','technologies.html']){
  const html=fs.readFileSync('src/hoi4companion/web/'+file,'utf8');
  assert.ok(html.includes('src="/action-plan.js"'),file);
 }
});

test('production instructions distinguish requested quotas, remaining capacity and exact designs',()=>{
 const r=row('p',{category:'production',production_targets:[
  {equipment_name:'Infantry equipment',requested_factories:10},
  {equipment_name:'Tank',variant_name:'T-I',quantity_mode:'remaining'}]});
 const result=plan.describe(r);
 assert.match(result.instructions.join(' '),/10.*requested|requested.*10/);
 assert.match(result.instructions.join(' '),/T-I.*remaining|remaining.*T-I/);
 assert.equal(result.hud.panel,'production');
 assert.equal(result.hud.status,'unresolved'); // Names alone are not qualified selectors.
 r.action.params.production_targets.forEach(t=>t.production_selector={kind:'equipment',value:'id'});
 assert.equal(plan.describe(r).hud.status,'candidate');
});

test('construction explains start versus completion without manufacturing quantities',()=>{
 const r=row('build',{category:'construction',building_name:'Military factory',state_name:'Berlin',quantity:2,quantity_mode:'additional'},
  {started_when:{op:'fact',key:'construction.queued',test:'contains',value:'state:64:arms_factory'},completion_condition:{op:'fact',key:'country.states.items.64.military_factories',test:'gte',value:7}});
 const info=plan.describe(r);
 assert.match(info.instructions.join(' '),/2 additional.*Military factory.*Berlin/);
 assert.match(info.monitoring,/queue.*completion/i);
 assert.equal(info.hud.status,'unsupported');
});

test('HUD eligibility distinguishes blocked resources from unsupported actions and current diagnostics',()=>{
 const r=row('law',{category:'politics',law_id:'war_economy',law_name:'War Economy'},
  {action_readiness:{resources_available:false,blocked:['insufficient_political_power'],unknown:[]}});
 assert.equal(plan.describe(r).hud.status,'waiting');
 r.action_readiness.resources_available=true;r.action_readiness.blocked=[];
 assert.equal(plan.describe(r).hud.status,'candidate');
 assert.equal(plan.describe(r,{political_key:'earlier'}).hud.status,'waiting');
 assert.equal(plan.describe(r,{usable:true,guide_id:'g',expected_guide_id:'g',overlay:{status:'observed',guide_id:'g',items:[{step_keys:['law'],code:'different_panel',message:'Open politics'}]}}).hud.message,'Open politics');
 assert.notEqual(plan.describe(r,{usable:true,guide_id:'g',expected_guide_id:'g',overlay:{status:'observed',guide_id:'other',items:[{step_keys:['law'],message:'OLD'}]}}).hud.message,'OLD');
});

test('source blocks remain numbered and split conservatively around dependency reordering',()=>{
 const a=(n,key,extra={})=>row(key,{source_action:{source_id:'doc',key:'word-'+n,ordinal:n}},extra);
 const rows=[a(1,'a'),a(1,'b',{tracking:'reminder'}),a(2,'c',{runtime_status:'completed'}),a(1,'d')];
 const groups=plan.blocks(rows);
 assert.deepEqual(groups.map(g=>g.number),[1,2,1]);
 assert.equal(groups[0].rows.length,2);assert.equal(groups[0].status,'ready');
 assert.equal(groups[1].status,'completed');
 assert.equal(plan.blocks([row('unknown',{},{tracking:'reminder'})])[0].status,'reminder');
});

test('reading categories do not turn unbound methods into structured orders',()=>{
 const r=row('method',{category:'checkpoints',instructions:['BUILD: forts along the border']},{tracking:'reminder'});
 assert.equal(plan.category(r),'construction');
 const info=plan.describe({...r,action:{...r.action,params:{...r.action.params,category:plan.category(r)}}});
 assert.deepEqual(info.instructions,['BUILD: forts along the border']);
 assert.equal(info.hud.status,'unsupported');
 assert.equal(plan.category(row('mixed',{category:'checkpoints',instructions:['ARMY: train\n- NAVY: gather']})),'army');
});

test('unrecognized instructions get a display topic from their wording, never a new binding',()=>{
 const topic=title=>{const r={key:'k',name:title,runtime_status:'ready',action:{kind:'manual',params:{category:'checkpoints'}}};const value=plan.category(r);assert.equal(r.action.params.category,'checkpoints');return value;};
 assert.equal(topic('INFRAs, then MILs in Mun (future in best)'),'construction');
 assert.equal(topic('design & produce SI-0(0/4 +wet ammo):2 (from Inf)'),'production');
 assert.equal(topic('Justify HUN, YUG — ASAP'),'diplomacy');
 assert.equal(topic('ASAP: Attache to NSPA (100PP)'),'diplomacy');
 assert.equal(topic('Industrial Concern (150PP)/ War Industrialist'),'politics');
 assert.equal(topic('Fix Trade (ignore deficit of 1-2)'),'trade');
 assert.equal(topic('Irredentist Advisor (150PP)'),'politics');
 assert.equal(topic('Armies: Rename, Split* & train'),'army');
 assert.equal(topic('Grant all statehoods'),'checkpoints');
 assert.equal(topic('Never spend PP on events'),'checkpoints');
 assert.equal(plan.category({key:'p',name:'Build forts',action:{kind:'special_project',params:{}}}),'projects');
});

test('legacy descriptive parameters cannot crash the plan or qualify an empty production layout',()=>{
 assert.equal(plan.category(row('old',{instructions:'legacy text'})),'checkpoints');
 const info=plan.describe(row('old',{category:'production',production_targets:[null],instructions:42}));
 assert.equal(info.hud.status,'unresolved');assert.deepEqual(info.instructions,[]);
});

test('production targets show the Rust match status and why an unmatched target is not detected',()=>{
 const sel={production_selector:{equipment_ids:['x'],variant_names:[],category:null}};
 const r=row('p',{category:'production',production_targets:[
  {equipment_name:'support equipment',requested_factories:5,...sel},
  {equipment_name:'I-16',requested_factories:5,...sel},
  {equipment_name:'Truck',requested_factories:4,...sel},
  {equipment_name:'infantry equipment',quantity_mode:'remaining',...sel}]},
  {production:{layout:[
   {basis:'requested',requested:5,assigned:5,target:5,lines:[{id:'1'}],reason:''},
   {basis:'requested',requested:0,assigned:0,target:5,lines:[],reason:'design_name_mismatch',near_lines:[{line_id:'2',name:'I-153',equipment:'small_plane_airframe_1',category:'military',why:'design_name_mismatch'}]},
   {basis:'requested',requested:0,assigned:0,target:4,lines:[],reason:'line_missing',near_lines:[]},
   {basis:'remaining',requested:150,assigned:33,target:23,lines:[{id:'7'}],reason:'remaining_line_not_last'}]}});
 // One line per product: the met count replaces the order; reasons stay separate.
 assert.deepEqual(plan.describe(r).instructions,[
  '✓ support equipment: 5 of 5 factories',
  'I-16: set 5 requested factories.',
  'Truck: set 4 requested factories.',
  'infantry equipment: allocate all remaining military factories; keep this line last.']);
 const naval={production_selector:{equipment_ids:['convoy_1'],variant_names:[],category:'naval'}};
 assert.deepEqual(plan.describe(row('n',{category:'production',production_targets:[{equipment_name:'convoys',display_name:'Convoy',requested_factories:1,...naval}]},{production:{layout:[{basis:'requested',requested:0,assigned:0,target:1,lines:[],reason:'line_missing'}]}})).instructions,['Convoy: set 1 requested dockyard.']);
 assert.deepEqual(plan.describe(r).observations,[
  'I-16: not detected — the line with this equipment has another name (I-153).',
  'Truck: not detected — no matching production line is observed.',
  'infantry equipment: detected, but it is not the last military line.']);
 assert.deepEqual(plan.describe(row('q',{category:'production',equipment_name:'Tank',requested_factories:2,...sel})).observations,[]);
});
