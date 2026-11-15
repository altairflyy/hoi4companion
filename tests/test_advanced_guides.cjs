'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
const model=require('../src/hoi4companion/web/importer-model.js');
test('advanced references stay grouped and searchable without becoming campaign actions',()=>{
 const data={documents:[{id:'a',name:'Support tier list',category:'companies',text:'S Tier\nEngineers'},
 {id:'b',name:'Production guide',category:'production',text:'Efficiency'}],mio_queues:[{id:'p',organization_id:'FIN_example',name:'Infantry',traits:['one','two']}]};
 const before=structuredClone(data),groups=model.advancedReferences(data,'engineers');
 assert.equal(groups.documents.length,1);assert.equal(groups.documents[0].id,'a');assert.deepEqual(data,before);
 assert.equal(model.advancedReferences(data,'FIN_example').mios.length,1);
 assert.equal(model.advancedReferences({}).documents.length,0);
});
test('MIO illustrations cannot be mistaken for tank or aircraft design recipes',()=>{
 const groups=model.referenceGroups({images:[{name:'Medium Tank',category:'mios'},{name:'T-I',category:'armor'}]});
 assert.equal(groups.find(g=>g.id==='mios').images.length,1);assert.equal(groups.find(g=>g.id==='armor').images.length,1);
});
test('advanced ZIP intake and export are visible outside the optional AI editor',()=>{
 const html=fs.readFileSync('src/hoi4companion/web/importer.html','utf8');
 for(const id of ['advancedZipFiles','advancedReferences','downloadMioQueues'])assert.ok(html.includes('id="'+id+'"'),id);
 assert.ok(html.indexOf('id="advancedZipFiles"')<html.indexOf('id="advancedImport"'));
});
