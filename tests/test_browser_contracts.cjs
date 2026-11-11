'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const web=path.join(__dirname,'../src/hoi4companion/web');

// CommonJS supplies this dependency implicitly. Exercise the shipped browser
// order so a design-bearing live packet cannot disable an otherwise valid view.
for(const page of fs.readdirSync(web).filter(name=>name.endsWith('.html'))){
 const html=fs.readFileSync(path.join(web,page),'utf8'),scripts=[...html.matchAll(/<script\b[^>]*\bsrc="\/([^"]+)"/g)].map(m=>m[1]);
 if(!scripts.includes('contracts.js'))continue;
 test(page+' loads production-design validation before live contracts',()=>{
  const window={HoiI18n:require('../src/hoi4companion/web/i18n.js')};
  const dependencies=['design-matching.js','contracts.js'];
  assert.ok(scripts.indexOf(dependencies[0])>=0&&scripts.indexOf(dependencies[0])<scripts.indexOf(dependencies[1]));
  for(const script of scripts.filter(s=>dependencies.includes(s)))vm.runInNewContext(fs.readFileSync(path.join(web,script),'utf8'),{window},{filename:script});
  const line={id:'line',equipment:'infantry_equipment_1',name:'Infantry equipment',category:'military',requested_factories:5,assigned_factories:3,active_factories:3,damaged_factories:0,design:{status:'observed',modules:{},upgrades:{}}};
  const packet={usable:true,sample:{country:'TST',country_overview:{country:'TST',production:{status:'observed',lines:[line]}}}};
  assert.equal(window.HoiContracts.fact(window.HoiContracts.state(packet),'production.lines').status,'calculated');
  line.design={status:'observed',modules:{},upgrades:{invalid:'not a number'}};
  assert.equal(window.HoiContracts.fact(window.HoiContracts.state(packet),'production.lines').status,'unknown');
 });
}
