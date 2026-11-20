# Shared agent handoff
Updated: 2026-11-20. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `docs/handoff-archive-2026-11-20.md`, `git log`.

## Next actions
Resume prompt (user pastes it after /clear, or says "Continua sempre qui"; the agent rewrites it at every step end and shows it in chat):
> `3ac001a` card HUD: step produzione già superato non torna più come step 1; lista Produzione dietro il pulsante "Produzione · N".
> `f6b1e39` Dottrine e spiriti come schede (voto, bonus, spiriti, commento, riepilogo rapido). Da vedere in gioco: card e pulsante.
> Poi attendi: asset release, GitHub Support; solo se chiesto, riquadri HUD Produzione e verifiche passo 9.
Plan agreed with the user 2026-11-19: do every code step below in order, one per session, without live checks; then one final session
with the user present for /deliver-build and all in-game checks (step 9). Each step: tests, invariant-reviewer, local commit, tick it in
`docs/ux-analysis-2026-11-19.md` `## Execution order` and here, rewrite the Resume prompt. Details of each item: sections D and E there.
1-7. DONE R7, R8, R9, R10, R11, R12, R15 (details: archive and `docs/ux-analysis-2026-11-19.md`).
8. DONE R13 Stage A (player reading of hand-confirmed lines, no AI; Stage B deferred). R14 blocked (IL-04).
9. PARTLY DONE 2026-11-20: rebuilt and running at `927e3d8`; suite checks in Recently verified. User skipped the rest; still NOT verified:
   importer card on a real draft (R6), ROM "INFRAs, then MILs in Mun" shown as construction (R7) with composed title (R8) in suite and HUD card (R9),
   waiting reasons and "On Revise…" resolved (R10), home preview with HOI4 closed (R11), library groups (R15), a saved line reading (R13); confirmed event answers (`docs/events-live.md`); research "UP" steps;
   HUD highlight alignment; map marker pan/zoom; metrics key "defensive" (reads WAR_SUPPORT_OFFNSIVE_WAR) against the UI; YUG ideology.
   Merge and push: done 2026-11-20.

## In progress
- 2026-11-20 PURGE (user order): third-party guide data (benchmark/review/probe fixtures, Wojtek guide, `docs/it/walkthrough.md`) is
  LOCAL ONLY in `.local/guide-corpus` (`scripts/guide-corpus.cjs`; tests skip without it). History rewritten with filter-branch on all
  branches and tags, force-pushed (main `f43c37a`, 4 remote branches, tags v0.1.0-v0.2.0): ALL commit hashes before it changed.
  Backup of the old history: `.local/repo-backup-before-purge-2026-11-20.bundle` (contains the files; never push it).
  Release ZIPs (+ .sha256) of v0.1.0-v0.1.3 deleted on user request (v0.2.0 kept). Still exposed: merged PR refs #1 #2 and cached
  old commits stay reachable by SHA until GitHub Support purges them. Never commit or push third-party guide content.

## Recently verified
- 2026-11-20 `3ac001a` HUD card: a production step is skipped as current step once a later step of its track is completed (cause:
  layouts stop matching when lines change or read briefly unknown). Production list behind a toggle (`production_open` in overlay-card.json).
  Rust unit tests green, rebuilt, app restarted. NOT seen in game.
- 2026-11-20 `f6b1e39` `doctrine-guide.js`: doctrine/spirit documents as cards (quick table, sections nav, sort by rating, spirits also in
  the spirits guide in green). npm test green; seen in the browser pane on the 5 local documents.
- 2026-11-20 `9ed86bb` Templates & references page: tabs (images, designs, advanced guides, MIO), side category/document lists,
  MIO and design searches; `reference-viewer.js` full-screen viewer (fit, 1:1, wheel/+- zoom, drag, prev/next, Esc). npm test green;
  rebuilt+restarted; seen working in the browser pane (suite, Italian). In the suite it covers only the iframe, not the sidebar.
- 2026-11-20 `8bfbdc5` reload bug: a save reload (tick < bound tick) made a `timeline_rewind` run with no set_aside and no checks
  (seen: run `fa0f389e` 3 aside/21 checks -> `f58745bb` empty). Checks now get `checked_at` (campaign tick, bound runs only, new
  confirmations only); rewind keeps set_aside + checks dated <= loaded tick. Storage/backend/guides tests green; rebuilt, restarted,
  API loads. NOT checked with a real reload. Old undated checks are not recoverable automatically.

## Open questions / hypotheses
- HUD production highlights look narrow (user, 2026-11-20). Box = row container size from memory (`gui_production.rs`, row+0x17AC/0x17B0
  x matrix scale), e.g. Artillery [8,872,508,983]; a scaled screenshot suggests it misses the right column and some height. NOT measured:
  the overlay window cannot be captured; needs a user screenshot with borders before touching qualified geometry.
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
