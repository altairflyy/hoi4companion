# Improve loop — worker session

You implement exactly one backlog item (given in your brief) during an unattended run.
The user is away: never ask questions. A coordinator session checks your result after you end.
Invariants, build and test commands are in `CLAUDE.md`; they apply to every step here.

## Before
1. Read `CLAUDE.md`, `docs/README.md` and the `docs/` file for the subsystem you touch, and your item in
   `docs/improvement-backlog.md`.
2. The current branch must be the backlog's `Branch:`, and `git status` must show only the backlog file
   (your item is marked `doing`, uncommitted). Otherwise end with a one-line report and change nothing.
3. If `Acceptance` is missing or not objective, write an objective one in the item first.

## Do
- The simplest change that meets `Acceptance`. No unrelated refactoring, no new UI options or text the item
  does not need.
- Guide evaluation logic only in Rust (`native/guide-core` or the backend crates), never in web JS.
- Behavior changes test-first: write the failing test, watch it fail, then implement.
- Fixing a bug: write in `Notes` why the existing tests missed it, and add the test that would have caught it.
- New user-visible strings go through the `add-translation` skill (English and Italian).
- Precision over coverage: never bind a guide step to game state you are not sure of; unknown stays unknown.

## Verify
1. Targeted tests while working (`node --test tests/<file>.cjs`, `node scripts/dev-native.cjs test <crate> …`).
2. Then `node scripts/test-native.cjs <crate>` for every crate you touched, and the JS suite against a
   freshly built, not installed, backend:
   `node scripts/dev-native.cjs build backend` then
   `HOI4_BACKEND_EXE=.local/rust-dev-backend-target/release/hoi4-backend.exe npm test`.
   Never run `scripts/build-backend.ps1`: it installs into `src/hoi4companion/_native/`.
   Send long output to the scratchpad and read the tail.
3. Regenerate presentation fixtures only as `CLAUDE.md` says, and read their git diff.
4. If the diff touches guide evaluation, live state, HUD, web pages, importer or packaging: run the
   `invariant-reviewer` agent on the working tree and fix what it reports.
5. Measure `Acceptance` and write before -> after in `Notes`. Live game behavior is always
   "NOT VERIFIED in game".

## UX items
- Before/after screenshots with the built-in browser only if `http://127.0.0.1:8766/suite.html` already
  answers (web files are served live; reload the page to see a change). Never start or stop the app.
  Save them in the scratchpad and put their full paths in `Notes`.
- If nothing answers: write "visual check NOT VERIFIED" in `Notes`.
- The commit subject contains `[ux-review]`: the user approves UX changes, not you.

## Finish
- **Done:** commit the code change alone (Conventional Commits, one commit, attribution line from your system
  reminder). Then set `Status: done <short-hash of that commit>`, fill `Notes`, and commit the backlog alone
  as `docs: mark <id> done`. Never amend.
- **Needs a user decision** (product or architecture choice, ambiguous intent): undo only your own changes
  (`git restore` / `git clean` on the files you changed or created), set `Status: needs-user: <one clear
  question with the options>`, commit the backlog as `docs: <id> needs a user decision`.
- **Cannot make it work:** write `gave up: <reason>` in your item's `Notes`, leave the tree as it is (commit
  nothing) and end; the coordinator stashes your changes and counts the attempt.
- Update `docs/AGENT_HANDOFF.md` only for a significant milestone, in the same docs commit.
- End with a 3-line report: what changed, tests run and their result, final status.

## Never
- Push, merge, rebase, switch or delete branches; `deliver-build`, `scripts/avvia-tutto.ps1` or any app start/stop.
- Write user data under `.local/` (build caches there are fine) or anything in `src/hoi4companion/_native/`.
- Interact with HOI4: no input, injection, memory writes or game/mod file changes.
- Start other sessions, or change other backlog items.
- Keep temporary files outside the session scratchpad.
