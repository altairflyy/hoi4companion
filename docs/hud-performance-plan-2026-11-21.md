# HUD responsiveness plan, part 2 (2026-11-21)

Follows `docs/hud-performance-plan-2026-11-20.md` (archived). Numbers: its section "Measured 2026-11-21, step 7".
Same invariants: read-only toward the game, unknown stays unknown, highlights only with a verified witness, Rust decides guide status.
Never raise the 3 s delivery budget or show an old card or old highlights as current.
Proposed by the step 7 session; order confirmed by the user 2026-11-21.

## Order from 2026-11-21 (user is remote and cannot play; agents never start or control HOI4)
Work that needs no live game first, live checks batched into one session when the user is back at the PC:
- done: step 4 (store reads no longer copy the whole progress state);
- done: step 3b, the reason-independent part of step 3 (`provider::gui_lane` no longer waits on the provider lock during a full read);
- done 2026-11-22: LIVE session (numbers: section "Measured 2026-11-22, live session" below);
- next: step 3a (keep the session through a full-read consistency failure), then step 4b (change-to-delivery still above the target),
  then step 5 (final live check).

## Steps (one per session; `/clear` between them; the Resume prompt in `docs/AGENT_HANDOFF.md` says which is next)
1. [x] Overlay cadence (small). Replace the ~15.6 ms timer rounding of `Wake::wait` / `Reads::wait` (`native/overlay/src/wake.rs`,
   `mailbox.rs`) so the native loop runs at ~16 ms again: for example `timeBeginPeriod(1)` for the HUD process lifetime, or a
   high-resolution waitable timer for the frame wait while a card button still wakes the loop at once. Measure first with a unit or
   manual timing check; trace `gap_ms` p50 should return to ~16 ms. Overlay tests; rebuild.
   Done 2026-11-21: measured first (unit test `wake::tests::a_fine_timer_keeps_short_waits_near_their_timeout`): a 5 ms wait took a
   median 15.5 ms on both the condvar and the channel with the default tick, 5.5 ms with a 1 ms tick. Fix: `wake::FineTimer`
   (`timeBeginPeriod(1)` + power-throttling opt-out) held for the HUD process lifetime; the trace `start` line has `fine_timer`.
   Live `gap_ms` NOT measured yet: step 5.
2. [x] Record why the qualified session closes. `Provider::collect` closes on any error and nothing keeps the error. Add to the provider
   metrics a bounded list of close events (time, stage: probe/full read/audit/tick/identity/open, error text), carry the last one in the
   HUD delivery, write it to `hud-trace.jsonl` (`close_stage`, `close_error`) and list it in `hud-trace-report.cjs`. No behavior change.
   Rust + report tests; rebuild. Then the USER plays ~10 min; read the close reasons before step 3.
   Done 2026-11-21 (code): `collect_inner` tags each failure with its stage; `metrics.closes` keeps the last 16 (`seq`, `stage`,
   `error`, `unix_ms`, `had_session`, `repeats` for the same failure again); the delivery and the expired placeholder carry the last
   one as `provider_close`; the trace writes `close_seq`/`close_stage`/`close_error` once; the report has "Qualified session closes".
   PENDING: the user's ~10 min of play and reading the reasons.
3. [ ] Keep the session through a transient read failure, by the reasons from step 2. Candidates, only where the cause allows it: a failed
   integrity audit drops the cached read but keeps the process handle and the GUI registry (same pid and creation time); a probe or full
   read failure retries the full read once before closing. Every failure still makes the delivery unknown at once; only a new complete
   qualified read restores it. Also stop `provider::gui_lane` from waiting on the provider lock during a full read after a reopen
   (3 geometry spikes of 878-1516 ms). Rust tests; rebuild.
   Done 2026-11-21 (3b, the lock part only): two locks were held through the full read, the HTTP `provider` owner and the
   provider's own mutex. The provider now publishes its GUI lane to a slot shared with its registry entry (set at session open,
   cleared by `close`), read by `provider::gui_lane(handle)` without the provider lock; `http.rs` keeps the handle in
   `provider_handle` (stored before `analysis_generation` moves). The lane may now answer during the first full read after a
   reopen; it still checks identity, pid and `session_id`. Test `the_registry_lane_answers_while_a_collect_holds_the_provider...`.
   Live 2026-11-22: geometry max 14.1 ms, 0 > 250 ms, also across the reopen at 10:14:53: 3b confirmed. 3a stays open.
   3a decided 2026-11-22 from the live closes (all stage `full_read`, no probe/audit/tick failure): on a full-read consistency failure
   ("Stato cambiato durante la lettura" `live_process.rs`, "Campione incoerente: riprovare" `acquisition.rs`, "Stato ricerca vuoto o
   duplicato" `live.rs`) keep the process handle and the GUI registry (same pid and creation time), drop the cached read, make the
   delivery unknown at once and retry the full read at the next tick; close only after N consecutive failures or any other error.
   Leave "Paese giocato non identificato con certezza" (seen once, 0.5 s after start while the game loaded) closing as today.
   Not needed: the audit candidate (0 audit failures seen).
4. [x] Full-read delivery pipeline after a change (p50 2.6 s, p90 3.1 s; 98 expiries in 20 min). Profile first: acquisition, `app.accept`
   (progress), packet, publish. Hypothesis: the 2.0 MB progress payload (2851 runs) is serialized and written whole on each change; if
   so, write only what changed or move the write after the publish. Target: change-to-delivery p90 < 1.5 s. Same effect expected on HUD
   action round trips (1.8-3.2 s), which wait behind it. Rust tests; rebuild.
   Done 2026-11-21: hypothesis wrong. Offline profile (`native/guides/tests/profile_progress.rs`, ignored, on a copy of the local
   store: 2905 runs, 54 active guides, 2.06 MB payload): a full write (`update` + SQLite save) takes ~70 ms; the cost was every READ
   (`get`, `metadata`, `records`) cloning the whole store state, ~8 ms each. `bind()` does a `get` per active guide plus one per
   observed guide, and `live_packet` one per guide after any write: ~0.4-0.9 s each. Fix: `progress::Store` answers read-only ops from
   the committed state (54 `get` 420 ms -> 0.4 ms; `metadata` 10 -> 0.3 ms); the write moves the old state into `pending` instead of
   copying it. Live 2026-11-22: change-to-delivery p50 1603 / p90 1955 ms (was 2634 / 3103): better, target p90 < 1.5 s NOT met.
4b. [ ] Change-to-delivery below p90 1.5 s (new 2026-11-22). Live breakdown of the 199 changed deliveries: acquisition p50 790 / p95 1141,
   progress (`app.accept`) p50 540 / p95 662, publish p50 333 / p95 478, packet ~2 ms. Profile live first which part of
   `app.accept` still takes ~0.5 s after step 4, and what publish does for 0.3 s; HUD actions (715-941 ms, all ~730) likely wait on
   the same work. Rust tests; rebuild.
5. [ ] Live check with the user: play ~10 min with the trace, compare with step 7 of the previous plan, update `docs/native-overlay.md`,
   archive this plan.

## Measured 2026-11-22, live session (build `603225f`, Companion started 09:57 local)
Trace `.local/hud-trace{.2,.1,}.jsonl`: 1003 s (10:09-10:25 local), 58.1k iterations, 0 fatal errors, 11 HUD actions. The first
~12 min rotated out of the 3 x 5 MB files (start line lost); the close list below comes from `/api/state` `metrics.closes`, which
covers the whole backend run.
- Step 1: loop gap p50 16.8 / p95 17.9 ms, loop p95 20 ms (was gap p50 30.9). Confirmed.
- Step 2, closes (backend run 09:57:15-10:25, 4 distinct, 9 with repeats), all stage `full_read`, all with an open session:
  "Paese giocato non identificato con certezza" (+0.5 s, game loading); "Stato cambiato durante la lettura" (+49 s); "Stato ricerca
  vuoto o duplicato" (+49.6 s, x3); "Campione incoerente: riprovare" (+50.4 s, x4, last 10:14:53). In the trace window 2 unknown
  episodes of 1.5 and 1.9 s ("In attesa di una lettura valida della partita", 10:13:44 and 10:14:53), was 6 of 4.7-6.6 s in 20 min.
  Unknown while foreground 0.3 % (was 4.5 %). Decision: step 3a above.
- Step 3b: geometry p50 2.8 / p95 5.9 / max 14.1 ms; 0 > 250 ms (was 3 of 878-1516 ms after a reopen). 5 GUI registry rebuilds, all
  off the request path (scan p50 569, max 634 ms). Loop stalls > 250 ms 0, blank card 0.
- Step 4: change-to-delivery (deliveries with acquisition or progress > 100 ms, 199 of 3168) p50 1603 / p90 1955 / max 2373 ms (was
  2634 / 3103 / 3725). Target p90 < 1.5 s NOT met: step 4b.
- HUD actions: 8 `set_aside` + 3 `check`, 715-941 ms round trip (was 1776-3240 ms).
- Transport p50 1.9 / p95 2.4 ms; observe round trip p50 5.0 / p95 8.2 ms.

Not in this plan: the quality follow-ups F1 (map pan/zoom) and F2 (DPI/UI scale, inactive projects, commander roster) from the previous
plan; they start after step 5.

Each code step: tests, `invariant-reviewer`, focused local commit, rebuild + restart, tick the box here, rewrite the Resume prompt.
Never push without asking.
