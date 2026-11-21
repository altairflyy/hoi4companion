'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('fs');
const path = require('path');
const { parse, report, format, stats } = require('../scripts/hud-trace-report.cjs');

const sample = fs.readFileSync(path.join(__dirname, 'fixtures', 'hud-trace-sample.jsonl'), 'utf8');

test('percentiles are nearest-rank and ignore non-numbers', () => {
  assert.deepEqual(stats([5, 1, 3, NaN, 2, 4]), { count: 5, p50: 3, p95: 5, max: 5 });
  assert.deepEqual(stats([]), { count: 0, p50: null, p95: null, max: null });
});

test('the report separates transport from backend handling and counts deliveries', () => {
  const { events, invalid } = parse(sample);
  assert.equal(invalid, 1, 'a damaged line is skipped, not fatal');
  const r = report(events);
  assert.equal(r.starts, 2);
  assert.equal(r.iterations, 7);
  assert.deepEqual(r.branches, { native: 5, idle: 1, background: 1 });
  assert.equal(r.timings.observe_round_trip_ms.max, 390);
  assert.equal(r.timings.backend_handling_ms.max, 20);
  assert.equal(r.timings.transport_ms.max, 388, 'transport = round trip - handling');
  assert.equal(r.timings.delivery_acquisition_ms.count, 2);
  assert.equal(r.timings.delivery_acquisition_ms.max, 1400);
  assert.equal(r.timings.action_round_trip_ms.max, 120);
  assert.equal(r.stalls.count, 1);
  assert.equal(r.stalls.worst[0].loop_ms, 400);
  assert.ok(Math.abs(r.unknown_delivery_in_foreground - 1 / 6) < 1e-9);
});

test('session closes are listed once each with their stage, even when a restarted HUD writes them again', () => {
  const t = 1700000000000;
  const lines = [
    { ev: 'iter', t, branch: 'idle', fg: true, close_seq: 4, close_at: t - 50, close_stage: 'audit', close_error: 'Lettura incoerente', close_had_session: true },
    { ev: 'iter', t: t + 900, branch: 'idle', fg: true, close_seq: 5, close_at: t + 800, close_stage: 'open', close_error: 'Processo non trovato', close_had_session: false },
    { ev: 'start', t: t + 1000 },
    { ev: 'iter', t: t + 1100, branch: 'idle', fg: true, close_seq: 5, close_at: t + 800, close_stage: 'open', close_error: 'Processo non trovato', close_had_session: false },
  ];
  const sc = report(lines).session_closes;
  assert.equal(sc.count, 2);
  assert.equal(sc.with_session, 1);
  assert.deepEqual(sc.by_stage, { audit: 1, open: 1 });
  assert.deepEqual(sc.events.map(c => [c.seq, c.stage, c.t]), [[4, 'audit', t - 50], [5, 'open', t + 800]]);
  const text = format(report(lines));
  assert.match(text, /Qualified session closes: 2 \(1 of an open session\), by stage: audit 1, open 1/);
  assert.match(text, /#4 audit: Lettura incoerente/);
  assert.match(text, /#5 open \(no session open\): Processo non trovato/);
  assert.equal(report(parse(sample).events).session_closes.count, 0, 'older traces have none');
});

test('a slow answer is not a stall when the loop itself kept turning', () => {
  const lines = [
    { ev: 'iter', t: 1000, branch: 'native', fg: true, loop_ms: 16, gap_ms: 17 },
    { ev: 'iter', t: 2200, branch: 'native', fg: true, loop_ms: 1200, gap_ms: 20, rt_ms: 1150 },
    { ev: 'iter', t: 2800, branch: 'native', fg: true, loop_ms: 600, gap_ms: 580 },
  ];
  const r = report(lines);
  assert.equal(r.stalls.count, 1, 'only the 580 ms pause of the loop');
  assert.equal(r.stalls.worst[0].loop_ms, 580);
  assert.equal(r.timings.loop_ms.max, 1200);
  assert.equal(r.timings.loop_gap_ms.max, 580);
});

test('blank intervals count only while HOI4 is foreground, with their cause', () => {
  const r = report(parse(sample).events);
  const blanks = r.blanks_in_foreground.intervals;
  assert.deepEqual(blanks.map(b => [b.cause, b.duration_ms, b.ended_by]), [
    ['delivery_unknown', 1000, 'show'],
    ['process_restart', 1116, 'show'],
  ], 'leaving the game is not a blank; a crash while shown is');
  assert.equal(blanks[1].error, 'Native backend is not running');
  assert.equal(r.fatal_errors.length, 1);
  assert.deepEqual(r.blanks_in_foreground.by_cause.delivery_unknown, { count: 1, total_ms: 1000, max_ms: 1000 });
  const text = format(r);
  assert.match(text, /delivery_unknown: 1, total 1000 ms/);
  assert.match(text, /process_restart/);
});

test('geometry spikes say whether the GUI registry was rebuilt, and blanks say why', () => {
  const t = 1700000000000;
  const lines = [
    { ev: 'iter', t, branch: 'native', fg: true, usable: true, card: true, marks: 6, geometry_ms: 900, snap_ms: 880, disc: 1, disc_ms: 860 },
    { ev: 'show', t, fg: true, card: true, marks: 6 },
    { ev: 'iter', t: t + 20, branch: 'native', fg: true, usable: true, card: true, marks: 6, geometry_ms: 400, snap_ms: 390, retry: 1 },
    { ev: 'iter', t: t + 40, branch: 'native', fg: true, usable: true, card: true, marks: 6, geometry_ms: 3, disc: 1, disc_ms: 700, disc_bg: true },
    { ev: 'iter', t: t + 60, branch: 'native', fg: true, usable: false, card: false, marks: 0, error: 'Nessuna indicazione live attuale', native_error: 'Vista focus di un’altra nazione' },
    { ev: 'hide', t: t + 60, fg: true, card: false, marks: 0, cause: 'no_card_or_marks', error: 'Nessuna indicazione live attuale' },
    { ev: 'show', t: t + 560, fg: true, card: true, marks: 1 },
  ];
  const r = report(lines);
  assert.equal(r.slow_geometry.count, 2);
  assert.equal(r.slow_geometry.with_discovery, 1);
  assert.deepEqual(r.slow_geometry.worst.map(g => [g.geometry_ms, g.discovered, g.discovery_ms, g.retry]), [[900, 1, 860, 0], [400, 0, null, 1]]);
  assert.deepEqual([r.registry_rebuilds.count, r.registry_rebuilds.background, r.registry_rebuilds.duration_ms.max], [2, 1, 860]);
  assert.deepEqual(r.unusable_in_foreground.by_error, { 'Nessuna indicazione live attuale': 1 });
  assert.deepEqual(r.unusable_in_foreground.native_errors, { 'Vista focus di un’altra nazione': 1 });
  assert.equal(r.blanks_in_foreground.intervals[0].reason, 'Nessuna indicazione live attuale');
  const text = format(r);
  assert.match(text, /Geometry > 250 ms: 2, 1 with a GUI registry rebuild/);
  assert.match(text, /reason: Nessuna indicazione live attuale/);
  assert.match(text, /GUI registry rebuilds: 2, 1 off the request path/);
});
