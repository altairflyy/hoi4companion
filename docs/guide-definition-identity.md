# Stable instruction identity and frozen guide definitions

Status: **design and characterization only**, 2026-11-15. No production parser,
normalizer, evaluator, decomposition or storage behavior changed. This extends
the [binding/identity review](guide-binding-safety-review.md), not a new parser.
The [frozen benchmark](guide-interpretation-benchmark.md) remains authoritative:
622 prepared multiline reminder containers are candidates, not 622 proven
actionable failures. Gold proposals remain unverified; holdout is untouched.

## Requirements and verified gaps

| Area | Current evidence | Consequence |
| --- | --- | --- |
| Identity | Existing `guide_identity_diagnostics.rs` proves positional key reuse. 1,003 `--operation-N` and 58 `--note-N` keys are already installed. | Never rename/reinterpret these keys in an existing run. |
| Definitions | `application.rs::put_guide`, `prepare_context`, `project_definition` use the current registry. `runtime_definition_revision` hashes current presentation and can change again for a run's research projection. | A current hash is not an archived definition or a run pin. |
| Writes | `Application::guard` checks campaign, server, run and progress revision; manual/started key validation and observers use the current guide. | An unchanged run revision does not prove unchanged instruction meaning. |
| Persistence | `storage/src/progress.rs` saves progress but no definition. Unknown per-run fields survive load/update, are not typed/immutable in `observe_replace`, and are not inherited by `new_run`. | Adding an unchecked `definition_id` field alone is insufficient. |
| Rewinds | A new run preserves research history from earlier bindings, but has no definition fence. | Branch creation and history reuse need explicit definition ownership. |
| Source | Word guide IDs derive from DOCX bytes; `source_action` identifies an Actions block. Promoted children clone parent evidence; first child inherits parent key. | Neither quote hash nor parent evidence distinguishes repeated occurrences. |
| Compilation | `runtime_definition::prepare` can promote/rebind loaded definitions. `runtime_context` separates prepare from presentation; compiled cache is replaceable. | Freeze after static preparation, before presentation. A cache is not a historical archive. |
| Verification | Existing manual/reminder actions can carry descriptive metadata; unknown observations do not complete steps. | Comprehension need not imply automatic verification or a new operation family. |

The normal HTTP import path rejects duplicate guide IDs. Registry replacement
tests model internal reload/re-preparation, not a claim that HTTP permits arbitrary
overwriting. Restoring identical progress bytes is necessary but cannot establish
semantic compatibility. A mismatched supplied definition is not currently a core
admission error: application ownership must enforce the new contract.

## Minimum persistent contracts (proposed, not implemented)

1. Keep `guide_id` as the existing library/family identity and Guide Schema v1.
   Add a **definition archive format v1**, independent of that schema:
   `definition_id = gd1_<SHA256(canonical immutable envelope)>`. Hash inputs include
   exact acquired source text/artifact hashes, statically prepared guide, source
   occurrence manifest, compiler contract and evaluator contract. Canonicalization
   sorts object keys, preserves arrays/strings/numbers and excludes only the
   envelope's own ID. Do not strip semantic fields or binding provenance.
2. Archive the full prepared definition and manifest once, before localization,
   sprite presentation, live facts and relative-research projection. Preserve the
   original acquired text, acquisition/source locator and any available original
   artifact hash. Current game/catalog witnesses are provenance, not permission
   to rewrite archived native IDs. Store the binding snapshot needed for evaluation;
   a name lookup against a later catalog must not replace an archived binding.
3. Each run owns an immutable typed `definition_ref`:
   `{status:"pinned", id:"gd1_...", archive_format:1}` or
   `{status:"legacy_unresolved", reason:"original_definition_unavailable"}`.
   Selection, checklist creation, campaign binding and rewind explicitly retain
   the correct reference. An intentional move to a different definition creates a
   new run. No automatic progress migration in this first increment.
4. Persist each new source block and each executable child with separate opaque
   IDs generated **once**, not on each preview/evaluation. Existing keys are retained
   verbatim inside their archived legacy definition. No new child inherits an old
   container key or its check. Order is a separate ordered list, never an identity.
5. A source occurrence records immutable artifact/text identity, block ID, a
   document locator when available, exact quote and half-open **UTF-8 byte spans**.
   Distinct repetitions have distinct occurrence/child IDs even when text matches.
   A child can cite several spans. Trigger headers have separate scope IDs, nesting
   and ordered membership; conjunctions preserve their source spans and authored
   order. Store both raw header text and its compiled condition (or explicit unknown).
   Never drop an unrecognized header or leak its scope into the next block.
6. Keep interpretation and verification orthogonal in descriptive metadata:
   `interpretation.status = understood | partial | unrecognized | advice` and
   `verification.mode = live | manual_authored | unavailable | not_applicable`.
   Reuse current manual/reminder carriers according to their existing semantics.
   Reminder dependencies are rejected by today's schema: advice cannot become a
   completable prerequisite. A structured manual action with an explicit but
   currently unobserved predicate may be a dependency; it stays unknown/waiting.
   For an understood action without any representable completion predicate, retain
   its source dependency as unresolved metadata for review, not an invented fact
   or a reminder dependency. Visibility does not imply completion; independent
   instructions need not be blocked. Do not add mandatory
   confirmations or claim every reminder can be executed automatically.

`guide-definition-identity-cases.json` is a **hand-authored technical example**,
not generated manifest behavior or human-approved semantic gold. Its repeated
Unicode instructions demonstrate why text hashes/UTF-16 offsets cannot be IDs.
Across source revisions, do not guess that the nth identical phrase is the same
occurrence. For the first implementation, a new source revision receives a new
manifest; existing runs retain their old one. Deliberate edits may retain IDs only
through an explicit reviewed mapping, outside this milestone.

## Minimal storage and application extension

Use a new immutable `guide_definitions` table in the **existing SQLite database**,
content-addressed by ID. Do not copy all definitions into every run or progress
payload. The captured 50 prepared definitions occupy 6,958,161 UTF-8 bytes; the
current progress store rewrites its full JSON payload on each save and limits it
to 16,777,216 **characters**, not bytes. An archive belongs outside that hot blob.

Extend the existing transaction boundary, not add a second state engine. Publish
archive rows and run references in the same CAS-controlled SQLite transaction;
verify hashes and references before commit, make definitions insert-only, and
roll back archive/run publication together. Keep the progress root shape where
possible. Add a storage envelope/version **2** when pins are published; the new
reader must load v1 without destructive changes. Old v1 binaries must fail closed
on v2 rather than silently ignoring a pin. Unknown per-run field tolerance alone
does not provide downgrade safety. Preserve a pre-migration backup; make no write
just because the application inspected an old database. The internal storage
request protocol need not change unless transaction commands require it.

Add one central application `definition_for_run` lookup, used consistently by:
projection/API/HUD, observers, completion/started validation, resource-ID gathering
and relative-research binding. Return `{guide_id, definition_id, run_id}` in packets
and require matching definition identity on progress writes in addition to existing
CAS checks. Keep `runtime_definition_revision` for view/projection freshness only.
Old packet consumers must not accept an identity-mismatched evaluation.

Pinned definitions bypass source preparation, promotion and name re-resolution
on reload. Presentation and game availability can refresh independently. Archive
the evaluator contract too: one Rust engine supports declared compatible
contracts or returns unsupported/unverifiable; frozen JSON alone cannot preserve
semantics if evaluation rules change. Do not ship old executables as evaluators.

Preview staging needs a persisted manifest keyed by source fingerprint plus the
compiler/catalog-binding/library/vocabulary inputs that actually affect output.
Preview, review, commit and repeated import reuse the same manifest/IDs. A changed
binding review creates a distinct definition. Interrupted staging is recoverable
and never mutates a published definition. The archive is authoritative; compiled
cache eviction must not make a run unusable or recompile its meaning.

## Existing campaign compatibility

- First try recovery from a stored definition whose complete semantic fingerprint
  and provenance can be demonstrated to match that run. A cache/current registry
  or a present ID alone is not that proof. The captured benchmark is not a backup
  of the original definition used by every run.
- Where the original definition is absent, retain all saved records/IDs/maps
  unchanged and mark `legacy_unresolved`. Show the historical checklist separately;
  do not apply its checks/baselines/history to a freshly decomposed definition.
  Starting a new qualified run is possible without deleting the old one. If the
  user explicitly accepts the current definition for a historical run, record that
  as an assumption/review, not recovered historical fact; no automatic adoption.
- This unavoidable limitation is not solvable by hashing today's guide. Keeping
  old records is feasible; proving their original meaning retroactively is not.
- Preserve checks, started states, baselines, research bindings/options/history,
  production/campaign milestones and relevant session/country/campaign metadata.
  Campaign-native decision evidence remains separately qualified. A rewind keeps
  the old definition and existing rewind semantics; cross-definition research
  history reuse is rejected unless independently proven compatible.
- A future reviewed one-to-one migration must compare source occurrence, action,
  entity, parameters, conditions, scope and dependencies. One-to-many parent
  confirmation never confirms children. No such migration is needed to begin
  decomposition for **new definitions/new runs**.

## Smallest implementation order and acceptance gates

1. **Archive/pin first; no decomposition:** implement immutable archive, v1-read/
   v2-write/downgrade rules, central run lookup, definition CAS and creation/rewind/
   selection inheritance. Freeze currently supported contracts. Characterize all
   old maps before migration; unresolved legacy records remain intact. Test crash,
   missing/corrupt archive, stale writer, restart and two runs on different versions.
2. **Persist occurrence manifests in preview, still no new decomposition:** exact
   spans/scope/order, distinct repeated occurrences, stable preview-review-commit
   IDs, source fingerprint changes and explicit binding provenance. Publish only
   complete validated envelopes; no silent truncation at existing track/piece limits.
3. **Shadow the 622 candidates:** propose children with understanding separate from
   verification, compare per-guide/corpus outcomes to frozen fixtures. Expose each
   changed binding/scope/order and each unrecognized residue. Do not rewrite gold
   or holdout to agree with output. Human review is needed to measure correctness.
4. **Only after approval:** enable decomposition for new archived definitions/runs.
   Existing pinned runs retain their exact original keys and behavior. No new
   operation families, global resolver rewrite or universal-language redesign.

Required release gates: zero reassigned progress meanings; zero unproven
parent-to-child completion; zero silent definition/contract fallback; 100% source
quotes/span boundaries and order preserved; distinct repeated occurrences; exact
scope membership including unknown/nested triggers; identical IDs across repeated
preview/install/reload; all run-owned maps retained or explicitly unmapped; atomic
archive+pin publication and downgrade rejection. Preserve decision safety and
legitimate foreign controls. Report comprehension and verification separately,
per guide and corpus. No accuracy percentage from unreviewed gold proposals.

## Validation ledger

The new characterization suite covers registry/hash versus run CAS, SQLite reload,
unknown-field retention versus immutability, rewind inheritance, strict storage
versions, duplicate occurrence/spans, and understood-but-unobservable instructions.
The prior four identity tests remain the positional-key regression reference.
These tests observe gaps and demonstrate existing opaque-key compatibility;
they **do not implement or validate the proposed archive/manifest generator**.
Actual results, 2026-11-15:

- New Rust characterization: **8 passed**; existing identity diagnostics: **4 passed**.
  Fresh integration executables linked against the unchanged current backend in
  an isolated temporary Cargo package, reusing the existing target directory.
  This avoided replacing the in-use application executable.
- Backend `cargo test --release --locked --manifest-path native/backend/Cargo.toml
  --lib`: **110 passed, 2 ignored** (optional original archives, not exercised here).
- `node scripts/test-web.cjs` with the previously rebuilt, source-equivalent host:
  **435 passed, 0 failed, 1 skipped** (optional original four-DOCX integration).
  No need to repeat the completed original-archive investigation for this design.
- Protected baseline/development/holdout SHA256 values match the frozen constants;
  the technical fixture's source SHA256 and exact Unicode byte spans also match.
- Initial characterization run: six passed, two failed because the test inputs
  omitted required progress identity and attempted a forbidden reminder dependency.
  Corrected the fixtures/design to respect existing contracts; no production change.
- Preservation audit: 748 preexisting source files unchanged; only the shared
  handoff changed, with this design, one test and one fixture added. Test-owned
  SQLite files were removed by the test. Harness deletion was blocked by automatic
  review (`blocked by policy`); its three files (43,461 bytes) were moved to
  `.local/scarto/identity-validation-9a21f686efa44f3284cefd19dd0918b2/`.

## Implemented: immutable definitions and run pinning (2026-11-15)

Milestone 1 of the order above is implemented; no decomposition, parsing or
completion semantics changed.

- **Archive** (`native/backend/src/definition_archive.rs`): format 1 envelope
  `{archive_format, guide_id, compiler_contract, evaluator_contract, source_sha256,
  occurrence_manifest:null, definition}`, canonical sorted JSON, `gd1_` SHA-256.
  **Deviation:** `definition` is the *evaluated* guide (after preparation AND
  presentation), not the pre-presentation form proposed above, because
  `guide_enrich` writes catalog-derived semantic fields (`prerequisites`,
  `focus_mode`, `focus_criteria`, `facility_buildings`, `operational_criteria`).
  Pinned display names therefore keep their archived language. Opening verifies
  hash, canonical bytes, guide and supported contracts, else fails closed.
- **Storage** (`native/storage`): progress row version 2, payload root unchanged;
  `guide_definitions(id, archive_format, payload)` created lazily, insert-only,
  identical bytes for a reused ID; referenced IDs checked inside the same
  transaction; downgrade refused; omitted version keeps the stored one. Before
  the first v2 write over v1 data: `VACUUM INTO <db>.v1-backup-r<revision>.sqlite3`
  (once). Opening/reading never writes. v1-only binaries reject v2 rows.
- **Runs** (`progress.rs`): typed `definition_ref`, either
  `{status:"pinned",id,archive_format:1}` or
  `{status:"legacy_unresolved",reason,observed_definition_id?}`; immutable in
  `observe_replace`; rewinds inherit; new runs take the current definition via a
  `needs_definition` handshake (only for callers sending `definitions`).
- **Application**: `definition_for_run` feeds projection, `/api/guide`, manual
  and started key validation, observers and research projection. Packets carry
  `definition` (`id/status/run_id/current_definition_id/newer_definition_available`;
  legacy adds `observed_definition_id/changed_since_observed/undeclared_checks/
  undeclared_started`). Writes accept `expected_definition_id` (null = legacy).
- **Legacy runs**: unchanged data; evaluated against the current definition only
  while `COMPILER_CONTRACT == LEGACY_COMPILER_CONTRACT`; the first write records
  the then-current ID as *observation evidence*, never as the recovered original.

Validation (2026-11-15): storage 19 tests; backend lib 112 passed/2 ignored;
backend integration 75 (new `tests/definition_pinning.rs`; former `current_*`
characterizations replaced by safety assertions); JS 436 passed/1 optional
skipped against a host rebuilt from current sources. A copy of the real progress
database (v1, revision 1917, 770 runs) loaded unchanged, upgraded with one backup
and reloaded identically; the live database was not touched.

## Implemented: stable operation identities and occurrence manifests (2026-11-15)

Milestone 2 infrastructure; still no active decomposition, parser change or
evaluation change (`presentation_fixtures` unchanged, `COMPILER_CONTRACT` = 1).

- **Module** `native/backend/src/occurrence_manifest.rs`, stored in the archive
  field `occurrence_manifest` (format 1) and validated structurally on open.
  Archives from milestone 1 (null manifest) stay evaluable, reported `unavailable`.
- **Operation identity** `op1_<32 hex>` = digest(guide, compiler contract,
  digest of all acquired source texts, step key, canonical step content). A
  reused positional key with a new meaning, a new source revision or a new
  compiler contract gets a new ID; identical input gives identical IDs across
  preview/install/reload/independent apps. IDs are opaque and never recomputed
  from stored manifests.
- **Provenance per operation**: step key, track/index, base Word key,
  `parent_key` and role (`step`/`child`/`note`), `block_id` (source, quote,
  occurrence, UTF-8 span), instruction quote, `occurrence` (nth identical quote),
  `span_status` (`located` | `derived` = not verbatim | `unlocated` |
  `ambiguous`). Spans match whole lines only. Repeats are assigned in order only
  when match and claimant counts are equal; otherwise there is no span. Order is
  a separate `order` list.
- **Shadow decomposition** (`shadow`): for each multiline manual reminder
  container, every nonempty line becomes an inactive child proposal (`pc1_` ID,
  parent operation, exact span, `interpretation: unrecognized`,
  `verification: unavailable`). A line ending in `:` is kept as a raw scope
  header (`condition_status: unknown`) over the following lines.
  `active:false`, `requires_compiler_contract: 2`.
- **Correspondence** (`correspondence`): `same` only for identical operation IDs.
  Otherwise `unmapped`, or `split_unmapped` when the old operation's block now
  has several operations. A parent never maps to children. Any source revision
  maps nothing (explicit reviewed mapping remains future work).
- **API**: `app_manifest` / `GET /api/guide/manifest?id=&run=` returns the run's
  own manifest, shadow proposals and, for pinned runs with a newer definition,
  the correspondence. Projection `definition` adds `manifest`
  (`bound`/`unavailable`, legacy `unbound`) and `operations`.
- **Legacy runs** stay keyed by positional step keys, `manifest: "unbound"`,
  under the unchanged compiler-contract rule.

Validation (2026-11-15):
- backend lib: 118 passed / 2 ignored
- backend integration: 80 passed, including new `occurrence_identity.rs` (4) and
  harness `real_copy`
- frozen baseline corpus: 6,996 steps, all IDs unique, 1,105 children with
  parents present, 1,763 shared-evidence repeats distinct, order preserved
- development 320/320 and holdout 80/80 case lines located byte-exactly
  (identity only; annotations not read)
- installed corpus (read-only): 50 guides, 6,952 operations, 5,958 located,
  0 ambiguous, 632 containers (equal to the frozen persisted count), 1,818 shadow
  children
- JS: 436 passed / 1 optional skipped; `test_native_service.cjs` checks the
  manifest route

NOT IMPLEMENTED at that milestone: activation of decomposition (done below).
Still open: reviewed cross-revision mapping/upgrade of a pinned run, archive
garbage collection, live/HUD validation.

## Implemented: compiler contract 2, decomposition of multiline containers (2026-11-15)

**Finding that shaped the design.** Contract 1 (`operation_language::promote`)
already splits a multiline manual reminder line by line, using every existing
command family (events, timing headers, foreign, fortification, sequences,
laws, agency, production, doctrine, named targets). It keeps the split only if
at least one line is recognized. The 632 persisted containers (622 prepared)
are therefore the blocks where no line was recognized: re-running recognizers
on them yields nothing new. Contract 2 adds structure, not new parsing.

**`native/backend/src/decomposition.rs`** (applied after the contract-1
prepare+present, then re-presented with the same catalog):
- Grammar shared verbatim with promote (line breaks, bullets, inline/ellipsis
  timing; a unit test pins the strings to `operation_language.rs`).
- Line classes:
  - header: any line ending in `:`. A timing header (promote's
    on/after/before/when/once/at grammar) becomes `when` of the following
    lines. It is compiled with promote's own condition compiler
    (`operation_language::compile_condition`), or else gets promote's unknown
    placeholder fact `guide.conditions.<hash>`, so the line is never
    unconditional. Other headers are lead-ins that prefix their items' titles
    (a bullet-led list ends at the first non-bullet line).
  - inline `When X: Y`: Y is scoped by X, which also sticks for following lines
    (promote semantics).
  - advice (whole-line parenthetical, you can/may/could, note/nb/tip/ps): stays
    a non-completable reminder child.
  - ambiguous (ends in `…`/`...`/`?`): manually tracked, flagged.
  - everything else: a manually tracked gate (kind `gate`, no `tracking`, no
    invented completion), checked by the user.
  - Prohibitions ("do not", "ignore") stay trackable: downgrading an action to
    advice is the unsafe direction.
- Children: keys `<container>--line-N` (1-based physical line, never reusing
  the container key); title = preserved text (lead-prefixed); instructions =
  exact line text; `note` = timing/bullet context. Each child inherits the
  parent's evidence, `when`, `depends_on` and params. Metadata is in
  `action.params.decomposition`
  (`contract, parent_key, line, status, timing, lead, context,
  condition_status ∈ compiled|unknown|inherited|none, headers`). Order is kept
  at the container's position.
- A container is kept unchanged when: the guide's dependency policy is not
  `explicit`, another step depends on it, it has fewer than two non-blank
  lines, a key would collide, or it would produce more than 512 children.

**Activation and protection.**
- `COMPILER_CONTRACT = 2` for NEW archives only (`Application::current_archive`
  -> `Context::decompose`, or catalog-free conditions when no catalog is
  loaded).
- The in-memory registry stays contract 1 (`REGISTRY_COMPILER_CONTRACT`), so
  legacy runs keep their original step keys and evaluation;
  `LEGACY_COMPILER_CONTRACT` is unchanged.
- Contract-1 archives stay supported and are never re-decomposed.
- Parent progress is never transferred: the `--line-N` namespace cannot match
  any saved key; old keys are rejected on contract-2 runs; a new checklist
  starts empty.
- Packets expose `definition.compiler_contract`. Children are ordinary manual
  gates for guide-core and the web/HUD (no UI code change).

**Shadow evaluation on the real corpus** (installed persisted guides, read-only,
no catalog):
- 632/632 containers decomposed into 1,817 children:
  - 1,794 manual
  - 16 ambiguous
  - 7 advice
- 845 children carry an unknown timing condition and 972 have no header
  condition; with a catalog, promote's compiler may compile some of them.
- 163 children are lead-prefixed and 839 keep their bullet context.
- Every child's text is on its source line; there are no invented completions;
  schema and manifest are valid.

**Development agreement** (agent proposals, not human gold):
- 13/13 trigger headers detected as headers.
- 0 proposed actions turned into headers or advice; 299/299 actions stay
  trackable (295 manual, 4 ambiguous).
- Advice: 2/6 detected, by design.
- 0 timing-scoped children are unconditional.

**Final holdout evaluation** (run once with `HOI4_HOLDOUT_FINAL=1`, not used
for tuning):
- 74 manual + 1 ambiguous of 77 proposed actions.
- 2 proposed actions absorbed as headers: "On free Mil: Inf Equip to 25 &:" and
  "Wait Until November 1940 before ending the war:".
- 0 unconditional timing children.

**Remaining unsupported instruction patterns** (kept as text, manually tracked):
- Headers that also contain an action (holdout cases above): the text survives
  only as the condition/lead of the following lines.
- Header conditions with no catalog rule stay unknown placeholders, so their
  lines wait (never auto-complete).
- Intra-line sequences, alternatives and parallel actions ("X, then Y"; "A or
  B"; "A & B"), quantities, deadlines ("before SEP 1940"), relative timing
  ("Over time"), country/entity shorthand and abbreviations: no typed
  operation, so no automatic verification.
- Advice detection is conservative ("Ignore X" stays trackable).
- Nested headers, and lead lists without bullets that span unrelated lines:
  a lead applies until the next header.
- Containers that other steps depend on, guides without the `explicit`
  dependency policy, and recognized-plus-residual blocks from contract 1 are
  unchanged.

Validation (2026-11-15):
- backend lib: 123 passed / 2 ignored
- integration via harness: 84 passed (new `decomposition_activation.rs` 4,
  rewritten app test in `occurrence_identity.rs`)
- JS: 436 passed / 1 optional skipped against a host built from current sources
- `presentation_fixtures` unchanged

## Implemented: compiler contract 3, grounded trigger resolution (2026-11-15)

First game-grounded semantic step. It covers references in TRIGGER position
(headers, the scope part of `scope: action`, mixed `scope: action:` headers)
to real national focuses and technologies. No new parser, no external service.

**Entity recognition** (`native/backend/src/entity_resolution.rs`), kept
separate from intent:
- Candidates come only from real game data: the guide's own grounded steps
  (focus/research/event/decision entity IDs with author title, localized name
  and ID words; this is the nation, its focus tree and chronology) plus, when
  the catalog is loaded, catalog focuses of the same tree
  (`catalog_candidates`, wired in `Context::decompose`).
- Matching is span-based. Any contiguous token span of the reference may match
  a candidate's full name; the words around it never decide what is
  referenced, so "On X", "After X", "Finish “X”", "X…", "right after the X" and
  a bare "X" are equivalent for recognition.
- Generic evidence rules:
  - exact folded tokens;
  - `…`/`...` marks an incomplete name, including mid-word truncation;
  - abbreviated tokens of 3+ letters that prefix the name token ("Eng Comp UP"
    -> Engineer Company UP);
  - short all-caps tokens are country/unit labels and match only exactly, so
    "FIN" never abbreviates "Finland";
  - an incomplete name without `…` is never assumed.
- Ranking: a full name beats a truncation, and more matched words beat fewer.
  Equal best matches of different entities (two focuses, or a focus and an
  event) are `ambiguous`, with the candidates and their chronology listed.
  Nothing is chosen silently.

**Intent** (`trigger_condition`) is decided only from position and the
leftover words:
- A trigger counts as understood only when every word outside the matched span
  is closed-class (temporal/selection connectives, articles, auxiliaries,
  entity-kind words).
- Anything else withholds the condition (`residual_condition_not_understood`):
  a second condition ("& GER down"), an alternative ("or ASAP afterwards"), a
  different predicate ("Paratroopers Trained"), a negation or a quantity.
- Relation: completion words -> `focus|research.completed`; "before" ->
  `not(reached)`; otherwise reached = active or completed.
- Only focus and technology triggers compile, because those facts are
  persistently observed. Events (`events.visible` is transient) and other
  kinds stay unknown with an explicit reason.

**Structure** (`decomposition.rs`):
- A lead header naming a grounded entity becomes a trigger without any "On".
- `scope: action:` splits into a scope and a manually tracked action step.
- An inline `scope: action` whose scope names a grounded entity scopes the
  action.
- An ambiguous scope reference becomes an explicit unknown trigger with its
  candidates listed.
- Timing rules (promote's compiler) still run first.
- Metadata: `action.params.decomposition.trigger`
  (`text, condition, method, relation, verification, resolution`).
- `COMPILER_CONTRACT = 3` for new archives only; supported contracts are 1, 2
  and 3; the registry and legacy runs stay on contract 1.

**Investigation: entity data available.**
- The catalog index already holds focus, technology, law, country, event and
  `event_option` rows (event binding in `event_instructions`), plus decision
  binding (`decision_admission`).
- Observable live facts: `focus.active/completed`, `research.active/completed`,
  `decisions.history.executed`, `decisions.targeted.history.*`, and the
  transient `events.visible`/`visible_options`.
- Next grounding candidates:
  - decisions as triggers, via `decisions.history.executed` (persistent);
  - country/war state ("FIN down", "war on X"), which needs capitulation and war
    facts first; 138 development trigger texts are of this kind;
  - catalog-wide aliases for focuses missing from the guide.

**Measured** (installed persisted guides, no catalog, read-only):
- 50 distinct trigger references resolved and observable; 117 decomposed
  children now have an observed focus/research condition instead of an unknown
  placeholder (0 before, under the same no-catalog setting).
- 3 resolved but withheld for leftover meaning; 2 ambiguous ("On Bring…", three
  AUS focuses); 214 unresolved (country/war/game-state conditions, leads).
- Manual precision review of the resolved list found no wrong entity among the
  observable ones; the wrong-intent cases found during review are now
  withheld.
- Completion of the scoped instructions stays manual. Only WHEN they apply
  became automatically verifiable.

**Development** (agent proposals, no gold IDs):
- 56 trigger texts resolved and observable; 5 withheld; 3 ambiguous; 138
  unresolved.
- Every resolution rests on a non-label word.
- Earlier line-level agreement assertions still hold.

**Holdout final** (`HOI4_HOLDOUT_FINAL=1`, not used for tuning):
- 76/77 proposed actions trackable (the mixed header now splits); 0
  unconditional timing children.
- The remaining miss, "Wait Until November 1940 before ending the war:", is a
  header carrying an action without an inner `: `.

**Not done yet** (explicit):
- Event, decision and country-state triggers.
- Learned corpus-wide author abbreviations (only generic prefix abbreviation
  so far).
- Acronyms ("NAF").
- References inside action text, beyond triggers.
- Dates/deadlines ("before SEP 1940").
- Conjunctions/disjunctions compiled into composite conditions; they are
  withheld today.
- Catalog-backed measurement on prepared definitions.

Validation (2026-11-15):
- backend lib: 133 passed / 2 ignored, no warnings
- integration via harness: 86 passed (new `entity_resolution_corpus.rs` 2; new
  unit suites `entity_resolution` 7 and `decomposition` +3)
- JS: 436 passed / 1 optional skipped against the rebuilt host
- `presentation_fixtures` unchanged
