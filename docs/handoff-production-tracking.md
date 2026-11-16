# Handoff: production step tracking (I-16 not detected)
Updated: 2026-11-16 17:50. RESOLVED: I-16 matched (ca95ba6), factory targets are a minimum (34dc7d2), step `word-0002` completed live after rebuild/restart.
Read `CLAUDE.md` and `docs/AGENT_HANDOFF.md` first. Rust is the only evaluator, and unknown never means complete.

## User report
In the Soviet guide "RUSSIA (Communist)_ Reverse Barbarossa" (`word-67736ac669bdcbf1`), step `word-0002`
("Production: Sup Equip:, I-16:5 Truck:4, Art, AA:2, Train:1, Inf Equip:ALL") is not tracked. The user put the I-16
into production, but the step does not read it. The user also wants better controls and feedback on production steps.

## Verified (live API, 2026-11-16 ~17:05, game running, `usable: true`)
- Live sample `state.sample.country_overview.production.lines` contains
  `small_plane_airframe_1 | name "I-16" | military | requested 15`, plus the other land and naval lines.
- The running backend still binds target 2 ("I-16") to `production_selector.equipment_ids: ["fighter_equipment_1"]`,
  `variant_names: []`. As a result, layout target 2 has `requested 0`, `lines []` and `reason ""`.
- Root cause: in the HoI4 designer, the I-16 is a design named "I-16" on the `small_plane_airframe_1` chassis. "I-16" is only
  a SOV localisation alias of the legacy row `fighter_equipment_1`.
- The running binary is OLD. `src/hoi4companion/_native/hoi4-backend.exe` dates from 16:05 (PID 38092, started 16:57).
  The build containing the fix is `.local/rust-backend-target/release/hoi4-backend.exe` (16:50).
  The user's `build-backend.ps1` compiled it, but `Copy-Item` failed ("file in use") because the app was running.
  So the user has never seen the fix.

## Uncommitted fix already in the working copy (by another session)
- `native/backend/src/production_binding.rs`, `national_model()`: a national model name selects the whole family
  (`small_plane_airframe` archetype `equipment_ids`) filtered by `variant_names` ["I-16", "Polikarpov I-16", "fighter_equipment_1"].
- `native/backend/src/production_context.rs`: unmatched targets carry `reason` and `near_lines`. These are shown on the guide card
  (`web/action-plan.js` observations, `translations.json`). The HUD step card does NOT show them yet.
- Tests: `native/backend/tests/guide_evaluation.rs` (`soviet_lines`, `bind_fighter`). That session reported
  `test-native.cjs backend` all pass and `npm test` 437/1 skipped. I did not re-run them.
- Docs: `docs/production-guide-monitoring.md#national-model-names-and-unmatched-targets-2026-11-16`.
- Do not mix these changes up with the other uncommitted work in the same tree: the HUD template images
  (`native/overlay/*`, `card_images.rs`, `hud_service.rs`, `web/guide.js`, `docs/native-overlay.md`).

## Deployed and verified live (2026-11-16 17:20)
- Rebuilt `_native` backend and overlay (17:16), restarted. Live projection: the I-16 selector is now the `small_plane_airframe` family plus names; target 2 lists `small_plane_airframe_1/I-16`, requested 15. `npm test` 437 pass / 1 skipped against the new binary.
- Step `word-0002` is still `active`: I-16 15 != 5 (hypothesis 1) and infantry remaining target 23 vs requested 150 with empty reason (hypothesis 2). Next actions 1-2 below are done.
- Walkthrough (7e9ab59): no "I issued the order" button; one "Completato" checkbox per manual step; "New checklist" moved to the saved-checklists section.

## Hypotheses (not verified) — why tracking can still look broken after deploying
1. **Exact equality**: completion is `all` of `guide.production.word-0002.target-N.requested` `eq` N. Live I-16 has
   requested 15 against target 5, so the step stays open even once the line is matched. A later guide step
   ("Change I-16 to F1ca … production to 15") raises it on purpose. Decide with the user whether an earlier
   production step should be satisfied by `gte`, count as "done when it was reached" (a run baseline or milestone), or
   step aside once a later step owns the line. Any such change must stay in Rust and keep unknown != complete.
2. **"ALL" remaining target**: target 7 (infantry, `remaining`) reports target 23 vs requested 150 and `reason ""`.
   Check that `remaining_allocated` is evaluated correctly, including when air or naval lines share the military factories.
3. **Renamed designs**: matching filters by `variant_names`. A design renamed in game (e.g. "F1ca") will stop matching
   "I-16". Check how the guide's later "Change I-16 to F1ca" step is bound, and whether the card says why it is unmatched.
4. **Re-binding of the existing run**: the existing SOV checklist should re-bind on load, according to the
   prepare refresh_bindings note in AGENT_HANDOFF. Confirm via `/api/guide/projection` after the restart.

## Exact next actions
1. Ask the user to stop the app (`scripts/avvia-tutto.ps1 -Stop`, or close it from the tray). Check with
   `Get-Process hoi4-backend,hoi4-desktop,hoi4-overlay` that nothing is left. Then run
   `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-backend.ps1` and restart with `avvia-tutto.ps1`.
2. Verify live with `GET /api/guide/active`, then `GET /api/guide/projection?id=<id>&live=1`. In it, check
   `evaluation.steps[key=word-0002].production.layout[1]`: it should list the `small_plane_airframe_1` line, requested 15.
3. Settle hypothesis 1 with the user before changing completion semantics. Then add a Rust test in `guide_evaluation.rs`
   and, if the presentation changes, a case in `tests/fixtures/presentation-cases.json` (regenerate per CLAUDE.md).
4. Show unmatched/mismatch reasons on the HUD step card, after the template-image session's overlay work is committed.
5. Commit only on the user's request. Keep the production fix in its own commit, separate from the HUD template images.
