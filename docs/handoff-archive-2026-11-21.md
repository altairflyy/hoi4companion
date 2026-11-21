# Handoff archive 2026-11-21

Finished history moved out of `AGENT_HANDOFF.md`. Details per step: `docs/hud-performance-plan-2026-11-20.md` (archived).

## Recently verified (moved)
- 2026-11-21 HUD step 3e (details in the plan): GUI registry discovery (635-892 ms, 8/8 slow geometries in the trace) runs on thread
  `hoi4-gui-discovery`; requests answer "GUI registry discovery in progress" (every panel unknown). live 71/0, overlay 76/0,
  backend 58+120/0, report test 5/0, invariant-reviewer: 1 WARN (target not bound to its session) fixed. NOT seen in game.
- 2026-11-21 HUD step 5 (details in the plan): the 4 card blanks of 2.4-3.8 s in the 10:09-10:27 trace were Rust mode `preview` (lost
  qualified session) shown as "campaign not valid"; now unknown, card kept <= 6 s dimmed "Updating…" without actions. overlay 75/0, backend
  58+120, npm 505/0, invariant-reviewer clean. NOT seen in game. Open: why the qualified session drops every few minutes.
- 2026-11-21 HUD step 4 (non-blocking loop, `mailbox.rs`; details in the plan): `01f7750`; overlay 74/0, npm 505/0; rebuilt, restarted. NOT seen in game.
- 2026-11-21 HUD step 2 Diagnose: 15.9 min of real play, numbers in the plan ("Measured 2026-11-21").
- 2026-11-21 HUD step 3 (backend delivery path, a-d; details in the plan): HUD actions republish instead of emptying the delivery, audit on its
  own thread, fresh 250 ms reads, trace reasons/GUI costs. Rust + report tests green. NOT seen in game. Step 1 trace tooling seen working 2026-11-21.
