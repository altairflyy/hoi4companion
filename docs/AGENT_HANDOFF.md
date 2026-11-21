# Shared agent handoff
Updated: 2026-11-21. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `-2026-11-20.md`, `-2026-11-21.md`, `git log`.

## Next actions
Resume prompt (user pastes it after /clear, or says "Continua sempre qui"; the agent rewrites it at every step end and shows it in chat):
> Piano reattività HUD parte 2 (`docs/hud-performance-plan-2026-11-21.md`, sezione "Order from 2026-11-21"): step 3b.
> Utente da remoto: niente partite live; mai avviare o comandare HOI4. Step 3b: `provider::gui_lane` non deve attendere il lock del provider
> durante una lettura completa (picchi geometria 878-1516 ms dopo un reopen). Prima leggi `native/backend/src/provider.rs` e `http.rs` `read`.
> Chiusura: test Rust, invariant-reviewer, commit locale, rebuild + riavvio, spunta lo step, riscrivi questo prompt (sessione LIVE).
Part 2 reordered 2026-11-21 at the user's request (remote, no live play): offline steps 4, 3b first; then ONE live session; then 3a, 5.
UX steps 1-9 (2026-11-19): archived, step 9 live checks still pending.

## In progress
- 2026-11-20 PURGE (user order): third-party guide data (benchmark/review/probe fixtures, Wojtek guide, `docs/it/walkthrough.md`) is
  LOCAL ONLY in `.local/guide-corpus` (`scripts/guide-corpus.cjs`; tests skip without it). History rewritten with filter-branch on all
  branches and tags, force-pushed (main `f43c37a`, 4 remote branches, tags v0.1.0-v0.2.0): ALL commit hashes before it changed.
  Backup of the old history: `.local/repo-backup-before-purge-2026-11-20.bundle` (contains the files; never push it).
  Release ZIPs (+ .sha256) of v0.1.0-v0.1.3 deleted on user request (v0.2.0 kept). Still exposed: merged PR refs #1 #2 and cached
  old commits stay reachable by SHA until GitHub Support purges them. Never commit or push third-party guide content.

## Recently verified
- 2026-11-21 part 2 step 4: progress store reads no longer clone the whole state (offline: 54 `get` 420 -> 0.4 ms; a write ~70 ms, so
  the 2 MB rewrite was NOT the cost). Storage/guides/backend tests green. Live change-to-delivery p90 NOT measured: LIVE session.
- 2026-11-21 part 2 step 2 (code): provider `metrics.closes` + delivery `provider_close` + trace `close_*` + report section. Backend 60+120,
  overlay 78, report 6 tests green. Close reasons NOT yet seen live: needs the user's play session.
- 2026-11-21 HUD part 2 step 1: overlay `wake::FineTimer` (1 ms tick for the HUD process). Unit test: a 5 ms wait took median 15.5 ms
  (condvar and channel) with the default tick, 5.5 ms with it. Overlay tests 77/77 green, invariant-reviewer clean. Live `gap_ms` NOT measured (step 5).
- 2026-11-21 HUD plan 2026-11-20 step 7 (live check, 1221 s of play on `f40e999`): stalls 0 (was 16), blank card 0, 7/7 GUI rebuilds off
  the request path, transport p95 2.5 ms (step 6 closed). New: 6 qualified-session drops (cause not recorded), 98 delivery expiries after
  a change (pipeline p50 2.6 s), actions 1.8-3.2 s, loop ~31 ms. Numbers: plan section "Measured 2026-11-21, step 7". Steps 3e-5 seen working.
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
- Subsystem docs: `docs/README.md`. HUD plan: `docs/hud-performance-plan-2026-11-21.md` (part 1 archived: `-2026-11-20.md`). Fast Rust loop: `docs/fast-local-rust.md`. Guide identity: `docs/guide-definition-identity.md`.
- Automation: skills `deliver-build`, `add-translation`, `improve-loop`; hooks `guard-game.cjs`, `web-test.cjs`; agent `invariant-reviewer`.
