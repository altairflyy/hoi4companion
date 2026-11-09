# Native catalog, tool supervision and persistence — 8 November 2026

This increment moves three service boundaries into Rust. It does **not** complete
the Rust-only application migration. The remaining boundaries are explicit in
[the migration plan](../rust-backend-migration.md).

## Implemented

- `native/catalog-core`: immutable catalog generations and bounded context
  caches; ID, English name, alias, country/scope and event-parent resolution;
  conservative spelling/word candidates and review suggestions. Existing
  importer, DLC compatibility and reference-review APIs use the native index.
  New generations retain national naming metadata and reference designs.
- A generated Unicode compatibility profile preserves the old normalization.
  Testing all changed Unicode characters caught 16 alphanumeric presentation
  characters whose normalization becomes empty; the generator now preserves
  those deletions. The profile is compiled into the library.
- `native/tools`: owned hidden process jobs, argument-vector quoting, explicitly
  inherited I/O, deadlines, output monitoring and cancellation of descendants.
  Processes are assigned to a Windows job while suspended, before execution.
  YouTube metadata/downloads and FFmpeg encode/probe commands use this service.
  A quick-exiting writer is also checked for excess output. Monitoring is a
  detected bound, not an exact disk-write ceiling.
- `native/storage`: full-synchronous SQLite transactions and optimistic revision
  checks with the existing version-1 database/JSON format. Conflicting writers,
  corrupt data and future versions are rejected without replacing the snapshot.
  Tests explicitly cover direct saves against a future database version.
- Source and portable build definitions include the libraries and dependency
  notices. Startup rejects missing/incompatible native components. Self-checks
  execute a catalog lookup, SQLite write/reload and an FFmpeg encode through the
  native tool supervisor. No new permanently running worker is introduced.

Definitions, live observations and per-campaign progression remain separate.
There are no new nation-specific strategies, automatic game clicks, game-file
writes or process-memory writes. Existing guide documents have not been edited.

## Verification and performance

- **1,193 Python tests passed**, with both catalog and guide-core shadow
  comparison enabled, preserving the existing suite and shared fixtures.
- **451 JavaScript tests passed**. Presentation code was not changed in this
  increment.
- **8 Rust tests passed** in the three new crates (5 catalog, 2 tool, 1 storage).
  Locked offline release builds, formatting and Clippy with warnings denied
  passed. The prior guide-core increment additionally passed its 8 Rust tests.
- The Windows ZIP built and passed extraction/relocation under a path containing
  spaces, manifest hashes, isolated empty profile, 8 web resources, native
  catalog lookup, SQLite write/reload, real FFmpeg HEVC encoding through the
  supervisor, CWTools/Jomini, yt-dlp worker invocation, desktop launcher reuse
  and the overlay bridge protocol. External Python/.NET/Node/FFmpeg were absent
  from the test PATH. These are deployment checks, not a visual HUD proof.
- Restarted only the verified Companion server after a consistent SQLite backup;
  **14 actual local HTTP responses** passed, including the active Hungary guide,
  coherent preview, persistence, installed catalog and web resources. Stored
  progress and its revision remained identical to the backup. The current HTTP
  catalog contains 43,120 entities, distinct from the cached benchmark corpus.
- The active Hungary guide SHA-256 remains
  `287d7d5f3188ae96dd0f70ff63eb5067994b64ff7cc7c12d964e98b6c1726f8e`.

Evidence: `.local/migration-final-python.log`, `.local/migration-final-js.log`,
`.local/*-final-rust.log`, `.local/native-boundary-{red,green}.log`,
`.local/native-migration-windows-{build,smoke}.log`,
`dist/windows-smoke-results.json`, `.local/native-migration-http.json`,
`.local/catalog-core-benchmark.json` and archive reports.

The cached installed catalog has **42,829 entries**. A matched benchmark checked
**1,098 query outputs** and **80,793 ID/name/alias normalizations** against the
legacy implementation; all outputs agreed. Alternating paired batches after
warm-up measured median query time **0.245 ms Python / 0.122 ms Rust including
serialization and FFI**. Initial index construction measured 2,691 / 2,083 ms.
These are static catalog measurements, not game/HUD latency, browser paint or
whole-application CPU/RAM consumption.

The final archive comparison with the installed release DLL used all **61
country guides**, with five synthetic runtime states per guide: **47 ready,
14 DLC-unavailable, 0 requiring revision and 305 identical runtime results**.
DLC-unavailable guides remain a separate category, not import errors. Preview
tests never install or rewrite those guides and do not prove that every
instruction has observable live evidence. The final report is
`.local/catalog-final-archive.json`.

Process tests create only isolated Python test children: exact Unicode/quoted
arguments, nonzero exit, timeout, parent-exit cleanup, cancellation, unrelated
process survival and output limits. The child-PID handshake uses atomic file
publication and an explicit parent-exit signal, avoiding a timing-dependent
test that could read a partially written PID.

SQLite compatibility tests use temporary databases with native and legacy
writers. Campaign/progress regression tests retain the original rewind,
research-binding, baseline and identity behavior.

## Remaining boundaries

Catalog extraction/construction, Word/ZIP authoring/import orchestration,
geography and route reconstruction still use their existing Python adapters.
Run/campaign binding and observation, native layout decoding, freshness/session
guards, HTTP/SSE and the HUD compatibility worker are also still Python. Their
existing tests passing does not mean these components have migrated.

The current portable package is therefore a mixed-runtime build. It does not
remove Python or complete phases 4–6. The Rust libraries are active services,
not an HTTP proxy disguising a resident Python backend. No real-game live read
or visual HUD alignment is claimed for this increment; those are separate gates.
