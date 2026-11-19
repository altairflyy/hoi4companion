# Shared agent handoff
Updated: 2026-11-19. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `git log`.

## Next actions
Resume prompt (user pastes it after /clear; the agent rewrites it at every session end and shows it in chat):
> Riqualifica HOI4 1.20, step resources (handoff 0b): la riga resources cattura 0x33e36e0, verificare lo slot come per buildings. File: `src/hoi4companion/live-abi-v1-hoi4-1.20.json`,
> `native/live/src/abi.rs` (test), `native/live/src/country.rs` (fn resources). HOI4 aperto. A fine step: commit, deliver-build, verifica live.
0. Live diplomacy (user priority), one step per session, spec `docs/superpowers/specs/2026-11-19-foreign-ideology-abbreviations-design.md`.
   Next: Step 1 (live ruling ideology of all countries; needs HOI4 running, read-only), then Step 2 (NSPA abbreviations + "Attache to").
   Start each step with superpowers:writing-plans on that step only. Tick here when done.
   Waits on item 0b (remaining domains on HOI4 1.20).
0b. Requalify HOI4 1.20.0.0 (user chose this; read-only copy `.local/hoi4-1.20/hoi4.exe`, sha256 `1beb2a38…b63a9f`), one domain per session.
   Done `e98ccbb`: ABI relocation set `live-abi-v1-hoi4-1.20.json`, all 23 groups static; live core/production/diplomacy/intel/events/projects observed.
   Done and live-observed: campaign_identity `d1851e7`, building database `a03f723`, equipment `7a91a2c`+`f21e668` (details: archive, commits).
   Opt-in test `HOI4_QUALIFY_EXE=<copy> node scripts/dev-native.cjs test live abi::` checks token and building slots.
   Resources row matches 6 getters (captures 0x33e36e0): same check needed. Slot-finding: getter list, then read-only RTTI probe of slots.
   NEXT SESSION: resources. Then, in order: politics.advisors + advisor costs; commander_assignments (CArmyGroup); decisions (read limit); metrics; doctrines.
   Also hash-pinned to old builds: map markers (`map_markers.rs`, 709f…) and production GUI (`gui_production.rs`, cf781…).
   Scratch tools may be gone: crate `abiprobe` (iced-x86; probe, dis, xref, imm…) in scratchpad `cb8fd0a3…`;
   cargo via `.local/toolchains` (set CARGO_HOME/RUSTUP_HOME);
   rebuild if missing. Analyse only the copy; never name the game folder in a write command (guard `guard-game.cjs`).
1. Verify the confirmed-event-answer work in game (read-only). `_native` backend rebuilt 2026-11-19 17:40 from `f21e668` (dev profile), app restarted; HOI4 1.20 running.
2. User exercises the Desktop shortcut and HUD in gameplay; investigate concrete issues with read-only game access.
3. UX roadmap, one step per session (user runs /clear between steps): do the first unchecked item of `## Execution order` in `docs/ux-analysis-2026-11-19.md`, tick it there and here. Done: R1, R2, R3+R5. Next: R4.
4. Decide IL-02 (event-answer narrowing binds nothing; options in `docs/improvement-backlog.md`), needs the user.

## In progress
- Branch `auto/improve-2026-11-18` (UX R1, R2, R3+R5 committed and rebuilt; HOI4 1.20 relocation `e98ccbb`, `d1851e7`, `a03f723`, `7a91a2c`, `f21e668`), NOT merged, NOT pushed. Holds the first `/improve-loop` run and the confirmed-event-answer work.
- Improve-loop run report: `docs/improvement-backlog.md` `## Run report`. Done IL-01, 03, 05, 06, 07, 08; IL-04 dropped (no qualified war-goal field).

## Recently verified
- User-confirmed ambiguous event answers (`197f141..320d441`): SQLite `confirmed_event_answers`, re-applied by Rust `confirmed_answers.rs`, suite "Remove confirmation".
  Tests: JS 465 pass / 0 fail / 3 skipped (dev backend), Rust backend all pass, invariant review clean. Live behavior NOT VERIFIED. Details `docs/events-live.md` last section.
- HUD event answers native (`bea90e8`): user saw Bosphorus answer A lit on screen. OCR path (`f6f620a`) stays as fallback, not verified on screen.
- Guide follows the played nation (`8703617`, `2a5aefe`): verified live ROM guide -> SOV guide.
- User confirmed floating card and map markers visible in game (2026-11-17).

## Open questions / hypotheses
- Research "UP" steps (`98bc02a`) and abbreviated/written-out references: unit and catalog checks pass, NOT verified live in game.
- In-game highlight alignment and fluidity not qualified. Map marker pan/zoom unverified. Unit stacks unqualified.
- Full live read still ~1.35 s (provincial section for all provinces); a reader-side province filter is a possible next step. Profile before optimizing.
- Compiled-guide cache keyed by backend binary hash (narrowing rejected as unsafe).
- Intermittent startup/import JS test seen once; no root cause claimed.
- Remaining semantic work: grounded decision/war/capitulation and composite triggers; justify/annex/integrate/invade/collab recognized only.
- Construction: shared-slot factory maximum unqualified.

## Working boundaries
- Preserve active guide, campaigns, imports and concurrent edits. Never close, foreground or control HOI4; closing/installing/restarting Companion/HUD is allowed.
- Commit finished work locally in focused commits, then rebuild/restart the app. Never push without asking.
- Session end (user runs /clear after each step): rewrite `Resume prompt` for the next step, in Italian, naming the step, the files likely
  to change, needed preconditions (HOI4 open, build) and the closing routine; then show it verbatim in chat in a fenced block to paste.
- System WebView2 Runtime is a prerequisite of the desktop host.

## Where things are
- Subsystem docs: `docs/README.md`. Fast Rust loop: `docs/fast-local-rust.md`. Guide identity: `docs/guide-definition-identity.md`.
- Pre-desktop fixes and migration notes: `docs/handoff-before-desktop-2026-11-17.md`. Production tracking: `docs/handoff-production-tracking.md`.
- Automation: skills `deliver-build`, `add-translation`, `improve-loop`; hooks `guard-game.cjs`, `web-test.cjs`; agent `invariant-reviewer`.
