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
