# Handoff archive 2026-11-20

Finished history moved out of `AGENT_HANDOFF.md`. Details per step: `docs/ux-analysis-2026-11-19.md` `## Execution order`.

## Recently verified (moved)
- 2026-11-20 R12: "Not this?" on cards/guided items only for correctable bindings (`correctable()` in `web/action-plan.js`), opens the associations
  page on that step; unknown requested step is named, not swapped. npm test 484 / 0 / 3 skipped. NOT rebuilt, NOT live. Details in the UX doc.
- 2026-11-20 R11: home "Active route" previews up to 3 steps per track from the Rust `live=0` evaluation (`suite-model.js preview()`);
  `tests/test_suite.cjs` DOM test in preview mode; npm test 481 / 0 / 3 skipped; real ROM guide gives 9 steps. NOT rebuilt, NOT seen with HOI4 closed.
- 2026-11-20 R10: readable waiting reasons (`runtime_reasons`), truncated "On Revise…" resolved by unique prefix. NOT rebuilt, NOT live.
- Pointer kept in the handoff: event answers are described in `docs/events-live.md` (last section).
- User-confirmed ambiguous event answers (`197f141..320d441`): SQLite `confirmed_event_answers`, re-applied by Rust `confirmed_answers.rs`, suite "Remove confirmation".
  Tests: JS 465 pass / 0 fail / 3 skipped (dev backend), Rust backend all pass, invariant review clean. Live behavior NOT VERIFIED. Details `docs/events-live.md` last section.
- HUD event answers native (`bea90e8`): user saw Bosphorus answer A lit on screen. OCR path (`f6f620a`) stays as fallback, not verified on screen.
- 2026-11-20 R13 A: `interpretations.rs` + `/api/guide/interpretations`; attached to projection steps after evaluation; associations page
  section, card badge/link. npm test 488 / 0 / 3 skipped; guides 105, backend 49+118, overlay 61. NOT rebuilt, NOT live. Details in the UX doc.
- 2026-11-20 R15: `app_guides` rows carry `definition_hash` (canonical stored definition without id/runtime stamps); suite library groups
  by nation, "Other documents" for entries without a nation, duplicate/same-title-version notes (`libraryGroups()` in `web/suite-model.js`).
  npm test 486 / 0 / 3 skipped; backend 49+117 pass. Corpus: no exact duplicates; HUN pair = different versions. NOT rebuilt, NOT live.
- 2026-11-20 LIVE (step 9, part 1): `deliver.cjs --overlay` rebuilt backend+overlay at `e7fa139`, app up; HOI4 open, ROM readable. Seen in the
  built-in browser: library groups (ROM first, HUN two versions, "Altri documenti" 2); R7/R8 composed title "Costruisci Infrastruttura in Muntenia,
  poi … Fabbrica militare in Muntenia"; 47 "Cosa significa?" links; a saved reading showed "Conferma tu" + "Tua lettura" after fix `847bf95`
  (plan page now copies readings from the projection), then removed. User closed step 9 without checking the other step-9 items (still NOT verified).
- 2026-11-20 LIVE: event answer not highlighted (SOV, `bftb_turkey.4` open). Cause: `country_events.rs` read grid/buttons at fixed reference offsets
  (0x1280, 0x10d8); this build's `gui_layout` has `event_grid` 5120, `container_drags` 4696, so `events` was `unavailable` and the step stayed
  unverifiable. Fixed to read the profile; after rebuild `events` observed, step `active`, observe returns rect on `bftb_turkey.4.a`. Overlay drawing
  confirmed on screen by the user (2026-11-20).
- 2026-11-20 LIVE `31ce96b`: HUD card stuck on SOV step 1 (`word-0001`, INFRA+MILs "maximum") while the suite moved on. Cause: unqualified MIL maximum
  completed only while queued; HUD "Done" wrote a check the declared completion ignored. Fix: such targets = any(step check, queued); HUD/suite
  "Done" only where the step's own check completes it. After rebuild the run moved to the new archive, `word-0001` completed, card at step 14.
