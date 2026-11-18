# Improve Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A user-invoked project skill `/improve-loop` that coordinates an unattended run of small, verified improvements, one backlog item per Claude Code session, on a dedicated local branch.

**Architecture:** The skill is prose, not code. `SKILL.md` holds the coordinator procedure (start, seed backlog, start one worker session per item, verify, stop, report). `worker.md` holds the per-item procedure a worker session follows. `docs/improvement-backlog.md` is the shared state: header (branch, start time, limits) plus one block per item.

**Tech Stack:** Claude Code project skills (`.claude/skills/<name>/SKILL.md` with frontmatter), desktop-app session tools (`mcp__ccd_session__start_session`, `mcp__ccd_host__request_keep_awake`), existing project scripts (`npm test`, `node scripts/test-native.cjs`, `node scripts/dev-native.cjs`), agent `invariant-reviewer`.

**Spec:** `docs/superpowers/specs/2026-11-18-improve-loop-design.md`

## Global Constraints

- Skill text in English, like `.claude/skills/deliver-build/SKILL.md`; frontmatter has `disable-model-invocation: true` (user-triggered only).
- The skill refers to `CLAUDE.md` for invariants, build and test commands; it does not copy them.
- Branch: `auto/improve-<YYYY-MM-DD>`, created from the current commit in the same checkout. No worktree.
- Limits: stop after 15 items `done`, or 8 hours after the start time in the backlog header, or no runnable `todo` item. Max 2 attempts per item. Max 3 items, then re-rank the backlog.
- One worker session at a time: `start_session` with `initiation: "user_asked"`, `context: "fresh"`, `use_worktree: false`, `permission_mode` omitted (inherits).
- Item statuses, exactly: `todo`, `doing`, `done <short-hash>`, `needs-user: <question>`, `dropped: <reason>`.
- UX commits contain `[ux-review]` in the subject.
- Never: push, merge, `deliver-build`, `avvia-tutto.ps1`, writes under `.local/` user data or `src/hoi4companion/_native/`, any game interaction.
- Temporary files only in the session scratchpad.
- Commits: Conventional Commits, one per item, ending with the attribution line the session's system reminder gives.

## Review Focus

- Worker session ends without a commit (crash, context limit, refusal): the coordinator must see `doing` with no new commit, stash the leftovers with `git stash push -u -m "improve-loop <id> attempt <n>"` (never discard), count the attempt, and continue.
- Coordinator restarted mid-run (user types `/improve-loop` or "continua" after notices were held): it must detect the existing backlog header and branch, check out the branch, and resume from the first `todo`, not reseed.
- Worker claims green tests that are red: the coordinator re-runs `npm test` and `node scripts/test-native.cjs <crate>` for the crates in the item's diff before accepting `done`.
- Dirty tree or a detached HEAD at start: stop before creating the branch, with a one-line reason.
- UX item while the app is not running: no backend is started; the item records "visual check NOT VERIFIED" instead of screenshots.

Each line above is pinned by a scenario in Task 3.

---

### Task 1: Backlog format and coordinator skill

**Files:**
- Create: `.claude/skills/improve-loop/SKILL.md`
- Create: `docs/improvement-backlog.md`

**Interfaces:**
- Produces: backlog item block format (used by Task 2 and the coordinator):
  ```markdown
  ### <id> — <title>
  - Area: functionality | guides | runtime | architecture | performance | ux | harness
  - Priority: <1-5, 1 highest> (impact, risk, cost in one line)
  - Source: <handoff line, survey result, benchmark, screenshot, file:line>
  - Acceptance: <objective check: command + expected result, or measured number before -> target>
  - Status: todo
  - Attempts: 0
  - Notes:
  ```
  `<id>` is `IL-<nn>`, zero-padded, never reused.
- Produces: backlog header fields `Branch:`, `Started:` (ISO local time), `Limits: 15 items / 8 h / 2 attempts`, and a final `## Run report` section.

- [ ] **Step 1: Write `docs/improvement-backlog.md`** as an empty backlog: title, one paragraph on what it is and who writes it, the header fields with placeholders, the item block format above as a commented template, an empty `## Items` section and an empty `## Run report` section.

- [ ] **Step 2: Write `SKILL.md` frontmatter**: `name: improve-loop`, a `description` saying it coordinates an unattended improvement run of one session per backlog item, `disable-model-invocation: true`.

- [ ] **Step 3: Write the "Start or resume" section** of `SKILL.md`:
  1. Read `CLAUDE.md`, `docs/AGENT_HANDOFF.md`, `git status`.
  2. If the backlog header has a `Branch:` that exists locally: check it out (only if the tree is clean), resume. Otherwise: tree must be clean and HEAD on a branch, else stop with one line; create `auto/improve-<date>`; write `Branch:` and `Started:`.
  3. Call `mcp__ccd_host__request_keep_awake` with `until: "session_idle"` (load it with ToolSearch first).

- [ ] **Step 4: Write the "Seed the backlog" section** (only when `## Items` is empty). Sources, in this order: "Not done" / "Residual" / "NOT verified" lines of `docs/AGENT_HANDOFF.md`; open items in `docs/events-live.md` and `docs/guide-interpretation-benchmark.md`; the survey `HOI4_REAL_CATALOG=1 node scripts/dev-native.cjs test backend --test integration real_catalog_contract::event_reference_survey -- --nocapture`; a UX pass over `src/hoi4companion/web/` (and screenshots of `http://127.0.0.1:8766/suite.html` with the built-in browser only if the app already answers there; never start it). Write at most 15 items ranked by priority, each with a measurable `Acceptance`. Commit the backlog: `docs: seed the improvement backlog`.

- [ ] **Step 5: Write the "Run one item" section**: pick the first `todo` by priority; set `doing`, `Attempts` +1; commit nothing yet; call `start_session` with the constraints above, `title` = `<id> <title>`, `name` = lowercase `<id>`, `prompt` = the item block plus "Follow `.claude/skills/improve-loop/worker.md` for this one item.", `background` = one paragraph: branch, unattended run, rules of `worker.md`. Then end the turn and wait for the session's end notice; do not poll.

- [ ] **Step 6: Write the "Check the result" section**: on the notice, read the item's status and `git log -1`. If `done <hash>` and the hash is `HEAD~1` (HEAD is the `docs: mark <id> done` commit): re-run `npm test` and `node scripts/test-native.cjs <crate>` for each crate under `native/` in `git show --stat <hash>`; red means `git revert --no-edit HEAD <hash>`, status back to `todo` (or `dropped: tests red after 2 attempts`). If no commit: stash leftovers as in Review Focus, status `todo` or `dropped` by attempt count. `needs-user` and `dropped` are accepted as written. Commit backlog updates with `docs: update the improvement backlog`.

- [ ] **Step 7: Write the "Next or stop" section**: after every 3 finished items, re-rank remaining `todo` items with what changed. Stop on the limits in Global Constraints or on a regression the revert did not fix (tree left at the last green commit). On stop: run `npm test` and `node scripts/test-native.cjs` (all crates); write `## Run report` (improvements by area, tools used, harness changes, tests PASS/FAIL/NOT VERIFIED, commits, `needs-user` questions, next priority); add 3-5 lines to `docs/AGENT_HANDOFF.md`; commit `docs: report the improvement run`; post the same report in chat.

- [ ] **Step 8: Write the "Never" list** from Global Constraints, as the last section.

- [ ] **Step 9: Check the skill is listed**: start `claude` or reload the app session and confirm `improve-loop` appears among the skills with the description. Expected: present, not auto-invoked.

- [ ] **Step 10: Commit**
```bash
git add .claude/skills/improve-loop/SKILL.md docs/improvement-backlog.md
git commit -m "feat(harness): add the improve-loop coordinator skill"
```

### Task 2: Worker procedure

**Files:**
- Create: `.claude/skills/improve-loop/worker.md`

**Interfaces:**
- Consumes: the item block format and statuses from Task 1.
- Produces: on exit, exactly one of: one commit + `Status: done <short-hash>`; `Status: needs-user: <question>` and a clean tree; nothing committed and leftovers in the tree (the coordinator handles them).

- [ ] **Step 1: Write the "Before" section**: read `CLAUDE.md`, the `docs/` file for the touched subsystem (index `docs/README.md`), the item. Confirm current branch is the backlog's `Branch:` and the tree is clean; otherwise stop with a one-line report. If `Acceptance` is missing or not objective, write it in the item first.

- [ ] **Step 2: Write the "Do" section**: simplest change that meets `Acceptance`; evaluation logic only in Rust (`native/guide-core` or backend crates); TDD for behavior changes (failing test first); for a bug, add to `Notes` why existing tests missed it and add the test; UI strings through the `add-translation` skill.

- [ ] **Step 3: Write the "Verify" section**: targeted tests first, then `npm test` and `node scripts/test-native.cjs <crate>` for touched crates; fixture regeneration only per `CLAUDE.md`, with the fixture diff read; run the `invariant-reviewer` agent when the diff touches guide evaluation, live state, HUD, web pages, importer or packaging, and fix what it reports; measure `Acceptance` numbers and write before -> after in `Notes`. Live compatibility is always written "NOT VERIFIED in game".

- [ ] **Step 4: Write the "UX items" section**: screenshots before/after with the built-in browser only if `http://127.0.0.1:8766/suite.html` already answers (web files are served live, so a page reload shows the change); save them in the scratchpad and name the paths in `Notes`; otherwise write "visual check NOT VERIFIED". Subject contains `[ux-review]`.

- [ ] **Step 5: Write the "Finish" section**: commit the code change; then write `Status: done <short-hash of that commit>` and commit it separately as `docs: mark <id> done` (no amend); the coordinator therefore checks `HEAD~1` as the code commit; a decision only the user can take → revert own changes with `git restore`/`git clean` limited to files this session created or changed, `Status: needs-user: <question>`, commit the backlog; end with a 3-line report (what changed, tests, status). Update `docs/AGENT_HANDOFF.md` only for a significant milestone.

- [ ] **Step 6: Write the "Never" list** (same as `SKILL.md`, plus: never start another session, never touch other backlog items).

- [ ] **Step 7: Commit**
```bash
git add .claude/skills/improve-loop/worker.md
git commit -m "feat(harness): add the improve-loop worker procedure"
```

### Task 3: Scenario check and handoff

**Files:**
- Modify: `docs/AGENT_HANDOFF.md` (one line under "Faster edit loop")

- [ ] **Step 1: Run a scenario check**: dispatch a fresh general-purpose subagent with only `SKILL.md`, `worker.md` and `docs/improvement-backlog.md`, asking what the coordinator or worker does in each case, and expecting these answers:
  1. Worker ends, item `doing`, no new commit, tree dirty → stash with the `improve-loop <id> attempt <n>` message, `todo` (attempt 1) or `dropped` (attempt 2), next item.
  2. `/improve-loop` typed again, header has `Branch: auto/improve-2026-11-18` that exists → checkout, resume first `todo`, no reseed.
  3. Worker wrote `done abc123` but `npm test` fails → `git revert --no-edit HEAD abc123`, attempt counted.
  4. Start with a modified tracked file → stop before branching, one-line reason.
  5. UX item, nothing answers on port 8766 → no app start, "visual check NOT VERIFIED".
  6. 15 items `done` → final suite, run report, handoff lines, stop.
  7. Item needs a product decision → `needs-user: <question>`, clean tree, coordinator moves on.
  Expected: every answer matches. Fix the skill text for any mismatch and re-ask only the failed cases.

- [ ] **Step 2: Run `npm test`**. Expected: same pass count as before this plan (no source changed), 0 failures.

- [ ] **Step 3: Add one handoff line**: improve-loop skill exists, how to launch (`/improve-loop` in a new session on this folder, auto permission mode), state in `docs/improvement-backlog.md`, never run yet.

- [ ] **Step 4: Commit**
```bash
git add docs/AGENT_HANDOFF.md
git commit -m "docs: note the improve-loop skill in the handoff"
```
