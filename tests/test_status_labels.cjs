'use strict';
// Each guide/live/HUD status is shown with one English word and one Italian word.
// A state worded differently in two views ("WAITING" vs "Waiting", "Unverified" vs
// "State not verified") reads as two different states, so every standalone status
// label is listed here with the state it presents.
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const messages=require('../src/hoi4companion/translations.json').messages;
const web=name=>fs.readFileSync(path.join(__dirname,'..','src','hoi4companion','web',name),'utf8');
const label=String.raw`(?:HoiI18n\.)?t\((["'])([^"']+)\1\)`;

// inline: the label is substituted inside a sentence, so its first letter is lower case by position.
const sites=[
 {file:'explorer.js',state:'done',re:String.raw`statusLabels=\{completed:${label}`},
 {file:'explorer.js',state:'in progress',re:String.raw`statusLabels=\{[^}]*active:${label}`},
 {file:'explorer.js',state:'unknown',re:String.raw`statusLabels=\{[^}]*unknown:${label}`},
 {file:'explorer.js',state:'unknown',re:String.raw`statusLabels\[code\]\|\|${label}`},
 {file:'explorer.js',state:'in progress',re:String.raw`Optional alternative'\),upcoming:[^,]*,active:${label}`},
 {file:'explorer.js',state:'waiting',re:String.raw`Optional alternative'\),upcoming:[^,]*,active:[^,]*,waiting:${label}`},
 {file:'guide-assist.js',state:'in progress',re:String.raw`labels=\{[^}]*active:${label}`},
 {file:'guide-assist.js',state:'waiting',re:String.raw`labels=\{[^}]*waiting:${label}`},
 {file:'guide.js',state:'done',re:String.raw`HoiI18n\.t\("Ordered · in the construction queue"\):${label}`},
 {file:'guide.js',state:'waiting',re:String.raw`runtime_status==='waiting'\?${label}:row\.runtime_status==='active'`},
 {file:'guide.js',state:'in progress',re:String.raw`tones=\{[^}]*active:${label}`},
 {file:'guide.js',state:'waiting',re:String.raw`tones=\{[^}]*waiting:${label}`},
 {file:'guide.js',state:'unknown',re:String.raw`tones=\{[^}]*unknown:${label}`},
 {file:'guide.js',state:'unknown',inline:true,re:String.raw`completed_provinces\?\?${label}`},
 {file:'guide.js',state:'in progress',re:String.raw`target\.queued===true\?' · '\+${label}`},
 {file:'technologies.js',state:'unknown',re:String.raw`unresearched:[^}]*unknown:${label}`},
 {file:'technologies.js',state:'in progress',re:String.raw`Next research"\),active:${label}`},
 {file:'collections.js',state:'done',re:String.raw`detail=i<current\?${label}`},
 {file:'collections.js',state:'in progress',re:String.raw`detail=i<current\?[^:]*:i===current\?${label}`},
 {file:'collections.js',state:'waiting',re:String.raw`detail=i<current\?[^:]*:i===current\?[^:]*:${label}`},
];
const capital=text=>text.charAt(0).toLocaleUpperCase()+text.slice(1);

test('every status label site is found in the page code',()=>{
 for(const site of sites)assert.match(web(site.file),new RegExp(site.re),`${site.file} ${site.state}: ${site.re}`);
});

test('each status uses one English and one Italian word with consistent case',()=>{
 const byState=new Map();
 for(const site of sites){
  const english=web(site.file).match(new RegExp(site.re))[2];
  assert.ok(Object.hasOwn(messages,english),`${english} has an Italian translation`);
  const italian=messages[english];
  if(site.inline){assert.equal(english,english.toLocaleLowerCase(),english);assert.equal(italian,italian.toLocaleLowerCase(),italian);}
  const words=byState.get(site.state)||{english:new Set(),italian:new Set()};
  words.english.add(site.inline?capital(english):english);words.italian.add(site.inline?capital(italian):italian);byState.set(site.state,words);
 }
 assert.deepEqual([...byState.keys()].sort(),['done','in progress','unknown','waiting']);
 for(const [state,words] of byState){
  assert.equal(words.english.size,1,`${state}: ${[...words.english].join(' | ')}`);
  assert.equal(words.italian.size,1,`${state}: ${[...words.italian].join(' | ')}`);
 }
});

test('retired status variants are gone from the code and the translations',()=>{
 const retired=['IN PROGRESS','WAITING','Unverified','State not verified','state not verified','Upcoming'];
 const code=fs.readdirSync(path.join(__dirname,'..','src','hoi4companion','web')).filter(f=>/\.(js|html)$/.test(f)).map(web).join('\n');
 for(const text of retired){
  assert.ok(!Object.hasOwn(messages,text),`translations.json still has "${text}"`);
  assert.ok(!code.includes(`"${text}"`)&&!code.includes(`'${text}'`),`web code still uses "${text}"`);
 }
});

test('a manual step has no standalone status word that competes with the others',()=>{
 // Manual steps are presented through action phrases ("Check in the game",
 // "Started · in progress (manual)"), never through a separate status word.
 for(const text of ['Manual','MANUAL','manual'])assert.ok(!Object.hasOwn(messages,text),text);
});
