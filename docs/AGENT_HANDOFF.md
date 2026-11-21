# Shared agent handoff
Updated: 2026-11-21. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `docs/handoff-archive-2026-11-20.md`, `git log`.

## Next actions
Resume prompt (user pastes it after /clear, or says "Continua sempre qui"; the agent rewrites it at every step end and shows it in chat):
> Piano reattività HUD (`docs/hud-performance-plan-2026-11-20.md`): step 3e "discovery GUI fuori dal percorso della richiesta".
> Prima `node scripts/hud-trace-report.cjs` sul trace del 2026-11-21 10:09-10:27 UTC: 12 geometry > 250 ms, tutti con un rebuild del registro GUI (634-918 ms).
> File: `native/backend/src/http.rs` (`native_geometry`, `GuiLane`), `native/live` (discovery, `refresh_extensions`). Durante la discovery il pannello resta unknown.
> Test Rust, invariant-reviewer, commit, rebuild+restart, spunta e riscrivi questo prompt.
Plan agreed with the user 2026-11-20: HUD steps 1-7 in that file, one per session; steps 1-5 done (3e now has its trace), 3-6 reordered, follow-up F1-F2 added 2026-11-21. UX steps 1-9 (2026-11-19): archived, step 9 live checks still pending.

## In progress
- 2026-11-20 PURGE (user order): third-party guide data (benchmark/review/probe fixtures, Wojtek guide, `docs/it/walkthrough.md`) is
  LOCAL ONLY in `.local/guide-corpus` (`scripts/guide-corpus.cjs`; tests skip without it). History rewritten with filter-branch on all
  branches and tags, force-pushed (main `f43c37a`, 4 remote branches, tags v0.1.0-v0.2.0): ALL commit hashes before it changed.
  Backup of the old history: `.local/repo-backup-before-purge-2026-11-20.bundle` (contains the files; never push it).
  Release ZIPs (+ .sha256) of v0.1.0-v0.1.3 deleted on user request (v0.2.0 kept). Still exposed: merged PR refs #1 #2 and cached
  old commits stay reachable by SHA until GitHub Support purges them. Never commit or push third-party guide content.

## Recently verified
- 2026-11-21 HUD step 5 (details in the plan): the 4 card blanks of 2.4-3.8 s in the 10:09-10:27 trace were Rust mode `preview` (lost
  qualified session) shown as "campaign not valid"; now unknown, card kept <= 6 s dimmed "Updating…" without actions. overlay 75/0, backend
  58+120, npm 505/0, invariant-reviewer clean. NOT seen in game. Open: why the qualified session drops every few minutes.
- 2026-11-21 HUD step 4 (non-blocking loop; details in the plan): reads and card actions on worker threads (`mailbox.rs`), the loop waits
  <= 16 ms per read, idle polls at 16 ms, one retry of a read failing at once. `01f7750`; overlay 74/0, npm 505/0; rebuilt, restarted,
  new overlay writes `gap_ms` (background only, HOI4 closed). NOT seen in game.
- 2026-11-21 HUD step 2 Diagnose: report on 15.9 min of real play in the plan ("Measured 2026-11-21"): steady loop fine (p95 18.6 ms), transport
  not a cause; stalls = geometry spikes 0.4-1.15 s and HUD actions 1.56 s; 13 delivery expiries from audits; 1 card blank 3.3 s, cause unknown.
- 2026-11-21 HUD step 3 (backend delivery path, a-d; details in the plan): HUD actions republish instead of emptying the delivery, audit on its
  own thread, fresh 250 ms reads, trace reasons/GUI costs. Rust + report tests green. NOT seen in game. Step 1 trace tooling seen working 2026-11-21.
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
- Subsystem docs: `docs/README.md`. HUD plan: `docs/hud-performance-plan-2026-11-20.md`. Fast Rust loop: `docs/fast-local-rust.md`. Guide identity: `docs/guide-definition-identity.md`.
- Automation: skills `deliver-build`, `add-translation`, `improve-loop`; hooks `guard-game.cjs`, `web-test.cjs`; agent `invariant-reviewer`.
