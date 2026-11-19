# Shared agent handoff
Updated: 2026-11-19. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `git log`.

## Next actions
Resume prompt (user pastes it after /clear; the agent rewrites it at every session end and shows it in chat):
> UX roadmap R4 (handoff 3): in `src/hoi4companion/web/action-reading.js` niente sostituzioni a metà frase; riga non compresa per intero =
> testo originale + glossario a fianco; test sulle tre frasi P3 di `docs/ux-analysis-2026-11-19.md`. Chiusura: test, invariant-reviewer,
> commit locale, deliver-build, spunta R4 lì e qui, handoff.
0. Live diplomacy spec `docs/superpowers/specs/2026-11-19-foreign-ideology-abbreviations-design.md`: DONE.
   Step 1 `3f0dbb6` (diplomacy.ideologies, live 439 tags), Step 2 `22820a2` (NSPA/RSPA, "Attache to"; live ROM step bound, ready, SPR fascism observed).
0b. HOI4 1.20.0.0 requalified (copy `.local/hoi4-1.20/hoi4.exe`, sha256 `1beb2a38…b63a9f`): all 17 country sections observed/partial live 2026-11-19.
   Commits after `f21e668`: resources `0c82777`, advisors `5677f10`, advisor costs `4994479`, commanders `f24566b`, decisions `7b72bf4`,
   metrics `ce550d0`, doctrines/design `fadd727`, map camera + production GUI `f5c0d0e` (evidence in each commit message).
0c. DONE 2026-11-19, user confirmed on screen: war support 0.588; ENG/FRA fascism (ROM puppets); ROM no active decisions, random counters right
   (improved_worker_conditions 2, war_propaganda_convoys 1, war_propaganda_casualties 1); production HUD boxes and construction map markers OK on 1.20.
   Still open: old metrics key "defensive" reads WAR_SUPPORT_OFFNSIVE_WAR (slot position); confirm against the UI before changing it. YUG not checked.
   Tools: crate `abiprobe` (dis, xref) in scratchpad `cb8fd0a3…`; analyse only the copy (guard `guard-game.cjs`).
1. Verify the confirmed-event-answer work in game (read-only). `_native` backend rebuilt 2026-11-19 from `22820a2` (dev profile), app restarted; HOI4 1.20 running.
2. User exercises the Desktop shortcut and HUD in gameplay; investigate concrete issues with read-only game access.
3. NEXT. UX roadmap, one step per session (user runs /clear between steps): do the first unchecked item of `## Execution order` in `docs/ux-analysis-2026-11-19.md`, tick it there and here. Done: R1, R2, R3+R5. Next: R4.
4. Decide IL-02 (event-answer narrowing binds nothing; options in `docs/improvement-backlog.md`), needs the user.

## In progress
- Branch `auto/improve-2026-11-18` (UX R1, R2, R3+R5 committed and rebuilt; HOI4 1.20 requalification `e98ccbb..f5c0d0e`, ideology `3f0dbb6`, `22820a2`), NOT merged, NOT pushed. Holds the first `/improve-loop` run and the confirmed-event-answer work.
- Improve-loop run report: `docs/improvement-backlog.md` `## Run report`. Done IL-01, 03, 05, 06, 07, 08; IL-04 dropped (no qualified war-goal field).

## Recently verified
- 2026-11-19: npm test 470 pass / 0 fail / 3 skipped; Rust live 66, guides 94, model 7, backend all pass; presentation fixtures unchanged.
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
