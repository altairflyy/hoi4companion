# Handoff archive 2026-11-20

Finished history moved out of `AGENT_HANDOFF.md`. Details per step: `docs/ux-analysis-2026-11-19.md` `## Execution order`.

## Recently verified (moved)
- 2026-11-20 R12: "Not this?" on cards/guided items only for correctable bindings (`correctable()` in `web/action-plan.js`), opens the associations
  page on that step; unknown requested step is named, not swapped. npm test 484 / 0 / 3 skipped. NOT rebuilt, NOT live. Details in the UX doc.
- 2026-11-20 R11: home "Active route" previews up to 3 steps per track from the Rust `live=0` evaluation (`suite-model.js preview()`);
  `tests/test_suite.cjs` DOM test in preview mode; npm test 481 / 0 / 3 skipped; real ROM guide gives 9 steps. NOT rebuilt, NOT seen with HOI4 closed.
- 2026-11-20 R10: readable waiting reasons (`runtime_reasons`), truncated "On Revise…" resolved by unique prefix. NOT rebuilt, NOT live.
