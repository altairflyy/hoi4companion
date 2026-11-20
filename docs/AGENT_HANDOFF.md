# Shared agent handoff
Updated: 2026-11-20. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `docs/handoff-archive-2026-11-20.md`, `git log`.

## Next actions
Resume prompt (user pastes it after /clear, or says "Continua sempre qui"; the agent rewrites it at every step end and shows it in chat):
> Roadmap UX chiusa; cronologia ripulita dai file delle guide di terzi (handoff, In progress). Attendi l'utente: asset delle release
> (ZIP v0.1.x eliminati) e richiesta a GitHub Support; poi, solo se chiesto, riquadri HUD di Produzione stretti e verifiche del passo 9.
Plan agreed with the user 2026-11-19: do every code step below in order, one per session, without live checks; then one final session
with the user present for /deliver-build and all in-game checks (step 9). Each step: tests, invariant-reviewer, local commit, tick it in
`docs/ux-analysis-2026-11-19.md` `## Execution order` and here, rewrite the Resume prompt. Details of each item: sections D and E there.
1-2. DONE R7 construction binder, R8 `reading` layer (archive, `docs/ux-analysis-2026-11-19.md`).
3. DONE R9 · guided mode (see Recently verified).
4. DONE R10 · readable waiting reasons and truncated focuses (see Recently verified).
5. DONE R11 · suite home preview without the game (see Recently verified).
6. DONE R12 · "Not this?" from the card (see Recently verified).
7. DONE R15 · library cleanup (see Recently verified).
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
- Improve-loop report: `docs/improvement-backlog.md` `## Run report`. IL-01, 02 (`b4d50c2`), 03, 05-08 done; IL-04 dropped.
- R6 `4acda08`: `import_reports::interpretation` (counts automatic/manual/reminder/uninterpreted, lists uninterpreted lines) in the import
  preview as `report.interpretation`; importer card `interpretationOverview`. Focus/research count as automatic (guide-core native membership).

## Recently verified
- 2026-11-20 `be4736f` construction queue + `143d834` I-16 retirement: verified live, details in `docs/handoff-archive-2026-11-20.md`.
- 2026-11-20 `7eaf611`: "NED Peace Conference*" links open the PEACE CONFERENCE PRIORITY row/table. `88a613f`: a line's condition (`decomposition.timing`, e.g. "At 24% paranoia") shown as "When:" under the title in suite and HUD card; rebuilt+restarted, NOT seen on screen.
- 2026-11-20 `facbdc5`+`f916886` LIVE (prepared guide API): inline event "On X: Y" scopes its line+bullets, threshold "At N%: Y" its line only (`decomposition.rs`); "at border" no longer read as AT; parent line labelled "Under the guide line". Cards NOT seen on screen.
- Event answers: native HUD answers seen on screen (`bea90e8`); user confirmations `197f141..320d441`, NOT live-verified. See archive.

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
