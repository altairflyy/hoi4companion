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
- Notes:
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
- Notes: Rule in `entity_resolution::resolve_trigger` (used only by `reference_candidates::ground_trigger`); unit test `a_trigger_prefix_tied_between_entities_resolves_only_to_a_unique_guide_step`. Survey before -> after: ambiguous 17 -> 9, compiled 52 -> 58, residual_condition_not_understood/resolved 2 -> 4. Newly compiled, hand-checked against game English names and the line's action: ITA "On Power…: Economist" -> ITA_power_to_the_king "Power to the King" (tied: Power Plants in Terni); ITA "On Seek…: Expand Lateran…" -> ITA_seek_papal_support "Seek Papal Support" (tied: Seek British Military Cooperation); ITA "On Strengthen…: Liturgical…" -> ITA_strengthen_the_papacy "Strengthen the Papacy" (tied: four other Strengthen focuses); POL "On Pan-Slavic…: justify HUN" -> POL_pan_slavism "Pan-Slavic Revanchism" (tied: POL_pan_slavic_revanchism, same English name); SPR "On The War…: Push north with OFF" -> SPA_the_war_in_the_north "The War in the North" (tied: The War of Vengeance); AUS "On Bring…: Military Exercises+Annex HUN" -> AUS_bring_hungary_back_into_the_fold "Bring Hungary Back Into the Fold". Now resolved but still unknown for their extra condition: AUS "On Reform The… & GER down" (Reform the Central Powers), SPR "On The National… (or when JAP war on CHI…)" (The National-Syndicalist State). Still ambiguous with two or more tied guide steps: SOV "Expand…done" (2 lines), NOR "Expand…", AUS "Bring…" (2), GER "Expand…", ENG "Imperial…", GER "Establish…", GER "Liberate…". `real_catalog_contract_comparison` updated (ITA Strengthen now compiled; SOV Expand…done asserted ambiguous) and green; the survey now prints tied candidates (TRIGCAND lines). test-native guides 69 pass, backend all pass; JS 460 pass / 0 fail / 3 skipped against the dev-built backend; invariant-reviewer: no violations. NOT VERIFIED in game.
- Checked: green 2026-11-18T20:28:02+01:00

### IL-02 — Narrow event-answer lines with several candidates to events the guide nation can receive
- Area: guides
- Priority: 1 (19 single named lines + 10 block lines show candidates but bind nothing; reuses existing `event_reach` receivers)
- Source: `event_reference_survey` 2026-11-18: `family 2_named candidates: 19`, `family 2_named(block) candidates: 10`, `event_bound: 22` (e.g. HUN "They can not…: Def against YUG" -> cand=wuw_GER_civil_war.5.a, WW_hungary.30.b); `docs/events-live.md` "event answers matching several received events".
- Acceptance: same survey command -> `family 2_named candidates` 19 -> lower and `event_bound` 22 -> higher. An answer binds only when exactly one candidate event can reach the guide's nation (receivers from `event_reach` / `nation_events`); otherwise candidates stay as they are. Each new binding hand-checked (event title and option text vs the guide line) in Notes. Rust test: one reachable candidate binds, two reachable stay candidates. `node scripts/test-native.cjs backend` green.
- Status: needs-user: The acceptance rule cannot bind anything, because every candidate line has either no candidate event or two or more candidate events that reach the guide's nation. Choose one: (a) drop IL-02 and keep these lines as candidates; (b) add a second, riskier narrowing to events reached through the focuses the guide itself selects (it would bind BEL "Dutch…" and HUN "They can not…", and it would wrongly pick `mtg_britain.100.b` for ENG "We shall…" unless a guard is added); (c) another rule you specify.
- Attempts: 1
- Notes: Baseline re-measured on 7d30a5c: `family 2_named candidates` 19, `family 2_named(block) candidates` 10, `event_bound` 22 (unchanged by IL-01). `choose` already binds a single candidate whose event the nation provably receives (`event_received_by_nation`). Tried the item's rule: a new `reachable` flag (traced receivers include the nation, even when other callers are untraced) and binding exactly one reachable candidate. The unit test passed, but the survey stayed at 19 / 10 / 22: on the real catalog `reachable` was true exactly where `compatible` was already true. Each candidate line has either 0 reachable events (HUN "They can not…": `wuw_GER_civil_war.5.a` and `WW_hungary.30.b` both have no traced receivers; ITA "By any means…", GER "Rearmament has…", FRA "Their factories…": single candidates with no receiver) or 2 and more (BEL "Dutch…" 2, FRA "Issue a…" 2, GER "Perhaps They…" 2, GER "We have no need…" 4, POR "We do not…" 2, ENG "We shall…" 15). A probe of guide-focus reach (the event's trigger chain leads back to a focus the guide selects) would narrow BEL "Dutch…" to `ww_BEL_event_political.8.a` (via `BEL_problem_of_language`), HUN "They can not…" to `WW_hungary.30.b` (via `wuw_HUN_reinstate_the_pragmatic_sanction`, but its receiver is unknown), ENG "We shall…" to `mtg_britain.100.b` "We shall never surrender our independence!" (doubtful), and GER "Perhaps They…" still to 2. All code changes were reverted; nothing committed except this note. Evidence: `il-02-evidence.md` in the run's shared notes folder (`side-session-notes/local_a3dd6797-ad0d-4d27-a4de-fb8a7441f8a3`). Live game behavior NOT VERIFIED in game.

### IL-03 — Make the intermittent JS startup/import test deterministic
- Area: harness
- Priority: 2 (a flaky test makes every improve-loop check unreliable; cheap)
- Source: `docs/AGENT_HANDOFF.md` "Prior startup/import test was intermittent; no unrelated root-cause fix claimed."
- Acceptance: identify the test by running the native-service / startup / import test files with `HOI4_BACKEND_EXE=.local/rust-dev-backend-target/release/hoi4-backend.exe node --test <file>` at least 10 times each; record failures before. After the fix: 10/10 consecutive passes of that file and one full `npm test` green. If no failure appears in 20 runs, set `dropped: not reproducible in 20 runs` with the evidence in Notes.
- Status: done f70b01f
- Attempts: 1
- Notes: Before: 20 sequential runs each against the dev backend: `test_native_service.cjs` 19 pass / 1 fail; `test_importer_model`, `test_importer_view`, `test_focus_loading`, `test_decision_admission_http`, `test_live_stream` 20/20 each. The failure was `assert.ok((await api('/api/guides')).value.guides.some(row=>row.id===JSON.parse(saved).id))` right after the second start. Root cause (instrumented copy of the test): the guide file the test deletes to simulate an interrupted import is the same guide as `saved` (`word-a2493e7bf6d8f233`, the only file in `guides/`). After restart, `resume_collections` republishes it in a background collection job, and the test queried `/api/guides` before awaiting that job, so the request sometimes won the race. The backend behaves as designed (the guide catalog cache key includes `definition_generation`, which `put_guide` bumps, so no stale listing). Fix in the test only: assert the guide after `imported(auto.value.id)` reports completion. Why tests missed it: the race is timing-dependent and the test itself was the defect; the reordered assertion now checks the intended contract (the resumed import restores the guide). After: 30/30 consecutive passes of `test_native_service.cjs`; full `npm test` 460 pass / 0 fail / 3 skipped (463).
- Checked: green 2026-11-18T20:53:11+01:00

### IL-04 — Observe "justify <TAG>" war-goal lines if a qualified live field exists
- Area: guides
- Priority: 2 (21 survey lines mention justify; today recognized only, `action_resolution` not_observable; risk: needs a qualified live field)
- Source: `event_reference_survey` 2026-11-18 (`family other unbound: 48`, `other(block) unbound: 43`, e.g. "justify HUN (31PP)", "Justify BUL ASAP"); `docs/AGENT_HANDOFF.md` "justify/annex/integrate/invade/collab recognized only".
- Acceptance: first check read-only (code, `docs/foreign-interactions-live.md`, live profiles) whether a qualified live field reports an ongoing or completed war-goal justification against a tag. None: set `dropped: no qualified war-goal field` with the evidence. Exists: bind justify lines via `political_binding::foreign` like `declare_war`, Rust integration test like `each_foreign_part_of_a_line_is_kept_once_observed`, survey `other_bound` 13 -> higher, nothing bound when the field is unknown, backend tests green. Live behavior NOT VERIFIED in game.
- Status: dropped: no qualified war-goal field
- Attempts: 1
- Notes: Read-only check, no code changed. `native/live/src/country_foreign.rs` `RELATIONS` qualifies 18 relation classes (war, guarantee, non-aggression pact, improve relations, embargo, access rights, subject, lend-lease, naval blockade, market access and similar); none is a war-goal justification. `src/hoi4companion/live-profiles-v1.json` and `live-abi-v1.json` contain no war-goal, justification or claim field; a search for `justif|wargoal|war_goal` in `native/live`, `native/model`, `native/backend/src` and the live profiles finds nothing. `docs/foreign-interactions-live.md` lists only `diplomacy.*` and `intelligence.*` facts, and `docs/events-live.md` states that justifying a war goal is not readable live. Justify lines therefore stay recognized only (`action_resolution` not_observable). Binding them would first need a new native acquisition of the war-goal justification state, qualified on the pinned executable, which is outside this item.

### IL-05 — Answer before every guide is prepared: active guide first, rest in background
- Area: performance
- Priority: 3 (every new backend exe re-prepares 50 guides, ~41 s before the app answers; high risk: startup concurrency)
- Source: `docs/AGENT_HANDOFF.md` "Not done: compiled-guide cache still keyed by the backend binary hash … Better next step: ready immediately, prepare active guide first, rest in background." Code: `native/backend/src/http.rs` startup (`[startup] guides ready`), `Application::prepare_context`.
- Acceptance: new Rust integration test: with several guides and a cold compiled cache, the server answers `/api/guide/projection` for the active guide before the other guides finish, and a guide not yet prepared is reported as preparing/unavailable (never evaluated or shown as done) until ready. Measure time to ready on a copy of the data dir in the scratchpad (`HOI4COMPANION_DATA_DIR`, never the real `.local` user data), before -> after in Notes. Backend tests and JS suite green. If the UI behavior while preparing needs a product decision, set needs-user with the options.
- Status: done 47d0f77
- Attempts: 1
- Notes: Startup prepares only the active guide; the rest join the registry from a background thread (`Application::set_context_first` / `apply_pending`, `Service::prepare_pending_guides`). A pending guide is absent from `/api/guides` and its projection answers 400 "Guide still being prepared: try again in a few seconds"; campaign binding and following the played nation wait while any guide is pending or failed (fail closed, from invariant review). UI shows the shorter list for ~33 s with no new UI; no product decision needed. Tests: integration `startup_preparation::the_server_answers_for_the_active_guide_before_the_other_guides_are_prepared`, unit `pending_guides_stay_unavailable_until_prepared_for_the_current_catalog`; backend crate all pass (lib 45, integration 104), JS 460 pass / 3 skipped / 0 fail against the dev-built backend. Measured on a scratch copy of the data dir (50 guides, real catalog, cold compiled cache, no HOI4 running), HEAD a2f5c6f vs this change: ready file 44.75 s -> 14.18 s (14.40 s rerun); first active-guide projection 50.1 s -> 21.1 s; all guides ready 44.4 s -> 47.6 s, a non-active guide projection 54.9 s -> 51.7 s (400 "being prepared" before). NOT verified in game.
- Checked: green 2026-11-18T22:59:09+01:00

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
