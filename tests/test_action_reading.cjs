'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
const {create}=require('../src/hoi4companion/web/action-reading.js');
const plan=require('../src/hoi4companion/web/action-plan.js'),i18n=require('../src/hoi4companion/web/i18n.js');
const vocabulary=require('../src/hoi4companion/guide_language.json'),reader=create(vocabulary);
const row=(category='checkpoints',instructions=[],extra={})=>({key:'step',name:instructions[0],runtime_status:'ready',tracking:'reminder',action:{kind:'manual',params:{category,instructions}},...extra});
// A line that is not understood in full keeps its source text; meanings move to the glossary.
const kept=(source,r)=>{const out=reader.read(source,r);assert.equal(out.text,source);return out;};
const meaning=(out,code)=>out.terms.find(term=>term.code===code)?.meaning;

test('partly understood lines are never rewritten mid-sentence (UX P3)',()=>{
 try{
  i18n.setLanguage('italian');
  assert.equal(meaning(kept('Never spend PP on events'),'PP'),'political power');
  assert.deepEqual(kept('ARMY: setup*, rename & train').terms,[]);
  const asap=kept('ASAP: Attache to NSPA (100 PP)');
  assert.equal(meaning(asap,'ASAP'),'as soon as the requirements allow');assert.equal(meaning(asap,'PP'),'political power');
  // Lines understood in full are still composed in the reader's language.
  assert.match(reader.read('ARMY: Recruit 13 INF, train ALL').text,/^Esercito: Recluta 13 divisioni · modello di fanteria \(INF\), Addestra tutte le divisioni$/);
 }finally{i18n.setLanguage('english');}
 const r=row('checkpoints',['Never spend PP on events','ASAP: Attache to NSPA (100 PP)']);
 const described=plan.describe(r,{reader});
 assert.deepEqual(described.instructions,r.action.params.instructions);
 assert.deepEqual(described.glossary.map(term=>term.code).sort(),['ASAP','PP']);
});

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
 const costs=kept('Fascist Demagogue (75PP), command (25 CP), navy (20NE)');
 assert.deepEqual(['PP','CP','NE'].map(code=>meaning(costs,code)),['political power','command power','naval experience']);
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
 assert.equal(meaning(kept(source,r),'F2'),'fighter');
 assert.equal(r.action.params.variant_name,'F2');
 const produce=kept('produce F2:5 (from Inf)');assert.equal(meaning(produce,'F2'),'fighter');assert.equal(meaning(produce,'Inf'),'infantry equipment');
 assert.equal(reader.read('NA Aviation').text,'NA Aviation');
});
test('common construction conventions retain their different ordering and original location',()=>{
 assert.equal(reader.read('Build INFRAs + MILs').text,'Build infrastructure, then military factories, in each indicated region before moving to the next');
 assert.equal(reader.read('Build INFRAs, then MILs').text,'Build infrastructure in all indicated regions first, then military factories');
 kept('Build INFRAs + MILs in Berlin');kept('Build INFRAs, then MILs in Berlin');
 const result=kept('Radio UP');assert.equal(meaning(result,'UP'),'next useful upgrade');assert.equal(result.notes.length,0);
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
 const result=kept('Navy: Finish all but cancel extra, then build convoys:5 & BBe: ALL');
 assert.equal(result.notes.some(n=>/Possible meaning of BBe/.test(n)),false);
 assert.match(reader.read('NAVY: BBe: ALL').text,/empty battleship \(BBe\): all remaining and future naval dockyards/);
 assert.doesNotMatch(reader.read('NAVY: BBe: ALL').text,/battlecruiser/);
 assert.equal(meaning(result,'BBe'),'empty battleship');
 const custom=create({conventions:{possible_abbreviations:{naval:{XYZ:'example ship'}}}});
 const uncertain=custom.read('NAVY: XYZ');
 assert.equal(uncertain.text,'NAVY: XYZ');
 assert.match(uncertain.notes[0],/Possible meaning of XYZ: example ship/);
 assert.match(reader.read('NAVY: XYZ').text,/XYZ/);
});

test('regimental companies and documented design accessories are explained in context',()=>{
 const result=kept('ARMY: Add IG, HW, MHW, R-SPAA to INF');
 assert.deepEqual(['IG','HW','MHW','R-SPAA'].map(code=>meaning(result,code)),['infantry guns','heavy weapons','motorized heavy weapons','regimental self-propelled anti-air']);
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
 assert.equal(meaning(kept('produce F2ca:5'),'F2ca'),'fighter, with cannons');
 assert.equal(meaning(kept('design T-I 1 (0/4/SL), FL-I2, TS3'),'T-I 1'),'tank, infantry variant');
 assert.equal(meaning(kept('design T-I 1 (0/4/SL)'),'SL'),'smoke launchers');
 assert.match(kept('design AR-H/S').terms.map(term=>term.meaning).join(' '),/improved small or high-velocity cannon, or a close support gun/);
 assert.equal(reader.read('Set up the production lines').text,'Set up the production lines');
 assert.equal(reader.read('F2ca',row('checkpoints')).text,'F2ca');
 assert.equal(reader.read('"T-I 1"',row('production')).text,'"T-I 1"');
 const named=row('production');named.action.params.variant_name='T-I 1 (0/4/SL)';
 const bound=kept('Produce T-I 1 (0/4/SL)',named);
 assert.equal(meaning(bound,'T-I 1 (0/4/SL)'),'tank, infantry variant · smoke launchers');
 assert.equal(named.action.params.variant_name,'T-I 1 (0/4/SL)');
});

test('building plurals and production reset and quota conventions are inline',()=>{
 const build=kept('Build 3 MILs and CIVs, then INFRAs');
 assert.deepEqual(['MILs','CIVs','INFRAs'].map(code=>meaning(build,code)),['military factories','civilian factories','infrastructure']);
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
 // "LPF + CAV Occupation Law" (no LAW: prefix) is explained in the glossary, not left unexplained.
 const law=kept('LPF + CAV Occupation Law',row('checkpoints',['LPF + CAV Occupation Law']));
 assert.equal(meaning(law,'LPF'),'Local Police Force');
 assert.deepEqual(law.terms.map(t=>t.code),['LPF','CAV']);
 // Distinctive confirmed author codes remain explained in unclassified actions.
 const elsewhere=kept('Setup LPF later',row('checkpoints',['Setup LPF later']));
 assert.equal(meaning(elsewhere,'LPF'),'Local Police Force');
 assert.equal(elsewhere.notes.length,0);
});
