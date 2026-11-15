'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
const b=require('../scripts/benchmark-guides.cjs');
function guide(text='Build 2 military factories'){
 return {id:'fixture',title:'Fixture',applies_to:{country:'GER'},sources:[{id:'src',type:'file',title:'Fixture',content_sha256:'abc',text:'[Table row]\n[Column 1]\nACTIONS\n[Column 2]\nRESEARCH PRIORITY*\n[Column 3]\nFOCUS PRIORITY\n[Table row]\n[Column 1]\n'+text+'\n[Column 2]\nBasic Machine Tools\n[Column 3]\nA focus'}],tracks:{operations:[{key:'word-0001',evidence:[{source_id:'src',quote:text}],tracking:'reminder',action:{kind:'manual',params:{instructions:[text]}}}]}};
}
test('source counts keep physical lines separate from original paragraphs and step counts',()=>{
 const g=guide('Build 2 factories\nTrain divisions');const a=b.analyze([g]);assert.equal(a.report.metrics.actions_physical_lines,2);assert.equal(a.report.metrics.installed_steps,1);assert.equal(a.report.per_guide[0].original_paragraphs,null);assert.equal(a.report.metrics.opaque_multiline_reminder_steps,1);assert.equal(a.report.quality_accuracy.status,'not_measured');
 const source={document_sha256:'abc',paragraphs:[{table_index:2,row_index:2,column_index:1,paragraph_index:1,text:'Build 2 factories\nTrain divisions '}]};const result=b.diagnoseGuide(g,g,source);assert.equal(result.summary.source_status,'original_docx_verified');assert.equal(result.summary.original_paragraphs,1);assert.equal(result.records[1].text,'Train divisions ');
});
test('a source mismatch cannot silently substitute current ZIP text for installed guide source',()=>{
 const g=guide('Original line'),before=JSON.stringify(g),result=b.diagnoseGuide(g,g,{document_sha256:'abc',paragraphs:[{text:'New line'}]});assert.equal(result.summary.source_status,'original_text_mismatch');assert.equal(result.records[0].text,'Original line');assert.equal(JSON.stringify(g),before);
});
test('payload presence and automatic predicates are independent of correctness and each other',()=>{
 const g=guide(),step=g.tracks.operations[0];step.action.params.building_name='Military factory';delete step.tracking;
 let a=b.analyze([g]);assert.equal(a.records[0].diagnostic_status,'typed_payload_present');assert.equal(a.records[0].verification.automatic_completion,false);assert.equal(a.report.quality_accuracy.status,'not_measured');
 delete step.action.params.building_name;step.completion={op:'fact',key:'a',test:'eq',value:true};a=b.analyze([g]);assert.equal(a.records[0].diagnostic_status,'untyped');assert.equal(a.records[0].verification.automatic_completion,true);
 assert.equal(b.automatic({op:'all',args:[{op:'fact',key:'a'},{op:'check',key:'b'}]}),false);assert.equal(b.automatic({op:'all',args:[]}),false);
});
test('unknown runtime failures are unmeasured rather than reused installed interpretation',()=>{
 const g=guide(),a=b.analyze([g],{runtime:{guides:[{id:g.id,error:'fixture error'}]}});assert.equal(a.report.metrics.runtime_errors,1);assert.equal(a.report.per_guide[0].runtime_steps,null);assert.equal(a.records[0].diagnostic_status,'not_measured');
});
test('advice and conjunction/trigger tags are explicitly heuristic and never executable',()=>{
 assert.equal(b.tags('Good Job!').disposition,'advice_candidate');assert.equal(b.tags('Ignore trade deficits of 1-2').disposition,'unclassified');
 const t=b.tags('After war: when ready, recruit INF and hire advisor (150PP)');for(const tag of ['trigger','trigger_header','nested_or_multiple_conditions','conjunction','pp_cost','abbreviation'])assert.ok(t.syntax.includes(tag),tag);
});
test('static country checks preserve unknown, alternatives, negation and absent original identity',()=>{
 assert.equal(b.countryCondition({op:'original_tag',value:'GER'},{tag:'SOV'}),null);
 assert.equal(b.countryCondition({op:'all',args:[{op:'original_tag',value:'GER'},{op:'unknown'}]},{original_tag:'SOV'}),false);
 assert.equal(b.countryCondition({op:'any',args:[{op:'original_tag',value:'GER'},{op:'unknown'}]},{original_tag:'SOV'}),null);
 assert.equal(b.countryCondition({op:'not',arg:{op:'unknown'}},{}),null);
});
test('foreign prefixes are not evidence of invalid binding and duplicate fields do not inflate identities',()=>{
 const g=guide('Repair mines'),step=g.tracks.operations[0];step.action.params.decision_id='SWE_repair';step.action.entity={kind:'decision',id:'SWE_repair'};
 const r=b.bindingAudit([g],{entries:[{kind:'decision',id:'SWE_repair',country_condition:{op:'unknown'}}]});assert.equal(r.by_kind.decision,1);assert.equal(r.counts.country_gate_unknown,1);assert.equal(r.issues.length,0);
 const invalid=b.bindingAudit([g],{entries:[{kind:'decision',id:'SWE_repair',country_condition:{op:'original_tag',value:'SOV'}}]});assert.equal(invalid.counts.contradiction_under_declared_origin,1);assert.match(invalid.context_assumption,/assumed/);
});
test('ambiguous or missing catalog definitions stay visible instead of choosing a first match',()=>{
 const g=guide();g.tracks.operations[0].action.params.decision_id='same';
 assert.equal(b.bindingAudit([g],{entries:[]}).counts.id_absent_or_scope_mismatch,1);
 assert.equal(b.bindingAudit([g],{entries:[{kind:'decision',id:'same'},{kind:'decision',id:'same'}]}).counts.multiple_catalog_definitions,1);
});
test('comparison rejects changed corpus/layers and detects same-count key semantic reassignment',()=>{
 const g=guide(),before=b.analyze([g]).report;const current=structuredClone(before);current.per_guide[0].step_identity[0].semantics_sha256='changed';const r=b.compare(before,current);assert.equal(r.comparable,true);assert.ok(r.changes.some(c=>c.metric==='step_identity'&&c.step_key==='word-0001'));
 current.dataset_sha256='different';assert.equal(b.compare(before,current).comparable,false);current.dataset_sha256=before.dataset_sha256;current.interpretation_layer='other';assert.equal(b.compare(before,current).comparable,false);
});
test('source-only sample is deterministic, representative and has disjoint guide/text holdout',()=>{
 const rows=[];for(let guide=0;guide<48;guide++)for(let line=0;line<12;line++)rows.push({id:`${guide}-${line}`,guide_id:`guide-${guide}`,country:'TST',text:`Guide ${guide} instruction ${line}`,block_text:'block',families:[line%2?'army':'politics'],syntax:[line%3?'conjunction':'trigger'],disposition:'action_candidate'});
 const sample=b.sample(rows,'dataset'),again=b.sample([...rows].reverse(),'dataset');assert.equal(sample.cases.length,400);assert.deepEqual(sample,again);
 const dev=sample.cases.filter(c=>c.split==='development'),holdout=sample.cases.filter(c=>c.split==='holdout');assert.equal(dev.length,320);assert.equal(holdout.length,80);const ids=new Set(dev.map(c=>c.guide_id)),text=new Set(dev.map(c=>b.norm(c.text).toLowerCase()));assert.ok(holdout.every(c=>!ids.has(c.guide_id)&&!text.has(b.norm(c.text).toLowerCase())));assert.equal(new Set(sample.cases.map(c=>c.guide_id)).size,48);assert.ok(sample.cases.every(c=>c.review.human_verified===false));
});
test('agent proposals never count as verified gold or leak holdout into default scoring',()=>{
 const record={id:'a',observed_steps:[]},gold={cases:[{id:'a',split:'development',review:{status:'pending',human_verified:false}},{id:'h',split:'holdout',review:{status:'approved',human_verified:true,reviewed_by:'synthetic-test-only',reviewed_at:'2000-01-01'},acceptance:[{kind:'no_structured_action'}]}]};const r=b.quality(gold,[record]);assert.equal(r.total_cases,1);assert.equal(r.human_approved,0);assert.equal(r.accuracy,null);assert.equal(r.assertion_coverage,0);
});
test('approved executable assertions fail safely and report individual partial/wrong outcomes',()=>{
 const review={status:'approved',human_verified:true,reviewed_by:'synthetic-test-only',reviewed_at:'2000-01-01'},record={id:'a',diagnostic_status:'typed_payload_present',assertion_attribution_complete:true,observed_steps:[{key:'x',action:{params:{law_id:'wrong'}}}]};const gold={cases:[{id:'a',guide_id:'fixture',split:'development',review,acceptance:[{kind:'step_count',equals:1},{kind:'field_equals',path:'action.params.law_id',equals:'correct'}]}]};const r=b.quality(gold,[record]);assert.equal(r.counts.partially_meets_approved_assertions,1);assert.equal(r.accuracy,0);
 assert.equal(b.assertionResult({kind:'field_equals',path:'missing'},record),null);assert.equal(b.assertionResult({kind:'field_equals',path:'__proto__.x',equals:1},record),null);assert.equal(b.assertionResult({kind:'field_equals',path:'key',equals:'x',quantifier:'all'},{observed_steps:[]}),false);
});

test('missing runtime or ambiguous/unmapped source linkage can never pass empty-action assertions',()=>{
 const review={status:'approved',human_verified:true,reviewed_by:'synthetic-test-only',reviewed_at:'2000-01-01'},gold={cases:[{id:'a',split:'development',review,acceptance:[{kind:'no_structured_action'},{kind:'step_count',equals:0}]}]};
 for(const diagnostic_status of ['not_measured','unmapped']){const result=b.quality(gold,[{id:'a',diagnostic_status,observed_steps:[]}]);assert.equal(result.accuracy,null);assert.equal(result.counts.not_measured,1);}
 assert.equal(b.assertionResult({kind:'field_equals',path:'key',equals:'x',quantifier:'bogus'},{observed_steps:[{key:'x'}]}),null);
});

test('a complete multiline quote is attributed once and cannot capture a later repeated phrase',()=>{
 const g=guide('Train divisions\nBuild factory\nTrain divisions'),step=g.tracks.operations[0];step.evidence[0].quote='Train divisions\nBuild factory';step.action.params.instructions=['Train divisions'];step.action.params.operation_id='train';delete step.tracking;
 const original={document_sha256:'abc',paragraphs:[{text:'Train divisions\nBuild factory',coordinate_kind:'original_docx'},{text:'Train divisions',coordinate_kind:'original_docx'}]};
 const r=b.diagnoseGuide(g,g,original);assert.equal(r.records[0].diagnostic_status,'typed_payload_present');assert.equal(r.records[2].diagnostic_status,'unmapped');assert.deepEqual(r.records[2].linked_step_keys,[]);
 const ambiguous=b.attributeGroups([{text:'Train'},{text:'Train'}],[{key:'a',quote:'Train',steps:[]}]);assert.equal(ambiguous.assignments.size,0);assert.equal(ambiguous.ambiguous.size,2);
 const anchored=b.attributeGroups([{text:'Train',coordinate_kind:'original_docx'},{text:'Train',coordinate_kind:'original_docx'}],[{key:'a',quote:'Train',ordinal:2,steps:[]}]);assert.equal(anchored.assignments.has(0),false);assert.equal(anchored.assignments.has(1),true);
});

test('overlapping ambiguous groups cannot hide behind a known parent assignment',()=>{
 const g=guide('A\nB\nB'),first=g.tracks.operations[0];first.evidence[0].quote='A\nB';first.action.params.instructions=['A','B'];g.tracks.operations.push({key:'word-0002',evidence:[{source_id:'src',quote:'B'}],tracking:'reminder',action:{kind:'manual',params:{instructions:['B']}}});
 const r=b.diagnoseGuide(g,g,{document_sha256:'abc',paragraphs:[{text:'A\nB'},{text:'B'}]});assert.equal(r.records[1].source_attribution,'ambiguous');assert.equal(r.records[1].assertion_attribution_complete,false);assert.equal(r.records[1].diagnostic_status,'unmapped');
});

test('normalized group members are not discarded when asserting absence or number of actions',()=>{
 const g=guide('Build factories'),first=g.tracks.operations[0];g.tracks.operations.push({key:'word-0001--operation-2',evidence:first.evidence,action:{kind:'manual',params:{building_id:'industrial_complex',instructions:['Construct factories']}}});
 const r=b.diagnoseGuide(g,g);assert.equal(r.records[0].observed_steps.length,2);assert.equal(r.records[0].assertion_attribution_complete,true);assert.equal(b.assertionResult({kind:'no_structured_action'},r.records[0]),false);
 const multi=guide('Build factories\nTrain units');multi.tracks.operations.push({key:'word-0001--operation-2',evidence:multi.tracks.operations[0].evidence,action:{kind:'manual',params:{building_id:'industrial_complex',instructions:['Construct factories']}}});assert.equal(b.diagnoseGuide(multi,multi).records[0].assertion_attribution_complete,false);
});

test('partial runtime capture exposes partial subtotals instead of presenting missing guides as zero',()=>{
 const g=guide(),a=b.analyze([g],{runtime:{guides:[{id:g.id,error:'failed'}]}}).report;assert.equal(a.metrics.runtime_steps,null);assert.equal(a.metrics.runtime_steps_measured,0);assert.equal(a.metrics.opaque_multiline_reminder_steps,null);assert.equal(a.metrics.verification,null);assert.equal(a.measurement_coverage.source_lines_not_measured,1);
});
test('checked-in real binding examples remain proposed and retain exact source/script proof',()=>{
 const f=JSON.parse(fs.readFileSync('tests/fixtures/guide-benchmark-bindings.json','utf8'));assert.equal(f.cases.length,6);assert.equal(f.annotation_status,'agent_proposed');assert.equal(f.human_review,'pending');for(const c of f.cases){assert.ok(c.source.content_sha256);assert.ok(c.source.quote);assert.ok(c.saved_binding.decision_id);}
});

test('frozen development and holdout cover 400 source lines without claiming human validation',()=>{
 const development=JSON.parse(fs.readFileSync('tests/fixtures/guide-benchmark-development.json')),holdout=JSON.parse(fs.readFileSync('tests/fixtures/guide-benchmark-holdout.json')),cases=[...development.cases,...holdout.cases];
 assert.equal(development.cases.length,320);assert.equal(holdout.cases.length,80);assert.equal(development.dataset_sha256,holdout.dataset_sha256);assert.equal(new Set(cases.map(c=>c.id)).size,400);assert.equal(new Set(cases.map(c=>c.guide_id)).size,48);
 const guides=new Set(development.cases.map(c=>c.guide_id)),texts=new Set(development.cases.map(c=>b.norm(c.text).toLowerCase()));
 for(const c of holdout.cases){assert.equal(c.split,'holdout');assert.equal(guides.has(c.guide_id),false);assert.equal(texts.has(b.norm(c.text).toLowerCase()),false);}
 for(const c of cases){assert.equal(c.review.human_verified,false);assert.equal(c.review.status,'pending');assert.equal(c.review.annotation_origin,'agent_reviewed_proposal');assert.ok(c.proposed_expected.justification.length>10);assert.ok(c.source.sha256);assert.ok(c.source.coordinate.actions_line_ordinal>0);assert.ok(c.context.block.includes(c.text));}
});

test('semantic comparison cannot treat an unbound external snapshot or changed catalog as a regression',()=>{
 const a=b.analyze([guide()]).report;a.interpretation_layer='rust_runtime_prepared';a.input_fingerprints={runtime_input_identity_status:'verified_before_and_after_capture',catalog_sha256:'one'};
 const c=structuredClone(a);c.input_fingerprints.runtime_input_identity_status='unbound_external_snapshot';assert.equal(b.compare(a,c).comparable,false);c.input_fingerprints.runtime_input_identity_status='verified_before_and_after_capture';c.input_fingerprints.catalog_sha256='two';assert.equal(b.compare(a,c).comparable,false);
});

test('runtime comparisons require matching complete input, vocabulary and catalog fingerprints',()=>{
 const a=b.analyze([guide()]).report;a.interpretation_layer='rust_runtime_prepared';a.input_fingerprints={runtime_input_identity_status:'verified_before_and_after_capture',input_identity:'inputs',vocabulary:'vocabulary',catalog_sha256:'catalog',tooling_sha256:'tool'};
 assert.equal(b.compare(a,structuredClone(a)).comparable,true);
 for(const key of ['input_identity','vocabulary','catalog_sha256','tooling_sha256']){const c=structuredClone(a);c.input_fingerprints[key]='changed';assert.equal(b.compare(a,c).comparable,false);c.input_fingerprints[key]=null;assert.equal(b.compare(a,c).comparable,false);}
});
