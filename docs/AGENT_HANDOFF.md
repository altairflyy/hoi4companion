# Shared agent handoff
Updated: 2026-11-19. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `git log`.

## Next actions
Resume prompt (user pastes it after /clear, or says "Continua sempre qui"; the agent rewrites it at every step end and shows it in chat):
> UX roadmap R9 (handoff 3): modalità guidata "Ora / Dopo / In attesa" nella suite e nella card HUD; leggi R9 e C "Modalità guidata"
> in `docs/ux-analysis-2026-11-19.md`; usa `reading` (R8) per i titoli. Verifica: test browser con valutazioni simulate, screenshot stretto.
> Chiusura: test, invariant-reviewer, commit locale, spunta R9 lì e qui, handoff. Rebuild e verifiche dal vivo solo nello step finale.
Plan agreed with the user 2026-11-19: do every code step below in order, one per session, without live checks; then one final session
with the user present for /deliver-build and all in-game checks (step 9). Each step: tests, invariant-reviewer, local commit, tick it in
`docs/ux-analysis-2026-11-19.md` `## Execution order` and here, rewrite the Resume prompt. Details of each item: sections D and E there.
1. DONE R7 · construction binder (see Recently verified).
2. DONE R8 · `reading` layer (see Recently verified).
3. R9 · guided mode "Now / Next / Waiting" in suite and HUD card. Check: browser tests with simulated evaluations, narrow-width screenshot.
4. R10 · readable waiting reasons (one `runtime_reason` -> sentence table in `translations.json`) and truncated focuses ("On Revise…")
   resolved by unique prefix (Rust). Check: Rust tests for unique and ambiguous prefix.
5. R11 · suite home useful without the game (preview of the first steps). Check: browser test in preview mode.
6. R12 · "Not this" correction from the card, reusing the associations page.
7. R15 · library cleanup: group by nation, flag duplicates by definition hash, separate non-national entries.
8. R13 (optional local AI for uninterpreted lines, user-approved proposals only) needs a user decision first; R14 blocked (no qualified war-goal field, IL-04).
9. FINAL, user present, HOI4 open: user runs `/deliver-build` (R6 `4acda08` and later are NOT rebuilt yet), then verify live:
   importer card on a real draft (R6), ROM "INFRAs, then MILs in Mun" shown as construction (R7) with composed title (R8), every UI step above; confirmed event answers (`docs/events-live.md`); research "UP" steps;
   HUD highlight alignment; map marker pan/zoom; metrics key "defensive" (reads WAR_SUPPORT_OFFNSIVE_WAR) against the UI; YUG ideology.
   Then ask whether to merge `auto/improve-2026-11-18` into main and whether to push.

## In progress
- Branch `auto/improve-2026-11-18`: UX R1-R6, HOI4 1.20 requalification, ideology, confirmed event answers, improve-loop run 1. NOT merged, NOT pushed.
- Improve-loop report: `docs/improvement-backlog.md` `## Run report`. IL-01, 02 (`b4d50c2`), 03, 05-08 done; IL-04 dropped.
- R6 `4acda08`: `import_reports::interpretation` (counts automatic/manual/reminder/uninterpreted, lists uninterpreted lines) in the import
  preview as `report.interpretation`; importer card `interpretationOverview`. Focus/research count as automatic (guide-core native membership).

## Recently verified
- 2026-11-19 R8: `reading.rs` + contract `tests/fixtures/reading-contract.json` (Rust and `test_reading_contract.cjs`); Italian strings added.
  guides 100 pass, backend all pass, npm test 475 / 0 / 3 skipped; invariant review clean. ROM guide on dev backend: 65 operations, 6 composed
  titles ("Ungheria (HUN), Jugoslavia (YUG)"). HUD card does not use `reading` yet. NOT rebuilt, NOT live.
- 2026-11-19 R7: binder accepts verbless lines, plurals, `then` (order only), unique national state prefixes (`start_owner`/`start_cores` new in catalog).
  Corpus 50 guides on a dev backend over a data copy: bound construction steps 7 -> 34, none ambiguous. guides 99 pass, backend all pass,
  npm test 472 / 0 / 3 skipped. Invariant review: no violation; `remove_core_of` fixed, other notes kept by design. NOT rebuilt, NOT live.
- 2026-11-19 HOI4 1.20 readings confirmed on screen by the user (war support, ENG/FRA ideology, ROM decisions, production boxes, map markers); details in the archive.
- User-confirmed ambiguous event answers (`197f141..320d441`): SQLite `confirmed_event_answers`, re-applied by Rust `confirmed_answers.rs`, suite "Remove confirmation".
  Tests: JS 465 pass / 0 fail / 3 skipped (dev backend), Rust backend all pass, invariant review clean. Live behavior NOT VERIFIED. Details `docs/events-live.md` last section.
- HUD event answers native (`bea90e8`): user saw Bosphorus answer A lit on screen. OCR path (`f6f620a`) stays as fallback, not verified on screen.

## Open questions / hypotheses
- R7 prefixes use game-start ownership only: a guide for a later bookmark could bind a prefix unique at start but not in its campaign.
- Research "UP" steps (`98bc02a`) and abbreviated/written-out references: unit and catalog checks pass, NOT verified live in game.
- In-game highlight alignment and fluidity not qualified. Map marker pan/zoom unverified. Unit stacks unqualified.
- Intermittent startup/import JS test seen once; no root cause claimed.

## Working boundaries
- Preserve active guide, campaigns, imports and concurrent edits. Never close, foreground or control HOI4; closing/installing/restarting Companion/HUD is allowed.
- Commit finished work locally in focused commits, then rebuild/restart the app. Never push without asking.
- Step end: rewrite `Resume prompt` for the next step (Italian: step, files, preconditions, closing routine) and show it in a fenced block.
- System WebView2 Runtime is a prerequisite of the desktop host.

## Where things are
- Subsystem docs: `docs/README.md`. Fast Rust loop: `docs/fast-local-rust.md`. Guide identity: `docs/guide-definition-identity.md`.
- Automation: skills `deliver-build`, `add-translation`, `improve-loop`; hooks `guard-game.cjs`, `web-test.cjs`; agent `invariant-reviewer`.
