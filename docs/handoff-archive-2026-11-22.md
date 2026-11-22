# Handoff archive 2026-11-22

Finished history moved out of `AGENT_HANDOFF.md`. Details per step: `docs/hud-performance-plan-2026-11-21.md`.

## Recently verified (moved)
- 2026-11-21 part 2 step 3b: HUD geometry no longer waits on the provider lock nor the HTTP provider owner during a full read (lane slot +
  `provider_handle`). Backend 61+120 green, new lock test, invariant-reviewer clean. Live spikes NOT measured: LIVE session.
- 2026-11-21 part 2 step 4: progress store reads no longer clone the whole state (offline: 54 `get` 420 -> 0.4 ms; a write ~70 ms, so
  the 2 MB rewrite was NOT the cost). Storage/guides/backend tests green. Live change-to-delivery p90 NOT measured: LIVE session.
- 2026-11-21 part 2 step 2 (code): provider `metrics.closes` + delivery `provider_close` + trace `close_*` + report section. Backend 60+120,
  overlay 78, report 6 tests green. Close reasons NOT yet seen live: needs the user's play session.
- 2026-11-21 HUD part 2 step 1: overlay `wake::FineTimer` (1 ms tick for the HUD process). Unit test: a 5 ms wait took median 15.5 ms
  (condvar and channel) with the default tick, 5.5 ms with it. Overlay tests 77/77 green, invariant-reviewer clean. Live `gap_ms` NOT measured (step 5).
- 2026-11-21 HUD plan 2026-11-20 step 7 (live check, 1221 s of play on `f40e999`): stalls 0 (was 16), blank card 0, 7/7 GUI rebuilds off
  the request path, transport p95 2.5 ms (step 6 closed). New: 6 qualified-session drops (cause not recorded), 98 delivery expiries after
  a change (pipeline p50 2.6 s), actions 1.8-3.2 s, loop ~31 ms. Numbers: plan section "Measured 2026-11-21, step 7". Steps 3e-5 seen working.
