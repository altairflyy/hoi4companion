# CLAUDE.md

This file provides permanent project instructions for coding agents working in this repository.

## Shared handoff
- At the start or resumption of development, read `docs/AGENT_HANDOFF.md` and inspect `git status` and relevant diffs before editing; never overwrite, discard, commit or push unrelated work.
- Update that handoff incrementally after each meaningful milestone, before switching tasks and at session end; do not wait until a long task finishes. Replace outdated state, keep it under 100 lines, and link to evidence rather than copying it.
- Separate verified results, hypotheses and pending work; record actual test outcomes and exact next actions. This repository file is shared across agents and requires no memory plugin or conversation history.

## Invariants
- **Read-only toward the game:** never click, inject code, write process memory, or modify game or mod files.
- **Unknown stays unknown:** missing or stale observations are `unknown`/`unverifiable` and never imply completion. The HUD withholds highlights it cannot verify.
- **Rust is the only guide evaluator:** `native/guide-core`, via the backend, decides step status, progress and recommendations for every guide; the backend stamps each one `runtime_engine: 'rust-guide-core'`. The web JS only presents the evaluation from `/api/guide/projection` and rejects one whose guide or run doesn't match. Without a valid evaluation, steps stay unverifiable; never recompute them in JS. Put evaluation changes in Rust.
- **No Python:** none in source, build, CI or the package (enforced by packaging and `tests/test_native_tooling.cjs`). Keep the code that reads data written by older Python versions, such as the legacy text, ID and equality rules noted in Rust comments. Shared behavior contracts live in `tests/fixtures/*.json`.
- **Live memory reading** is limited to the builds qualified in `src/hoi4companion/live-profiles-v1.json` and `live-abi-v1.json`. A similar version string does not make a build valid.

## Build and run (Windows, Node 24+, Rust/MSVC; .NET only for CWTools)
```powershell
powershell -NoProfile -File scripts/build-backend.ps1   # copies dll + exes into src/hoi4companion/_native/
powershell -NoProfile -File scripts/build-overlay.ps1   # also: build-parser.ps1, build-cwtools.ps1
powershell -NoProfile -File scripts/avvia-tutto.ps1     # start; add -Stop to stop. http://127.0.0.1:8766/suite.html
```

## Tests
```bash
npm test                                  # all tests/test_*.cjs
node --test tests/test_guide_model.cjs    # one JS file
node scripts/test-native.cjs [crate] [--dev|--lto]   # cargo test --release --locked, fast non-LTO profile; --lto = production profile; crates: parser observer guide-core catalog-core tools storage backend overlay desktop
node scripts/dev-native.cjs test backend --test integration guide_evaluation::   # one backend integration module
```
- **Fast local loop:** local `test-native.cjs`, `dev-native.cjs` and the `-Dev` build switch (`build-backend.ps1 -Dev`, `build-overlay.ps1 -Dev`) share `.local/rust-dev-<backend|core|desktop>-target`: optimized, incremental, no LTO. The deliver-build skill uses `-Dev` by default (`--release` for LTO). Packaging always builds with the production LTO profile; never package `-Dev` output. Put the crate before flags: `test-native.cjs backend --lto`.
- **Rebuild the backend after Rust changes:** `tests/test_native_service.cjs` runs `src/hoi4companion/_native/hoi4-backend.exe` (or `HOI4_BACKEND_EXE`). Without a rebuild it tests a stale binary. It is skipped when not on win32.
- **Filtered Rust tests:** prefer `node scripts/dev-native.cjs test <crate> [cargo args]`, which uses the same cache as local `test-native.cjs`. Raw `cargo test` needs the same `CARGO_TARGET_DIR` and `CARGO_PROFILE_RELEASE_*` variables, or everything rebuilds from scratch.
- **Backend integration tests are one executable:** `native/backend/tests/integration/main.rs` declares each former `tests/<name>.rs` as a module, so a change links one test binary, not thirteen. Add new integration tests as modules there.
- **Browser tests never evaluate guides.** They read Rust evaluations from `tests/fixtures/presentation-evaluations.json`, generated from `presentation-cases.json` by `native/backend/tests/integration/presentation_fixtures.rs`. Evaluation rules are asserted in Rust (`native/backend/tests/integration/guide_evaluation.rs`, `contracts.rs`). After a change to evaluation or to a case, run `HOI4_UPDATE_PRESENTATION_FIXTURES=1 node scripts/test-native.cjs backend`, then review the fixture's git diff before keeping it.
- CI runs `test-native.cjs --ci` (LTO off, so never package from that output). A release tag must equal `v` + the `package.json` version.

## Architecture
- `native/` holds independent crates, each with its own `Cargo.lock` and no shared workspace. `backend` depends on all of them except `overlay`.
- `backend` has two binaries: `hoi4-backend` (HTTP/SSE server, routes in `http.rs`) and `hoi4-desktop` (the GUI launcher behind `HOI4 Companion.exe`).
- `overlay` (`hoi4-overlay`) is a separate Direct2D HUD process. It has no guide logic of its own and uses the backend's active guide through `/api/overlay`.
- `tools` runs external tools (FFmpeg, yt-dlp, Node, CWTools) inside Job Objects. `cwtools` is an F#/.NET host with pinned vendored sources.
- `src/hoi4companion/web/` is plain JS with no bundler. Modules are UMD IIFEs (`module.exports` in Node, `window.Hoi*` in the browser), so tests `require()` the page files directly.
- The `src/hoi4companion/*.json` files are build inputs that CI cache keys hash. UI strings are in `translations.json` (English default, plus Italian).
- User data lives in `.local/` in a source checkout, or in `HOI4COMPANION_DATA_DIR` if set. The prepared-guide cache never stores live observations or campaign progress.
- Before changing a subsystem, read its document in `docs/` (indexed in `docs/README.md`).
