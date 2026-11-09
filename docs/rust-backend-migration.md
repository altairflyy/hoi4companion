# Rust backend migration

The target architecture has one Rust backend for state, rules, progression and
HTTP/SSE; a separate Rust/Direct2D HUD; a JavaScript presentation layer; and
external tools supervised only while their jobs need them. The default source
launcher and the native Windows ZIP use the standalone Rust backend and direct
Rust HUD transport. Neither starts the Python server or HUD worker. Python
sources remain as development tools and independent compatibility test oracles.

The 2026-11-09 installation is operational for static guides/importing. The HOI4
executable installed that morning has an unqualified SHA-256: live acquisition
is explicitly unavailable until its profile is verified on a real campaign.
No current game/HUD visual validation is claimed.

Executed checks are recorded in the [guide decision report](audits/2026-11-08-rust-guide-core.md)
and the [catalog, tools and storage report](audits/2026-11-08-native-services.md).
Current ongoing work is recorded in the [native state ledger](audits/2026-11-08-native-state.md).
The current cutover and remaining verification gates are recorded in the
[2026-11-09 native cutover report](audits/2026-11-09-native-cutover.md).

## Migration boundaries

| Phase | Implemented boundary | Remaining gate |
| --- | --- | --- |
| 1. Baseline | Python/JavaScript suites, shared runtime fixtures, archive importer audit and reproducible matched benchmark | Keep these checks for each subsequent cutover |
| 2. Guide decisions | Rust schema validation, prepared domain projections, compiled graph, three-valued conditions, start/completion dependencies, priorities, capacities, progressive research and choices, production revisions | Keep independent compatibility checks |
| 3. Presentation | Browser and HUD consume coherent backend decisions; compact current/active/waiting views and an expandable complete plan; unchanged cards stay attached | Real browser visual inspection and real game/HUD alignment remain separate verification |
| 4. Import/catalogs/tools | Native source intake, complete import preview and operational-language compiler; source/media jobs; catalogs, authoring/Word intake, route proofs, DLC preflight, production recipes and saved-recipe inventory | Current external YouTube acquisition and optional paid provider calls were not repeated; local FFmpeg and archive parity are tested separately |
| 5. State/API/storage | Standalone HTTP/SSE, all existing routes, source/runtime enrichment, atomic campaign persistence, native HUD projection/geometry and bounded binary-frame fallback | New game builds and supported GUI sections require their own qualified profiles and real observations |
| 6. Windows cutover | Native desktop launcher, autodiscovery, lifecycle control, Rust-only application ZIP with on-demand tools and isolated installation smoke test | Browser visual inspection and current game/HUD alignment remain unverified; the new game executable is not admitted |

No HTTP proxy that simply keeps Python resident is counted as completing phases
4–6. Python used by an on-demand external utility is a separate issue from a
resident application backend. No nation, guide, HOI4 build offset or game I/O is
embedded in the decision crate.

## Catalog, tool and persistence boundaries

`native/catalog-core` owns ID/name/alias resolution, country/scope restriction,
word candidates and review suggestions. `resolution.py` preserves its public
interface and delegates queries through the backend C ABI. Import compilers borrow
that same immutable catalog through an owned native handle, without JSON round trips
for the full catalog or a second resident catalog library. A compiled handle is
an immutable catalog generation; bounded per-context caches never contain live
facts. `with_entries` retains national names/adjectives and reference designs
when compiling a compatibility/review generation. Static geography and focus-route recovery execute in the native backend. Import
orchestration and operational-language compilation execute in Rust; Python
adapters remain for regression comparison and development scripts.

The bundled Unicode profile preserves the previous English canonicalization,
Roman levels, case folding and transliteration without a Python call for each
native query. Regenerate it with `scripts/generate-catalog-text-profile.py` and
run the Unicode/parity tests after a profile update. The legacy resolver remains
an opt-in test oracle: `HOI4_CATALOG_CORE_VERIFY=1` compares outputs and refuses a
divergence. There is no automatic fallback or strategy inference.

`native/tools` creates suspended, hidden Windows processes, assigns them to a
kill-on-close job before resuming, and inherits only the explicitly listed I/O
handles. An opaque owned handle is required to cancel a job; there is no PID
attachment or arbitrary process termination API. Deadline and output monitoring
exist only while an external tool runs. Descendants are terminated on parent
exit, cancellation, timeout or owner closure. Output growth is checked every
25 ms and at exit; it is a detected bound, not a hard disk-write quota. Video
job orchestration is native; browser JavaScript presents progress. FFmpeg,
yt-dlp, Node and CWTools run only for their associated jobs.

`native/storage` owns opening/closing SQLite connections, full-synchronous
transactions and optimistic revision checks. Existing v1 database bytes/JSON
remain readable by both implementations. Conflicting writes and unknown future
storage versions are rejected, preserving the original snapshot. The backend now
owns binding, observations, rewind transitions and durable commits as one service.
`RunStore` is a facade; explicit ID/I/O injection remains a fixture transport.

The Rust host links the native services; its HUD loads the adjacent native image
library directly. Development facades can load the same DLL without an extra
worker. Missing or incompatible native libraries produce explicit startup errors.
The Windows self-check executes catalog lookup, SQLite write/reload and an actual
FFmpeg encode through the native supervisor in an isolated profile.

## Decision contract

The public Guide Schema v1 and existing API responses are preserved. The Rust
crate accepts typed compiled definitions and a bounded, versioned `prepared-v1`
input containing qualified facts and domain projections. It owns status,
dependency admission, readiness, priorities, choices and proposed steps. It does
not own acquisition, timestamps, disk access, click execution or persistence.

`guide_core.py` delegates schema validation, state preparation, domain projections
and decisions to Rust. The backend keeps 16 immutable compiled definitions
separately from observations. No live facts or previous completions survive in
a compiled handle.
The default server owns these compiled handles directly; it does not run a
resident interpreter or proxy requests to a compatibility server.

`guide_runtime.evaluate` uses Rust. The original `evaluate_python` is retained as
a development oracle, not a fallback. `HOI4_GUIDE_CORE_VERIFY=1` compares both
implementations and rejects a divergent result. Normal execution does not run
the oracle. A missing/incompatible native library reports an explicit deployment
error rather than silently using another engine.

## Browser and HUD

`GET /api/guide/projection?id=...&live=0|1` returns one locked snapshot containing
the definition revision, server identity, guide, current state, current run and
Rust evaluation. `live=0` never polls the native reader. Existing guide/runtime
and checklist endpoints remain available.

Definitions mark `runtime_engine: rust-guide-core`. Such pages use the supplied
evaluation and suspend recommendations while it is absent or invalid. They do
not fall back to the JavaScript interpreter. The JavaScript interpreter remains
for compatibility and shared regression fixtures. Clients reject late packets
from a different guide/server/run and invalidate old evaluations on a run write.
The HUD compatibility feed uses the same packet rather than combining a streamed
state with an independently fetched evaluation.

The walkthrough shows the current operational instruction, active work and a
short waiting list. The complete plan remains available with its original
source order, filters, confirmations, evidence and references. A collapsed plan
defers card rendering. Cards whose relevant data is unchanged retain their DOM
nodes and expanded details; links to a specific operation open the full plan.
National focuses and research remain in their dedicated trees.

## Reproducible checks

Build the library with `scripts/build-guide-core.ps1`, or with the corresponding
locked Cargo build and copy it into `src/hoi4companion/_native`. A locally
installed Rust toolchain under `.local/toolchains` is supported. No system
PowerShell execution policy needs changing.

```powershell
$env:PYTHONPATH='src;tests'
$env:HOI4_GUIDE_CORE_VERIFY='1'
py -3 -m unittest discover -s tests -p 'test_*.py'
Remove-Item Env:HOI4_GUIDE_CORE_VERIFY
$files=@(Get-ChildItem tests -Filter '*.cjs' | ForEach-Object {$_.FullName})
node --test @files
cargo test --locked --manifest-path native/guide-core/Cargo.toml
cargo clippy --locked --manifest-path native/guide-core/Cargo.toml --all-targets -- -D warnings
py -3 scripts/audit-guide-archive.py 'collection.zip' --output '.local/archive.json' --runtime-parity
py -3 scripts/benchmark-guide-core.py '.local/guides/example.json' --output '.local/guide-core-benchmark.json'
```

The benchmark compares identical outputs and excludes warm-up. Its native time
includes the Python adapter, serialization and FFI. It measures offline preview,
not browser paint, HUD response, live acquisition or overall memory consumption.
Archive parity adds synthetic preview/unknown/initial/completed/wrong-country
states. These are not observations from real campaigns. Missing DLC content is
reported separately from guide errors.

The compatibility gates preserve every existing API, guide/checklist version,
campaign rewind guard, partial/unknown evidence state, importer recovery,
template comparison, geography, source reference, media job lifecycle and
supported native GUI control. A smaller Rust subset is not feature parity.

`scripts/build-native-windows.py` creates the native package from an explicit
allowlist, without personal guides/settings/campaigns or build caches.
`scripts/smoke-native-windows.py` verifies manifest hashes, a clean installation,
guide import/selection, restart persistence, native HUD binary-frame transport
and owned shutdown with the package's actual executables. It does not substitute
for a real browser/game alignment test. Build snapshots are disabled in
`.cargo/config.toml`; retain one current optimized build rather than per-edit
incremental or duplicate target directories.
