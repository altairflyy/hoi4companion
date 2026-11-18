'use strict';
// Add UI strings to src/hoi4companion/translations.json (English key ->
// Italian text) at the top of "messages". Keeps the file's line endings,
// skips keys that already exist, and refuses to write invalid JSON.
// Usage: node add.cjs '[["English text","Testo italiano"], ...]'
//        node add.cjs path/to/pairs.json
const fs=require('fs'),path=require('path');
const FILE=path.resolve(__dirname,'../../../src/hoi4companion/translations.json');
const input=process.argv[2];
if(!input){console.error('usage: node add.cjs \'[["English","Italiano"],...]\' | pairs.json');process.exit(2);}
const pairs=JSON.parse(fs.existsSync(input)?fs.readFileSync(input,'utf8'):input);
if(!Array.isArray(pairs)||!pairs.every(p=>Array.isArray(p)&&p.length===2&&p.every(s=>typeof s==='string'&&s.length)))
 throw Error('expected an array of [english, italian] string pairs');
const file=process.env.TRANSLATIONS_FILE||FILE;
const raw=fs.readFileSync(file,'utf8');
const nl=raw.includes('\r\n')?'\r\n':'\n';
const messages=JSON.parse(raw).messages;
const fresh=pairs.filter(([key])=>!Object.hasOwn(messages,key));
const skipped=pairs.length-fresh.length;
const head='"messages": {'+nl,at=raw.indexOf(head);
if(at<0)throw Error('"messages" block not found');
const out=raw.slice(0,at+head.length)+fresh.map(([k,v])=>`    ${JSON.stringify(k)}: ${JSON.stringify(v)},${nl}`).join('')+raw.slice(at+head.length);
JSON.parse(out);
if(fresh.length)fs.writeFileSync(file,out);
console.log(`added ${fresh.length}, skipped ${skipped} existing`);
if(fresh.length)console.log('translations.json is compiled into backend and overlay: rebuild both (deliver-build).');
