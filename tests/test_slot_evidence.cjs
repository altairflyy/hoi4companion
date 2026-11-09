const test=require('node:test'),assert=require('node:assert/strict');
const c=require('../src/hoi4companion/web/contracts.js');
test('duplicate or excessive observed research slots cannot inflate HUD capacity',()=>{
 for(const slots of [[{slot:1,technology_id:null},{slot:1,technology_id:null}],Array.from({length:65},()=>({technology_id:null})),[{slot:true,technology_id:null}],[{slot:-1,technology_id:null}],[{slot:65,technology_id:null}]])
  assert.equal(c.state({usable:true,sample:{country:'TST',research_slots:slots}}).facts['research.slots'].status,'unknown');
 for(const slots of [[{technology_id:null}],[{slot:0,technology_id:null},{slot:1,technology_id:'radio'}]])
  assert.equal(c.state({usable:true,sample:{country:'TST',research_slots:slots}}).facts['research.slots'].status,'observed');
});
