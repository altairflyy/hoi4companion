# HUD responsiveness plan (2026-11-20)

User report: the HUD is slow and clumsy, sometimes stalls, and sometimes disappears for a few seconds. Target: it should feel
like a real-time overlay (immediate response, never blank while HOI4 is in the foreground), without relaxing any invariant
(read-only toward the game, unknown stays unknown, highlights only with a verified witness, Rust decides guide status).

## What the code does today (read 2026-11-20, not measured)
- Rendering is already DirectComposition + a DXGI composition swap chain (`render.rs`): the drawing technology is not the bottleneck.
- `native/overlay/src/main.rs` `run()` is one thread doing, in order: control actions, a blocking `observe` HTTP request to the backend,
  stability holds, `present`, a 500 ms status file write, then `sleep(16 ms)`. Every backend delay stalls the whole loop.
- Backend `observe` (`native/backend/src/http.rs`, `overlay_latest` + `native_geometry`) reads the cached HUD delivery, then samples GUI
  geometry from process memory inside the request. A delivery older than its 3 s budget is "unknown" (`live_delivery_fresh: false`):
  the guide list is emptied, `usable` is false, the loop takes the idle branch.
- Step card hold `CARD_HOLD` = 4 s (`stability.rs`); highlights hold 150 ms / 750 ms deadline. A full state read was measured at ~1.4 s;
  a progress write or a slow read can push the delivery past 3 s, and past 3 s + 4 s the card goes away: likely the "disappears for a few
  seconds" symptom. HYPOTHESIS until step 1 measures it.
- Possible other causes to rule out in step 1: overlay process exit/restart on a `?` error in `run()`, `renderer.hide()` on the idle
  branch (`current.is_none()`), foreground detection flapping, slow `native_geometry` (> 750 ms deadline).

## Steps (one per session; `/clear` between them; the Resume prompt in `docs/AGENT_HANDOFF.md` says which is next)
1. [x] Measure. Overlay: bounded trace `.local/hud-trace.jsonl` (rotate at ~5 MB): per iteration timestamp, branch (idle/native/capture),
   observe round trip, backend handling time (so transport = round trip - handling), reply size, `geometry_ms`, `state_age_ms`, `live_delivery_fresh`, card shown, marks count, and every hide/blank with its cause;
   process start and fatal error lines. Backend: add delivery age and acquisition/evaluation durations to the `observe` reply if missing.
   `scripts/hud-trace-report.cjs`: p50/p95/max of each timing and the list of blank intervals while HOI4 was foreground, with causes;
   a unit test on a small fixture trace. Tests, invariant-reviewer, commit, rebuild, restart. Then the USER plays ~10 min with the HUD.
   Done 2026-11-20: overlay `trace.rs` writes `hud-trace.jsonl` next to `overlay-status.json` (5 MB x 3 files): `iter` (branch, fg, loop_ms,
   rt_ms, bytes, handling_ms, geometry_ms, state_age_ms, fresh, card, marks, `delivery` timings on each new delivery), `show`/`hide` with cause,
   `action`, `start`, `fatal`. Backend adds `handling_ms` to every `/api/overlay` answer and `delivery_pipeline_ms` (acquisition, progress,
   packet, publish) to the HUD delivery. Report: `node scripts/hud-trace-report.cjs [--data-dir DIR] [--json]`.
2. [x] Diagnose. Done 2026-11-21 on the user's trace (see "Measured 2026-11-21" below). Steps 3-6 reordered from the numbers.
3. [ ] Backend delivery path (was step 5). Never by raising the 3 s budget or holding an old card as confirmed. In order of measured cost:
   a) HUD actions: `invalidate()` must not lock `reading` (held by the poll for a whole audit) and the action must not empty the delivery
   until the next poll: republish one delivery evaluated from the last state plus the new progress right after the progress write.
   b) Audits never expire the delivery: fast probes keep publishing while an integrity audit (and its `app.accept`/publish work) runs,
   or the audit work moves off the delivery thread. c) Fix the cadence: the 250 ms loop meets the 250 ms `reading.cached` window, so
   every second poll is a cache hit and deliveries come every ~510 ms. d) Trace: add the reply `error`/`status` of unusable deliveries
   and the geometry `diagnostics` (`snapshot_ms`, discovery) to `hud-trace.jsonl`, to confirm the hypotheses of C and I below.
   e) If d) confirms GUI registry discovery as the geometry spike, run discovery/`refresh_extensions` off the request path (the request
   answers that panel as unknown meanwhile). Rust tests; rebuild.
4. [ ] Non-blocking HUD loop (was step 3). Backend requests move to a worker thread (latest result wins, one request in flight); control
   actions on a queue with their own replies; drawing, card drag and buttons never wait on HTTP. Same gates and deadlines. Also: the idle
   branch polls at the native cadence (today it sleeps ~100 ms, so every unknown lasts >= 110 ms); one immediate retry of a request that
   fails with no reply before treating it as connection lost (6 seen). Tests for the mailbox.
5. [ ] Card through an unusable delivery (was step 4, changed by I). Measured unknown gaps are <= 1.75 s, already inside `CARD_HOLD` 4 s:
   the "updating…" hold through unknown is no longer the main need. The one blank seen came from a CURRENT delivery without a card. With
   the reason from 3d: if it is a transient unusable read with the guide/run unchanged, keep the last card dimmed with an "updating…" mark
   (never as confirmed) up to a bounded limit; hide at once on a guide/run change, HOI4 in background or a real state without a card.
6. [ ] Transport: NOT needed by the numbers (transport p50 1.7 ms, p95 2.5 ms; 2 outliers ~300 ms in 15.9 min). Close it at step 7 unless
   the step 7 trace disagrees. The failed-request retry moved to step 4.
7. [ ] Live check with the user: `/deliver-build`, play ~10 min with the trace on, compare the report with the numbers below, update
   `docs/native-overlay.md`, archive this plan.

## Measured 2026-11-21 (step 2; trace `.local/hud-trace*.jsonl`, overlay pid 35308)
Session 09:28:52-09:45:45 UTC: 951.5 s with HOI4 foreground, 45k native iterations, 2 overlay starts in the files, 0 fatal errors.
- A. Steady state is fine: native loop p50 16.4 / p95 18.6 / p99 20.8 ms; observe round trip p50 3.9 / p95 9.3 ms; backend handling
  p50 2.0 / p95 7.3 ms; transport p50 1.7 / p95 2.5 ms; reply 21-27 KB. HTTP per frame is not the cause of the slowness.
- B. Whole-loop stalls > 250 ms while foreground: 16, 10.8 s in total. Drawing, card drag and buttons freeze for their duration.
- C. Geometry spikes: 12 requests of 400-1150 ms (about one per minute), handling = `geometry_ms`, so the time is inside
  `GuiLane::geometry`. The GUI registry mutex is not shared with the collector, so it is not a lock wait on the audit. The first geometry
  after start took 1150 ms; most spikes have 5-8 marks (research/production panels). HYPOTHESIS: registry discovery or `refresh_extensions`
  after a panel change (the trace does not record `diagnostics.discovery`; step 3d).
- D. HUD actions: `check` 104-108 ms (3 of 5); `set_aside` and one `check` 1563 ms. Cause in code: `invalidate()` locks `reading`, which
  `poll()` holds for its whole acquisition, and `app_checks`/`app_set_aside` wait for the application lock held by `app.accept`.
  The overlay loop blocks for the whole round trip (loop 1578 ms twice).
- E. After every action the delivery is emptied: highlights off (unknown) for 1.31-1.75 s until the next poll (5 times).
- F. Delivery expiry (3 s budget): 13 times. Probes every ~510 ms take 8-30 ms; an integrity audit every ~6.2 s takes 740-1540 ms of
  acquisition; when the state changed (87 of 1828 deliveries) `app.accept` adds 775-1890 ms and publish 220-480 ms, so no delivery for
  2.3-3.5 s. Most expiries last one idle iteration (~110 ms); worst 09:41:56-09:42:03, three in 7 s (0.4-1.3 s each).
- G. Unknown while foreground: 17.9 s (1.9 %); idle branch 21.4 s; marks 0 for 25.0 s. Also 6 requests with no reply (0 bytes,
  0.3-23 ms) and 2 one-frame unusable deliveries.
- H. Cadence: deliveries every 511 ms p50 (p10 292, p90 812) although the loop is 250 ms: see step 3c.
- I. Card blank (the "disappears for a few seconds" symptom): once, 09:44:25.585 +3330 ms, cause `no_card_or_marks`: a current delivery
  (age 142 ms, ~800 bytes, `usable: false`, not unknown) without a card; ended by a delivery with a card whose first geometry took 579 ms.
  Before it, 3 s with 0 marks and 0.1 ms geometry. The trace does not store the reply's `error`/`status`, so the cause is UNKNOWN
  (hypothesis: a game screen where the qualified read is unusable; the user left HOI4 6 s later). `CARD_HOLD` does not apply to a
  known delivery.
- Background branch sleeps ~110 ms: return to foreground shows within one iteration (09:44:59.612 -> 09:44:59.712).

## Follow-up after step 7 (quality, not responsiveness; added 2026-11-21 at the user's request)
A separate plan, started only after step 7. Each item needs a measurement or a user screenshot before any qualified geometry changes.
- F1. Map pan and zoom: consistent marker coordinates while the map moves, no misalignment or flicker (map marker pan/zoom is unverified,
  see the handoff's open questions).
- F2. Native qualification of edge cases: variable DPI/UI scale, inactive projects, commander roster.
Not adopted from the same list, by the step 2 numbers: fewer WGC captures and less OCR (no capture iteration in the trace), IPC and
reply size (transport p95 2.5 ms), caches across game sessions (addresses change per process). Adaptive GUI polling and Windows
foreground notifications only as CPU savings, if step 7 still shows a cost.

Each code step: tests, `invariant-reviewer`, focused local commit, rebuild + restart (memory: deliver updated build), tick the box here,
rewrite the Resume prompt. Never push without asking.
