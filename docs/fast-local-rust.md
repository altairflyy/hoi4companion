# Fast local Rust development

Web UI changes are served directly from `src/hoi4companion/web`: reload the
desktop view to pick up JavaScript/CSS/HTML edits, without rebuilding Rust.
Desktop window/lifecycle changes live in the small `native/desktop` crate and
do not link the backend evaluator or catalog crates.

```powershell
node scripts/dev-native.cjs check backend --bin hoi4-backend
node scripts/dev-native.cjs build backend --bin hoi4-backend
node scripts/dev-native.cjs test backend --lib test_name
node scripts/dev-native.cjs test backend --test integration guide_evaluation::test_name
node scripts/dev-native.cjs build overlay
node scripts/dev-native.cjs build desktop
```
The first build fills a separate `.local/rust-dev-*-target` cache; later builds are incremental and optimized without whole-program LTO. Cargo arguments after the crate select targets or test filters. Outputs are never installed by this command; for integration tests, explicitly set `HOI4_BACKEND_EXE` to the dev executable. This loop does not replace release validation or packaging: use the existing release scripts for those.

The same cache and profile serve:
- `node scripts/test-native.cjs [crate]` (local default; `--lto` selects the production profile and its cache, `--ci` the CI caches);
- `build-backend.ps1 -Dev` and `build-overlay.ps1 -Dev`, which install development binaries into `_native` (the deliver-build skill does this by default; `--release` for LTO).

Backend integration tests are one executable, `native/backend/tests/integration/main.rs`, with each test file as a module: a change links one test binary instead of thirteen.

Cache directories are `.local/rust-dev-<backend|core|desktop>-target`. Packaging always builds with the production LTO profile; never package `-Dev` output. Put the crate before flags (`test-native.cjs backend --lto`). Prefer `dev-native.cjs test <crate> [cargo args]` over raw `cargo test`, which needs the same `CARGO_TARGET_DIR` and `CARGO_PROFILE_RELEASE_*` variables or rebuilds everything from scratch.

Backend tests (`test-native.cjs backend` and `dev-native.cjs test backend`, for every crate in `BACKEND_CRATES` of `scripts/native-tooling.cjs`; their builds use the backend cache family, never the core one) also compile the backend crate itself at `opt-level=0` (dependencies stay optimized) in `.local/rust-dev-backend-tests-target`. Measured after a one-line edit: `test-native.cjs backend` 17 s instead of 59 s (282 tests); test execution itself 2.2 s instead of 1.4 s. The release profile still applies, so debug assertions and overflow checks stay off as in the shipped build. CI and `--lto` test the optimized backend. Measured but not adopted: `opt-level=1` (54 s), 256 codegen units (65 s), `rust-lld` (no gain).
