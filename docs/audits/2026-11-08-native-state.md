# Native state migration in progress — 8 November 2026

This work continues the full migration. It is not a completion report. The user
explicitly requires the entire plan completed before shutdown; the previous
shutdown was premature. No shutdown should be scheduled at an intermediate gate.

Implemented since the native-services report:

- Rust-owned run stores in `native/storage/src/progress.rs`: load validation,
  run creation/selection, checks/started state, session rotation, qualified
  campaign binding, country separation, rewind branches and research history.
  The ABI holds a pending mutation until SQLite persistence succeeds; failures
  roll back the state. Native handles own the authoritative state. The Python
  observation adapters have only a temporary mutation buffer.
- `native/backend` now owns semantic state signatures, revisions, subscriber
  limits, slow-client coalescing, stale expiry, wakeups and identical-state
  recovery. A transitional Python worker still invokes the existing acquisition
  callback. Clock injection replaced tests' mutation of private timestamps.
- Focus/research decoding moved to `native/backend/src/live.rs`, including the
  primary country selector, map, RTTI, owner, membership, duplicate, level and
  completion guards. Both the output and exact watch ranges match the preserved
  independent decoder. Unselected negative country-map entries remain allowed.
- Actual Windows core reads use `live_process.rs`: one borrowed read-only handle,
  immutable-PE RTTI with a bounded per-read cache, native read budget and byte
  witnesses. Fixture callbacks remain only for tests/custom memory transports.
  The existing final double-check and process-session check remain required.

Executed checks: 1,200 Python tests passed after this integration, with guide and
catalog shadow comparisons enabled. The focused reader suite passed 74 tests.
It includes genuine Windows ReadProcessMemory over an owned 64 MiB test arena,
with synthetic PE/RTTI and budget rejection. This is **not game-live validation**.
The native events/campaign compatibility fixtures and Rust tests also pass.
Subsequent work must retain these checks; new changes may invalidate this count.

Relevant evidence: `.local/progress-core-{green,integration,full-python}.log`,
`.local/backend-events-{green,integration}.log`,
`.local/backend-state-full-python.log`, `.local/backend-live-core-green.log`,
`.local/backend-live-process-integration.log`, `.local/backend-live-full-python.log`.

Build/package definitions include the new backend DLL. The previous Windows ZIP
and smoke result predate this state increment and must be rebuilt/retested at
the final cutover. Updated native dependency notices also need regenerating.

Next boundaries: native executable/process qualification and provider cache,
remaining country/UI decoders and normalization/observation adapters, catalog
and import orchestration, HTTP/SSE hosting, removal of the HUD Python worker,
and the final Rust backend Windows distribution. The existing Python server
and HUD worker have not yet been removed. No completion or live/HUD proof is
claimed while those gates remain.

## Country, acquisition and normalization increment

All 17 existing country-section decoders now run in Rust, including production
and complete equipment recipes, projects/breakthrough points, independent laws
and active appointments, decisions/event windows, foreign relations and operative
missions, doctrines, effective war support and provincial fortifications. Existing
per-build capability gates remain intact: moving a decoder does not qualify an
unsupported field for a new executable. The callbacks are explicit fixture/custom
transports; real Windows acquisition uses retained native profile/game objects.
GUI discovery uses a separate bounded provisional read budget and selected event
bindings are always revalidated before publishing.

Native executable SHA/PE/RTTI admission, read-only process discovery/path/module/
creation checks, full acquisition, page-local masked watch compilation, process
cache invalidation/audits, campaign UUID+seed and last-good/stale monitor state
now belong to the backend. Python's ordinary `NativeProvider` only calls this
service. `live_callback_transport.py` is selected only for explicitly injected
transports, never as recovery from a failed real read.

`state.rs`, `state_legacy.rs` and `state_memberships.rs` own v1 normalization.
The previous Python normalizer is an independent test oracle. Exact float JSON
round trips are enabled in backend/guide/catalog/storage cores: the real-state
comparison caught one-ULP production-value drift at the JSON boundary. Regression
cases retain adjacent float thresholds.

Executed evidence:

- `.local/backend-provider-full-python.log`: 1,208 Python tests, before the
  normalization switch; `.local/backend-normalization-full-python.log`: 1,212
  tests after the switch, with guide/catalog verification enabled.
- `.local/backend-country-parity-all.log`: 46 focused tests, including independent
  section values and exact watch evidence. Other focused regression groups cover
  callback soft failures, agency creation, points and production GUI admission.
- Backend Rust tests: six passed; other native crate results are recorded in
  `.local/{guide-core,catalog-core,storage}-exact-rust.log`.
- `.local/native-acquisition-live-proof.json`: real Hungarian campaign loaded via
  Computer Use; all 20 top-level fields exactly match independent legacy decoding.
  The unchanged probe reuses the full sample. One measurement was 78 ms initial
  collection and 1.289 ms probe; this is one run, not an FPS or broad speed claim.
- `.local/native-state-live-proof.json`: 313 normalized facts on the same paused
  real campaign exactly match values and provenance; no differing facts remain.
  Native tick was 60919921. These proofs do not establish HUD alignment.

The actual game was opened, the existing Hungarian autosave loaded, and no game
commands, game-file edits or process-memory writes were issued. A background
switch reported user input and was not retried without a new observation; do not
reuse its old screenshot for any later UI action.

Remaining: prepared guide/domain adapters and baseline observations; guide schema
normalization/validation; catalog construction/import orchestration and media jobs;
Rust HTTP/SSE host; native HUD GUI adapters/compatibility worker removal; final
Windows package/retest. Existing Windows artifacts remain historical. No shutdown
has been scheduled and no plan completion is claimed at this boundary.

## Continuation: native application, HTTP and source catalogs

The default RunStore now uses backend-owned UUID generation, transitions,
qualified observations and durable commits. Only explicitly injected fixture
IDs/I/O use the prior transaction adapter. BackendProgressOwnerTests covers real
SQLite restart/conflict rollback; the campaign suite also forces an actual
filesystem failure and checks rollback.

`application.rs` separates definitions, live state and run progress. It owns
active selection, revision guards and coherent projections. `hoi4-backend.exe`
is a standalone loopback HTTP/SSE development host, not a Python proxy. Its
current routes cover guides, active selection, progress, projections, state,
framework state and web files. Unported routes are explicit errors; this host
has NOT replaced the normal launcher and is NOT a final distribution.

Source catalogs now use native installation/mod/DLC/localization overlays and
focus/shared/continuous branch construction. Python public model adapters remain.
The old source loops are retained only as independent parity oracles. Direct
Jomini documents avoid an intermediate JSON depth regression. Reviewed recovery
fingerprints now explicitly sort JSON object keys, retaining array order, so a
host enabling serde_json preserve_order cannot change content admission.

Executed checks:
- `.local/backend-owner-full-python.log`: 1,225 Python tests passed with guide
  and catalog reference verification, before the subsequent HTTP/source ports.
- `.local/backend-http-tests.log`: 6 HTTP/application tests passed; no Python
  service process used by the HTTP tests.
- `.local/backend-installation-adapters.log`: 40 installation/mod/DLC tests passed.
- `.local/backend-focus-adapters.log`: 93 focus/parser/tooltip/sprite tests passed.
- `.local/native-installation-proof.json`: actual installation equality for 81
  focus files, 13 technology files, 454 GUI files, 13 DLCs and 131,075 display
  labels plus 131,075 independent English labels.
- `.local/native-focus-catalog-proof.json`: all 67 trees, 12,274 focuses and 14
  continuous focuses equal to the independent previous source projection.
- `.local/backend-focus-clippy.log`: backend all-target clippy with warnings
  denied passed; `.local/backend-focus-rust.log`: 8 backend tests passed.

Remaining work still includes static eligibility/script expansion, technology
and GUI visual catalogs, operational catalogs, importer/media orchestration,
remaining API routes, native HUD GUI adapters, packaging and final real UI/HUD
checks. No guide definition/game file/memory was modified, no Git commit created,
no final completion or shutdown scheduled.

### Static eligibility and technology layout continuation

Native source services now also load scripted triggers, evaluate conservative
country/DLC selectors, and construct technology definitions and GUI geometry.
The ordinary Python model APIs delegate these bulk operations; GUI nodes and
catalog metadata remain separate from live observations.

Executed evidence:
- `.local/backend-technology-adapters.log`: 39 tests passed after source adapters.
- `.local/backend-layout-adapters.log`: 24 technology/layout/visual tests passed.
- `.local/native-technology-proof.json`: 953 scripted trigger definitions equal;
  43 visible / 24 excluded trees equal; 539 technologies in 9 folders equal.
- `.local/native-technology-layout-proof.json`: all 9 folder layouts and all
  technology data equal, including captions, rectangles, hidden branches and
  ambiguous shared descendants. Mapped/total memberships: infantry 34/41,
  support 42/42, armour 28/28, artillery 27/27, navy 38/38, naval support 39/39,
  air 41/41, electronics 27/27, industry 43/43. The 7 unmapped memberships are
  preserved existing limitations, not new mapped/verified results.
- `.local/backend-source-javascript.log`: 451 passed, zero skipped/failed.
- `.local/backend-layout-clippy.log`: backend all-target warnings-denied passed.

A release build is in progress for installing the current verified DLL and the
explicit native-host executable. The normal launcher still uses the compatibility
application until remaining API/catalog/import/media/HUD parity gates close.

### Sprite identity and raster continuation (not final cutover)

`assets.rs` owns 25,670 installed definitions, texture-root/replace_path lookup,
frame admission and opaque SHA-256 asset identities. `texture.rs` uses bounded
native image/image_dds decoding, preserving the existing DX10 BGRA-stride case.
Python SpriteLibrary delegates ordinary loading, resolution and PNG caching;
its old decoder remains only for the explicit compatibility/test entrypoint.
No source texture is written. Cache writes are atomic and refused inside game
or mod roots, including paths containing parent components.

Executed evidence:
- `.local/backend-assets-adapters.log`: 20 sprite/focus/technology visual tests
  passed before adding the cache traversal test.
- `.local/backend-codec-tests.log`: 4 native asset tests passed, including PNG,
  TGA, BMP, DXT1/DXT5, frame cropping, DX10 BGRA and cache-root refusal.
- `.local/native-assets-proof.json`: all 25,670 definitions and warnings equal;
  83 sampled real textures match decoded RGBA pixels exactly. This is a sample,
  not an exhaustive assertion about every installed/mod texture.
- `.local/backend-assets-clippy.log`: all-target warnings-denied passed.
- `.local/backend-source-full-python.log`: 1,240 tests passed on installed release
  before the subsequent asset port. The current debug asset library is tested
  via HOI4_BACKEND_LIBRARY; its final release installation is still pending.
- `.local/parser-host-features-tests.log`: parser tests passed with the host's
  serde_json preserve_order feature enabled (20 unit + 4 integration tests).

New pinned image dependencies are image 0.25.8 and image_dds 0.7.2 with encoding
features disabled for DDS. Regenerate third-party notices before distribution.
Reference API: https://docs.rs/image_dds/0.7.2/image_dds/

### November 9 source generation and HTTP continuation

The asset release was installed; the previous pending-release note is historical.
GUI projections and scripted/DLC focus-rule expansion now also execute in Rust.
Political, event/option, decision, doctrine, building, geography, equipment and
special-project catalogs are native source services. The Python public loaders
delegate to them; independent reference loaders remain development oracles.
Static map adjacency uses a two-generation cache and one bitmap traversal,
preserving horizontal wrapping, missing-land uncertainty and impassable edges.

Executed evidence:
- `.local/native-focus-rule-proof.json`: all 7,513 focus criteria equal.
- `.local/native-visuals-proof.json`: focus and technology GUI projections equal;
  130 referenced texture headers measured rather than all sprite definitions.
- `.local/native-operational-source-proof.json`: 33,929 political identities,
  107 doctrines and the complete 35,581-operation catalog equal, including
  source order and warnings. Static topology equals for 10,154 present land
  provinces and 53,882 directed edges.
- `.local/backend-operational-adapters.log`: 56 targeted tests passed.
- `.local/native-catalog-generation-proof.json`: all 22 source fields equal;
  43 focus-tree view responses, the technology overview and all 9 technology
  folder responses equal. The unfiltered tree index required a shared-definition
  fix, separately rechecked in `.local/native-generation-all-trees-recheck.log`.
- `.local/backend-catalog-full-python.log`: 1,258 passed, with guide/catalog
  independent verification enabled; `.local/backend-catalog-javascript.log`:
  451 passed, no failures or skips. These counts precede the capability/i18n tests.
- `test_backend_http`: 5 tests passed after native catalog/config GET, tree,
  technology, sprite and refresh routes. Config POST and final i18n parity were
  not covered by that checkpoint.
- `.local/backend-catalog-release.log`: optimized DLL and executable installed;
  DLL SHA-256 4059410e8af764f3e89dbb760641a89cd840291be16062966ceba71dc8c44f22.
  A later capability-guard release supersedes this checksum when installed.

Application input ordering is preserved when no default guide is explicit;
the regression test reproduced accidental alphabetical selection before the fix.
No imported guide, HOI4 file or game memory was changed. No new live/HUD visual
alignment proof, final distribution or completed cutover is claimed. The normal
launcher still uses the compatibility server until all remaining gates close.


### November 9 authoring and import compiler continuation

Additional source boundaries now execute in Rust: entity/reference exports,
scripted identity alternatives, portable authoring/document compilation, guide
repository loading and association revalidation, static guide/resource enrichment,
Word/ZIP intake, campaign-bound research presentation, focus source-slot proofs,
whole-guide branch resolution, research lanes/choices, and DLC preflight.
Python adapters retain row object identity during in-place compilation because
import orchestration already holds references to those rows. A focused test
caught and fixed replacing them before the cutover.

Executed checkpoints:
- `native-entity-catalog-proof.json`: 7,992 focus/research/continuous identities and
  43,573 AI-export rows match the independent reference.
- `native-identity-generation-proof.json`: 43,120 filtered resolver rows plus
  national naming context match; no live state is stored in these generations.
- `native-word-archive-proof.json`: all 65 Word documents in the supplied archive
  list, extract and compile identically. This is not a full playability proof.
- `native-guide-enrichment-proof.json`: current 126-step guide enrichment and
  literal project costs match; source definitions are unchanged.
- `backend-focus-import-tests.log`: 72 focused tests passed before the whole-route
  cutover; `backend-focus-route-tests.log`: 80 passed afterward.
- `test_backend_focus_import`: independent source-slot search comparison across
  100 graphs at three work budgets; limits publish no partial proof.
- `backend-research-tests.log`: 83 research/choice/import tests passed.
- `backend-focus-research-full-python.log`: 1,294 tests passed with independent
  guide/catalog verification enabled, before the subsequent DLC ownership cutover.
- `backend-import-generation-tests.log`: 28 focused native HTTP/catalog, DLC,
  source-slot and research tests passed after native DLC integration.
- `backend-compatibility-clippy.log`: all-target warnings-denied passed.

The initial all-document focus/research comparison was stopped as an audit-only
process to install a missing immutable route-check cache. It is not counted as a
completed archive proof. The optimized installed release, complete current archive
comparison, normal launcher cutover, final distribution and real HUD alignment
remain separate gates. No game process was opened or modified in this continuation.


### November 9 shared source ownership and verified import identity parity

A real installation exposed a transport problem: constructing DLC preflight by
resending both localization tables and the unfiltered source tree exceeded the
32 MiB request boundary. The boundary was not weakened. Store refresh now opens
one complete native catalog generation; compatibility checks, entity projections
and the sprite adapter borrow that same owner. A replacement is published only
after all adapter values decode successfully. Shared visible/all-tree definitions
retain object identity. This also removes repeated individual source-loader calls
from normal refresh; the former orchestration remains a development oracle.

Additional executed evidence:
- `native-source-owner-proof.json`: all 21 compared source fields, all sprite
  definitions, resolver rows/national metadata and the selected tree view equal
  the independent former orchestration on the installed game. The recorded
  timings ran concurrently with other checks and are not a controlled benchmark.
- `native-focus-research-archive-proof.json`: **65/65** documents match for focus
  routes, research compilation, inferred scope and DLC assessment; zero
  differences; installed guide file hashes remain unchanged. Structural equality
  is not a claim that every source instruction is live-verifiable or playable.
- `backend-source-owner-corrections-tests.log`: 90 targeted tests passed after
  preserving source-inventory mutation references and continuous-focus ordering.
- `backend-political-cutover-tests.log`: 83 political/doctrine/operation tests
  passed. Native bindings retain authored conditions and preserve parameter
  references held by import orchestration; unresolved execution remains unknown.
- `backend-political-source-owner-full-python.log`: **1,298 passed** with independent
  guide/catalog verification enabled, before the subsequent foreign-binding port.
- `backend-import-cohort-javascript.log`: **451 passed**, no failures/skips.
- `backend-political-source-owner-rust-tests.log`: backend **8 passed**.
- `backend-foreign-cutover-tests.log`: **33 passed** after the foreign-binding port.
- `backend-foreign-clippy.log`: all-target warnings-denied passed.

The current optimized release for the foreign binding is still building. An
additional whole-archive Word availability comparison is running. Native host
boot/default launcher, operational-language/production/template compiler, media
and analysis routes, HUD compatibility worker and final distribution remain
open gates; no new game/HUD visual proof or completed migration is claimed.


### November 9 geography and foundational operation cutover

The optimized foreign-binding release completed and is superseded by the
foundation release (`backend-foundation-release.log`). Installed DLL SHA-256:
`56348f940f34f5a604952871aef37728e8ba47f26e16f3cc9d429ff180b1ec03`.
Static geography now has one immutable native owner with bounded handles; the
Python adapter preserves its public indices and topology-generation identity.
Agency, special-project, state-building and provincial-fortification bindings
now execute in Rust. Declared predicates remain authoritative; bindings express
qualified observation criteria, not proof that an order was issued.

Executed evidence:
- `native-word-inventory-proof.json`: all 65 documents and 61 country-guide
  availability records equal the former logic; 14 DLC-unavailable guides, zero
  differences. These are content restrictions, not import review problems.
- `native-geography-proof.json`: indices equal on 1,081 states / 10,272 provinces;
  all 30,842 tested state, province and name resolutions match for both building
  scopes. No live ownership/controller claim is made by this static proof.
- `backend-geography-cutover-tests.log`: 59 geography, fortification, topology,
  live-generation and operation tests passed.
- `backend-foundation-cutover-tests.log`: 118 targeted tests passed after the
  optimized operation adapter was installed.
- `backend-foundation-clippy.log`: all-target warnings-denied passed.
- The next full Python checkpoint is running in `backend-foundation-full-python.log`.

No imported guide or installed game file was changed. The normal application
still uses its compatibility HTTP server. Core decisions, acquisition, source
ownership and the boundaries above are native; remaining operational language,
production/templates, media/analysis routes, native HUD adapter, final launcher
and distribution gates are not marked complete. No shutdown was scheduled.


`backend-foundation-full-python.log` subsequently completed with **1,301 passed**
on the installed optimized backend, with guide/catalog independent verification
enabled. The next boundary is literal reference/recipe identity for production;
template archive persistence and the complete production compiler are not yet
counted as ported.

## 2026-11-09: operation composition and reference library

The optimized backend now owns the complete structured operation-binding pass,
source-column adoption milestones, production recipe identity, passive template
ZIP intake, PNG/JPEG validation, content-addressed image storage and atomic
manifest publication. Python compatibility entry points delegate these services
without reimplementing the binding loop. Three additional standalone HTTP route
names cover template upload, reference inventory and reference images; this is
not full native-host parity.

Executed evidence:
- `native-reference-library-proof.json`: actual membership archive has 98
  designs and 86 images; manifest/IDs and all image bytes equal the former
  implementation. The read-only saved-recipe inventory equals it on 109 file
  entries. This proves file evidence, not campaign load or HUD alignment.
- `backend-operations-library-targeted.log`: 43 targeted tests passed before
  compatibility activation. Backend Rust tests: 8 passed; all-target clippy
  passed with warnings denied.
- `backend-operations-library-release.log`: optimized library and native host
  installed; DLL SHA-256
  `246f2af3f959f8eb64bbda40fb3cde1d8d3119bab00a75dcb45c019451be7bee`.
- Dependency notices include the pinned zune-core/zune-jpeg repository licenses
  and provenance because their published crate archives omit the license texts.

The first following full Python run was **not a passing checkpoint**: 1,313
tests ran, with 64 errors from newly added timing operations not yet present in
the installed release, plus two tool fixture startup timeouts and one failed
startup assertion. The timing operations subsequently passed direct debug-core
comparisons. A separate owned-child diagnostic showed two successive Windows
Store interpreter startups taking about 4.5 seconds under concurrent work.
Fixture preparation now has a bounded 15-second allowance; the supervisor's
150-ms deadline and five-second descendant-termination assertions are unchanged.
All 8 standalone native-tool tests then passed. This is a test-harness correction,
not evidence of an application speed improvement. A clean full checkpoint after
the next compiler release remains required.

Imported guide definitions and installed game files were not changed. Whole
archive operation comparison is still running; its final counters must be read
from `native-operation-archive-proof.json`. The default server/HUD compatibility
worker remain Python, so phases 4–6 are still open. No shutdown was scheduled.

## 2026-11-09: source-language compiler checkpoint

`native-operation-archive-proof.json` completed: **65/65 documents and 3,806
structured operation reports match**, with zero differences and unchanged guide
files. This compares structured bindings, not every prose interpretation.

The next optimized compiler release was installed from
`backend-actions-vocabulary-release.log`; DLL SHA-256
`1258ec785db4651de549ac298e7813253de5f4d2e06615a03eaf5ead59cb1b21`.
Compatibility entry points now delegate bounded timing expressions, explicit
event answers, named offices/spirits/decisions, production shorthand, compound
action sequencing and reading hints to native services. Supplied vocabulary is
preserved across native generations; it is not replaced by a compile-time-only
abbreviation snapshot. Source ordinals remain presentation metadata, separate
from executable start/completion dependencies.

A full run first found premature finalization of a temporary catalog in four
compatibility command adapters. They now retain their immutable catalog owner
through the native call; a direct-list-catalog regression test covers the case.
After that correction, `backend-commands-full-python.log` completed with **1,325
passing Python tests**, using a private debug backend and independent guide/catalog
verification. `backend-commands-javascript.log`: **451 passed, zero skipped**.
The separate activated-compiler check passed 99 targeted tests.

While preparing additional command tests, the fortification Python oracle was
accidentally copied from the compatibility facade. Its evidence projection was
re-established as an independent Python calculation rather than a native call;
`backend-independent-fortification-oracle-tests.log` has **33 passed**, including
unknown/malformed selectors, province evidence and construction projections.
This oracle correction is not a change to application behavior.

The complete native prose-expansion pass and whole-preview orchestrator are
implemented candidates. Do not count their final cutover as complete until the
archive language comparison and full candidate test logs finish. Default HTTP
hosting and the HUD compatibility worker still remain to be migrated. No current
game/HUD visual proof, distribution cutover or shutdown is claimed.

### 2026-11-09 importer and static-analysis continuation

The optimized whole-preview/import compiler was installed from
`backend-native-import-release.log` (DLL SHA-256
`750aacb868b6d20a66e4a5cf62c422f30b2cfa55fc1f1afb1a6388be5e7cba0d`).
The activated public entry points passed **1,331 Python tests** with both
independent catalog and guide verification enabled
(`backend-native-import-cutover-full-python.log`). Complete prose expansion
matched **65 documents, 7,371 source steps and 8,597 compiled steps**, plus the
one installed guide, without source file changes
(`native-operation-language-proof.json`). Whole-preview archive validation is
still running and must not be counted as complete yet. The initial debug audit
was stopped after retaining its partial report; the same audit was restarted
against a private optimized DLL, not by weakening its comparisons.

Added native optimistic association editing, retaining raw source definitions
separately from local overrides and run progress. Validation checks identity,
kind, country/tree scope, stale expected value and newly introduced global
focus-route errors before atomic publication. Restoring the original identity
restores its original progressive-research rule. HTTP tests cover unchanged
source bytes and progress records; no actual user associations were edited.

Native CWTools service now verifies pinned rule/host manifests, compiles all
rule descriptions through Jomini, builds mod-overlay installation manifests,
normalizes full upstream reports and persists unreadable/recovered source
history separately. The partial demo analyzer is a separate port, not an
installation fallback. Sixteen independent comparisons over 24 fixture documents
matched its previous results. The actual packaged CWTools executable was also
invoked on a local fixture: report equality excludes only separately measured
elapsed time. No installed-game analysis or current live/HUD proof is implied.
Heavy analysis holds no reader/application/catalog lock while the host runs;
changed catalog generations invalidate cached reports and reject stale results.

The associations/static-analysis candidate passed **1,344 Python tests** with
independent guide/catalog verification (`backend-analysis-candidate-full-python.log`),
**451 JavaScript tests** (`backend-analysis-javascript.log`), **8 backend Rust
unit tests** and all-target Clippy with warnings denied. The first full candidate
run had one failure because the integrity-error wording changed; compatibility
wording was restored before the successful run. API import payload tests also
preserve rejection of non-object requests and unknown fields.

Native route-name coverage is **30 of 39**, with **9 route names remaining**;
this count is not full functional parity or completion of the migration.
The next optimized release is being built before activating these new Python
compatibility facades. Default hosting/HUD still require Python. Network source
intake, optional API extraction, media jobs, full runtime enrichment/boot,
Python-free HUD and the final Windows distribution remain explicit gates.
No commit, release publication, game modification or shutdown occurred here.

### 2026-11-09 media/source migration candidate

The prior association/static-analysis release completed and its activated public
facades passed **1,344 installed Python tests** with both independent verifiers
(`backend-analysis-installed-full-python.log`). DLL SHA-256:
`5f9f065f3e4d85b7197cd8f45a3aac3770d7890f41f2170d616d813ce68bd0c4`.
The optimized full import-preview audit completed: **65/65 exact report matches**,
zero differences, with installed guide bytes unchanged
(`native-import-preview-proof.json`). This is offline import parity, not proof
that every guide is executable on the user's DLC configuration or campaign.

The native host now implements all **39 legacy route names**, including media,
source acquisition and the optional provider extraction workflow. Route-name
coverage is not full integration/feature parity. The default launcher and HUD
compatibility worker still require Python; boot/runtime enrichment and the
Python-free HUD remain open gates.

Native source acquisition validates all DNS answers and pins curl to one checked
address; each redirect is revalidated, proxies/config are disabled, input/output
and deadlines are bounded, and private/reserved addresses are refused. A real
public `https://example.com/` acquisition succeeded
(`backend-source-web-smoke.log`). HTML/manual/caption identities and warnings
were compared with the existing implementation. Optional provider draft parsing
preserves the supplied source records, rejects incomplete/refused responses,
and never installs; no provider API call or paid extraction was performed.
Authorization content is excluded from process command arguments and logs.

The Rust media owner uses the same owned Windows job supervisor directly.
A real FFmpeg test generated local JPEG/MP4 previews, reused the cache, served
byte ranges and rejected unknown references. A complete native HTTP video job
prepared a short registered local source, created a separate copied AAC track,
encoded H.265/15 FPS and reused its verified cache. A separate synthetic 610-second
source split at 0-300, 290-590 and 580-610 seconds, preserving 10-second overlap
and the 150 MB hard cap. The source bytes were preserved. YouTube media download
on this new pipeline is not yet verified. Hardware selection is exercised by
probes/calibration, but no measured hardware speedup is claimed.

`backend-video-integration-tests.log`: **29 targeted tests passed**.
`backend-media-services-full-python.log`: **1,355 Python tests passed** with both
independent guide/catalog verification enabled. All-target Clippy with warnings
denied passed (`backend-video-clippy.log`). New locked URL/encoding dependency
notices regenerated successfully (`backend-media-notices.log`). The optimized
media build is still running at this checkpoint; no public compatibility facade
is switched to a feature absent from its installed DLL.

User explicitly revoked shutdown on completion on 2026-11-09. **Do not shut down
the PC.** No game files, campaign memory or imported guide definitions were
modified. No current game/HUD visual validation, commit, GitHub publication or
new distribution is claimed.


### 2026-11-09 - runtime context and native HUD candidate

The optimized media release finished and is installed (SHA-256: `84cd2d969f8343fb8bb8eda896addb97159dbfaa4363c7c32299dbd3b04c28de`). The subsequent GUI/HUD additions are private debug candidates. No default launcher or public HUD transport has been switched.

Runtime preparation and presentation now share the immutable installation source cohort. An actual installed guide comparison passed for both definition and presentation; its source file was unchanged (`.local/native-runtime-context-proof.json`). Boot attaches that context, and catalog/template reload compiles outside the reader/application lock before guarded publication. Existing run identity and checks survive reload.

The HUD candidate adds read-only native widget geometry, exact RTTI/owner/country checks, focus/research parent transforms, folder/tab clipping, masked visibility witnesses, bounded registry discovery and cache invalidation, event/office/production observations, and admission against the same active application guide. Specializations and roles remain unknown when the existing profiles do not qualify them. Modern-profile event-answer offsets are still withheld exactly as before; visible known popups suppress tree geometry. No game memory or files were modified.

A native HUD API checks revision again after geometry. Its target cache separates application evaluation from fresh GUI coordinates: selection, source generation, progress and live observation changes invalidate the cache; unchanged frames do not recompile the guide. OCR and local sprite fallback are being ported and compared before transport cutover.

At this checkpoint: **1,372 Python tests passed**, with independent guide/catalog verification and native target-projection comparisons; **451 JavaScript tests passed**; **11 backend Rust tests passed**, including animation-bit versus real-visibility mutation and full-byte witness preservation. Native HTTP/GUI targeted suite: **37 passed**. Independent registry discovery without supplied widget bindings also passed. Source/runtime equivalence and synthetic screenshot fixtures are not current game/HUD visual proof.

Remaining migration gates: finish and qualify the direct Rust HUD transport/visual fallback; native startup/autodetection and external-tool packaging; final Windows artifact and current browser/game/HUD checks. The installed default still has Python compatibility orchestration, so the complete migration is not yet claimed. The PC must remain on.
