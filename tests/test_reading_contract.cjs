'use strict';
// The web UI presents Rust readings (native/guides/src/reading.rs); it never builds them.
const test=require('node:test'),assert=require('node:assert/strict');
const plan=require('../src/hoi4companion/web/action-plan.js');
const fixture=require('./fixtures/reading-contract.json');
const rowOf=c=>({...c.step,runtime_status:'ready',reading:c.reading});
const titles={
 'construction line with then phases and a guide note':'Build Infrastrutture in Muntenia, then build Fabbrica militare in Muntenia',
 'recognized justification with its political power cost':'Justify a war goal on Ungheria (HUN)',
 'ASAP prefix and a tag without a localized name':'Justify a war goal on YUG',
 'bound action on several countries written with an alias':'Improve relations with Regno Unito (ENG), Francia (FRA)',
 'scripted localisation is never shown as a name':'Justify a war goal on SOV'
};

test('every fully understood reading becomes the card title; the others keep the original text',()=>{
 for(const c of fixture.cases){
  const composed=plan.composedTitle(c.reading);
  if(c.reading?.understood){assert.equal(composed,titles[c.name],c.name);assert.equal(plan.describe(rowOf(c)).title,composed,c.name);}
  else{assert.equal(composed,null,c.name);assert.equal(titles[c.name],undefined,c.name);}
 }
});

test('a composed construction title replaces the region list but keeps every target',()=>{
 const c=fixture.cases.find(c=>c.name.startsWith('construction line'));
 const view=plan.describe(rowOf(c));
 assert.deepEqual(view.instructions,[]);
 const partial=plan.describe(rowOf(fixture.cases.find(c=>c.name.startsWith('construction targets bound'))));
 assert.deepEqual(partial.instructions,['In Prussia orientale.']);
});

test('a reading the UI cannot trust is never composed',()=>{
 const base=fixture.cases[0].reading;
 for(const reading of [null,{...base,understood:false},{...base,title:null},{...base,title:{parts:[{template:1,args:[]}]}},{...base,title:{parts:[]}}])
  assert.equal(plan.composedTitle(reading),null);
});
