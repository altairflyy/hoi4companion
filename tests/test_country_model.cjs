'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const model=require('../src/hoi4companion/web/country-model.js');
const current=(overview={})=>({usable:true,sample:{country:'POL',country_overview:{country:'POL',divisions:{status:'observed',total:0,by_type:[]},...overview}}});
test('zero divisions are observed without replacing unknown manpower with zero',()=>{
 const r=model.evaluate(current());assert.equal(r.sections.divisions.total,0);assert.equal(r.sections.divisions.status,'observed');assert.equal(r.sections.manpower.status,'unavailable');assert.equal(r.sections.manpower.available,undefined);
});
test('stale and disabled samples remove all national statistics',()=>{
 const stale=current({manpower:{status:'observed',available:123}});stale.usable=false;
 for(const r of [model.evaluate(stale),model.evaluate(current(),false)]){assert.equal(r.sections.divisions.total,undefined);assert.equal(r.sections.manpower.available,undefined);assert.equal(r.country,null);}
});
test('country mismatch and unavailable sections never leak earlier values',()=>{
 const mismatch=model.evaluate(current({country:'ROM'}));assert.equal(mismatch.mode,'unavailable');assert.equal(mismatch.sections.divisions.total,undefined);
 const r=model.evaluate(current({production:{status:'unavailable',lines:[{assigned_factories:99}],reason:'layout non riconosciuto'}}));assert.equal(r.sections.production.lines,undefined);assert.equal(r.sections.production.reason,'layout non riconosciuto');
});
test('partial coverage preserves its limitation and signed balances remain distinguishable',()=>{
 const r=model.evaluate(current({equipment:{status:'partial',reason:'copertura incompleta',items:[{stockpile:0,balance_per_day:-2}]}}));assert.equal(r.sections.equipment.status,'partial');assert.equal(r.sections.equipment.reason,'copertura incompleta');assert.equal(model.number(-2,{signed:true}),-2);assert.equal(model.number(-2),null);assert.equal(model.number(0,{integer:true}),0);assert.equal(model.number(NaN),null);assert.equal(model.number('120'),null);assert.equal(model.number(1.5,{integer:true}),null);
});
test('numeric zero has no negative sign and production rates truncate at game display precision',()=>{
 assert.equal(Object.is(model.number(-0,{signed:true}),-0),false);
 assert.deepEqual(model.productionOutput(1.059,'military'),{value:1.05,digits:2});
 assert.deepEqual(model.productionOutput(52.0099,'military'),{value:52,digits:2});
 assert.deepEqual(model.productionOutput(.006199,'naval'),{value:.0061,digits:4});
 assert.deepEqual(model.productionOutput(0,'naval'),{value:0,digits:2});
 assert.equal(model.productionOutput(null,'military'),null);
});
