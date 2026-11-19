# Ambiguous Event Confirmation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show ambiguous event answers in their own HUD color, let the player confirm the guide's answer on the HUD card, store that confirmation durably in the user's SQLite database, and re-apply it to every run of the guide.

**Architecture:** Confirmations are rows in a new SQLite table written atomically with the progress save (same pattern as `guide_definitions`). The backend applies them, in Rust (`native/guides`), to the definition returned by `definition_for_run`, never to the archive or the prepared cache. HUD proposals carry a semantic role; the overlay maps roles to one palette.

**Tech Stack:** Rust (crates `storage`, `guides`, `hud`, `backend`, `overlay`), rusqlite, Direct2D overlay, plain JS web suite, `translations.json`.

**Spec:** `docs/superpowers/specs/2026-11-19-ambiguous-event-confirmation-design.md`

## Global Constraints

- Read-only toward the game: no input, injection, memory writes, or game/mod file changes.
- Unknown stays unknown: a confirmation applies only when guide id, step key, quote and a still-compatible candidate all match; otherwise the row is kept and not applied.
- Rust is the only evaluator; the web JS only presents and calls routes. No Python anywhere.
- Definition archives (`guide_definitions`) and prepared-guide caches never contain confirmations.
- Positional step keys are never reinterpreted: the quote must match too.
- UI strings only through `translations.json` (English plus Italian), with the `add-translation` skill.
- After an evaluation change: `HOI4_UPDATE_PRESENTATION_FIXTURES=1 node scripts/test-native.cjs backend`, then review the fixture diff.
- Live behavior NOT VERIFIED in game until the user checks it.

## Plan-level decision found in the code (differs from the spec, tell the user)

`reference_candidates` keeps the candidates of **one reference** per line (the first truncated one). A line like
"We cannot…, We shall…" holds two references. A confirmation is therefore keyed by the **reference text**
(`quote` = `reference_candidates.text`, for example "We cannot…"), not by the whole line. The line becomes a
bound, checkable step only when the confirmed reference is the line's only reference (`reference_candidates::references(line).len() == 1`).
On a line with several references the confirmation is stored and shown ("confirmed by you") but the line stays a
reminder and the HUD click does not check it.

## Review Focus

1. A confirmation whose option is no longer a compatible candidate after a catalog or game update: kept, not applied, the line shows its candidates (test in Task 2).
2. A positional key that now points at a different line (same `step_key`, different quote): not applied (Task 2).
3. Undo right after a confirmation that replaced an older one: the older row comes back, not nothing (Task 3).
4. The HUD "I chose this" click arriving after the popup closed or the step changed: refused, nothing stored (Task 3).
5. A two-reference line: the click stores the confirmation but checks nothing (Task 2 and Task 3).

---

### Task 1: Storage table for confirmed answers

**Files:**
- Create: `native/storage/src/confirmed_answers.rs`
- Modify: `native/storage/src/lib.rs` (`Request`, `Operation::Load`, `Operation::Save`)

**Interfaces:**
- Produces: table `confirmed_event_answers(guide_id TEXT NOT NULL, step_key TEXT NOT NULL, quote TEXT NOT NULL, event_id TEXT NOT NULL, option_id TEXT NOT NULL, confirmed_at TEXT NOT NULL, PRIMARY KEY(guide_id, step_key, quote))`, created lazily by the first save that writes one (like `guide_definitions`).
- Produces: `Request.answers: Option<Vec<Value>>`, each `{"op":"put", guide_id, step_key, quote, event_id, option_id, confirmed_at}` or `{"op":"delete", guide_id, step_key, quote}`, applied in the same transaction as the progress save.
- Produces: `Load` result gains `"answers": [ {guide_id, step_key, quote, event_id, option_id, confirmed_at}, … ]` (empty array when the table does not exist).
- Produces: `pub fn validate(row: &Value) -> Result<(), String>` in `confirmed_answers.rs` (non-empty strings, each at most 512 chars, ids of `[A-Za-z0-9_.-]`).

- [ ] **Step 1: Write failing tests** in `confirmed_answers.rs`: `put_then_load_returns_the_row`, `put_on_same_key_and_quote_replaces`, `delete_removes_only_that_quote`, `rows_survive_reopening_the_database`, `invalid_row_rejects_the_whole_save` (progress revision unchanged), `load_without_table_returns_empty_answers`.
- [ ] **Step 2: Run** `node scripts/test-native.cjs storage` — expected FAIL.
- [ ] **Step 3: Implement** validation before opening the writable connection, `CREATE TABLE IF NOT EXISTS` plus `INSERT OR REPLACE` / `DELETE` inside the existing save transaction, and the `answers` array in `Load`.
- [ ] **Step 4: Run** `node scripts/test-native.cjs storage` — expected PASS.
- [ ] **Step 5: Commit** `feat(storage): store confirmed event answers with the progress save`.

### Task 2: Apply confirmations to a definition (Rust, guides)

**Files:**
- Create: `native/guides/src/confirmed_answers.rs` (declare in `native/guides/src/lib.rs`)

**Interfaces:**
- Consumes: rows from Task 1; `political_binding::bind` with the probe used by `reference_candidates::bind` (`{"action":{"kind":"manual","params":{"category":"politics","event_id":…,"event_option_id":…}}}`); `reference_candidates::references`.
- Produces: `pub fn apply(guide: &mut Value, answers: &[Value], index: &Index, country: &Value) -> Result<(), String>`. For each row of `guide` whose `key == step_key` and `action.params.reference_candidates.text == quote` and whose `reference_candidates.event_options` contains `{id: option_id, event_id}`:
  - always set `reference_candidates.confirmed = {"event_id", "option_id", "basis": "user_confirmed"}`;
  - when the line has exactly one reference and `political_binding::bind` returns `event_visible_option`: remove `tracking`, copy the bound `started_when`, set `action.params` from the bound step with `reference_resolution.basis = "user_confirmed"`. Completion stays manual (no `completion` fact).
  - Otherwise change nothing.

- [ ] **Step 1: Write failing tests**: `single_reference_line_becomes_a_bound_checkable_step` (assert `tracking` absent, `started_when` is the `events.visible_options` fact for `"E:E.b"`, `reference_resolution.basis == "user_confirmed"`, row `kind == "gate"` without `tracking`, which is what `manual_keys` in the backend accepts); `two_reference_line_records_but_stays_a_reminder`; `quote_mismatch_is_not_applied`; `option_no_longer_a_candidate_is_not_applied`; `other_guide_rows_are_ignored`.
- [ ] **Step 2: Run** `node scripts/dev-native.cjs test guides confirmed_answers` — expected FAIL.
- [ ] **Step 3: Implement `apply`.**
- [ ] **Step 4: Run** `node scripts/test-native.cjs guides` — expected PASS.
- [ ] **Step 5: Commit** `feat(guides): apply user-confirmed event answers to a run's definition`.

### Task 3: Backend: load, apply, confirm, undo, remove

**Files:**
- Modify: `native/backend/src/application.rs` (`definition_for_run`, new op `app_answers`), `native/guides/src/progress_store.rs` (carry `answers` through load/save), `native/backend/src/http.rs` (bridge kind `confirm_answer`; routes `GET`/`POST /api/guide/answers`)
- Test: new module `native/backend/tests/integration/confirmed_answers.rs` declared in `native/backend/tests/integration/main.rs`

**Interfaces:**
- Consumes: Task 1 storage fields, Task 2 `confirmed_answers::apply`, the catalog index the backend already holds (`runtime_context`, `catalog.import_index()`).
- Produces: `definition_for_run` returns the definition with `apply` run on a clone (the archive and `archived_guides` entry stay unchanged). `manual_keys` and the projection both see it.
- Produces: `app_answers` with `{"guide_id", "update": {"op":"confirm", "key", "quote", "event_id", "option_id"}}` → one progress save that puts the row and, when the applied definition makes `key` a manual key, sets `checks[key] = true`; returns `{"previous": <row or null>, "checked": bool}`. `{"op":"restore", "key", "quote", "previous": <row or null>}` → puts `previous` back or deletes, and unchecks `key`. `{"op":"remove", "key", "quote"}` → deletes the row only.
- Produces: bridge `{"kind":"confirm_answer","guide_id","key","quote","event_id","option_id"}` accepted only when the latest overlay frame has a `visible_candidate` proposal for that key and option (like `hud_check_allowed`); bridge `{"kind":"restore_answer", …}` for Undo.
- Produces: `GET /api/guide/answers?id=<guide>` → rows of that guide; `POST /api/guide/answers` with `{"guide_id","key","quote","op":"remove"}`.

- [ ] **Step 1: Write failing integration tests**: `confirm_stores_the_row_and_checks_a_single_reference_line`; `confirm_on_two_reference_line_stores_but_does_not_check`; `restore_brings_back_the_replaced_row_and_unchecks`; `confirm_refused_when_no_visible_candidate_matches` (nothing stored); `confirmation_applies_to_a_new_run_of_the_same_guide`; `archive_payload_never_contains_confirmed`; `remove_deletes_the_row_and_the_line_is_ambiguous_again`.
- [ ] **Step 2: Run** `node scripts/dev-native.cjs test backend --test integration confirmed_answers::` — expected FAIL.
- [ ] **Step 3: Implement** the load/save plumbing, the `definition_for_run` application, `app_answers`, bridge kinds and routes.
- [ ] **Step 4: Run** `node scripts/test-native.cjs backend`, then `HOI4_UPDATE_PRESENTATION_FIXTURES=1 node scripts/test-native.cjs backend`; review `git diff tests/fixtures/presentation-evaluations.json` (expected: no change unless a case holds a confirmation).
- [ ] **Step 5: Commit** `feat(backend): confirm, undo and remove ambiguous event answers`.

### Task 4: HUD proposals carry a role

**Files:**
- Modify: `native/hud/src/hud_targets.rs` (`visible_candidate` and the event proposal for bound rows)

**Interfaces:**
- Produces: every proposal gains `"role"`: `"ambiguous"` for `resolution == "visible_candidate"` (plus `"reference_text"` = the line's `reference_candidates.text`), `"choice"` where `tone == "choice"` today, `"active"` where status is active and panel is not `event`, `"next"` otherwise. A bound row whose `reference_resolution.basis == "user_confirmed"` gets `"confirmed_by_user": true`. Existing fields stay.

- [ ] **Step 1: Write failing tests**: `visible_candidate_has_role_ambiguous_and_reference_text`; `user_confirmed_binding_is_next_and_flagged`; `choice_and_active_roles_match_previous_tone`.
- [ ] **Step 2: Run** `node scripts/test-native.cjs hud` — expected FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `node scripts/test-native.cjs hud` — expected PASS.
- [ ] **Step 5: Commit** `feat(hud): give every HUD proposal a color role`.

### Task 5: Overlay palette, ambiguous marker and "I chose this" button

**Files:**
- Create: `native/overlay/src/palette.rs`
- Modify: `native/overlay/src/render.rs` (`Paint`, `Highlight`, marker drawing, `Button`, `ControlAction`), `native/overlay/src/card.rs` (button layout), `native/overlay/src/main.rs` (`marks`, bridge request for the new action)

**Interfaces:**
- Consumes: Task 4 `role`, `reference_text`, `confirmed_by_user`; Task 3 bridge kinds.
- Produces: `pub enum Role { Next, Active, Choice, Ambiguous, Done }` and `pub fn color(role: Role) -> D2D1_COLOR_F` with Next amber `{1.0,0.83,0.30}`, Active cyan `{0.27,0.84,1.0}`, Choice purple `{0.78,0.59,1.0}`, Ambiguous pink `{1.0,0.45,0.75}`, Done green `{0.42,0.88,0.52}`; every brush in `render.rs` comes from `color(role)`.
- Produces: `Highlight.role: Role` replacing `active`/`choice`; Ambiguous draws a dashed border and prefixes the caption with `"? "`; `confirmed_by_user` appends the translated "confirmed by you".
- Produces: `Button::ConfirmAnswer`, shown on the card only when the current proposal's role is Ambiguous or `confirmed_by_user`; `ControlAction::ConfirmAnswer {guide_id, key, quote, event_id, option_id}`; Undo after it sends `restore_answer` with the returned `previous`.

- [ ] **Step 1: Write failing tests**: `palette_roles_have_the_spec_colors`; `marks_map_role_from_proposal`; `confirm_button_only_for_ambiguous_or_confirmed` (card hit-test layout like `card.rs:302`); `undo_after_confirm_sends_restore_with_previous`.
- [ ] **Step 2: Run** `node scripts/test-native.cjs overlay` — expected FAIL.
- [ ] **Step 3: Implement**; add strings "I chose this", "confirmed by you", "ambiguous answer" with the `add-translation` skill (Italian: "Ho scelto questa", "confermata da te", "risposta ambigua").
- [ ] **Step 4: Run** `node scripts/test-native.cjs overlay` and `powershell -NoProfile -File scripts/build-overlay.ps1 -Dev` — expected PASS / build OK.
- [ ] **Step 5: Commit** `feat(overlay): one palette, ambiguous marker and HUD answer confirmation`.

### Task 6: Web suite label and "Remove confirmation", docs

**Files:**
- Modify: `src/hoi4companion/web/guide.js` (near line 199, candidate rendering), `src/hoi4companion/translations.json`, `docs/events-live.md`, `docs/campaign-progress.md`, `docs/AGENT_HANDOFF.md`
- Test: `tests/test_guide_references.cjs`

**Interfaces:**
- Consumes: `reference_candidates.confirmed` (Task 2) in the projection; `POST /api/guide/answers` (Task 3).
- Produces: a step with `reference_candidates.confirmed` shows "Confirmed by you: <answer name>" and a "Remove confirmation" button that posts `{"guide_id","key","quote","op":"remove"}` and reloads the projection. No evaluation in JS.

- [ ] **Step 1: Write failing test** `confirmed_candidate_shows_label_and_remove_action` in `tests/test_guide_references.cjs`.
- [ ] **Step 2: Run** `node --test tests/test_guide_references.cjs` — expected FAIL.
- [ ] **Step 3: Implement**, strings through `add-translation`; update the three docs (what is stored, where, how to remove, NOT VERIFIED in game).
- [ ] **Step 4: Run** `node scripts/dev-native.cjs build backend` then `HOI4_BACKEND_EXE=.local/rust-dev-backend-target/release/hoi4-backend.exe npm test` — expected 0 fail.
- [ ] **Step 5: Commit** `feat(web): show and remove user-confirmed event answers`.
