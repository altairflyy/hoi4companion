'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js'),contracts=require('../src/hoi4companion/web/contracts.js');
const summary=require('../src/hoi4companion/web/operational-summary.js');
const observed=value=>({status:'observed',source:'fixture',value});
const instruction=(key,n,order,when)=>({key,title:key,when,action:{kind:'manual',params:{category:'production',equipment_id:'sup',requested_factories:n,production_revision:{group:'setup',order},production_selector:{equipment_ids:['sup'],variant_names:[],category:'military'}}},completion:{op:'fact',key:'guide.production.'+key+'.requested',test:'eq',value:n}});
function definitions(){const rows=[instruction('initial',1,0),instruction('current',7,1,{op:'fact',key:'focus.completed',test:'contains',value:'phase'}),instruction('future',30,2,{op:'fact',key:'unknown.condition',test:'eq',value:true})];delete rows[0].when;return {schema_version:1,id:'phases',title:'Phased guide',dependency_policy:'explicit',applies_to:{country:'TST'},tracks:{operations:rows}};}
const live=()=>({schema_version:1,usable:true,facts:{country:observed('TST'),'focus.completed':observed(['phase']),'production.lines':observed([{id:'1',equipment:'sup',name:'Support',category:'military',requested_factories:7,assigned_factories:3,active_factories:3,damaged_factories:0}])}});

test('a reached phase retires the initial layout without treating missing adoptions as fulfilled',()=>{
 const g=definitions(),condition={op:'fact',key:'politics.advisors',test:'contains',value:'expert'};
 g.tracks.adoptions=[{key:'advisor',action:{kind:'manual'},completion:condition}];
 g.tracks.operations[1].action.params.production_milestones=[{step:'advisor',token:JSON.stringify(['advisor','politics.advisors','expert']),condition}];
 const r=runtime.evaluate(g,live()),rows=new Map(r.steps.map(x=>[x.key,x]));
 assert.equal(rows.get('initial').reason,'production_superseded');
 assert.equal(rows.get('initial').completion.value,null);
 assert.equal(rows.get('current').reason,'milestone_unknown');
 assert.equal(rows.get('current').production_admitted,false);
 const items=summary.project(g,r,live()).items;
 assert(!items.some(x=>x.step_key==='initial'));assert(items.some(x=>x.step_key==='current'));
});

test('no phase proof or missing adoption without a boundary keeps existing goals; dependencies remain closed',()=>{
 const g=definitions(),condition={op:'fact',key:'politics.advisors',test:'contains',value:'expert'};
 g.tracks.adoptions=[{key:'advisor',action:{kind:'manual'},completion:condition}];
 g.tracks.operations[1].action.params.production_milestones=[{step:'advisor',token:JSON.stringify(['advisor','politics.advisors','expert']),condition}];
 g.tracks.other=[{key:'after',action:{kind:'manual'},depends_on:['current']},{key:'after-start',action:{kind:'manual'},depends_on_started:['current']}];
 let r=runtime.evaluate(g,live()),rows=new Map(r.steps.map(x=>[x.key,x]));
 assert.equal(rows.get('after').reason,'dependency_unknown');assert.equal(rows.get('after-start').reason,'dependency_unknown');
 const stale=live();stale.usable=false;assert(!runtime.evaluate(g,stale).steps.some(x=>x.superseded_by));
 const missing=live();delete missing.facts['focus.completed'];assert.equal(runtime.evaluate(g,missing).steps[0].superseded_by,undefined);
 delete g.tracks.operations[1].when;assert.equal(runtime.evaluate(g,live()).steps[0].superseded_by,undefined);
});

test('the current production phase replaces old goals without historical completion and keeps future goals gated',()=>{
 const g=definitions(),s=live(),r=runtime.evaluate(g,s),before=JSON.stringify({g,s});
 assert.equal(r.steps[0].reason,'production_superseded');assert.equal(r.steps[0].completion.value,null);
 assert.equal(r.steps[1].status,'completed');assert.equal(r.steps[2].reason,'condition_unknown');
 const projected=summary.project(g,r,s);assert.deepEqual(projected.items.map(x=>x.step_key),['current']);
 assert.equal(JSON.stringify({g,s}),before);
});
test('revision validation rejects malformed values and explicit dependencies remain binding',()=>{
 const g=definitions();g.tracks.operations[1].depends_on=['initial'];
 const r=runtime.evaluate(g,live());assert.equal(r.steps[1].status,'waiting');assert.equal(r.steps[0].superseded_by,undefined);
 for(const revision of [{group:'x',order:true},{group:'',order:0},{group:'x',order:-1},{group:'x',order:1,extra:true}]){
  const bad=definitions();bad.tracks.operations[0].action.params.production_revision=revision;assert.throws(()=>contracts.guide(bad));
 }
});
test('unknown revision gates with matching counts cannot release cross-track start or completion dependencies',()=>{
 const g=definitions();g.tracks.other=[{key:'after',action:{kind:'manual'},depends_on:['current']},{key:'after-start',action:{kind:'manual'},depends_on_started:['current']}];
 const s=live();delete s.facts['focus.completed'];const rows=new Map(runtime.evaluate(g,s).steps.map(r=>[r.key,r]));
 assert.equal(rows.get('current').completion.value,null);assert.equal(rows.get('current').original_completion.value,true);
 for(const key of ['after','after-start'])assert.equal(rows.get(key).reason,'dependency_unknown');
});
