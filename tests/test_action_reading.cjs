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
 assert.match(reader.read('INF',row()).text,/infantry template \(INF\)/);
 assert.equal(reader.read('INFRASTRUCTURE',row('construction')).text,'INFRASTRUCTURE');
});
test('coded designs get explanations while proper names and quotes retain their spelling',()=>{
 const source='design F2; select "INF"; LPF Industries';
 const r=row('production',[source]);r.action.params.variant_name='F2';r.action.params.person_name='LPF Industries';
 assert.equal(reader.read(source,r).text,'design F2 (fighter design); select "INF"; LPF Industries');
 assert.equal(r.action.params.variant_name,'F2');
 assert.match(reader.read('produce F2:5 (from Inf)').text,/F2 \(fighter design\).*infantry equipment/);
 assert.equal(reader.read('NA Aviation').text,'NA Aviation');
});
test('common construction conventions retain their different ordering and original location',()=>{
 assert.match(reader.read('Build INFRAs + MILs in Berlin').text,/each indicated region before moving to the next in Berlin/);
 assert.match(reader.read('Build INFRAs, then MILs in Berlin').text,/all indicated regions first, then military factories in Berlin/);
 const result=reader.read('Radio UP');assert.equal(result.text,'Radio next useful upgrade (UP)');assert.equal(result.notes.length,0);
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

test('documented naval variants explain hull codes and ALL refers to dockyards',()=>{
 const result=reader.read('NAV: CVe:2 DDt:3 DDes:1 CLna:ALL');
 assert.match(result.text,/empty carrier \(CVe\)/);
 assert.match(result.text,/torpedo destroyer \(DDt\)/);
 assert.match(result.text,/escort destroyer \(DDes\)/);
 assert.match(result.text,/light cruiser without armor \(CLna\)/);
 assert.match(result.text,/all remaining and future naval dockyards/);
 assert.doesNotMatch(result.text,/military factories/);
 assert.ok(result.notes.some(n=>/dockyards/.test(n)));
 assert.match(reader.read('Navy: BCe:ALL').text,/empty battlecruiser.*all remaining and future naval dockyards/);
 assert.match(reader.read('Navy: SS, train ALL').text,/submarine.*Exercise all ships/);
 assert.match(reader.read('MIL: Inf:ALL').text,/all remaining and future military factories/);
 assert.match(reader.read('ASAP: NAV: CVe:ALL').text,/as soon as the requirements allow.*empty carrier.*all remaining and future naval dockyards/);
});

test('confirmed BBe is explained inline, never silently corrected to BCe',()=>{
 const result=reader.read('Navy: Finish all but cancel extra, then build convoys:5 & BBe: ALL');
 assert.match(result.text,/empty battleship \(BBe\): all remaining and future naval dockyards/);
 assert.equal(result.notes.some(n=>/Possible meaning of BBe/.test(n)),false);
 assert.doesNotMatch(result.text,/battlecruiser/);
 assert.equal(result.terms.some(t=>t.code==='BBe'&&t.meaning==='empty battleship'),true);
 const custom=create({conventions:{possible_abbreviations:{naval:{XYZ:'example ship'}}}});
 const uncertain=custom.read('NAVY: XYZ');
 assert.equal(uncertain.text,'Navy: XYZ');
 assert.match(uncertain.notes[0],/Possible meaning of XYZ: example ship/);
 assert.match(reader.read('NAVY: XYZ').text,/XYZ/);
});

test('regimental companies and documented design accessories are explained in context',()=>{
 const result=reader.read('ARMY: Add IG, HW, MHW, R-SPAA to INF');
 for(const meaning of ['infantry guns','heavy weapons','motorized heavy weapons','regimental self-propelled anti-air'])assert.ok(result.text.includes(meaning));
 assert.match(reader.read('AIR FORCE: MG, SL').text,/machine gun.*smoke launchers/);
 assert.match(reader.read('ASAP: NAV: CVe:1').text,/as soon as the requirements allow/);
 const original=row('production',[],{});original.action.params.variant_name='CVe';
 assert.equal(reader.read('CVe',original).text,'empty carrier (CVe)');
});

test('confirmed naval explanations appear inline in Italian',()=>{
 try{
  i18n.setLanguage('italian');
  const result=reader.read('NAV: CVe:1 BBe:ALL');
  assert.match(result.text,/portaerei vuota/);
  assert.match(result.text,/cantieri navali rimanenti e futuri/);
  assert.match(result.text,/corazzata vuota \(BBe\)/);
  assert.equal(result.notes.some(n=>/Possibile significato di BBe/.test(n)),false);
 }finally{i18n.setLanguage('english');}
});

test('documented aircraft and ground-design notation is explained without inventing upgrade values',()=>{
 assert.match(reader.read('produce F2ca:5').text,/F2ca \(fighter, with cannons design\)/);
 assert.match(reader.read('design T-I 1 (0\/4\/SL), FL-I2, TS3').text,/T-I 1 \(tank, infantry variant design\)/);
 assert.match(reader.read('design T-I 1 (0\/4\/SL)').text,/0\/4\/smoke launchers \(SL\)/);
 assert.match(reader.read('design AR-H\/S').text,/improved small or high-velocity cannon, or a close support gun/);
 assert.equal(reader.read('Set up the production lines').text,'Set up the production lines');
 assert.equal(reader.read('F2ca',row('checkpoints')).text,'F2ca');
 assert.equal(reader.read('"T-I 1"',row('production')).text,'"T-I 1"');
 const named=row('production');named.action.params.variant_name='T-I 1 (0/4/SL)';
 const bound=reader.read('Produce T-I 1 (0/4/SL)',named);
 assert.match(bound.text,/T-I 1 \(0\/4\/SL\) \(tank, infantry variant · smoke launchers design\)/);
 assert.equal(named.action.params.variant_name,'T-I 1 (0/4/SL)');
});

test('building plurals and production reset and quota conventions are inline',()=>{
 const build=reader.read('Build 3 MILs and CIVs, then INFRAs');
 assert.match(build.text,/military factories \(MILs\)/);assert.match(build.text,/civilian factories \(CIVs\)/);
 assert.match(build.text,/infrastructure \(INFRAs\)/);
 assert.match(reader.read('NAV: BBe:0->ALL').text,/Remove this line’s assignments, then allocate all remaining and future naval dockyards/);
 assert.match(reader.read('MIL: Inf->10').text,/bring to 10 military factories/);
 assert.equal(reader.read('MILs Industries',row('checkpoints')).text,'MILs Industries');
});

test('a confirmed coded naval reference receives inline explanation without changing its identity',()=>{
 const r=row('production',['NAVY: BBe:ALL']);r.action.params.variant_name='BBe';
 const before=structuredClone(r);
 assert.match(reader.read(r.action.params.instructions[0],r).text,/empty battleship \(BBe\)/);
 assert.deepEqual(r,before);
 const stale=structuredClone(vocabulary);stale.conventions.possible_abbreviations={naval:{BBe:'empty battleship'}};
 assert.equal(create(stale).read('NAVY: BBe').notes.length,0);
});

test('occupation wording selects the occupation vocabulary and unexplained codes stay visible',()=>{
 // "LPF + CAV Occupation Law" (no LAW: prefix) is explained, not left raw.
 const law=reader.read('LPF + CAV Occupation Law',row('checkpoints',['LPF + CAV Occupation Law']));
 assert.match(law.text,/Local Police Force \(LPF\)/);
 assert.deepEqual(law.terms.map(t=>t.code),['LPF','CAV']);
 // Distinctive confirmed author codes remain readable in unclassified actions.
 const elsewhere=reader.read('Setup LPF later',row('checkpoints',['Setup LPF later']));
 assert.equal(elsewhere.text,'Setup Local Police Force (LPF) later');
 assert.equal(elsewhere.notes.length,0);
});
