'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
for(const page of ['index','technologies','country','guide','importer','associations'])test(page+' keeps guide import in permanent navigation',()=>{
 const html=fs.readFileSync('src/hoi4companion/web/'+page+'.html','utf8');
 const nav=html.match(/<nav\b[^>]*aria-label="Views"[^>]*>([\s\S]*?)<\/nav>/)?.[1];
 assert.ok(nav,'main navigation must be present');
 assert.match(nav,/<a\b[^>]*href="\/importer"[^>]*>Import guide<\/a>/);
 assert.doesNotMatch(nav,/\bhidden\b/,'guide selection cannot hide the import entry');
});
