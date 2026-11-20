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

## Moved from handoff 2026-11-20 (construction queue, I-16 retirement)
- 2026-11-20 LIVE `31ce96b` (archive) + `be4736f`: step 1 reopened: one unlocatable queue line made `construction.queued` unknown. New fact `construction.queued_known`
  (located lines, positive use only); `word-0001` completed again. `143d834`: a later "Change A to B" line marks earlier production targets
  named A `retired_by` it (`production_retirement.rs`); guide-core supersedes them once that line is completed. Live: `word-0002`
  Superseded by `word-0011--line-2`, card at step 18. Note: a fully met rest makes the whole step "Superseded" (existing rule).

## Moved from handoff 2026-11-20 (set-aside step)
- Event answers: native HUD answers seen on screen (`bea90e8`); user confirmations `197f141..320d441`, NOT live-verified. See archive.
- 2026-11-20 `facbdc5`+`f916886` LIVE (prepared guide API): inline event "On X: Y" scopes its line+bullets, threshold "At N%: Y" its line only (`decomposition.rs`); "at border" no longer read as AT; parent line labelled "Under the guide line". Cards NOT seen on screen.
- 2026-11-20 `7eaf611`: "NED Peace Conference*" links open the PEACE CONFERENCE PRIORITY row/table. `88a613f`: a line's condition (`decomposition.timing`, e.g. "At 24% paranoia") shown as "When:" under the title in suite and HUD card; rebuilt+restarted, NOT seen on screen.

## Moved from the handoff on 2026-11-20 (evening)
- Improve-loop report: `docs/improvement-backlog.md` `## Run report`. IL-01, 02 (`b4d50c2`), 03, 05-08 done; IL-04 dropped.
- R6 `4acda08`: `import_reports::interpretation` (counts automatic/manual/reminder/uninterpreted, lists uninterpreted lines) in the import
  preview as `report.interpretation`; importer card `interpretationOverview`. Focus/research count as automatic (guide-core native membership).
1-2. DONE R7 construction binder, R8 `reading` layer (archive, `docs/ux-analysis-2026-11-19.md`).
3. DONE R9 · guided mode (see Recently verified).
4. DONE R10 · readable waiting reasons and truncated focuses (see Recently verified).
5. DONE R11 · suite home preview without the game (see Recently verified).
6. DONE R12 · "Not this?" from the card (see Recently verified).
7. DONE R15 · library cleanup (see Recently verified).
