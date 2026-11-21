# HUD responsiveness plan, part 2 (2026-11-21)

Follows `docs/hud-performance-plan-2026-11-20.md` (archived). Numbers: its section "Measured 2026-11-21, step 7".
Same invariants: read-only toward the game, unknown stays unknown, highlights only with a verified witness, Rust decides guide status.
Never raise the 3 s delivery budget or show an old card or old highlights as current.
Proposed by the step 7 session; order confirmed by the user 2026-11-21.

## Steps (one per session; `/clear` between them; the Resume prompt in `docs/AGENT_HANDOFF.md` says which is next)
1. [x] Overlay cadence (small). Replace the ~15.6 ms timer rounding of `Wake::wait` / `Reads::wait` (`native/overlay/src/wake.rs`,
   `mailbox.rs`) so the native loop runs at ~16 ms again: for example `timeBeginPeriod(1)` for the HUD process lifetime, or a
   high-resolution waitable timer for the frame wait while a card button still wakes the loop at once. Measure first with a unit or
   manual timing check; trace `gap_ms` p50 should return to ~16 ms. Overlay tests; rebuild.
   Done 2026-11-21: measured first (unit test `wake::tests::a_fine_timer_keeps_short_waits_near_their_timeout`): a 5 ms wait took a
   median 15.5 ms on both the condvar and the channel with the default tick, 5.5 ms with a 1 ms tick. Fix: `wake::FineTimer`
   (`timeBeginPeriod(1)` + power-throttling opt-out) held for the HUD process lifetime; the trace `start` line has `fine_timer`.
   Live `gap_ms` NOT measured yet: step 5.
2. [ ] Record why the qualified session closes. `Provider::collect` closes on any error and nothing keeps the error. Add to the provider
   metrics a bounded list of close events (time, stage: probe/full read/audit/tick/identity/open, error text), carry the last one in the
   HUD delivery, write it to `hud-trace.jsonl` (`close_stage`, `close_error`) and list it in `hud-trace-report.cjs`. No behavior change.
   Rust + report tests; rebuild. Then the USER plays ~10 min; read the close reasons before step 3.
3. [ ] Keep the session through a transient read failure, by the reasons from step 2. Candidates, only where the cause allows it: a failed
   integrity audit drops the cached read but keeps the process handle and the GUI registry (same pid and creation time); a probe or full
   read failure retries the full read once before closing. Every failure still makes the delivery unknown at once; only a new complete
   qualified read restores it. Also stop `provider::gui_lane` from waiting on the provider lock during a full read after a reopen
   (3 geometry spikes of 878-1516 ms). Rust tests; rebuild.
4. [ ] Full-read delivery pipeline after a change (p50 2.6 s, p90 3.1 s; 98 expiries in 20 min). Profile first: acquisition, `app.accept`
   (progress), packet, publish. Hypothesis: the 2.0 MB progress payload (2851 runs) is serialized and written whole on each change; if
   so, write only what changed or move the write after the publish. Target: change-to-delivery p90 < 1.5 s. Same effect expected on HUD
   action round trips (1.8-3.2 s), which wait behind it. Rust tests; rebuild.
5. [ ] Live check with the user: play ~10 min with the trace, compare with step 7 of the previous plan, update `docs/native-overlay.md`,
   archive this plan.

Not in this plan: the quality follow-ups F1 (map pan/zoom) and F2 (DPI/UI scale, inactive projects, commander roster) from the previous
plan; they start after step 5.

Each code step: tests, `invariant-reviewer`, focused local commit, rebuild + restart, tick the box here, rewrite the Resume prompt.
Never push without asking.
