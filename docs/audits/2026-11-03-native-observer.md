# Read-only Rust observer integration — 3 November 2026

The measured migration targets are sample revalidation (live state and native
GUI geometry) and selected-byte change probes. Python still decodes game
layouts, qualifies process/country identity, evaluates guides, and coordinates
the app. Rust/Jomini, CWTools and the Rust/Direct2D renderer are unchanged.

## Implementation and evidence boundaries

- `native/observer` is a dependency-free DLL with ABI v1. It borrows the
  caller's already qualified process handle; process access uses only
  `ReadProcessMemory`. It does not open processes, follow game offsets, write
  memory, invoke game functions, or cache observed states/coordinates.
- All ranges and byte selections are validated before reading. Partial or
  inaccessible reads fail. Full-byte witnesses remain full-byte witnesses;
  the existing GUI visibility-bit comparison remains masked to bit 3.
- `NativeSample` and `GuiSample` use the same native verifier. GUI diagnostics
  expose verification engine/duration. The native HUD's revision, visibility,
  country, ownership, geometry and 750 ms freshness gates remain unchanged.
- Change probes retain the two-pass semantics and compare selected bytes only,
  not gaps. An immutable packet is prepared after each qualified sample,
  replaced after a full refresh, and discarded on close/error/session change.
- A source checkout without a compiled observer keeps the original reader.
  A present DLL with invalid ABI or a failed read never triggers a silent
  fallback. Windows distribution builds require the DLL and self-test it.
- The local DLL is built and installed under `src/hoi4companion/_native`.
  Setuptools and Windows packaging include the new component. This task did
  not publish or rebuild a complete Windows release archive.

## Measurements

Reproduce on Windows with `PYTHONPATH=src` and
`python scripts/benchmark-observer.py --output results.json`.
The paired comparison uses the same 1,024 owned-process buffers, 512 visibility
witnesses, warm-up, 11 alternating batches and 30 iterations per batch. Sample
measurements include Python packing and the FFI call. Initial acquisition still
uses the existing reader. Watch packet construction occurs once per qualified
sample, not once per unchanged probe.

| Measured work | Python median | Rust median | Reduction |
|---|---:|---:|---:|
| Revalidation | 1.973 ms | 1.084 ms | 45.0% |
| Two-pass change probe | 0.238 ms | 0.033 ms | 86.0% |
| Acquisition plus revalidation | 4.439 ms | 3.548 ms | 20.1% |

These are controlled memory/CPU measurements, **not game FPS or visible HUD
latency**. An earlier paired run measured 46.0%, 85.6%, and 19.4% respectively.
The gains substantially exceed that run-to-run variation. Preparing a watch
packet on every probe produced only about 1% improvement and was discarded;
the final integration prepares it only when replacing qualified evidence.

## Verification

- Full Python suite: **1,130 passed**; no tests skipped.
- Full JavaScript suite: **420 passed**; no tests skipped.
- Rust observer: **5 passed**; `cargo clippy --all-targets -- -D warnings` passed.
- New regressions cover exact/masked bytes, full-byte precedence, unselected
  gaps, malformed/bounded packets, unreadable memory, absent library, wrong ABI,
  generated Python/native comparisons, session identity, plan replacement,
  errors and closing/reopening. A spawned test process was read through a
  query/VM_READ-only handle; its own code changed its own test buffer.
- Source deployment self-check executed the observer DLL, Jomini, CWTools,
  FFmpeg and the existing runtime checks. The observer self-check correctly
  records `game_live_verified: false`.
- Imported Hungary guide SHA-256 remains
  `287d7d5f3188ae96dd0f70ff63eb5067994b64ff7cc7c12d964e98b6c1726f8e`.
- `git diff --check` passed. Existing Pillow fixture deprecation warnings remain.

Local evidence is in `.local/rust-live-{baseline,final-benchmark,deployment-check}.json`,
`.local/rust-live-{python,js,cargo,clippy}.log`, and the focused failing-test logs.
The final installed observer DLL SHA-256 is
`98b053cc2939eaf64a16856754ecbe74e08e5539237017d9cfa1208786a06bbe`.

## Counter and remaining verification

**2 measured hot paths migrated; 0 failing tests; 1 integration check pending:**
actual game/HUD observation and end-to-end visual latency. HOI4 was closed and
was not launched. Game files, game memory, guide definitions and campaign
progress were not modified. No autonomous strategy or new guide rules were added.
