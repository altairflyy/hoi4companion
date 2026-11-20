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
2. [ ] Diagnose. Run the report on the user's trace; write the measured causes and numbers here; reorder/adjust steps 3-6. No code.
3. [ ] Non-blocking HUD loop. Backend requests move to a worker thread (latest result wins, one request in flight); control actions on a
   queue with their own replies; drawing, card drag and buttons never wait on HTTP. Same gates and deadlines. Tests for the mailbox.
4. [ ] Card never blinks on transient unknowns. While HOI4 is foreground and the guide/run is unchanged, keep the last card through an
   unknown delivery, drawn dimmed with a "updating…" mark (never as confirmed) up to a bounded limit (e.g. 30 s); hide at once on a current
   delivery without a card, a guide/run change, HOI4 in background or pause. Highlights keep their present gates. Translation strings.
5. [ ] Backend delivery freshness (content set by step 2): e.g. progress writes and audits never hold the delivery, a fast partial read
   between full reads, geometry sampling off the request path. Rust tests; rebuild.
6. [ ] Transport (only if step 2 shows its overhead matters; today: new loopback TCP connection per request, `Connection: close`, full JSON snapshot each ~16 ms, `native/common/loopback.rs`): keep-alive or a named pipe, smaller replies, and/or push instead of poll: backend pushes HUD deliveries/geometry over the existing
   subscription when they change; the overlay polls only as fallback.
7. [ ] Live check with the user: `/deliver-build`, play ~10 min with the trace on, compare the report with step 1 numbers, update
   `docs/native-overlay.md`, archive this plan.

Each code step: tests, `invariant-reviewer`, focused local commit, rebuild + restart (memory: deliver updated build), tick the box here,
rewrite the Resume prompt. Never push without asking.
