# Handoff archive 2026-11-22

Finished history moved out of `AGENT_HANDOFF.md`. Details per step: `docs/hud-performance-plan-2026-11-21.md`.

## Recently verified (moved)
- 2026-11-22 UX step 2: «Ora» prima delle metriche, «Ora / Poi» prima di scorciatoie e riferimenti. Edge headless 1282 × 832 con fixture isolate: M1 0/0, M2 1/1/1, anteprima 0/2 nel Piano.
  Test mirati 74/74, npm test 513 passati, 3 saltati. Backend e overlay ricompilati, Companion su 8766. Catture in `.local/ux-captures/step-2/`. Verifica in WebView2 e in partita reale aperta.
- 2026-11-22 UX step 1: embedded reference navigation and Settings keyboard loop pass in Edge headless and desktop WebView2. M1 0/0, M2 2/2/2, M3 1/1 with synchronized titles after fix, M4 0/6.
  Codex reran baseline and current checkout at 1282 × 832 with fixtures. Enter activates both reference links, Import ZIPs and Saved guides. Targeted tests 68/68, npm test 510 passed, 3 skipped.
  Evidence and capture logs are linked from `docs/ux-implementation-plan.md`.
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
- 2026-11-22 step 4b part 2: full read verifies each domain once (refresh pass), event discovery reads each parent vtable once,
  other guides observed after the HUD delivery. Live, 4 deliveries: acquisition ~1000 -> ~760 ms; HOI4 then closed (12:23, not
  by the agent), so the delivery p90 and the deferral are NOT measured. Tests green (live 72, backend 65 + 120, report 7), reviewer clean.
- 2026-11-22 step 4b part 1: fewer evaluation copies (see plan). Live, same game: change-to-delivery p90 2442 -> ~1810 ms, target 1.5 s
  NOT met. Tests green (guides 112, backend 64 + 120, report 7), reviewer clean. Live: 3a retried a full read once, 0 closes.
- 2026-11-22 step 3a (`provider.rs` `settle`): full-read consistency failure keeps handle/GUI registry, unknown at once, retry next
  collect, close at the 8th in a row. Backend tests green (lib 64, integration 120), reviewer clean. Live effect NOT measured (step 5).
- 2026-11-22 live session (1003 s traced, build `603225f`): gap p50 16.8 ms (step 1 ok); geometry max 14 ms after a reopen
  (3b ok); closes all `full_read` consistency errors (step 3a decided); change-to-delivery p90 1955 ms, target 1.5 s NOT met (4b).
- UX step 3: Edge headless con fixture isolate, M4 6/6. Spunta salvata, «Avvio non verificabile» e 0/98 nel Piano.
  JS mirati 35/35, npm 514 passati e 3 saltati. Rust backend 69 + 120 passati, live 72 passati. Esiti e inventario nel piano UX.
  Backend e overlay ricompilati dopo `1b3c779`, HTTP 200 e riprova Edge isolata riuscita. Catture in `.local/ux-captures/step-3/`.
  Prove manuali in WebView2 e in partita ancora aperte.
