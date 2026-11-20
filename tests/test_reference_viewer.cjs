'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
const viewer=require('../src/hoi4companion/web/reference-viewer.js');

test('fit scales an image to the stage, centred, enlarging at most 2x',()=>{
 const stage={width:1000,height:500};
 assert.deepEqual(viewer.fitView(stage,{width:2000,height:500}),{scale:0.5,x:0,y:125});
 assert.deepEqual(viewer.fitView(stage,{width:100,height:100}),{scale:2,x:400,y:150});
 assert.equal(viewer.fitScale(stage,{width:0,height:0}),1);
});

test('zoom keeps the pixel under the cursor and panning stays inside the image',()=>{
 const stage={width:1000,height:500},image={width:1000,height:500};
 const zoomed=viewer.zoomAt({scale:1,x:0,y:0},2,{x:250,y:100},stage,image);
 assert.deepEqual(zoomed,{scale:2,x:-250,y:-100});
 assert.deepEqual(viewer.clampView({scale:2,x:300,y:-900},stage,image),{scale:2,x:0,y:-500});
 assert.equal(viewer.zoomAt({scale:1,x:0,y:0},100,{x:0,y:0},stage,image).scale,8);
});

test('reference page loads the viewer before the importer and keeps the four reference panels',()=>{
 const html=fs.readFileSync('src/hoi4companion/web/importer.html','utf8');
 assert.ok(html.indexOf('/reference-viewer.js')<html.indexOf('/importer.js'));
 for(const id of ['referenceTabs','referencePanelImages','referencePanelDesigns','advancedReferences','referencePanelMio','advancedDocumentList','mioSearch','referenceDesignSearch'])assert.ok(html.includes('id="'+id+'"'),id);
});
