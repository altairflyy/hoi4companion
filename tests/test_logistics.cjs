'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),c=require('../src/hoi4companion/web/contracts.js'),model=require('../src/hoi4companion/web/country-model.js');
const logistics=()=>({schema_version:1,status:'observed',source:'native_logistics_ui_formula',aggregation:'equipment_archetype',interval_days:7,items:[{id:'infantry_equipment',aggregation:'equipment_archetype',gross_stockpile:10,logistics_stockpile:10,needed:15,available:-5,balance_per_day:-1,status:'observed',reason:null}]});
const sample=l=>({country:'NOR',country_overview:{country:'NOR',equipment:{status:l.status,items:[{id:'infantry_equipment_1',stockpile:10,needed:null,balance_per_day:null}],logistics:l}}});
const key='country.equipment.logistics.items.infantry_equipment.available';
test('qualified archetype facts preserve negative availability and do not copy it to model rows',()=>{const s=c.state({usable:true,sample:sample(logistics())});assert.deepEqual(c.fact(s,key),{value:-5,status:'calculated',source:'native_logistics_ui_formula'});assert.equal(c.fact(s,'country.equipment.items.infantry_equipment_1.stockpile').value,10);assert.equal(c.fact(s,'country.equipment.items.infantry_equipment_1.needed').status,'unknown');assert.equal(c.fact(s,'equipment.logistics').value.interval_days,7);assert.deepEqual(c.state({usable:true,sample:sample(logistics())}),s);});
test('missing component remains unknown while independent native daily flow stays usable',()=>{const l=logistics();l.status=l.items[0].status='partial';l.items[0].needed=l.items[0].available=null;const s=c.state({usable:true,sample:sample(l)});assert.equal(c.fact(s,key).status,'unknown');assert.equal(c.fact(s,key.replace('available','balance_per_day')).value,-1);});
test('wrong provenance invalid intervals duplicate roots inconsistent deficit and nonfinite values are rejected',()=>{for(const mutate of [l=>l.source='stockpile_delta',l=>l.interval_days=0,l=>l.interval_days=31,l=>l.items.push({...l.items[0]}),l=>l.items[0].available=5,l=>l.items[0].gross_stockpile=-10,l=>l.items[0].balance_per_day=Infinity,l=>l.items[0].needed=null]){const l=logistics();mutate(l);assert.equal(c.validLogistics(l),false);assert.equal(c.fact(c.state({usable:true,sample:sample(l)}),key).status,'unknown');const v=model.evaluate({usable:true,sample:sample(l)});assert.equal(v.sections.equipment.logistics.status,'unavailable');}});
test('current country and calculated provenance are required for logistics facts',()=>{const l=logistics(),m=sample(l);m.country='GER';assert.equal(c.fact(c.state({usable:true,sample:m}),key).status,'unknown');for(const bad of [{status:'observed',source:'native_logistics_ui_formula',value:-5},{status:'calculated',source:'legacy_adapter',value:-5}])assert.equal(c.fact(c.state({schema_version:1,usable:true,facts:{[key]:bad}}),key).status,'unknown');const stale=model.evaluate({usable:false,sample:sample(l)});assert.equal(stale.sections.equipment.logistics,undefined);});

test('logistics presentation requires an exact country identity while retaining legacy stockpile rows',()=>{
 for(const country of [undefined,'GER',123]){
  const raw=sample(logistics());raw.country_overview.country=country;
  const section=model.evaluate({usable:true,sample:raw}).sections.equipment;
  assert.equal(section.logistics.status,'unavailable');assert.deepEqual(section.logistics.items,[]);
  if(country===undefined)assert.equal(section.items[0].stockpile,10);
 }
});


for(const row of require('./fixtures/logistics.json'))test('shared logistics contract: '+row.name,()=>{const f=c.fact(c.state(row.input),row.key);assert.equal(f.status,row.status);assert.equal(f.value,row.value);});

