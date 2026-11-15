# Shared agent handoff
Updated: 2026-11-15. Repository-relative paths throughout; replace stale state, do not append session histories.

## Objective and progress
- Priority: understand the installed same-author guides using the existing Rust architecture, not build a universal language interpreter.
- COMPLETE: baseline, proposed gold, binding audit, identity diagnostics, decision import-admission fix ([report](decision-binding-admission.md)) and [identity/versioning design](guide-definition-identity.md).
- COMPLETE (Claude, 2026-11-15) milestone 1: immutable guide definitions + run-to-definition binding ([§](guide-definition-identity.md#implemented-immutable-definitions-and-run-pinning-2026-11-15)).
- COMPLETE milestone 2: stable operation identities + occurrence manifests ([§](guide-definition-identity.md#implemented-stable-operation-identities-and-occurrence-manifests-2026-11-15)).
- COMPLETE milestone 3: compiler contract 2, decomposition of the 632 opaque multiline reminder containers into `--line-N` children, ACTIVE for NEW archived definitions only ([§ design, results, unsupported patterns](guide-definition-identity.md#implemented-compiler-contract-2-decomposition-of-multiline-containers-2026-11-15)).
- COMPLETE milestone 4 (first grounded semantics): compiler contract 3, wording-independent resolution of trigger references to real focuses/technologies with separate intent rules ([§ contract 3](guide-definition-identity.md#implemented-compiler-contract-3-grounded-trigger-resolution-2026-11-15)). 117 children gained an observed focus/research condition (50 distinct triggers); completion of instructions stays manual.
- NEXT: decision and country/war-state triggers (needs capitulation/war facts), composite (and/or) trigger conditions, catalog-backed measurement, human review of children in UI/HUD.

## Verified architecture and decisions
- `CLAUDE.md` is the permanent instruction source; read it first, then this file and relevant diffs.
- Rust guide-core evaluates guides; backend exposes evaluations; JS presents them; separate Rust HUD consumes the active guide.
- Game access stays read-only; unknown observations never imply completion. No Python or new AI integration.
- Preserve source text, order, trigger scope, provenance and existing guide/campaign/run progress.
- User decisions: (A) legacy runs stay `legacy_unresolved`, evaluated against the contract-1 registry while `REGISTRY_COMPILER_CONTRACT == LEGACY_COMPILER_CONTRACT` (both 1); never auto-transfer progress. (B) pinned runs strictly immutable; no automatic upgrade/adoption.
- Milestone 3 decisions (autonomous, documented): contract 2 = contract-1 compile + `decomposition::decompose` (no new parser; promote already tries every command family, so containers = blocks with zero recognized lines). Unrecognized lines become manual gates (user-confirmed), advice stays reminder, headers become conditions (promote's compiler or unknown placeholder) or title lead-ins. Prohibitions ("do not", "ignore") stay trackable: never downgrade an action.
- Milestone 4 decisions: recognition (span match against grounded candidates, no reliance on "On"/"After") is separate from intent (trigger position + closed-class leftover words). A trigger compiles only if fully understood and its fact is persistently observable (focus, research); everything else stays unknown with a stated reason. Ambiguity is kept visible.

## Implementation map
- Storage (`native/storage/src/{lib,progress}.rs`): row v2, insert-only `guide_definitions`, atomic archive+pin save, one-time `<db>.v1-backup-r<rev>.sqlite3`, typed immutable `definition_ref`, rewind inherits, legacy `observed_definition_id` evidence only.
- `native/backend/src/definition_archive.rs`: `gd1_` envelope of the evaluated definition + manifest; `COMPILER_CONTRACT=3` (new archives), `REGISTRY_COMPILER_CONTRACT=1`, `LEGACY_COMPILER_CONTRACT=1`, supported compilers [1,2,3]; `seal()` helper.
- `native/backend/src/occurrence_manifest.rs`: `op1_` IDs (guide+contract+source digest+key+step content), UTF-8 spans, `--line-N` children located via their recorded line; `shadow`/`correspondence`.
- `native/backend/src/decomposition.rs`: line classes, children, metadata `action.params.decomposition` (incl. `trigger`), mixed `scope: action:` headers, entity-named leads/prefixes as triggers; grammar pinned verbatim to `operation_language.rs`.
- `native/backend/src/entity_resolution.rs`: candidates (guide steps + same-tree catalog focuses), span matcher, `resolve`, `fully_understood`, `trigger_condition`. `Context::decompose` passes catalog candidates.
- `operation_language.rs`: only `pub(crate) generated` + new `compile_condition` (promote's own condition compiler); promote unchanged. `runtime_context.rs`: `Context::decompose` (decompose + re-present).
- `application.rs`: `current_archive` compiles contract 2 for NEW runs; registry stays contract 1; `definition_for_run`, packet `definition.{compiler_contract,manifest,operations}`, `app_manifest`, `expected_definition_id` CAS. `http.rs`: `GET /api/guide/manifest`. Web: `guide-session.js` packet/pin check.

## Milestone 4 validation (2026-11-15, final sources)
- Backend lib 133 passed / 2 ignored, no warnings. Harness integration 86 passed / 0 failed (new `tests/entity_resolution_corpus.rs` 2; `presentation_fixtures` unchanged). `npm test` vs rebuilt host: 436 passed / 1 optional skipped.
- Installed corpus (read-only, no catalog): 50 resolved+observable distinct triggers -> 117 children with observed conditions (was 0); 3 withheld (leftover meaning), 2 ambiguous, 214 unresolved (mostly country/war state). Manual precision review: no wrong entity among observable resolutions.
- Development: 56 resolved/observable, 5 withheld, 3 ambiguous, 138 unresolved; line agreement unchanged (13/13 headers, 299/299 actions trackable). Holdout FINAL (not tuned): 76/77 actions trackable, 0 unconditional.
- Changed (milestone 4): `decomposition.rs`, `definition_archive.rs`, `entity_resolution.rs` (new), `lib.rs`, `runtime_context.rs`, tests `decomposition_activation.rs`, `entity_resolution_corpus.rs` (new), `occurrence_identity.rs`; docs. Live DB still written only by the running old app (PID 38308), no backup created.

## Milestone 3 validation (2026-11-15)
- Backend lib 123 passed / 2 ignored, no warnings. Harness integration 84 passed / 0 failed (new `tests/decomposition_activation.rs` 4; `occurrence_identity` app test rewritten for contract 2; `presentation_fixtures` unchanged).
- Development (agent proposals): 13/13 trigger headers -> headers; 299/299 proposed actions stay trackable (0 -> header, 0 -> advice); advice 2/6 (conservative by design); 0 unconditional timing children.
- Holdout FINAL (once, `HOI4_HOLDOUT_FINAL=1`, not tuned): 75/77 actions trackable; 2 header-with-embedded-action lines absorbed as headers; 0 unconditional.
- Installed corpus (read-only, no catalog): 632/632 containers -> 1,817 children (1,794 manual, 16 ambiguous, 7 advice); 845 unknown-condition, 972 unconditioned-by-header; text preserved, no invented completion, schema+manifest valid.
- App protection test: contract-1 pinned run and legacy run keep container keys and checks; new run is contract 2 with empty checks; old key and advice child rejected as checks; child manually checkable.
- `npm test` vs rebuilt `hoi4-pinning-host.exe`: 436 passed / 1 optional skipped / 0 failed.
- Changed files (milestone 3): `application.rs`, `decomposition.rs` (new), `definition_archive.rs`, `lib.rs`, `occurrence_manifest.rs`, `operation_language.rs`, `runtime_context.rs`, `tests/decomposition_activation.rs` (new), `tests/occurrence_identity.rs`, design doc, this file. Live DB untouched (no backup created); running app not stopped.
- Earlier milestones: storage 19 tests; real-data COPY v1 rev 1917 / 770 runs loaded identically and upgraded with backup (copy deleted).
- NOT VERIFIED: live gameplay/HUD rendering of children; catalog-backed compile rate of header conditions; prepared (622) container count under contract 2.

## Migration behaviour and rollback
- First write by the new binary on a v1 DB: backup next to the DB, then v2; old binaries refuse v2. Rollback = stop app, restore the backup (loses later progress).
- Existing user runs are legacy: they keep contract-1 behaviour. Decomposed children appear only on NEW checklists ("new run"), never on existing ones.
- The running user app (old code at `.local/rust-backend-target/release/hoi4-backend.exe`) keeps the live DB at v1 until restarted on a rebuilt backend.

## Remaining limitations / risks
- Contract 3 gaps: event/decision/country-war triggers, composite and/or conditions (withheld), dates/deadlines, acronyms, learned corpus abbreviations, references inside action text, header-with-action lacking an inner `: `. Resolution precision reviewed manually on the installed corpus only (no human gold IDs).
- Unsupported patterns (details in design §): header conditions without catalog rule (unknown placeholder -> child waits); intra-line sequences/alternatives/parallel actions, quantities, deadlines, shorthand stay text-only (manual, no automatic verification); nested headers; leads without bullets apply until next header.
- Containers referenced by dependencies, non-`explicit` guides and contract-1 recognized+residual blocks are not decomposed.
- Pinned runs keep archived display language; fixes reach only new checklists. Archives never garbage-collected. ~14% of operations without exact span (derived/unlocated).
- `Context::decompose` re-presents the guide (as research projection already does); presentation idempotence assumed, covered only by tests.

## Exact next actions
1. User: rebuild/restart when convenient (`scripts/build-backend.ps1` after stopping the app); then start a NEW checklist to see children. First write creates the v1 backup.
2. Review children and resolved triggers on 2-3 real guides in the suite/HUD (`GET /api/guide/manifest`, `decomposition.trigger`); turn mistakes into development cases. Holdout has been used for final reports twice (milestones 3-4), never for tuning; add a fresh holdout before claiming accuracy.
3. Next semantic step: decision triggers via `decisions.history.executed`, then war/capitulation facts for country-state triggers (138 development triggers), then composite and/or conditions.
4. With the catalog loaded, measure compiled vs unknown header conditions on prepared definitions (via `GET /api/guide/manifest` or a harness test).
5. Unchanged: four historical source fragments, 30 weak decision labels, human gold review.

## Harness and operating notes
- Running app locks `.local/rust-backend-target/release/hoi4-backend.exe`; never stop it to test. Use `.local/pinning-harness/Cargo.toml` (git-ignored): links the backend lib, runs every `native/backend/tests/*.rs` + `real_copy.rs`, builds `hoi4-pinning-host.exe` (use as `HOI4_BACKEND_EXE` for `npm test`). Env: `CARGO_HOME/RUSTUP_HOME=.local/toolchains/*`, `CARGO_TARGET_DIR=.local/rust-backend-target`; `cargo test --release --offline --manifest-path .local/pinning-harness/Cargo.toml --tests` (~5 min relink). Opt-in envs: `HOI4_REAL_COPY=<copy>` (never the live DB), `HOI4_HOLDOUT_FINAL=1`.
- Git: `main`, HEAD `34a799e`; large pre-existing dirty tree (recheck). Index empty. No commit/push.
- Prior artifacts preserved under `.local/scarto/`.
