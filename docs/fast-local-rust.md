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
