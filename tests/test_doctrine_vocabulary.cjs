const test=require('node:test'),assert=require('node:assert/strict');
const runtime=require('../src/hoi4companion/web/guide-runtime.js');
const importer=require('../src/hoi4companion/web/importer-model.js');
const observed=value=>({status:'observed',source:'fixture',value});
const guide={schema_version:1,id:'doctrine',dependency_policy:'explicit',applies_to:{country:'TST'},tracks:{operations:[
 {key:'doctrine',action:{kind:'manual',params:{category:'army',doctrine_id:'grand_assault'}},completion:{op:'fact',key:'doctrines.selected',test:'contains',value:'grand_assault'}}]}};
const state=value=>({schema_version:1,usable:true,facts:{country:observed('TST'),...value}});

test('doctrine identity is not proof of selection, research completion or mastery',()=>{
 for(const facts of [{},{'research.completed':observed(['grand_assault'])},{'doctrines.mastered':observed(['grand_assault'])},{'country.experience.army':observed(0)},{'doctrines.selected':observed('grand_assault')}]){
  const row=runtime.evaluate(guide,state(facts)).steps[0];assert.notEqual(row.status,'completed');assert.equal(row.completion.value,null);
 }
 assert.equal(runtime.evaluate(guide,state({'doctrines.selected':observed([])})).steps[0].completion.value,false);
 assert.equal(runtime.evaluate(guide,state({'doctrines.selected':observed(['grand_assault'])})).steps[0].status,'completed');
 const stale={...state({'doctrines.selected':observed(['grand_assault'])}),usable:false};assert.notEqual(runtime.evaluate(guide,stale).steps[0].status,'completed');
});

test('doctrine milestones admit production only after qualified selection',()=>{
 const input=structuredClone(guide),condition=guide.tracks.operations[0].completion;
 input.tracks.operations.push({key:'production',action:{kind:'manual',params:{category:'production',production_milestones:[{step:'doctrine',condition,token:JSON.stringify(['doctrine','doctrines.selected','grand_assault'])}]}},completion:{op:'fact',key:'allocation.done',test:'eq',value:true}});
 const missing=runtime.evaluate(input,state({'allocation.done':observed(true)})).steps[1];assert.equal(missing.reason,'milestone_unknown');
 assert.equal(missing.production_admitted,false);
 const completed=runtime.evaluate(input,state({'allocation.done':observed(true),'doctrines.selected':observed(['grand_assault'])})).steps[1];assert.equal(completed.status,'completed');
});

test('doctrines belong to the research attachment exactly once',()=>{
 const row={kind:'doctrine',id:'grand_assault',name:'Grand Assault',domain:'land'},sections=importer.catalogSections([row]);
 assert.deepEqual(sections.filter(s=>s.catalog.some(e=>e.id===row.id)).map(s=>s.id),['research']);
});
