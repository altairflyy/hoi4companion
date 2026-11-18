---
name: improve-loop
description: Coordinate an unattended run of small, verified improvements to HOI4 Companion on a dedicated local branch, one Claude Code session per backlog item (docs/improvement-backlog.md). Seeds and ranks the backlog, starts each worker session, re-checks its tests, stops on limits and writes the run report.
disable-model-invocation: true
---

# Improve loop — coordinator

This session coordinates; it never implements an item itself. Workers follow
`.claude/skills/improve-loop/worker.md`. State lives in `docs/improvement-backlog.md`,
never only in this conversation: after a context compaction, re-read it and `git log`.
Invariants, build and test commands are in `CLAUDE.md`; they apply to every step here.
The user is away: never stop to ask. A decision only the user can take becomes a
`needs-user` item and the run goes on.

## 1. Start or resume
Run shell commands that set environment variables (`VAR=x cmd`) with the Bash tool, not PowerShell.

Run every command in the foreground (Bash `timeout` up to 600000 ms); never use `run_in_background`, Monitor
or wakeups. The app lets this session start a worker only in a turn that began from a user message or from a
worker's end notice; a turn woken by a finished background command cannot start one.

1. Read `CLAUDE.md`, `docs/AGENT_HANDOFF.md`, the backlog, and run `git status`, `git branch --show-current`
   and `git branch --list "auto/improve-*"`.
2. Load with ToolSearch, in one `select:` call: `mcp__ccd_host__request_keep_awake`,
   `mcp__ccd_session__start_session`, `mcp__ccd_session_mgmt__list_sessions`.
3. It is a **resume** if an `auto/improve-*` branch (current or not) has no `## Run report` content yet in its
   backlog (`git show <branch>:docs/improvement-backlog.md`); a finished run is never resumed. If several qualify, take
   the most recent by commit date. Then:
   - if that branch is not current: the tree must be clean (otherwise stop with one line), then `git switch` to it;
   - an item left `doing`: first call `list_sessions` with `{"linked": true}`; if the session named after the
     item id is still running, end the turn and wait for its end notice. Otherwise handle it as in step 4
     "no commit" (that works on a dirty tree);
   - a `done` item without a `Checked:` line: run the step 4 checks for it now;
   - add a line `- Resumed: <local ISO time>` under `Started:`; the 8 h limit counts from the latest
     `Resumed:` (the user restarting the run is a request to continue);
   - commit `docs: resume the improvement run`, then continue at section 2 (seeding is skipped if Items exist).
4. Otherwise this is a **new run**:
   - the tree must be clean and HEAD on a branch; otherwise stop with one line saying why, before creating anything;
   - `git switch -c auto/improve-<YYYY-MM-DD>` (add `-2`, `-3` if it exists);
   - write `Branch:` and `Started:` (local ISO time) in the backlog and commit `docs: start the improvement run`.
5. Call keep-awake with `until: "session_idle"` and `reason: "Unattended HOI4 Companion improvement run"`.
   It may lapse while this session waits; each worker session keeps the machine awake during its own turns.

## 2. Seed the backlog (only when `## Items` is empty)
Collect candidates, in this order, and keep only those with an objective acceptance check:
- "Not done", "Residual", "Remaining", "NOT verified" and "Still open" lines in `docs/AGENT_HANDOFF.md`, `docs/events-live.md`, `docs/guide-interpretation-benchmark.md`.
- The real-catalog survey (needs `.local/` data; skip if it reports skipped):
  `HOI4_REAL_CATALOG=1 node scripts/dev-native.cjs test backend --test integration real_catalog_contract::event_reference_survey -- --nocapture`
  Redirect long output to the scratchpad and read the summary.
- A UX pass over `src/hoi4companion/web/` and `translations.json`: unclear labels, inconsistent status words
  (done / waiting / unobservable / unknown / manual), missing explanations of errors. If
  `http://127.0.0.1:8766/suite.html` already answers, look at it in the built-in browser. Never start the app.
- Harness friction seen while doing the above (slow or flaky commands, missing checks).

Write at most 15 items in the backlog format, ranked by priority (impact and frequency up, risk and cost down).
Prefer items that move a measured number or fix a defect. Do not add items that only add UI options or text.
Commit: `docs: seed the improvement backlog`.

## 3. Run one item
1. Pick the highest-priority `todo` item. None left: go to step 6.
2. Set `Status: doing`, `Attempts:` +1. Do not commit this change.
3. Call `start_session` with:
   - `initiation: "user_asked"`, `context: "fresh"`, `use_worktree: false`, `permission_mode: "auto"`
     (if the app refuses it because this session is not in auto mode, set the item back to `todo` and
     `Attempts` -1, and stop with one line: workers would stall on permission prompts);
   - `title`: `<id> <title>` (under 60 chars), `name`: the id in lowercase (`il-03`);
   - `prompt`: the full item block, then: "Follow `.claude/skills/improve-loop/worker.md` for this one item only.";
   - `background`: "Unattended improvement run of HOI4 Companion on branch `<branch>`, coordinated by another session. One worker session per backlog item, one at a time, in the same checkout. The user is away: never ask questions; follow `worker.md`."
4. End the turn. Do not poll: the app continues this session when the worker ends.

Only one worker session runs at a time: they share the checkout, branch and build caches.

## 4. Check the result
On the worker's end notice, re-read the item and run `git status` and `git log --oneline -3`.
- **`done <hash>`**, `<hash>` is `HEAD~1` and HEAD is `docs: mark <id> done`: re-run
  `node scripts/test-native.cjs <crate>` for each crate under `native/` listed by `git show --name-only <hash>`,
  plus `backend` whenever any of them is not `overlay` or `desktop` (backend depends on all the others),
  then `node scripts/dev-native.cjs build backend` and
  `HOI4_BACKEND_EXE=.local/rust-dev-backend-target/release/hoi4-backend.exe npm test`
  (output to the scratchpad, read the tail).
  Green: add `- Checked: green <local ISO time>` to the item. Red: `git revert --no-edit <hash>` (the code
  commit only; the `docs: mark` commit keeps the worker's `Attempts` and `Notes`), re-run the same checks (still
  red: stop, step 5), add the failing test name to `Notes`, and set the status by the attempt rule below
  (`dropped: tests red after 2 attempts`).
- **No new commit** (worker crashed, hit a limit, or gave up and wrote `gave up: <reason>` in `Notes`): if the
  tree has changes besides the backlog, stash them without the backlog, so the item's `doing`, `Attempts` and
  `Notes` stay in place:
  `git stash push -u -m "improve-loop <id> attempt <n>" -- . ":(exclude)docs/improvement-backlog.md"`
  (never discard). Name the stash in `Notes`; set the status by the attempt rule (`dropped: no result after 2 attempts`).
- **`needs-user: …`** or **`dropped: …`** written by the worker: accept it; the tree must be clean (stash it as above if not).
- Anything else (commits not matching the item, a different branch checked out): switch back to the run
  branch, stash leftovers as above, `dropped: worker left an inconsistent state`.

**Attempt rule:** `Attempts` is the number of worker sessions started for the item (step 3 adds 1 before each).
After a failed attempt: `Attempts` < 2 means `Status: todo`; otherwise `Status: dropped: <reason>`.
Commit backlog changes: `docs: update the improvement backlog`.

## 5. Next or stop
- After every 3 finished items (done, needs-user or dropped), re-rank the remaining `todo` items with what
  changed; add or drop items if the evidence says so.
- Stop when: 15 items are `done`, or 8 hours passed since the latest `Started:`/`Resumed:`, or no `todo` item is left, or a regression
  remains after the revert in step 4 (stop right after the revert commit; never reset).
- Otherwise go to step 3.

## 6. Finish
1. Run the full checks: `node scripts/test-native.cjs` (all crates), `node scripts/dev-native.cjs build backend`,
   `HOI4_BACKEND_EXE=.local/rust-dev-backend-target/release/hoi4-backend.exe npm test`; output to the scratchpad.
   Never run `scripts/build-backend.ps1` (it installs into `src/hoi4companion/_native/`).
2. Fill `## Run report` in the backlog, short:
   improvements by area (functional, technical, UX — UX commits are marked `[ux-review]`), significant skills
   and tools used, harness changes, tests PASS / FAIL / NOT VERIFIED (live game behavior is always
   NOT VERIFIED unless checked in game), commits (`git log --oneline <start>..HEAD`), `needs-user` questions,
   stashes left, next priority.
3. Add 3-5 lines on the run to `docs/AGENT_HANDOFF.md` (keep it under 100 lines).
4. Commit `docs: report the improvement run`. Post the same report in chat.

## Never
- Push, merge, rebase or delete branches; `deliver-build`, `scripts/avvia-tutto.ps1` or any app start/stop.
- Write user data under `.local/` (build caches there are fine) or anything in `src/hoi4companion/_native/`.
- Interact with HOI4: no input, injection, memory writes or game/mod file changes.
- Run two worker sessions at once, or implement an item in this session.
- Discard changes: stash or revert instead.
- Keep temporary files outside the session scratchpad.
