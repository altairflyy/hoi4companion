'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const g=require('../src/hoi4companion/web/doctrine-guide.js');
// Synthetic text in the layout of an authored doctrine document (no third-party content).
const DOCTRINES=['ALPHA DOCTRINES','ALPHA • ADVANCED GUIDE','READING THE GUIDE — Ratings are the author\'s.','PART I — QUICK REFERENCE','Best per category.',
 '[Table row]','[Column 1]','CATEGORY','[Column 2]','TOP PICK','[Column 3]','ALTERNATIVE',
 '[Table row]','[Column 1]','Grand Doctrines','[Column 2]','GRAND ONE (9/10)','[Column 3]','GRAND TWO (5/10) / GRAND THREE (5/10)',
 'RELATED VIDEO   •   BUILDS','PART II — GRAND DOCTRINES','GRAND TWO (5/10)','Notable Buffs:','Speed: +10%','Spirits:','First Spirit','Second Spirit, Third Spirit',
 'Comment: Fine for','armor.','GRAND ONE (9/10)','Notable Buffs:','Planning: +10%','Comment: Best default.',
 'PART III — SUB-DOCTRINES','INFANTRY','LINE A (10/10)','Notable Buffs:','ALL INFANTRY','Comment: Strong.','LINE B (0/10)','Notable Buffs:','Nothing','Comment: No.',
 '[Document links]','https://example.com/doc'].join('\n');
const SPIRITS=['RELATED VIDEO','🪖 Best Army Spirits','Spirit of the Academy','Bold Move (Default Pick) – Good early.','Why not Other? → Situational.',
 'Spirit of the Army','Relief (Democracy) – XP gain.','Branch One (early) -> Branch Two (late) – Switch later.','Key Takeaways','Army Defaults: Bold Move, Relief','https://example.com/v'].join('\n');

test('doctrine documents read as rated cards grouped by section, in the author\'s order',()=>{
 const r=g.read({category:'doctrines',text:DOCTRINES});
 assert.equal(r.kind,'doctrines');
 assert.equal(r.title,'Alpha Doctrines');
 assert.deepEqual(r.intro,['Ratings are the author\'s.']);
 assert.deepEqual(r.quick.header,['Category','Top Pick','Alternative']);
 assert.deepEqual(r.quick.rows[0].columns[1],[{name:'Grand Two',score:5},{name:'Grand Three',score:5}]);
 assert.deepEqual(r.groups.map(x=>[x.part,x.title,x.items.map(i=>i.name)]),[
  ['Grand Doctrines','Grand Doctrines',['Grand Two','Grand One']],['Sub-Doctrines','Infantry',['Line A','Line B']]]);
 const two=r.groups[0].items[0];
 assert.deepEqual(two.spirits,[['First Spirit'],['Second Spirit','Third Spirit']]);
 assert.equal(two.comment,'Fine for armor.');
 assert.equal(two.best,false);assert.equal(r.groups[0].items[1].best,true);
 assert.deepEqual(r.groups[1].items[0].buffs,['ALL INFANTRY'],'an upper-case buff stays a buff');
 assert.equal(r.groups[1].items[1].score,0);
 assert.deepEqual(r.links,['https://example.com/doc']);
});

test('spirits documents read as branches, slots and entries with their qualifiers and notes',()=>{
 const r=g.read({category:'doctrines',text:SPIRITS});
 assert.equal(r.kind,'spirits');
 assert.equal(r.branches[0].icon,'🪖');assert.equal(r.branches[0].title,'Best Army Spirits');
 const [academy,army]=r.branches[0].slots;
 assert.deepEqual(academy.entries[0],{name:'Bold Move',tags:['Default Pick'],text:'Good early.',notes:['Why not Other? → Situational.']});
 assert.deepEqual(army.entries[1].name,'Branch One -> Branch Two');
 assert.deepEqual(army.entries[1].tags,['early','late']);
 assert.deepEqual(r.takeaways,[{label:'Army Defaults',text:'Bold Move, Relief'}]);
 assert.deepEqual(r.links,['https://example.com/v']);
});

test('other documents and unrecognized layouts are left to the original text',()=>{
 assert.equal(g.read({category:'mios',text:DOCTRINES}),null);
 assert.equal(g.read({category:'doctrines',text:'Just a paragraph.\nAnother one.'}),null);
 assert.equal(g.read({category:'doctrines',text:'PART I — X\nONLY (5/10)'}),null,'too few rated entries');
 assert.equal(g.read(null),null);
 assert.equal(g.display('PEOPLE\'S MILITIA'),'People\'s Militia');
 assert.equal(g.display('Mixed Case stays'),'Mixed Case stays');
 assert.equal(g.rated('X (11/10)'),null);
});

test('the references page loads the reader before the page script',()=>{
 const html=require('node:fs').readFileSync(require('node:path').join(__dirname,'../src/hoi4companion/web/importer.html'),'utf8');
 assert.ok(html.indexOf('/doctrine-guide.js')>0&&html.indexOf('/doctrine-guide.js')<html.indexOf('/importer.js'));
});
