'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const design=require('../src/hoi4companion/web/design-matching.js'),contracts=require('../src/hoi4companion/web/contracts.js'),production=require('../src/hoi4companion/web/production-progress.js');
const recipe=()=>({modules:{gun:'small_cannon',engine:'engine_1',spare:'empty'},upgrades:{armor:'0',engine:'2'}});
const observed=(value)=>({status:'observed',source:'fixture',value});
const liveDesign=()=>({status:'observed',modules:{engine:'engine_1',gun:'small_cannon'},upgrades:{engine:2}});
const line=(id='one',configuration=liveDesign())=>({id,equipment:'light_tank_chassis_1',name:'Renamed vehicle',category:'military',requested_factories:7,assigned_factories:4,active_factories:3,damaged_factories:1,...(configuration===undefined?{}:{design:configuration})});
const step=()=>({key:'tank',action:{kind:'manual',params:{category:'production',requested_factories:7,production_selector:{equipment_ids:['light_tank_chassis_1'],variant_names:['Guide name'],category:'military'},design_reference:recipe()}},completion:{op:'fact',key:'guide.production.tank.requested',test:'eq',value:7}});
const state=lines=>contracts.state({schema_version:1,usable:true,facts:{country:observed('TST'),'production.lines':observed(lines),'country.production.effective_factories.military':observed(20)}});
const context=(lines,s=step(),baselines={})=>production.context({tracks:{production:[s]}},state(lines),baselines);

test('complete recipes compare occupied slots and upgrade levels, independently of insertion order',()=>{
 assert.deepEqual(design.configuration(recipe()),{modules:{gun:'small_cannon',engine:'engine_1'},upgrades:{engine:2}});
 assert.deepEqual(design.compare(recipe(),liveDesign()),{status:'match',differences:[]});
 const actual=liveDesign();actual.modules.spare='armor';actual.upgrades.engine=3;actual.upgrades.armor=1;delete actual.modules.gun;
 assert.deepEqual(design.compare(recipe(),actual),{status:'different',differences:[
  {kind:'modules',key:'gun',expected:'small_cannon',actual:null},{kind:'modules',key:'spare',expected:null,actual:'armor'},
  {kind:'upgrades',key:'armor',expected:0,actual:1},{kind:'upgrades',key:'engine',expected:2,actual:3}]});
});

test('recipe input and live input keep separate upgrade qualification rules',()=>{
 for(const level of ['00','01','-1','1.0','2\n',true,1.5,10001,null])assert.equal(design.configuration({modules:{},upgrades:{engine:level}}),null);
 for(const value of [{modules:{},upgrades:{}},{status:'observed',modules:{},upgrades:{engine:'2'}},{status:'observed',modules:{},upgrades:{engine:true}},{status:'unavailable',modules:{}},{status:'unavailable',upgrades:{}}])assert.equal(design.validLiveDesign(value),false);
 assert.equal(design.validLiveDesign({status:'unavailable'}),true);
 assert.equal(design.validLiveDesign({status:'observed',modules:{},upgrades:{}}),true);
 assert.equal(design.configuration({modules:{'bad slot':'gun'},upgrades:{}}),null);
 assert.equal(design.configuration({modules:{slot:'bad module'},upgrades:{}}),null);
 assert.equal(design.configuration({modules:{slot:'gun\n'},upgrades:{}}),null);
 assert.equal(design.configuration({modules:{},upgrades:Object.fromEntries(Array.from({length:257},(_,i)=>['u'+i,0]))}),null);
});

test('recipe matching ignores display names but respects equipment and production category',()=>{
 const s=production.selectorFor(step()),wrong=line('wrong');wrong.design.modules.gun='machine_gun';wrong.name='Guide name';
 const other=line('other');other.equipment='medium_tank_chassis_1';const naval=line('naval');naval.category='naval';
 const result=design.matching(s,[line(),wrong,other,naval]);
 assert.deepEqual(result.rows.map(r=>r.id),['one']);assert.equal(result.complete,true);assert.equal(result.design_check.status,'match');
 assert.deepEqual(result.design_check.candidates.map(r=>r.status),['match','different']);
 assert.equal(context([line(),wrong]).details.get('tank').requested,7);
});

test('one unknown same-equipment candidate makes recipe counts unknown even alongside a match',()=>{
 for(const unknown of [{status:'unavailable'}]){
  const current=line('unknown',unknown),result=context([line(),current]);
  assert.equal(result.state.facts['guide.production.tank.requested'].value,null);assert.equal(result.state.facts['guide.production.tank.started'].value,null);
  const detail=result.details.get('tank');assert.equal(detail.design_check.status,'unknown');assert.deepEqual(detail.lines,[]);assert.equal(detail.requested,null);
 }
 const missing=line();delete missing.design;assert.equal(context([missing]).details.get('tank').design_check.status,'unknown');
});

test('different recipe and missing equipment are known zero, unrelated unknown recipes are excluded',()=>{
 const wrong=line();wrong.design.modules.gun='machine_gun';let detail=context([wrong]).details.get('tank');
 assert.equal(detail.assigned,0);assert.equal(detail.design_check.status,'different');
 const unrelated=line('other',{status:'unavailable'});unrelated.equipment='other';detail=context([unrelated]).details.get('tank');
 assert.equal(detail.assigned,0);assert.deepEqual(detail.design_check,{status:'missing',candidates:[]});
});

test('generic selectors retain variant-name matching when no recipe is supplied',()=>{
 const s=step();delete s.action.params.design_reference;let detail=context([line()],s).details.get('tank');assert.equal(detail.requested,0);assert.equal(detail.design_check,undefined);
 const row=line();row.name='Guide-name';detail=context([row],s).details.get('tank');assert.equal(detail.requested,7);
 s.action.params.production_selector.variant_names=[];assert.equal(context([line()],s).details.get('tank').requested,7);
});

test('production contracts reject malformed optional live designs without accepting string levels',()=>{
 for(const configuration of [null,{},[],{status:'unavailable',modules:{}},{...liveDesign(),upgrades:{engine:'2'}},{...liveDesign(),upgrades:{engine:true}}])assert.equal(contracts.fact(state([line('bad',configuration)]),'production.lines').status,'unknown');
 for(const configuration of [liveDesign(),{status:'unavailable'}])assert.equal(contracts.fact(state([line('good',configuration)]),'production.lines').status,'observed');
 const old=line();delete old.design;assert.equal(contracts.fact(state([old]),'production.lines').status,'observed');
});

test('relative baseline identity normalizes recipes and recursively sorts map keys like Python',()=>{
 const s=step(),p=s.action.params;p.quantity_mode='additional';p.requested_factories=3;s.completion={op:'fact',key:'guide.production.tank.added',test:'gte',value:3};
 p.design_reference={modules:{'2':'two','10':'ten',gun:'small_cannon'},upgrades:{z:'0',engine:'2'}};
 const spec=production.relativeSpec(s);assert.equal(spec.fact_key,'production_requested:{"category":"military","design":{"modules":{"10":"ten","2":"two","gun":"small_cannon"},"upgrades":{"engine":2}},"equipment_ids":["light_tank_chassis_1"],"variant_names":["Guide name"]}');
 p.design_reference={upgrades:{engine:2},modules:{gun:'small_cannon','10':'ten','2':'two',spare:'empty'}};
 assert.equal(production.relativeSpec(s).fact_key,spec.fact_key);p.design_reference.modules.gun='other';assert.notEqual(production.relativeSpec(s).fact_key,spec.fact_key);
 p.design_reference={modules:{}};assert.equal(production.selectorFor(s),null);assert.equal(production.relativeSpec(s),null);
});

test('unknown designs cannot satisfy remaining allocation or priority checks',()=>{
 const s=step();s.action.params.quantity_mode='remaining';s.action.params.production_priority='last';
 const result=context([line('unknown',{status:'unavailable'})],s),detail=result.details.get('tank');
 assert.equal(detail.target,null);assert.equal(detail.priority_satisfied,null);assert.equal(result.state.facts['guide.production.tank.remaining_allocated'].value,null);assert.equal(result.state.facts['guide.production.tank.priority_satisfied'].value,null);
});

test('browser pages load independent recipe helper before contracts and production code',()=>{
 for(const page of ['index','guide','technologies','importer']){
  const html=fs.readFileSync('src/hoi4companion/web/'+page+'.html','utf8');assert(html.indexOf('/design-matching.js')<html.indexOf('/contracts.js'));assert(html.includes('/design-matching.js'));
 }
 const sandbox={HoiI18n:require('../src/hoi4companion/web/i18n.js')};
 for(const file of ['design-matching','contracts','production-progress'])vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/'+file+'.js','utf8'),sandbox);
 assert.equal(sandbox.HoiDesignMatching.compare(recipe(),liveDesign()).status,'match');
 assert.equal(sandbox.HoiProductionProgress.context({tracks:{production:[step()]}},state([line()]),{}).details.get('tank').requested,7);
});
