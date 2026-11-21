#!/usr/bin/env node
'use strict';
// Summarizes the HUD responsiveness trace written by the overlay
// (`hud-trace.jsonl` next to `overlay-status.json`, rotated to `.1`/`.2`).
// Prints p50/p95/max of every timing and the intervals in which the HUD was
// blank while HOI4 was in the foreground, with their cause.
//
//   node scripts/hud-trace-report.cjs [trace.jsonl ...] [--data-dir DIR] [--json]
//
// Without files it reads DIR/hud-trace.2.jsonl, .1.jsonl and .jsonl in that
// order (oldest first); DIR defaults to HOI4COMPANION_DATA_DIR or `.local`.
const fs = require('fs');
const path = require('path');

const STALL_MS = 250;

function parse(text) {
  const events = [];
  let invalid = 0;
  for (const line of String(text).split(/\r?\n/)) {
    if (!line.trim()) continue;
    try {
      const event = JSON.parse(line);
      if (event && typeof event === 'object' && typeof event.ev === 'string') events.push(event);
      else invalid++;
    } catch { invalid++; }
  }
  return { events, invalid };
}

function quantile(sorted, q) {
  if (!sorted.length) return null;
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(q * sorted.length) - 1));
  return sorted[index];
}
function stats(values) {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!sorted.length) return { count: 0, p50: null, p95: null, max: null };
  return { count: sorted.length, p50: quantile(sorted, 0.5), p95: quantile(sorted, 0.95), max: sorted[sorted.length - 1] };
}

function report(events) {
  const iterations = events.filter(e => e.ev === 'iter');
  const series = {
    observe_round_trip_ms: [], backend_handling_ms: [], transport_ms: [], reply_bytes: [], geometry_ms: [],
    state_age_ms: [], loop_ms: [], delivery_acquisition_ms: [], delivery_progress_ms: [], delivery_packet_ms: [],
    delivery_publish_ms: [], action_round_trip_ms: [],
  };
  const branches = {};
  let foreground = 0, unknownForeground = 0;
  const stalls = [], slowGeometry = [], unusable = {}, nativeErrors = {};
  for (const e of iterations) {
    branches[e.branch] = (branches[e.branch] || 0) + 1;
    if (e.fg) { foreground++; if (e.fresh === false) unknownForeground++; }
    if (Number.isFinite(e.rt_ms)) series.observe_round_trip_ms.push(e.rt_ms);
    if (Number.isFinite(e.handling_ms)) series.backend_handling_ms.push(e.handling_ms);
    if (Number.isFinite(e.rt_ms) && Number.isFinite(e.handling_ms)) series.transport_ms.push(Math.max(0, e.rt_ms - e.handling_ms));
    if (Number.isFinite(e.rt_ms) && Number.isFinite(e.bytes)) series.reply_bytes.push(e.bytes);
    if (Number.isFinite(e.geometry_ms)) {
      series.geometry_ms.push(e.geometry_ms);
      if (e.geometry_ms > STALL_MS) {
        slowGeometry.push({ t: e.t, geometry_ms: e.geometry_ms, snap_ms: e.snap_ms ?? null, discovered: e.disc || 0,
          discovery_ms: e.disc_ms ?? null, refresh_ms: e.refresh_ms ?? null, retry: e.retry || 0, marks: e.marks });
      }
    }
    if (e.fg && e.usable === false) { const key = e.error || '(no error)'; unusable[key] = (unusable[key] || 0) + 1; }
    if (e.fg && e.native_error) nativeErrors[e.native_error] = (nativeErrors[e.native_error] || 0) + 1;
    if (Number.isFinite(e.state_age_ms)) series.state_age_ms.push(e.state_age_ms);
    if (Number.isFinite(e.loop_ms)) {
      series.loop_ms.push(e.loop_ms);
      if (e.loop_ms > STALL_MS) stalls.push({ t: e.t, loop_ms: e.loop_ms, branch: e.branch, rt_ms: e.rt_ms ?? null });
    }
    const d = e.delivery;
    if (d && typeof d === 'object') {
      for (const [key, name] of [['acquisition', 'delivery_acquisition_ms'], ['progress', 'delivery_progress_ms'], ['packet', 'delivery_packet_ms'], ['publish', 'delivery_publish_ms']]) {
        if (Number.isFinite(d[key])) series[name].push(d[key]);
      }
    }
  }
  for (const e of events) if (e.ev === 'action' && Number.isFinite(e.rt_ms)) series.action_round_trip_ms.push(e.rt_ms);

  // Blank intervals while HOI4 is foreground: from a `hide` in the foreground
  // to the next `show`, until HOI4 leaves the foreground, or to the end. A
  // process restart while something was shown is a blank of its own.
  const blanks = [];
  let open = null, shown = false, last = null, error = null;
  const close = (end, endedBy) => {
    if (open) blanks.push({ start: open.start, end, duration_ms: end != null && open.start != null ? end - open.start : null, cause: open.cause, ended_by: endedBy, ...(open.error ? { error: open.error } : {}), ...(open.reason ? { reason: open.reason } : {}) });
    open = null;
  };
  for (const e of events) {
    if (e.ev === 'start') {
      if (shown && last && last.fg !== false) { close(last.t, 'restart'); open = { start: last.t, cause: 'process_restart', error }; }
      else close(e.t, 'restart');
      shown = false; error = null;
    } else if (e.ev === 'fatal') {
      error = e.error;
      if (open) open.error = e.error;
    } else if (e.ev === 'hide') {
      shown = false;
      if (e.fg && !open) open = { start: e.t, cause: e.cause || 'unknown', ...(e.error ? { reason: e.error } : {}) };
    } else if (e.ev === 'show') {
      shown = true;
      close(e.t, 'show');
    } else if (e.ev === 'iter' && e.fg === false && open) {
      close(e.t, 'game_background');
    }
    if (e.ev === 'iter') last = e;
  }
  if (open) close(last ? last.t : null, 'end_of_trace');
  const causes = {};
  for (const b of blanks) {
    const c = causes[b.cause] || (causes[b.cause] = { count: 0, total_ms: 0, max_ms: 0 });
    c.count++;
    if (Number.isFinite(b.duration_ms)) { c.total_ms += b.duration_ms; c.max_ms = Math.max(c.max_ms, b.duration_ms); }
  }
  const timings = {};
  for (const [name, values] of Object.entries(series)) timings[name] = stats(values);
  const times = events.map(e => e.t).filter(Number.isFinite);
  return {
    span_ms: times.length ? Math.max(...times) - Math.min(...times) : 0,
    starts: events.filter(e => e.ev === 'start').length,
    fatal_errors: events.filter(e => e.ev === 'fatal').map(e => ({ t: e.t, error: e.error })),
    iterations: iterations.length,
    branches,
    foreground_iterations: foreground,
    unknown_delivery_in_foreground: foreground ? unknownForeground / foreground : 0,
    timings,
    stalls: { threshold_ms: STALL_MS, count: stalls.length, worst: stalls.sort((a, b) => b.loop_ms - a.loop_ms).slice(0, 10) },
    blanks_in_foreground: { count: blanks.length, by_cause: causes, intervals: blanks },
    // Geometry spikes with what they did: a registry rebuild or not.
    slow_geometry: { threshold_ms: STALL_MS, count: slowGeometry.length, with_discovery: slowGeometry.filter(g => g.discovered > 0).length,
      worst: slowGeometry.sort((a, b) => b.geometry_ms - a.geometry_ms).slice(0, 10) },
    unusable_in_foreground: { by_error: unusable, native_errors: nativeErrors },
  };
}

function format(r) {
  const n = v => (v == null ? '-' : Number.isInteger(v) ? String(v) : v.toFixed(1));
  const time = t => (Number.isFinite(t) ? new Date(t).toISOString().slice(11, 23) : '?');
  const out = [];
  out.push(`HUD trace: ${(r.span_ms / 1000).toFixed(1)} s, ${r.iterations} iterations, ${r.starts} process start(s), ${r.fatal_errors.length} fatal error(s)`);
  out.push(`Branches: ${Object.entries(r.branches).map(([k, v]) => `${k} ${v}`).join(', ') || '-'}`);
  out.push(`Unknown delivery while HOI4 foreground: ${(r.unknown_delivery_in_foreground * 100).toFixed(1)}% of ${r.foreground_iterations}`);
  out.push('');
  out.push('timing                         count      p50      p95      max');
  for (const [name, s] of Object.entries(r.timings)) {
    out.push(`${name.padEnd(28)} ${String(s.count).padStart(7)} ${n(s.p50).padStart(8)} ${n(s.p95).padStart(8)} ${n(s.max).padStart(8)}`);
  }
  out.push('');
  out.push(`Loop stalls > ${r.stalls.threshold_ms} ms: ${r.stalls.count}`);
  for (const s of r.stalls.worst) out.push(`  ${time(s.t)} ${n(s.loop_ms)} ms (${s.branch}, observe ${n(s.rt_ms)} ms)`);
  out.push('');
  out.push(`Blank while HOI4 foreground: ${r.blanks_in_foreground.count}`);
  for (const [cause, c] of Object.entries(r.blanks_in_foreground.by_cause)) out.push(`  ${cause}: ${c.count}, total ${n(c.total_ms)} ms, longest ${n(c.max_ms)} ms`);
  for (const b of r.blanks_in_foreground.intervals.slice(0, 30)) {
    out.push(`  ${time(b.start)} +${n(b.duration_ms)} ms ${b.cause} (ended by ${b.ended_by})${b.error ? ` error: ${b.error}` : ''}${b.reason ? ` reason: ${b.reason}` : ''}`);
  }
  const g = r.slow_geometry;
  out.push('');
  out.push(`Geometry > ${g.threshold_ms} ms: ${g.count}, ${g.with_discovery} with a GUI registry rebuild`);
  for (const s of g.worst) {
    out.push(`  ${time(s.t)} ${n(s.geometry_ms)} ms (snapshot ${n(s.snap_ms)}, rebuilds ${s.discovered} ${n(s.discovery_ms)} ms, event scan ${n(s.refresh_ms)} ms, retries ${s.retry}, marks ${s.marks})`);
  }
  const u = r.unusable_in_foreground;
  if (Object.keys(u.by_error).length) out.push('', 'Unusable deliveries while HOI4 foreground (iterations):');
  for (const [error, count] of Object.entries(u.by_error).sort((a, b) => b[1] - a[1])) out.push(`  ${count} ${error}`);
  if (Object.keys(u.native_errors).length) out.push('', 'Native geometry errors while HOI4 foreground (iterations):');
  for (const [error, count] of Object.entries(u.native_errors).sort((a, b) => b[1] - a[1])) out.push(`  ${count} ${error}`);
  for (const f of r.fatal_errors) out.push(`Fatal ${time(f.t)}: ${f.error}`);
  return out.join('\n');
}

function defaultFiles(dir) {
  return ['hud-trace.2.jsonl', 'hud-trace.1.jsonl', 'hud-trace.jsonl'].map(f => path.join(dir, f)).filter(f => fs.existsSync(f));
}

function main(argv) {
  const files = [];
  let dir = process.env.HOI4COMPANION_DATA_DIR || '.local', json = false;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--json') json = true;
    else if (argv[i] === '--data-dir') dir = argv[++i];
    else files.push(argv[i]);
  }
  const inputs = files.length ? files : defaultFiles(dir);
  if (!inputs.length) {
    console.error(`No HUD trace found in ${dir} (hud-trace.jsonl). Start the HUD, play, then run this again.`);
    return 1;
  }
  const { events, invalid } = parse(inputs.map(f => fs.readFileSync(f, 'utf8')).join('\n'));
  const result = report(events);
  result.files = inputs;
  result.invalid_lines = invalid;
  console.log(json ? JSON.stringify(result, null, 2) : format(result));
  return 0;
}

if (require.main === module) process.exitCode = main(process.argv.slice(2));
module.exports = { parse, report, format, stats, main };
