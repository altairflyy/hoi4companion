# Shared agent handoff
Updated: 2026-11-22. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `-2026-11-20.md`, `-2026-11-21.md`, `-2026-11-22.md`, `git log`.

## Next actions
Resume prompt (user pastes it after /clear, or says "Continua sempre qui"; the agent rewrites it at every step end and shows it in chat):
> Piano qualità HUD (`docs/hud-quality-plan-2026-11-22.md`): step 2, verifica live F1 con l'utente. Precondizione: un passo guida
> con marcatori mappa (cantieri). L'utente fa ~5 min di pan e zoom a vari livelli e manda 2-3 screenshot a camera ferma. Poi
> `node scripts/hud-trace-report.cjs`, sezione "Map markers": decidi quali ipotesi H1-H4 reggono e scrivi i numeri nel piano.
> Nessuna modifica alla geometria in questo step. Mai avviare o controllare HOI4.
PAUSED 2026-11-22 (user): no more live game tests for now; every remaining HUD quality step needs live data or a screenshot.
HUD responsiveness plan part 2 closed 2026-11-22 (step 5, target met). Optional leftovers (full read, HUD `project`): plan step 4b.
UX steps 1-9 (2026-11-19): archived, step 9 live checks still pending.
UX plan 2026-11-22 (`docs/ux-implementation-plan.md`, desktop only, no game): next is step 1. Prompt: «Leggi docs/ux-implementation-plan.md
e docs/AGENT_HANDOFF.md. Esegui solo lo step 1 (correzioni rapide e misura di partenza).»

## In progress
- 2026-11-20 PURGE (user order): third-party guide data (benchmark/review/probe fixtures, Wojtek guide, `docs/it/walkthrough.md`) is
  LOCAL ONLY in `.local/guide-corpus` (`scripts/guide-corpus.cjs`; tests skip without it). History rewritten with filter-branch on all
  branches and tags, force-pushed (main `f43c37a`, 4 remote branches, tags v0.1.0-v0.2.0): ALL commit hashes before it changed.
  Backup of the old history: `.local/repo-backup-before-purge-2026-11-20.bundle` (contains the files; never push it).
  Release ZIPs (+ .sha256) of v0.1.0-v0.1.3 deleted on user request (v0.2.0 kept). Still exposed: merged PR refs #1 #2 and cached
  old commits stay reachable by SHA until GitHub Support purges them. Never commit or push third-party guide content.

## Recently verified
- 2026-11-22 HUD quality plan F1 step 1 (`1677f08`): map marker diagnostics + trace fields + report "Map markers", no behavior
  change. Tests green (live 72, overlay 79, backend 65 + 120, JS 508/511, 3 skipped), invariant-reviewer clean. Live data NOT seen yet.
- 2026-11-22 HUD plan part 2 step 5 (836 s of play on `0e4b001`): change-to-delivery p50 1166 / p90 1365 ms (target < 1.5 s met;
  was 2634 / 3103 on 2026-11-21), no delivery past 3 s, 0 session closes in play, 2 full-read retries (~1.5 s unknown each), gap p50
  16.9 ms, geometry max 26.9 ms. HUD actions NOT measured (none done). Plan archived; numbers in its "Measured 2026-11-22, step 5".
- 2026-11-20 `3ac001a` HUD card: a production step is skipped as current step once a later step of its track is completed (cause:
  layouts stop matching when lines change or read briefly unknown). Production list behind a toggle (`production_open` in overlay-card.json).
  Rust unit tests green, rebuilt, app restarted. NOT seen in game.
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
- Subsystem docs: `docs/README.md`. HUD plans (archived): `docs/hud-performance-plan-2026-11-20.md`, `-2026-11-21.md`; quality plan `docs/hud-quality-plan-2026-11-22.md`. Fast Rust loop: `docs/fast-local-rust.md`. Guide identity: `docs/guide-definition-identity.md`.
- Automation: skills `deliver-build`, `add-translation`, `improve-loop`; hooks `guard-game.cjs`, `web-test.cjs`; agent `invariant-reviewer`.
