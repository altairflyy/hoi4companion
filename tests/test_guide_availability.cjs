'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const availability=require('../src/hoi4companion/web/guide-availability.js');

test('DLC-incompatible guide options stay visible but grey and disabled with requirements',()=>{
 const option={textContent:'Country Guide',classList:{toggle(key,on){this[key]=on;}}};
 availability.decorate(option,{blocked:true,requirements:[{enable:['Pack'],disable:[]}]});
 assert.equal(option.disabled,true);assert.equal(option.classList['guide-dlc-unavailable'],true);
 assert.match(option.textContent,/Country Guide — Requires DLC: Pack/);
 assert.equal(option.title,'Requires DLC: Pack');
});
test('unknown compatibility never disables a guide and OR/NOT requirements retain their meaning',()=>{
 const option={textContent:'Guide'};availability.decorate(option,{blocked:false,unknown:true});
 assert.equal(option.disabled,false);assert.equal(option.textContent,'Guide');
 assert.equal(availability.notice({blocked:true,requirements:[{enable:['A'],disable:[]},{enable:[],disable:['B']}]}),
              'Requires DLC: A / Requires disabling DLC: B');
});
