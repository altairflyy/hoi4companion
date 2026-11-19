# Shared agent handoff
Updated: 2026-11-19. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `git log`.

## Next actions
1. Rebuild the stale `_native` backend + overlay (deliver-build skill) for the confirmed-event-answer work, then verify it in game (read-only).
2. User exercises the Desktop shortcut and HUD in gameplay; investigate concrete issues with read-only game access.
3. Decide IL-02 (event-answer narrowing binds nothing; options in `docs/improvement-backlog.md`), needs the user.

## In progress
- Branch `auto/improve-2026-11-18` (HEAD `3a55134`), NOT merged, NOT pushed. Holds the first `/improve-loop` run and the confirmed-event-answer work.
- Improve-loop run report: `docs/improvement-backlog.md` `## Run report`. Done IL-01, 03, 05, 06, 07, 08; IL-04 dropped (no qualified war-goal field).

## Recently verified
- User-confirmed ambiguous event answers (`197f141..320d441`): SQLite `confirmed_event_answers`, re-applied by Rust `confirmed_answers.rs`, suite "Remove confirmation".
  Tests: JS 465 pass / 0 fail / 3 skipped (dev backend), Rust backend all pass, invariant review clean. Live behavior NOT VERIFIED. Details `docs/events-live.md` last section.
- HUD event answers native (`bea90e8`): user saw Bosphorus answer A lit on screen. OCR path (`f6f620a`) stays as fallback, not verified on screen.
- Guide follows the played nation (`8703617`, `2a5aefe`): verified live ROM guide -> SOV guide.
- Events page with historical path (`4723991`, `515cea6`): verified live SOV, all 5 Reverse Barbarossa answered events listed.
- Startup: active guide prepared first (IL-05 `47d0f77`), cold ready 44.75 s -> 14.18 s; parallel preparation `34e68b2`.
- Desktop host (`native/desktop`, tao + wry): lifecycle, owned children, authenticated stop verified 2026-11-17. Details `docs/desktop-suite.md` and the archive.
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
- System WebView2 Runtime is a prerequisite of the desktop host.

## Where things are
- Subsystem docs: `docs/README.md`. Fast Rust loop: `docs/fast-local-rust.md`. Guide identity: `docs/guide-definition-identity.md`.
- Pre-desktop fixes and migration notes: `docs/handoff-before-desktop-2026-11-17.md`. Production tracking: `docs/handoff-production-tracking.md`.
- Automation: skills `deliver-build`, `add-translation`, `improve-loop`; hooks `guard-game.cjs`, `web-test.cjs`; agent `invariant-reviewer`.
