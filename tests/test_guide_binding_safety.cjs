'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),crypto=require('node:crypto');
const audit=require('../scripts/audit-guide-bindings.cjs'),benchmark=require('../scripts/benchmark-guides.cjs');
const fixture=()=>JSON.parse(fs.readFileSync('tests/fixtures/guide-binding-safety-audit-v1.json','utf8'));
test('baseline, development expectations and holdout remain byte-identical',()=>{
 const hashes=audit.protectedHashes();assert.equal(Object.keys(hashes).length,3);assert.deepEqual(hashes,audit.PROTECTED);
});
test('narrow shadow guard diagnoses actual weak decision/event collisions without blocking all abbreviations',()=>{
 const r=fixture(),cases=r.decision_shadow_policy.cases;assert.equal(r.scope.documents,50);assert.equal(r.scope.prepared_steps,6996);assert.equal(r.scope.entity_references,4451);assert.equal(cases.length,182);
 assert.deepEqual(r.decision_shadow_policy.narrow_policy_counts,{unchanged_strong_source:148,unchanged_weak_no_event_rival:30,requires_review:4});
 const dangerous=cases.filter(c=>c.narrow_shadow_admission.status==='requires_review');assert.deepEqual(new Set(dangerous.map(c=>c.guide_id)),new Set(['word-19549c55066166c1','word-3894e1e0f74f0a0d','word-67736ac669bdcbf1','word-3a9266c2c389189e']));
 for(const c of dangerous){assert.equal(c.legacy_matches.length,1);assert.equal(c.legacy_matches[0].id,c.saved_id);assert.equal(c.legacy_matches[0].match_evidence,'weak_or_unclassified');assert.ok(c.event_option_rival_count>0);}
 for(const label of ['Repair The Mines','Establish Madrid','Establish Valladolid','War Bonds']){const c=cases.find(c=>c.query.toLowerCase()===label.toLowerCase());assert.ok(c);assert.notEqual(c.narrow_shadow_admission.status,'requires_review');}
});
test('unknown provenance cannot be treated as an implicit inference or an explicit correction',()=>{
 const row={reference:{kind:'decision',id:'a'},source:{replayable:true}},result={matches:[{kind:'decision',id:'a',match_evidence:'weak_or_unclassified'}]},events={matches:[{kind:'event_option',id:'e.a'}]};
 assert.equal(audit.narrowAdmission(row,result,events).status,'not_measured');assert.equal(audit.narrowAdmission(row,result,events,'explicit').status,'out_of_scope_explicit_choice');assert.equal(audit.narrowAdmission(row,result,events,'implicit').status,'requires_review');
 assert.equal(audit.narrowAdmission(row,result,undefined,'implicit').status,'not_measured');assert.equal(audit.narrowAdmission(row,{matches:[{kind:'decision',id:'a',match_evidence:'exact_normalized_alias'}]},events,'implicit').status,'unchanged_strong_source');
});
test('role, person and event-parent conflicts are detected rather than reduced to valid ID existence',()=>{
 const guide={id:'g',tracks:{operations:[{key:'x',action:{params:{advisor_id:'a',role:'high_command',person_id:'wrong',event_id:'e1',event_option_id:'e2.a',decision_id:'d',target_country:'GER',target_state:1}}}]}};
 const r=audit.mechanicalAudit([guide],[{kind:'advisor',id:'a',slot:'political_advisor',character_id:'actual'},{kind:'event_option',id:'e2.a',event_id:'e2'},{kind:'decision',id:'d',targeted:true}]);
 assert.equal(r.counts.advisor_role_conflicts,1);assert.equal(r.counts.advisor_person_conflicts,1);assert.equal(r.counts.event_parent_conflicts,1);assert.equal(r.counts.targeted_decision_conflicts,1);
});
test('identity snapshot detects semantic reassignment even with unchanged key and total count',()=>{
 const g={id:'g',tracks:{operations:[{key:'word-0001--operation-2',action:{kind:'manual',params:{decision_id:'old'}},evidence:[{source_id:'s',quote:'same'}]}]}},changed=structuredClone(g);changed.tracks.operations[0].action.params.decision_id='new';
 assert.equal(benchmark.identitySnapshot(g)[0].key,benchmark.identitySnapshot(changed)[0].key);assert.notEqual(benchmark.identitySnapshot(g)[0].semantics_sha256,benchmark.identitySnapshot(changed)[0].semantics_sha256);
 const a=benchmark.analyze([g]).report,b=structuredClone(a);b.per_guide[0].step_identity=benchmark.identitySnapshot(changed);assert.ok(benchmark.compare(a,b).changes.some(c=>c.metric==='step_identity'&&c.status==='changed'));
});
test('sixty independent source reviews remain proposals and do not overwrite gold expectations',()=>{
 const raw=fs.readFileSync('tests/fixtures/guide-benchmark-development.json'),development=JSON.parse(raw),review=JSON.parse(fs.readFileSync('tests/fixtures/guide-gold-review-v1.json')),byId=new Map(development.cases.map(c=>[c.id,c]));
 assert.equal(crypto.createHash('sha256').update(raw).digest('hex'),review.input_development_sha256);assert.equal(review.human_verified,false);assert.equal(review.cases.length,60);assert.equal(new Set(review.cases.map(c=>c.id)).size,60);assert.equal(Object.keys(review.selection.countries).length,20);
 assert.deepEqual(review.verdict_counts,{proposed_correction:15,uncertain:14,agree:31});
 for(const c of review.cases){const original=byId.get(c.id);assert.ok(original);assert.equal(original.text,c.source_text);assert.equal(original.guide_id,c.guide_id);assert.deepEqual(original.source,c.source);assert.equal(original.review.human_verified,false);assert.equal(original.review.status,'pending');assert.ok(c.justification.length>10);assert.ok(c.issue_or_proposed_amendment);}
 for(const category of ['entity','multiline','trigger','quantity'])assert.ok(review.selection.counts[category]>=15,category);
});
