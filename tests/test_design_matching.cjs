'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const design=require('../src/hoi4companion/web/design-matching.js'),contracts=require('../src/hoi4companion/web/contracts.js');
const recipe=()=>({modules:{gun:'small_cannon',engine:'engine_1',spare:'empty'},upgrades:{armor:'0',engine:'2'}});
const observed=(value)=>({status:'observed',source:'fixture',value});
const liveDesign=()=>({status:'observed',modules:{engine:'engine_1',gun:'small_cannon'},upgrades:{engine:2}});
const line=(id='one',configuration=liveDesign())=>({id,equipment:'light_tank_chassis_1',name:'Renamed vehicle',category:'military',requested_factories:7,assigned_factories:4,active_factories:3,damaged_factories:1,...(configuration===undefined?{}:{design:configuration})});
const selector={category:'military',equipment_ids:['light_tank_chassis_1'],variant_names:['Guide name'],design:{modules:{gun:'small_cannon',engine:'engine_1'},upgrades:{engine:2}}};
const state=lines=>contracts.state({schema_version:1,usable:true,facts:{country:observed('TST'),'production.lines':observed(lines),'country.production.effective_factories.military':observed(20)}});

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
 const s=selector,wrong=line('wrong');wrong.design.modules.gun='machine_gun';wrong.name='Guide name';
 const other=line('other');other.equipment='medium_tank_chassis_1';const naval=line('naval');naval.category='naval';
 const result=design.matching(s,[line(),wrong,other,naval]);
 assert.deepEqual(result.rows.map(r=>r.id),['one']);assert.equal(result.complete,true);assert.equal(result.design_check.status,'match');
 assert.deepEqual(result.design_check.candidates.map(r=>r.status),['match','different']);
});

test('production contracts reject malformed optional live designs without accepting string levels',()=>{
 for(const configuration of [null,{},[],{status:'unavailable',modules:{}},{...liveDesign(),upgrades:{engine:'2'}},{...liveDesign(),upgrades:{engine:true}}])assert.equal(contracts.fact(state([line('bad',configuration)]),'production.lines').status,'unknown');
 for(const configuration of [liveDesign(),{status:'unavailable'}])assert.equal(contracts.fact(state([line('good',configuration)]),'production.lines').status,'observed');
 const old=line();delete old.design;assert.equal(contracts.fact(state([old]),'production.lines').status,'observed');
});

test('browser pages load independent recipe helper before contracts and production code',()=>{
 for(const page of ['index','guide','technologies','importer']){
  const html=fs.readFileSync('src/hoi4companion/web/'+page+'.html','utf8');assert(html.indexOf('/design-matching.js')<html.indexOf('/contracts.js'));assert(html.includes('/design-matching.js'));
 }
 const sandbox={HoiI18n:require('../src/hoi4companion/web/i18n.js')};
 for(const file of ['design-matching','contracts'])vm.runInNewContext(fs.readFileSync('src/hoi4companion/web/'+file+'.js','utf8'),sandbox);
 assert.equal(sandbox.HoiDesignMatching.compare(recipe(),liveDesign()).status,'match');
});
