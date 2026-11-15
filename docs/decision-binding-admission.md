# Decision import admission — 2026-11-15

## Root cause and scope

`recipient::political` already extracts `original_tag`, `tag`, nested AND/OR and
single-clause NOT from allowed/visible/available blocks. `operational_catalog`
combines decision, group and category constraints; duplicate definitions use OR.
Unknown scopes/predicates remain unknown. `catalog-core::country_test` evaluates
current `tag`, but deliberately has no original-country context. A current country
cannot safely supply that missing fact.

The ENG/SOV mistakes therefore involve two issues: original-country restrictions
remain unknown, and a unique prefix result in the decision family is treated as
an identity despite competing event answers. Merely adding `original_tag` as a
synonym for `tag` would reject legitimate changes of national context. The French
case is ambiguous, not a proved ownership contradiction or proved event identity.
See the frozen [investigation](guide-binding-safety-review.md) for source evidence.

## Implemented boundary

`native/backend/src/decision_admission.rs` runs only in `import_preview.rs`, after
existing promotion/binding and before validity/publication. It:

- Evaluates the existing constraint AST conservatively, with independent current
  and original-country arguments. Static preview has no qualified original tag;
  it never substitutes the guide country or target country for one.
- Rejects demonstrably incompatible current-tag constraints, preserving the ID.
- Requests revision for a weak **inferred** decision name with compatible event
  answer rivals. Queries use the existing native resolver, separately by kind.
  Exact names, full aliases and vocabulary aliases are not blanket-blocked.
- Leaves unknown restrictions unknown and selects no replacement candidate.
- With conditional execution/timing, does not claim the declared guide tag is the
  future actor's tag; event candidates are not filtered by that uncertain actor.
- Reports `decision_validation` entries with ownership, admission status and
  candidates. Errors make `valid=false`; the existing API/collection publication
  paths already re-preview and require validity.

This does not change the global resolver, instruction decomposition, runtime
evaluation, installed definitions, step keys, game state or stored progress.
`informal_targets` shares its unchanged literal-name pattern with provenance
capture so PP/timing suffixes cannot hide the original weak name.

## Persistent provenance

`action.params.decision_resolution` is optional descriptive metadata:

```json
{"schema_version":1,"id":"native_id","origin":"inferred","query":"original name fragment"}
```

Origins are `explicit_id`, `inferred`, `user_reviewed`, `legacy_unknown`.
An authoring `{ "id": ... }` reference is marked explicit when compiled, where
that intent is actually known. Newly resolved names retain their original query.
Preexisting IDs without a record are legacy/unknown, not retrospectively explicit.
The record survives JSON serialization and repeated previews; a record for a
different ID is rejected rather than silently downgraded. A generic `reviewed`
flag does not rewrite inference provenance. An explicit/reviewed declaration
does not prove ownership or successful in-game execution.

Remaining gaps: this metadata is not signed human-review evidence; existing data
cannot recover lost provenance. No new decision-association UI is added.
External compilers which omit the metadata yield legacy_unknown IDs. Existing
legacy ambiguous bindings are retained, not automatically repaired. No historical
provenance is reconstructed from the ID, quote or filename. Full Word ZIP and
real HTTP publication tests are distinct from direct preview replay.

## Verification

The 2026-11-15 integration task found **no production regression**; it adds
`tests/test_decision_admission_http.cjs`, without changing application code.
Before the original fix, its eight focused native contracts had six failures.

### Executable and isolation

The ordinary binary rebuild could not replace the running `hoi4-backend.exe`
(Windows access denied). The existing service was left alone. A temporary Cargo
package instead compiled the **exact repository `native/backend/src/main.rs`**,
with locked dependencies, release settings and the existing backend Cargo target.
It runs the actual HTTP application, not a fake server or mock compiler.
Validated executable SHA-256:
`f05591faa09fc6f241e9ed679ba4e418fd551b789672068f67831c14f9cdfadd`.

Every test server uses a random loopback port and its own temporary SQLite,
guide, collection, reference, selection, association and documents directories.
No live provider is configured. Original user guides/campaigns are not test targets.

### Verified workflows

| Result | Actual scope |
| --- | --- |
| PASS | Fixture ZIP: nine real DOCX payloads traverse extraction/catalog/preview; four ambiguous cases require revision, five controls install manually, including a literal authored ID. |
| PASS | Automatic fixture ZIP: four ambiguities withheld; six valid/already-present items retained. Repeated uploads/commits preserve IDs, metadata and installed file bytes. |
| PASS | Four **original corpus DOCX** files copied unchanged from the cached archive into an isolated ZIP; all four original guide IDs verified against the real installed game catalog, read-only with isolated documents/mod settings. |
| PASS | Original ZIP automatic and manual commit: zero installed, four need revision. `reviewed=true` cannot install them. Re-preview and duplicate upload retain their inferred provenance and original IDs. |
| PASS | The legitimate Norwegian `SWE_repair_the_mines` reference succeeds against both fixture and real catalogs; no foreign-prefix rejection. |
| PASS | HTTP authoring/document, preview, JSON serialization, re-preview, reviewed installation, persisted guide read and process reload preserve all four provenance states. |
| PASS | Manual and automatic collection re-preview/publication preserve all four states from persisted report fixtures made using real HTTP preview. These seeded legacy/review reports do **not** pretend Word contains review metadata. |
| PASS | Three fixture-server restarts preserve active selection, existing run/checks and loaded IDs; legacy file bytes and all remembered installed files remain unchanged. No historical ID is retrospectively labelled explicit. |

Country inference deliberately needs two supporting focuses; fixture Word files
include those anchors. An initial fixture without them became unbound reminders,
not false decision bindings. No production inference/decomposition was changed.

### Executed suites

- Full JS suite with the freshly rebuilt host and original-corpus options:
  **436 passed, zero failures or skips**, including eight HTTP integration checks.
- Backend lib: **110 passed**, two ignored by default; the frozen-corpus test
  subsequently ran separately and passed. The supplied advanced archive test was
  not run; it is unrelated to this change.
- Fresh native integration executables: **57 passed** (10 contracts, 4 benchmark
  contracts, 39 guide evaluations, 4 identity diagnostics). Temporary wrappers
  restore the fixture-root macro for an external manifest; original test sources,
  fixtures and expectations are unchanged. The initial wrong-root harness failure
  was corrected before these passing results.
- Catalog binding safety: **3 passed**. Frozen native replay: **1 passed**;
  182 source-name cases produce 4 ambiguities, 178 unchanged, no replacements.
  All 50 definitions/182 old decision IDs are unchanged except added provenance
  in the guard-only trial; existing key meanings/source/order stay equal.
- Protected baseline/development/holdout remain byte-identical and unregenerated.

For reproduction, rebuild the backend and set `HOI4_BACKEND_EXE` to that exact
executable before `node scripts/test-web.cjs`. The original-document test is
opt-in via `HOI4_DECISION_CORPUS_ZIP` (the four original DOCX files) and
`HOI4_DECISION_GAME_DIR` (read-only source); it otherwise explicitly skips.
Native lib/opt-in `HOI4_BINDING_SNAPSHOT` settings remain those in `CLAUDE.md`.

NOT VERIFIED: full reimport of every original guide/archive, graphical UI/HUD
interaction, and execution in a real live campaign. The existing user service
was not replaced/restarted. Lost legacy provenance and intended replacements for
the four flagged records remain unresolved; external removal of provenance still
yields legacy_unknown. No automatic historical repair or semantic gold approval.
Cleanup NOT COMPLETE: automatic review rejected deletion with `blocked by policy`.
Only owned host/test binaries, fingerprints and temporary sources were moved to
`.local/scarto/decision-integration-mwhsx08a/`; the original temporary directory
and those artifacts' active build paths are absent. Shared caches and the running
user service remain untouched. The validated executable is retained under
`artifacts/hoi4-binding-check-mwhsx08a.exe`; selected original input under
`stage/original-four.zip`. No owned test process remains.
