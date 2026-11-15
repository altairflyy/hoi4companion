const test=require('node:test'),assert=require('node:assert/strict');
const importer=require('../src/hoi4companion/web/importer-model.js');

test('doctrines belong to the research attachment exactly once',()=>{
 const row={kind:'doctrine',id:'grand_assault',name:'Grand Assault',domain:'land'},sections=importer.catalogSections([row]);
 assert.deepEqual(sections.filter(s=>s.catalog.some(e=>e.id===row.id)).map(s=>s.id),['research']);
});
