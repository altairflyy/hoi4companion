'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const presentation=require('../src/hoi4companion/web/guide-presentation.js');
const model=require('../src/hoi4companion/web/guide-model.js');
const step=(key,params={},extra={})=>({key,title:key,depends_on:[],action:{kind:'manual',params:{category:'construction',...params}},...extra});
const guide=(steps,extra={})=>({schema_version:1,id:'chronology',tracks:{operations:steps},...extra});
const keys=value=>value.operations.map(row=>row.key);

test('Word Actions positions are a source order, not missing timestamps or runtime dependencies',()=>{
 const source={id:'word',type:'file',text:'[Table row]\n[Column 1]\nActions'};
 const before=step('first',{source_action:{source_id:'word',key:'word-1',ordinal:1}});
 const after=step('second',{source_action:{source_id:'word',key:'word-2',ordinal:2}});
 const input=guide([after,before],{dependency_policy:'explicit',sources:[source]});
 const result=presentation.evaluate(input);
 assert.deepEqual(keys(result),['first','second']);assert.deepEqual(result.operation_order.unplaced,[]);
 assert.deepEqual(result.operations.map(r=>r.depends_on),[[],[]]);
 input.tracks.operations[1].depends_on=['second'];
 assert.deepEqual(keys(presentation.evaluate(input)),['second','first']);
 assert.ok(presentation.evaluate(input).operation_order.conflicts.some(c=>c.basis==='source_action'));
});

test('a rejected temporal edge does not discard compatible constraints on later steps',()=>{
 const input=guide([step('C',{source_seconds:30}),step('A',{source_seconds:10,game_date:'1937.1.1'}),step('B',{source_seconds:20,game_date:'1936.1.1'})]);
 const result=presentation.evaluate(input);
 assert.deepEqual(keys(result),['B','A','C']);
 assert.equal(result.operation_order.conflict_count,1);
 assert.equal(result.operation_order.conflicts[0].basis,'source_time');
});

test('partially rejected timestamp groups preserve compatible earlier relations',()=>{
 const input=guide([step('later',{source_seconds:30}),step('earlier-a',{source_seconds:10},{depends_on:['middle']}),step('earlier-b',{source_seconds:10}),step('middle',{source_seconds:20},{depends_on:['earlier-b']})]);
 const result=presentation.evaluate(input),ordered=keys(result);
 assert.ok(ordered.indexOf('earlier-b')<ordered.indexOf('middle'));
 assert.ok(ordered.indexOf('middle')<ordered.indexOf('earlier-a'));
 assert.ok(ordered.indexOf('earlier-a')<ordered.indexOf('later'));
 assert.ok(ordered.indexOf('earlier-b')<ordered.indexOf('later'));
 assert.ok(result.operation_order.conflict_count>0);
});

test('a linked visual frame is separate from the chronological source timestamp',()=>{
 const input=guide([step('design',{}, {evidence:[{source_id:'video',basis:'explicit',timestamp_seconds:489}]}),step('next',{source_seconds:500})],{
  sources:[{id:'video',type:'youtube',url:'https://www.youtube.com/watch?v=abcdefghijk'}],
  source_inventory:{schema_version:1,items:[{id:'visual',source_id:'video',basis:'visual',timestamp_seconds:511,step_keys:['design']}]}});
 const before=JSON.stringify(input),result=presentation.evaluate(input),design=result.operations.find(r=>r.key==='design');
 assert.deepEqual(design.frame_reference,{url:'https://www.youtube.com/watch?v=abcdefghijk',seconds:511});
 assert.equal(design.source_seconds,489);assert.deepEqual(keys(result),['design','next']);assert.equal(JSON.stringify(input),before);
});

test('a later campaign date never moves an instruction ahead of earlier video actions lacking dates',()=>{
 const input=guide([step('later',{game_date:'1939.3.17',source_seconds:1740}),step('agency',{source_seconds:101}),step('build',{source_seconds:117})]);
 assert.deepEqual(keys(presentation.evaluate(input)),['agency','build','later']);
});

test('start and completion dependencies precede a dependent even when timestamps disagree',()=>{
 for(const field of ['depends_on','depends_on_started']){
  const input=guide([step('after',{source_seconds:10},{[field]:['project']}),step('project',{source_seconds:200})]);
  assert.deepEqual(keys(presentation.evaluate(input)),['project','after'],field);
 }
});

test('dependencies through a hidden focus still constrain the operational list',()=>{
 const input=guide([step('after',{source_seconds:10},{depends_on:['focus']}),step('before',{source_seconds:200})]);
 input.tracks.focus=[{key:'focus',depends_on:['before'],action:{kind:'select_focus',entity:{kind:'focus',id:'F'}}}];
 assert.deepEqual(keys(presentation.evaluate(input)),['before','after']);
});

test('date-only steps follow numeric campaign chronology including unpadded months',()=>{
 assert.deepEqual(keys(presentation.evaluate(guide([step('october',{game_date:'1936.10.1'}),step('february',{game_date:'1936.2.1'}),step('january',{game_date:'1936.1.1'})]))),['january','february','october']);
});

test('a dated video anchor places a date-only instruction without inventing its timestamp',()=>{
 const result=presentation.evaluate(guide([step('later-video',{source_seconds:600,game_date:'1938.1.1'}),step('early-date',{game_date:'1936.1.1'}),step('opening-video',{source_seconds:10,game_date:'1936.1.1'})]));
 const order=keys(result);assert.ok(order.indexOf('early-date')<order.indexOf('later-video'));
 assert.ok(order.indexOf('opening-video')<order.indexOf('later-video'));
 assert.equal(result.operations.find(r=>r.key==='early-date').source_seconds,null);
});

test('zero timestamps and equal-time source order are retained',()=>{
 const input=guide([step('late',{source_seconds:20}),step('zero',{source_seconds:0}),step('same-a',{source_seconds:5}),step('same-b',{source_seconds:5})]);
 assert.deepEqual(keys(presentation.evaluate(input)),['zero','same-a','same-b','late']);
});

test('unplaced instructions retain their relative authored order after dated or timed instructions',()=>{
 const result=presentation.evaluate(guide([step('unknown-a'),step('late',{source_seconds:20}),step('unknown-b'),step('early',{source_seconds:10})]));
 assert.deepEqual(keys(result),['early','late','unknown-a','unknown-b']);
 assert.deepEqual(result.operation_order.unplaced,['unknown-a','unknown-b']);
 assert.equal(result.operations.at(-1).source_seconds,null);
});

test('a prerequisite without timestamps is not buried after its timed dependent',()=>{
 const result=presentation.evaluate(guide([step('after',{source_seconds:10},{depends_on:['unknown']}),step('unknown')]));
 assert.deepEqual(keys(result),['unknown','after']);
});

test('independent video clocks are not compared as if their seconds shared a timeline',()=>{
 const input=guide([
  step('video-a',{source_seconds:200},{evidence:[{source_id:'source-a',basis:'explicit'}]}),
  step('video-b',{source_seconds:10},{evidence:[{source_id:'source-b',basis:'explicit'}]})
 ],{sources:[{id:'source-a',type:'youtube',url:'https://www.youtube.com/watch?v=AAAAAAAAAAA'},{id:'source-b',type:'youtube',url:'https://www.youtube.com/watch?v=BBBBBBBBBBB'}]});
 assert.deepEqual(keys(presentation.evaluate(input)),['video-a','video-b']);
});

test('conflicting source and game dates are reported instead of overriding declared dependencies',()=>{
 const result=presentation.evaluate(guide([step('dependent',{source_seconds:10,game_date:'1936.1.1'},{depends_on:['predecessor']}),step('predecessor',{source_seconds:200,game_date:'1938.1.1'})]));
 assert.deepEqual(keys(result),['predecessor','dependent']);
 assert.ok(result.operation_order.conflicts.some(c=>c.basis==='game_date'));
 assert.ok(result.operation_order.conflicts.some(c=>c.basis==='source_time'));
});

test('documented campaign dates win over contradictory video times without changing either value',()=>{
 const result=presentation.evaluate(guide([step('later',{source_seconds:10,game_date:'1938.1.1'}),step('earlier',{source_seconds:200,game_date:'1936.1.1'})]));
 assert.deepEqual(keys(result),['earlier','later']);
 assert.equal(result.operations[0].source_seconds,200);assert.equal(result.operations[1].game_date,'1938.1.1');
 assert.equal(result.operation_order.conflicts[0].basis,'source_time');
});

test('legacy runtime barriers are not rewritten by chronological presentation',()=>{
 const input=guide([step('late',{source_seconds:200}),step('early',{source_seconds:10})]);
 for(const row of input.tracks.operations)delete row.depends_on;
 const before=model.evaluate(input).runtime,result=presentation.evaluate(input);
 assert.deepEqual(keys(result),['early','late']);assert.deepEqual(result.model.runtime,before);
});

test('presentation sorting is deterministic and does not change runtime priorities or authored data',()=>{
 const input=guide([step('late',{source_seconds:20},{priority:50}),step('early',{source_seconds:10})]);
 const before=JSON.stringify(input),runtime=model.evaluate(input).runtime;
 const first=presentation.evaluate(input),second=presentation.evaluate(input);
 assert.deepEqual(first,second);assert.deepEqual(first.model.runtime,runtime);assert.equal(JSON.stringify(input),before);
 assert.deepEqual(keys(first),['early','late']);assert.equal(first.model.runtime.tracks.operations.next,'late');
});

test('long operational dependency chains are ordered without recursion',()=>{
 const steps=Array.from({length:1200},(_,i)=>step('step-'+i,{source_seconds:1200-i},{depends_on:i?['step-'+(i-1)]:[]}));
 const result=presentation.evaluate(guide(steps));assert.deepEqual(keys(result),steps.map(s=>s.key));
});
