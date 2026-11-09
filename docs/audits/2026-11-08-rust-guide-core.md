# Rust guide core and compact walkthrough — 8 November 2026

This is the first backend migration increment, not removal of the Python server.
Phase 1 (baseline) and phase 2 (guide decisions) are implemented and tested.
Phase 3 (shared presentation and compact UI) is implemented with automated/API
verification; visual browser/game verification remains outstanding. Phases 4–6
remain: import/catalog/tool ownership, state/API/persistence ownership, and the
final Rust Windows distribution. See [migration boundaries](../rust-backend-migration.md).

## Changes

- Added `native/guide-core`, a Rust library with typed conditions/actions, compiled
  dependency graphs and bounded versioned C ABI. It owns the existing guide
  decisions: completion/start, conditions/phases, priorities, readiness/resources,
  parallel slots, optional alternatives, progressive research choices and
  production revision admission. Definitions remain separate from observations.
- Public `guide_runtime.evaluate` now calls the native library. The original
  Python evaluator is an opt-in comparison oracle, without automatic fallback.
  Qualified-state normalization and domain projections still use Python adapters.
- Browser pages and the HUD feed consume coherent state/progress/evaluation
  packets from `/api/guide/projection`, with guide/server/run/revision guards.
  An explicitly withheld run (`null`, e.g. campaign unverified) is valid and never
  admits the saved checks. Writes invalidate old evaluations before displaying
  a new run. No click, panel closure or resource decrease proves execution.
- Walkthrough presentation separates current instructions, active work, waits
  and an expandable complete plan. Source ordering, original instructions,
  conditions, confirmations, media and reference links are retained. Unchanged
  cards remain attached; collapsed plans defer card rendering. Operation anchors
  open the full plan.
- Fixed repeated informal compilation: `load_guide` returns independent copies,
  so an object-identity cache never hit. Compilation now keys by source content
  and catalog identity, preserving authored resource order. Source edits and
  catalog changes invalidate it.
- An observed intermittent Windows HTTP reset occurred when rejecting foreign
  origins before consuming their small POST body. Rejection now drains only a
  bounded declared body under a timeout; unknown/large/transfer-encoded uploads
  are not drained. The origin guard and refusal remain unchanged.
- Updated source/portable packaging inputs and startup/self-check for the new
  DLL. This is not a newly published release or a completed Rust-only package.

## Executed verification

- **1,177 Python tests passed**, with `HOI4_GUIDE_CORE_VERIFY=1` to compare Rust
  and Python outcomes throughout the suite. This includes 65 shared runtime
  fixtures, campaign/progress, resources, upgrades, production and API tests.
- **451 JavaScript tests passed**, including late guide responses, new-run
  invalidation, unverified campaign packets, server-owned decisions, retained
  card identity, deferred rendering, links and existing tree behavior.
- **8 Rust tests passed**. Locked offline release build, formatting check and
  Clippy with warnings denied passed.
- **61 archive guides** re-imported in preview without installation:
  **47 ready, 14 DLC-unavailable, 0 requiring revision**. Five synthetic states
  per guide gave **305 identical native/reference evaluations**. This is not
  a live campaign test or proof every source instruction is observable.
- **50 consecutive real local HTTP requests** with foreign origins received
  403 in the dedicated regression test, without a connection-reset error.
- Restarted only Companion and checked **14 HTTP responses**, including the
  active Hungary guide, its coherent preview packet, walkthrough, technology and
  importer pages, and changed JavaScript/CSS. Persisted progress remained
  available. A SQLite backup exists in `.local/rust-core-progress-before.sqlite3`.
- Hungary guide SHA-256 remained
  `287d7d5f3188ae96dd0f70ff63eb5067994b64ff7cc7c12d964e98b6c1726f8e`.
- Installed DLL matches the release build, SHA-256
  `fb68bba640602c5bd414039bbcbaa49431c8996166d8e6208e9b38b7c5543129`.

The matched benchmark uses the unchanged 126-step Hungary definition in offline
preview. Eleven alternating paired batches of ten evaluations, excluding warm-up,
gave medians **4.653 ms Python / 3.535 ms native including its adapter and FFI**.
First compilation was 4.681 ms in this benchmark. This is not HUD latency or
overall application resource usage. Actual HTTP definition enrichment is a
different workload: the initial full enrichment took about eight seconds;
subsequent coherent preview packets measured **49–77 ms** in the final probe.
No first-load or end-to-end gameplay latency guarantee is made.

Evidence: `.local/rust-core-final-{python,js}.log`,
`.local/rust-core-archive.{json,log}`, `.local/rust-core-benchmark.json`,
`.local/rust-core-http-verification.json`, focused RED/green logs, and native
Cargo command results. No game files or memory were modified and no imported
guide definition was rewritten.

## Remaining limits

The browser tool returned `Local browser use is disabled in Work settings`.
Consequently no visual browser result is claimed. HOI4 was not opened or used
for this increment, and native HUD alignment/live responsiveness was not
revalidated. The HUD remains Rust with its existing Python compatibility worker.

Python still owns the application server, normalization/domain evidence adapters,
catalog construction, import orchestration, layout decoding and persisted run
coordination. These must migrate with their existing feature/parity tests before
the resident Python runtime can be removed. The existing source checkout and
portable build definitions now require the native guide library; no updated
GitHub release or Rust-only executable was built/published in this increment.
