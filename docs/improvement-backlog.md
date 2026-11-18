# Improvement backlog

Shared state of the unattended improvement loop (`/improve-loop`, see
`.claude/skills/improve-loop/`). The coordinator session seeds and ranks the items,
starts one worker session per item and checks the result. Each worker session changes
only its own item. Users read this file to review a run and answer `needs-user` items.

## Run
- Branch: auto/improve-2026-11-18
- Started: 2026-11-18T19:32:15+01:00
- Resumed: 2026-11-18T19:51:25+01:00
- Limits: 15 items done / 8 h from Started / 2 attempts per item

<!--
Item format (ids IL-01, IL-02, ... are never reused):

### IL-01 — <title>
- Area: functionality | guides | runtime | architecture | performance | ux | harness
- Priority: <1-5, 1 highest> (impact, risk, cost in one line)
- Source: <handoff line, survey result, benchmark, screenshot, file:line>
- Acceptance: <objective check: command + expected result, or measured number before -> target>
- Status: todo | doing | done <short-hash> | needs-user: <question> | dropped: <reason>
- Attempts: 0
- Notes: Rule in  (used only by ); unit test . Survey before -> after: ambiguous 17 -> 9, compiled 52 -> 58, residual_condition_not_understood/resolved 2 -> 4. Newly compiled, hand-checked against game English names and the line's action: ITA "On Power…: Economist" -> ITA_power_to_the_king "Power to the King" (tied: Power Plants in Terni); ITA "On Seek…: Expand Lateran…" -> ITA_seek_papal_support "Seek Papal Support" (tied: Seek British Military Cooperation); ITA "On Strengthen…: Liturgical…" -> ITA_strengthen_the_papacy "Strengthen the Papacy" (tied: four other Strengthen focuses); POL "On Pan-Slavic…: justify HUN" -> POL_pan_slavism "Pan-Slavic Revanchism" (tied: POL_pan_slavic_revanchism, same English name); SPR "On The War…: Push north" -> SPA_the_war_in_the_north "The War in the North" (tied: The War of Vengeance); AUS "On Bring…: ... Annex HUN" -> AUS_bring_hungary_back_into_the_fold "Bring Hungary Back Into the Fold". Two now resolved but still unknown for their extra condition: AUS "On Reform The… & GER down" (Reform the Central Powers), SPR "On The National… (or when JAP war on CHI…)" (The National-Syndicalist State). Still ambiguous with two or more tied guide steps: SOV "Expand…done" (x2 lines), NOR "Expand…", AUS "Bring…" (x2), GER "Expand…", ENG "Imperial…", GER "Establish…", GER "Liberate…".  updated (ITA Strengthen now compiled; SOV Expand…done asserted ambiguous) and green; survey now prints tied candidates (TRIGCAND). test-native guides 69 pass, backend all pass; JS 460 pass / 0 fail / 3 skipped with dev-built backend; invariant-reviewer: no violations. NOT VERIFIED in game.
- Checked: (coordinator, after a green re-run)
-->

## Items

### IL-01 — Resolve ambiguous "On X…" trigger prefixes with the guide's own steps
- Area: guides
- Priority: 1 (17 trigger lines stay unknown only because the prefix matches several entities; moderate risk, Rust-only)
- Source: `event_reference_survey` 2026-11-18: `trigger reason unknown:reference_not_resolved/ambiguous: 17` (e.g. ITA "On Power…", "On Seek…", "On Strengthen…", POL "On Pan-Slavic…", SOV "On Expand…done"); `docs/events-live.md` "Still open: … ambiguous prefixes".
- Acceptance: `HOI4_REAL_CATALOG=1 node scripts/dev-native.cjs test backend --test integration real_catalog_contract::event_reference_survey -- --nocapture` -> `reference_not_resolved/ambiguous` 17 -> lower and `trigger reason compiled` 52 -> higher. A prefix is resolved only when exactly one candidate is a focus (or decision/tech) that the same guide names as a step; otherwise it stays unknown. Every newly compiled trigger hand-checked against the game English name and listed in Notes. New Rust unit test for the rule (unique match among guide steps resolves, two matches stay unknown). `real_catalog_contract_comparison` (HOI4_REAL_CATALOG=1) and `node scripts/test-native.cjs backend` green.
- Status: done de1aff0
- Attempts: 1
- Notes:

### IL-02 — Narrow event-answer lines with several candidates to events the guide nation can receive
- Area: guides
- Priority: 1 (19 single named lines + 10 block lines show candidates but bind nothing; reuses existing `event_reach` receivers)
- Source: `event_reference_survey` 2026-11-18: `family 2_named candidates: 19`, `family 2_named(block) candidates: 10`, `event_bound: 22` (e.g. HUN "They can not…: Def against YUG" -> cand=wuw_GER_civil_war.5.a, WW_hungary.30.b); `docs/events-live.md` "event answers matching several received events".
- Acceptance: same survey command -> `family 2_named candidates` 19 -> lower and `event_bound` 22 -> higher. An answer binds only when exactly one candidate event can reach the guide's nation (receivers from `event_reach` / `nation_events`); otherwise candidates stay as they are. Each new binding hand-checked (event title and option text vs the guide line) in Notes. Rust test: one reachable candidate binds, two reachable stay candidates. `node scripts/test-native.cjs backend` green.
- Status: todo
- Attempts: 0
- Notes:

### IL-03 — Make the intermittent JS startup/import test deterministic
- Area: harness
- Priority: 2 (a flaky test makes every improve-loop check unreliable; cheap)
- Source: `docs/AGENT_HANDOFF.md` "Prior startup/import test was intermittent; no unrelated root-cause fix claimed."
- Acceptance: identify the test by running the native-service / startup / import test files with `HOI4_BACKEND_EXE=.local/rust-dev-backend-target/release/hoi4-backend.exe node --test <file>` at least 10 times each; record failures before. After the fix: 10/10 consecutive passes of that file and one full `npm test` green. If no failure appears in 20 runs, set `dropped: not reproducible in 20 runs` with the evidence in Notes.
- Status: todo
- Attempts: 0
- Notes:

### IL-04 — Observe "justify <TAG>" war-goal lines if a qualified live field exists
- Area: guides
- Priority: 2 (21 survey lines mention justify; today recognized only, `action_resolution` not_observable; risk: needs a qualified live field)
- Source: `event_reference_survey` 2026-11-18 (`family other unbound: 48`, `other(block) unbound: 43`, e.g. "justify HUN (31PP)", "Justify BUL ASAP"); `docs/AGENT_HANDOFF.md` "justify/annex/integrate/invade/collab recognized only".
- Acceptance: first check read-only (code, `docs/foreign-interactions-live.md`, live profiles) whether a qualified live field reports an ongoing or completed war-goal justification against a tag. None: set `dropped: no qualified war-goal field` with the evidence. Exists: bind justify lines via `political_binding::foreign` like `declare_war`, Rust integration test like `each_foreign_part_of_a_line_is_kept_once_observed`, survey `other_bound` 13 -> higher, nothing bound when the field is unknown, backend tests green. Live behavior NOT VERIFIED in game.
- Status: todo
- Attempts: 0
- Notes:

### IL-05 — Answer before every guide is prepared: active guide first, rest in background
- Area: performance
- Priority: 3 (every new backend exe re-prepares 50 guides, ~41 s before the app answers; high risk: startup concurrency)
- Source: `docs/AGENT_HANDOFF.md` "Not done: compiled-guide cache still keyed by the backend binary hash … Better next step: ready immediately, prepare active guide first, rest in background." Code: `native/backend/src/http.rs` startup (`[startup] guides ready`), `Application::prepare_context`.
- Acceptance: new Rust integration test: with several guides and a cold compiled cache, the server answers `/api/guide/projection` for the active guide before the other guides finish, and a guide not yet prepared is reported as preparing/unavailable (never evaluated or shown as done) until ready. Measure time to ready on a copy of the data dir in the scratchpad (`HOI4COMPANION_DATA_DIR`, never the real `.local` user data), before -> after in Notes. Backend tests and JS suite green. If the UI behavior while preparing needs a product decision, set needs-user with the options.
- Status: todo
- Attempts: 0
- Notes:

### IL-06 — Speed up the real-catalog event reference survey
- Area: harness
- Priority: 4 (228 s per run slows every guide-reference item; low risk, test-only)
- Source: seeding run 2026-11-18: `event_reference_survey` took 228.24 s after a 26 s build.
- Acceptance: find where the time goes (timestamps, no guessing). Survey test time 228 s -> under 120 s with the same command, identical summary JSON before and after (diff in Notes). Only test/support code changes; no production behavior change.
- Status: todo
- Attempts: 0
- Notes:

### IL-07 — One word per status in the UI strings
- Area: ux
- Priority: 4 (same states worded differently: "unverifiable"/"Unverifiable", "state not verified"/"State not verified", "WAITING"/"Waiting"; low risk)
- Source: `translations.json` scan 2026-11-18 (1703 messages).
- Acceptance: a JS test lists the status labels used by guide/live/HUD views and asserts each state (done, in progress, waiting, unknown/unverifiable, manual) uses one English and one Italian word with consistent case. Strings change only through the add-translation skill. `npm test` green with the dev backend. Commit subject has `[ux-review]`. No new UI options or text.
- Status: todo
- Attempts: 0
- Notes:

## Run report
