'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
const {create}=require('../src/hoi4companion/web/action-reading.js');
const plan=require('../src/hoi4companion/web/action-plan.js'),i18n=require('../src/hoi4companion/web/i18n.js');
const vocabulary=require('../src/hoi4companion/guide_language.json'),reader=create(vocabulary);
const row=(category='checkpoints',instructions=[],extra={})=>({key:'step',name:instructions[0],runtime_status:'ready',tracking:'reminder',action:{kind:'manual',params:{category,instructions}},...extra});

test('occupation law and garrison are distinct, and abbreviated names stay qualified',()=>{
 const law=reader.read('LAW: LPF + CAV');
 assert.match(law.text,/Occupation law: Local Police Force.*garrison template: cavalry/);
 const named=reader.read('LAW: LPF + Ratsu…');
 assert.match(named.text,/garrison template: Ratsu…/);assert.equal(named.notes.length,1);
 assert.doesNotMatch(named.text,/Ratsuv|confirmed|completed/i);
});
test('resource costs, widths and division compositions are expanded without guessing battalions',()=>{
 const result=reader.read('ARMY: Bring OFF to 24w(10/2+FL,Rec->Log) (35AE), 48 INF->OFFs');
 assert.match(result.text,/combat width 24\(10\/2\+flame tank/);
 assert.match(result.text,/35 army experience/);assert.match(result.text,/offensive infantry with self-propelled artillery template \(OFFs\)/);
 assert.match(reader.read('Fascist Demagogue (75PP), command (25 CP), navy (20NE)').text,/75 political power.*25 command power.*20 naval experience/);
});
test('production, army, navy and air clauses use different meanings in one source action',()=>{
 const result=reader.read('MIL: Inf:10 Art:ALL\n- ARMY: Recruit 13 INF, train ALL\n- NAVY: DD, train ALL\n- AIR FORCE: train ALL');
 assert.match(result.text,/Military production: infantry equipment/);
 assert.match(result.text,/Army: Recruit 13 divisions · infantry template/);
 assert.match(result.text,/Exercise all divisions/);assert.match(result.text,/Navy: destroyer.*Exercise all ships/);
 assert.match(result.text,/Air force: Exercise all air wings/);
 assert.ok(result.notes.some(n=>/remaining.*future/.test(n)));
 assert.equal(reader.read('Def against YUG',row('army')).text,'Def against YUG');
});
test('doctrine names require a domain, keeping ambiguous words and unknown acronyms intact',()=>{
 assert.match(reader.read('Land Doc: SD').text,/Streamlined Deployment/);
 assert.match(reader.read('Air Doc: SD').text,/Strategic Destruction/);
 const unknown=reader.read('SD, XYZ and scaffolding');assert.equal(unknown.text,'SD, XYZ and scaffolding');assert.equal(unknown.notes.length,1);
 assert.equal(reader.read('INF',row()).text,'INF');
 assert.equal(reader.read('INFRASTRUCTURE',row('construction')).text,'INFRASTRUCTURE');
});
test('exact entity names, design names and quoted instructions are not rewritten',()=>{
 const source='design F2; select "INF"; LPF Industries';
 const r=row('production',[source]);r.action.params.variant_name='F2';r.action.params.person_name='LPF Industries';
 assert.equal(reader.read(source,r).text,source);
 assert.match(reader.read('produce F2:5 (from Inf)').text,/F2 \(fighter design\).*infantry equipment/);
 assert.equal(reader.read('NA Aviation').text,'NA Aviation');
});
test('common construction conventions retain their different ordering and original location',()=>{
 assert.match(reader.read('Build INFRAs + MILs in Berlin').text,/each indicated region before moving to the next in Berlin/);
 assert.match(reader.read('Build INFRAs, then MILs in Berlin').text,/all indicated regions first, then military factories in Berlin/);
 const result=reader.read('Radio UP');assert.equal(result.text,'Radio UP');assert.ok(result.notes.some(n=>/next useful level/.test(n)));
});
test('display expansion leaves runtime, dependency, source and completion data untouched',()=>{
 const r=row('army',['ARMY: Recruit 13 INF, train ALL'],{depends_on:[{step:'project',state:'started'}],started_when:{op:'check',key:'start'},completion_condition:{op:'check',key:'done'}});
 const before=structuredClone(r),old=plan.describe(r),now=plan.describe(r,{reader});
 assert.deepEqual(r,before);assert.deepEqual(now.hud,old.hud);assert.equal(now.monitoring,old.monitoring);
 assert.match(now.instructions[0],/infantry template/);assert.equal(r.action.params.instructions[0],before.action.params.instructions[0]);
 assert.deepEqual(plan.blocks([r]),plan.blocks([before]));
});
test('English and Italian readers share meanings, not a translation of IDs or runtime state',()=>{
 try{
  i18n.setLanguage('italian');const result=reader.read('LAW: LPF + CAV (150PP)');
  assert.match(result.text,/Legge d’occupazione: Forza di polizia locale/);
  assert.match(result.text,/modello della guarnigione: modello di cavalleria/);
  assert.match(result.text,/150 potere politico/);
 }finally{i18n.setLanguage('english');}
 assert.match(reader.read('LAW: LPF').text,/Occupation law: Local Police Force/);
});
test('new guides can extend the shared vocabulary without hardcoded country aliases',()=>{
 const custom=create({abbreviations:{occupation:{ABC:'Example law'},divisions:{XYZ:'Example division'}}});
 assert.match(custom.read('LAW: ABC + XYZ').text,/Example law.*Example division/);
 assert.equal(reader.read('LAW: ABC + XYZ').text,'Occupation law: ABC; garrison template: XYZ');
});
test('all web entry points load reading rules before the operational view',()=>{
 for(const file of ['guide.html','index.html','technologies.html']){
  const html=fs.readFileSync('src/hoi4companion/web/'+file,'utf8');
  assert.ok(html.indexOf('src="/action-reading.js"')>=0);assert.ok(html.indexOf('src="/action-reading.js"')<html.indexOf('src="/action-plan.js"'));
 }
});

test('occupation wording selects the occupation vocabulary and unexplained codes stay visible',()=>{
 // "LPF + CAV Occupation Law" (no LAW: prefix) is explained, not left raw.
 const law=reader.read('LPF + CAV Occupation Law',row('checkpoints',['LPF + CAV Occupation Law']));
 assert.match(law.text,/Local Police Force \(LPF\)/);
 assert.deepEqual(law.terms.map(t=>t.code),['LPF','CAV']);
 // A code known only in another domain is offered as a possible meaning,
 // never substituted into the instruction.
 const elsewhere=reader.read('Setup LPF later',row('checkpoints',['Setup LPF later']));
 assert.equal(elsewhere.text,'Setup LPF later');
 assert.match(elsewhere.notes[0],/Possible meaning of LPF: Local Police Force/);
});
