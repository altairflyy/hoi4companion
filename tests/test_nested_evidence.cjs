const test=require('node:test'),assert=require('node:assert/strict');
const contracts=require('../src/hoi4companion/web/contracts.js');
test('unavailable nested observations do not become facts through a partial parent',()=>{
 for(const status of ['unavailable','unknown','stale']){
  const value=contracts.state({usable:true,sample:{country:'TST',country_overview:{country:'TST',
   politics:{status:'partial',power:{status,available:120},advisors:{status,items:[]}},manpower:{status:'partial',available:0}}}});
  for(const key of ['country.politics.power.available','country.politics.advisors.items.count']){
   assert.equal(value.facts[key].status,'unknown');assert.equal(contracts.condition({op:'fact',key,test:'gte',value:0},value),null);
  }
  assert.equal(value.facts['country.manpower.available'].value,0);
 }
});
