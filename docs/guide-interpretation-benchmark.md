# Same-author guide interpretation baseline

Baseline captured on 2026-11-15. This investigation adds **diagnostics only**:
no production parsing/evaluation, application UI, guide files, campaign records,
game files or game memory were changed. Existing unrelated working-tree changes
were retained. No commit or push was made.

## Scope and verified accounting

The unit of this benchmark is the installed collection, not every document in
every archive and not a universal natural-language corpus.

| Measurement | Verified result |
| --- | ---: |
| Installed JSON documents | 50 |
| National strategy guides | 48, covering 23 countries |
| Ancillary documents installed as guides | 2: VIDEO LINK and equipment-import instructions |
| Persisted steps | 6,952: 1,672 focus, 2,155 research, 3,125 operations |
| Current offline Rust-prepared steps | 6,996; 50 preparations succeeded, zero errors |
| Nonempty physical lines in Actions | **4,291**, not the previously suggested 4,420 |
| Original country DOCX sources verified | 47; 1,946 Actions paragraphs containing 4,190 physical lines |
| Additional country Actions lines | 101, from the preserved source of the older Hungarian EU guide |
| Persisted multiline manual-reminder steps | **632** |
| Current prepared multiline manual-reminder steps | **622** |

All 49 installed documents for which an archive entry was located were checked
against original DOCX XML (47 country guides and the two ancillary documents).
Original and acquired Actions text matched after trimming line-edge whitespace
for comparison only. Original characters and source coordinates are retained in
the cases. No internal whitespace, wording or ordering was normalized away.

`word-322dcfaf73196563`, the older Hungarian EU-path guide, has preserved acquired
text but no matching original archive entry. Its coordinates are explicitly
`captured_text_line`, not invented DOCX paragraphs. The other 47 country guides
have original table/row/column/paragraph/physical-line coordinates.

**A physical line is not an atomic instruction.** A Word paragraph may contain
multiple physical lines; one physical line may contain several commands,
conditions, alternatives and exceptions. Neither 4,291 nor 6,996 is a count of
correctly understood atomic actions. The 632/622 counters identify multiline
reminder containers, not 632/622 proven comprehension failures.

## Interpretation, binding and verification are separate axes

The checked-in baseline has these diagnostic outcomes for the 4,291 source lines:

| Source-to-definition diagnostic | Lines | Meaning |
| --- | ---: | --- |
| Typed payload present | 435 | A matching piece carries structured fields; correctness/completeness is **not** proven |
| Partially typed | 330 | Some matching pieces have fields, with residual/partial coverage |
| Untyped | 3,478 | Linked definition still presents raw instructions without measured structured fields |
| Unmapped | 48 | No matching emitted instruction; this includes consumed trigger headers and is not automatically an omission |

These are **not** the requested correctness labels. Human-approved expectations
are needed to distinguish correctly structured instructions, partially understood
instructions, unrecognized actionable instructions and genuine advice. The tool
reports accuracy as `null`, never as a fabricated recognition percentage.

The source-only triage finds 2,612 action candidates, 139 heading candidates,
43 simple advice candidates and 1,497 unclassified lines. These regex tags are
deliberately labelled heuristic. An unclassified line such as an exception or
abbreviated appointment can still be actionable; it is not discarded as advice.

Declared automatic-start predicates are attributable to 363 lines and declared
automatic-completion predicates to 741. These counts overlap and are **not live
reader coverage**. A declaration of a fact does not prove the reader can observe
it. Missing/unknown evidence must remain unknown. Explicit `check` predicates
are counted separately; there were zero in these linked completions, which does
not mean all remaining instructions can be automatically verified.

Source linkage is conservative: a complete quote needs a unique contiguous
source span or a matching original paragraph ordinal. Competing matches cannot
share a source occurrence silently. A single-line quote retains all group
members, including normalized instructions. Multiline members whose precise line
cannot be established are not discarded to manufacture an absence/count PASS.
Only **1,980 lines** currently have complete attachment for executable assertions;
the other lines still have source/diagnostic records, but uncertain attachment
stays not measured for semantic scoring. This is distinct from missing runtime:
all 50 runtime definitions were successfully captured.

There are 906 source lines with trigger candidates, 624 prepared steps with a
timing declaration and 475 such steps containing a generated
`guide.conditions.*` placeholder. Their denominators differ. Trigger-scope
accuracy remains unscored; a compiled predicate is not proof of correct scope.

## Where the current pipeline loses structure

Source evidence is in `native/model/src/word_sources.rs`,
`operation_language.rs`, `operation_binding.rs`, `timing_compiler.rs` and
`runtime_definition.rs`.

1. Word preparation creates one candidate per nonempty Actions paragraph.
   `explicit_action` handles a narrow set of standalone declarations. An entire
   mixed construction/agency line may therefore initially become a reminder.
2. The later language pass promotes manual reminders only under the explicit
   dependency policy. It splits on physical newlines and recognizes selected
   command families. Commas, conjunctions and informal fragments are not a
   general decomposition grammar.
3. **At least one recognized command is needed before the pass replaces an
   opaque original with its pieces.** If none is recognized, the original stays
   intact. This couples decomposition to language recognition, **not to successful
   automatic verification**. A recognized command can still have manual or
   start-only monitoring. Unknown residual pieces are deliberately retained.
4. A recognized subset can coexist with a residual reminder. For example, the
   Polish `MIL: Train, Truck:1, Inf:->10, ...` block contains structured targets for
   trains/trucks but retains other allocations as residual text. Counting only
   that first completion would overstate comprehension of the whole block.
5. Trigger headers can propagate to subsequent pieces in the same original
   block. Mixed `and`/`or` conditions and missing implied subjects remain
   uncompiled rather than receiving invented precedence. Scope must be checked
   against source expectations; current diagnostics do not declare it correct.
6. Many army/front/occupation/production-method instructions have meaningful
   player intent even when neither a unique game identity nor an automatic
   completion witness is available. They must not be counted as genuine advice
   solely because they are currently reminders.

### Family triage, not validated failure counts

The following overlapping families contain action-candidate lines without a
measured typed payload. A source line can appear in several families. They are
prioritization queues, **not additive or human-verified failure totals**.

| Family | Untyped | Unmapped |
| --- | ---: | ---: |
| Army | 772 | 0 |
| Campaign strategy | 600 | 0 |
| Construction | 311 | 0 |
| Diplomacy | 227 | 3 |
| Intelligence | 152 | 2 |
| Production | 150 | 0 |
| Politics | 118 | 0 |
| Doctrine | 98 | 0 |
| Focus/research references inside Actions | 71 | 0 |
| Events | 29 | 0 |
| Special projects | 26 | 0 |

The 1,497 source lines not classified by these heuristics require a separate
triage; they are not evidence of absence of actionable instructions.

## Entity audit and known dangerous associations

The frozen catalog contains 43,250 entries. The current prepared definitions
contain 4,451 distinct per-step typed entity references after duplicate fields
for the same entity are collapsed. All were found in the catalog; no duplicate
bound definition or absent/scoped-out ID was encountered by this check.
**Existence is not resolution accuracy.**

Country diagnostics: 3,843 references have no country-condition metadata exposed
for their entity kind, 501 have an unknown gate, 104 have no static contradiction,
and **three contradict the ordinary declared-origin assumption**:

| Guide / key | Authored text | Saved and runtime-prepared association | Script restriction |
| --- | --- | --- | --- |
| ENG `word-19549c55066166c1` / `word-0006` | We shall… | `SOV_propaganda_defend_moscow` | `original_tag = SOV` |
| SOV `word-3894e1e0f74f0a0d` / `word-0007` | Mobilize The… | `GER_mobilize_the_selbschutz` | Category requires `original_tag = GER` |
| SOV `word-67736ac669bdcbf1` / `word-0008` | Mobilize The… | `GER_mobilize_the_selbschutz` | Same category restriction |

The assumptions are explicit: a guide country is taken to be the original
starting country for this diagnostic, not live evidence of an actual campaign's
origin. These are strong contradictions for ordinary starts. They are not an
exhaustive manual audit of all entities and do not prove that a fresh import of
the original names currently reproduces them. The offline runtime still preserves
these saved associations.

`tests/fixtures/guide-benchmark-bindings.json` contains script-relative paths,
line ranges, SHA-256 hashes, original quotes and reasoning for these three cases
and three controls. Norway's `SWE_repair_the_mines` is **not** declared wrong:
the source says to repair after Sweden falls, and the scripts use control/damage
requirements. Two Spanish `SPA_*` cases have `original_tag = SPR` in their
category and likewise demonstrate why ID prefixes are not country gates.

The resolver preserves `original_tag` conditions but cannot equate a current
country tag with an observed original country. Unknown predicates remain unknown;
short-prefix resolution can still choose a sole remaining textual candidate.
Future work must improve the contextual evidence, not weaken this distinction.

## Proposed gold and holdout

- `tests/fixtures/guide-benchmark-development.json`: 320 cases.
- `tests/fixtures/guide-benchmark-holdout.json`: 80 cases in a **separate file**.
- Source-only stratification covers all 48 country guides, multiple operation
  families, costs, abbreviations, conjunctions, negations, nested/elliptical
  triggers, multiline blocks and incomplete names.
- Whole guides are assigned to one split. Identical normalized instruction text
  is excluded across the sampled splits. This does not claim that every near
  paraphrase or common author template has been eliminated.
- Each case contains exact text, parent block, neighboring paragraphs, source
  hash and coordinates, proposed actions/entities/trigger scope/relationships and
  a case-specific justification.
- **All 400 are agent-reviewed proposals; zero are human-verified.** Their
  proposed dispositions are 376 actionable, 13 trigger headers, 6 advice,
  1 context and 4 unresolved. These are proposals, not measured corpus accuracy.
- Proposed names generally remain `name_only`/`requires_context`; native IDs are
  not invented to make the benchmark look resolved. The separate six-case binding
  audit supplements the source sample with concrete identity safety cases.

Before scoring a case, review its source and expected meaning, provide approved
machine assertions under `acceptance`, and explicitly set
`review.status = "approved"`, `human_verified = true`, `reviewed_by` and
`reviewed_at`. Software verifies that approval metadata is present; it cannot
certify that a human actually performed the review. Agent proposals are never
automatically promoted to human labels.

Supported assertions are `no_structured_action`, `step_count` with `equals`, and
`field_equals` with an exact property path, `equals`, and optional `quantifier`:
`any` (default) or `all`. Unsupported/missing assertions are **not measured**, not
passed. For example, a reviewed law case could require
`{"kind":"field_equals","path":"action.params.law_id","equals":"partial_economic_mobilisation"}`
after checking the actual game ID and authored intent. This example is not an
assertion about a particular corpus case.

Accuracy means the fraction meeting **all approved executable assertions**, with
partial/failing/not-measured cases listed individually. It is not a claim of full
strategy correctness. Default scoring selects only development. Holdout scoring
requires explicit `--split holdout`; do not inspect/tune its labels during parser
development or repeatedly select changes by holdout results.

## Compatibility and preservation requirements

`word_sources::add` assigns `word-NNNN` from the growing global step vector.
Promotion keeps the first piece under the original key and names later pieces
`--operation-N`/`--note-N`. Adding/reordering pieces can therefore assign an old
key to a different action, even if total counts or the source document ID match.

Checks, started state, construction baselines, relative-research bindings/history
and production records refer to these keys. Runtime definition hashes identify a
changed projection; **they do not migrate key meaning**. Current preparation also
adapts previously installed definitions in memory, so protecting only new imports
would be insufficient.

The baseline stores every prepared step's track, position, source key/ordinal,
evidence hash and semantic hash. Comparison reports added/removed/reused keys and
changes to source correlation or action/condition/dependency meaning, even when
counts are unchanged. This is a compatibility alarm, not a migration mechanism.

Any later implementation must preserve original source text and immutable source
identity, relative order, parent/child provenance, trigger boundaries, guide and
campaign/run isolation, start-versus-completion dependencies and unknown states.
Do not transfer progress through an ambiguous old-to-new mapping. Freeze old
meaning or require an explicit versioned mapping; unrelated manual confirmations,
research targets and baselines must never silently move to another operation.

## Reproduction and outputs

Files added by this investigation:

| File | Purpose |
| --- | --- |
| `scripts/benchmark-guides.cjs` | Offline corpus diagnostics, source correlation, sampling, approved assertions and comparison |
| `scripts/benchmark-guide-sources.ps1` | Independent, bounded original Word ZIP/XML extraction |
| `native/backend/examples/guide_benchmark.rs` | Read-only current Rust static-preparation snapshot through existing interfaces |
| `native/backend/tests/integration/guide_benchmark_contracts.rs` | Four current-behavior diagnostic contracts |
| `tests/test_guide_benchmark.cjs` | Diagnostic correctness and frozen-sample safeguards |
| `tests/test_guide_benchmark_sources.cjs` | Original extraction, coordinates, duplicate entries and unsafe-XML rejection |
| `tests/fixtures/guide-benchmark-baseline.json` | Frozen corpus/per-guide counters, all prepared step identities and input fingerprints |
| `tests/fixtures/guide-benchmark-development.json` | 320 proposed expected interpretations |
| `tests/fixtures/guide-benchmark-holdout.json` | 80 separately held-out proposed interpretations |
| `tests/fixtures/guide-benchmark-bindings.json` | Six real script-evidenced country-binding cases, still awaiting human approval |
| `tests/fixtures/guide-benchmark-contract-cases.json` | Small synthetic native diagnostic inputs, not human gold |
| `docs/guide-interpretation-benchmark.md` | Findings, limits, acceptance gates and the proposed implementation plan |

Run from the repository root. No third-party packages are added. Node uses the
existing PowerShell/.NET XML/ZIP APIs on Windows; the native example invokes
existing Rust request interfaces and does not attach to HOI4 or open progress.
It reads the actual installation/mod catalog and can take about two minutes for
this collection; ordinary unit tests do not repeat that preparation.

```powershell
$env:CARGO_HOME="$PWD\.local\toolchains\cargo"
$env:RUSTUP_HOME="$PWD\.local\toolchains\rustup"
$env:CARGO_TARGET_DIR="$PWD\.local\rust-backend-target"
& .local\toolchains\cargo\bin\cargo.exe build --release --locked `
  --manifest-path native\backend\Cargo.toml --example guide_benchmark

node scripts/benchmark-guides.cjs --capture-runtime --verify-sources `
  --gold tests/fixtures/guide-benchmark-development.json `
  --baseline tests/fixtures/guide-benchmark-baseline.json `
  --output "$env:TEMP\hoi4-benchmark-result.json"
```

Use `--data-dir DIR` for another installed collection. Its settings must already
contain the valid game installation. Without `--capture-runtime`/`--runtime`, the
tool measures only persisted definitions and explicitly labels that limitation.

Optional `--records-output FILE` writes full source-line diagnostics;
`--runtime-output NEW.json` retains an explicitly requested verified native
snapshot for subsequent `--runtime NEW.json` runs. Otherwise all native/source
extraction intermediates are created outside the repository and removed in
`finally`. Reuse a frozen snapshot for repeated analysis; do not rebuild or open
the game merely to rerun counters. External snapshots without verified input
identity are marked unbound and cannot be used for regression comparison.

A frozen runtime snapshot measures the **already captured native engine**, not
new Rust code edited afterwards. Rebuild the example and make a fresh native
capture after parser/evaluator changes; do not use a cached interpretation to
claim that the new parser was exercised. The report retains the diagnostic
executable hash separately from current checkout source fingerprints.

`--sample-output NEW.json --holdout-output NEW-HOLDOUT.json` creates a new source
sample scaffold and **refuses to overwrite annotations**. It does not recreate
the independently proposed expected meanings. Corpus ID, input hashes,
vocabulary, catalog hash and diagnostic executable hash pin the baseline.
Capture checks guide/settings/association/library/vocabulary fingerprints before
and after native preparation and rejects mixed snapshots. The diagnostic Node
and XML-extractor source files have their own tooling fingerprint, separate from
the native engine executable.

Comparison refuses to call a changed corpus, catalog, source identity,
settings/library/association/vocabulary identity, diagnostic tooling or
interpretation layer a parser regression. Missing required fingerprints also
reject comparison. It reports per-guide counter changes,
individual key/semantic differences and newly introduced static entity issues.
Changes in diagnostic coverage alone do not establish an improvement.

## Acceptance gates for later changes — not implemented parser changes

1. **Approval first:** review this baseline and the proposed source meanings.
   Populate approved assertions before publishing a semantic accuracy number.
2. **Safety gate:** no new incorrect entity bindings on approved cases or the
   static-origin diagnostic; zero loss of a negation, condition, alternative,
   authored instruction or source reference. An unresolved result is preferable
   to an unsupported positive binding. Foreign-prefix controls must stay valid.
3. **Comprehension gate:** improve approved development assertions by operation
   family without reducing any previously passing safety case. Report complete,
   partial, unrecognized actionable, genuine advice and not-measured separately.
   Do not increase completion coverage merely to increase recognition counts.
   Never improve accuracy by dropping previously scored hard cases: compare
   individual approved assertions on a fixed case set and disclose coverage losses
   or changed expectations separately.
4. **Identity gate:** zero silent semantic reassignment of persisted keys. Every
   key change must have a reviewed preservation/migration outcome, with replay
   fixtures for old checks, started actions, construction baselines and research
   bindings in isolated campaign data. No real user database is a test target.
5. **Holdout gate:** after a candidate is selected on development, run the locked
   holdout separately. Require no new false bindings/scope-loss or regression of
   approved assertions. Report its approved denominator, not an unsupported 95%
   target or a confidence interval for this deliberately stratified sample.
6. **Corpus gate:** inspect every per-guide regression, not just aggregate gains.
   Preserve missing-DLC availability as separate from interpretation errors.
   No memory-reader or UI changes are justified by benchmark counter gains.

## Prioritized plan after approval

1. Resolve the three unsafe historical associations and add contextual negative
   binding tests, preserving original-country versus current-tag uncertainty.
2. Within the existing Word/language pipeline, make source decomposition and
   semantic recognition independent of live monitoring. Start with the author's
   recurring headers, continuations, cost suffixes and production/build lists;
   retain uninterpreted residuals and explicit ambiguity.
3. Establish scoped trigger clauses and explicit sequential/parallel/alternative
   relationships, including start/completion differences. Keep unsupported or
   mixed conditions unknown rather than inventing precedence.
4. Expand the author's existing abbreviation/identity vocabulary for appointments,
   units, production and diplomacy. Match by entity kind, country evidence, role,
   branch and prior source definitions; avoid generic fuzzy first-match rules.
5. Add typed player operations for army/front/method instructions even where
   automatic execution evidence is unavailable. Assess comprehension and live
   observability on separate scorecards.
6. Only then assess whether any framework change is necessary. The present
   evidence supports incremental Rust changes, not a rewrite or an AI component.

## Validation record

- Offline native example: release build succeeded; corpus preparation succeeded
  for all 50 documents. The final measured capture took 108.941 seconds on this machine;
  this is not a gameplay/HUD latency measurement.
- New diagnostic JS/source-extractor tests: 22 passed.
- Complete current JS suite: **422 passed**, including those tests and existing
  isolated native-service coverage (`node scripts/test-web.cjs`).
- Four newly compiled Rust diagnostic tests: **4 passed** when their test binary
  was run directly. They cover decomposition/monitoring separation, residuals,
  current trigger inheritance, key order, unsupported conditions and minimal
  country/name probes. They do not replay the full real-name import catalog.
- The broader Cargo test command was blocked while trying to replace an in-use
  `hoi4-backend.exe`. No process was stopped; this is **not** reported as a passing
  complete Rust suite. `npm test` initially could not resolve `node` through its
  Windows command wrapper; the identical script entry was run directly above.
- Human semantic approval, exhaustive entity-meaning review, live campaign
  execution, UI/HUD alignment and behavior after future parser changes remain
  **NOT VERIFIED**. They were not silently converted to PASS.
- Verified frozen-snapshot replay: comparable, **zero counter/key/semantic
  differences**. All 320 development and 80 holdout source IDs were found.
- Independent review exposed six diagnostic safety defects (missing-runtime
  PASS, uncontrolled inputs, reused quote spans, partial totals, ambiguous
  overlap and discarded normalized members). Each received a focused correction
  and regression protection; no production parser change was used to make them pass.
- Automatic review rejected recursive deletion of the manually retained session
  artifacts with `blocked by policy`. Its 13 owned intermediate files
  (97,600,673 bytes) were moved intact to
  `.local/scarto/guide-benchmark-session-qD8J9M`; the original temporary directory
  is absent. Four separate source-audit scratch files were deleted by their
  author. No benchmark helper process remains. Required benchmark/fixture outputs
  are the source-tree files listed above, not this discard folder.

## Per-document diagnostic summary

Typed/partial columns indicate payload presence, not approved comprehension.

| Country | Document | Actions lines | Typed | Partial | Untyped | Unmapped | Multiline reminders |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| POL | POLAND (Non-Aligned)_ Wojtek Conquers The World!   — Difficulty_⭐⭐⭐⭐.docx | 71 | 15 | 4 | 49 | 3 | 6 |
| HUN | HUNGARY (Non-Aligned)_ BROKEN EU Path! — Difficulty_⭐⭐⭐⭐ (NEW & UPDATED FOR 1.19.4).docx | 101 | 12 | 23 | 64 | 2 | 11 |
| ITA | ITALY (Non-Aligned)_ Papal Roman Empire — Difficulty_⭐⭐.docx | 88 | 9 | 2 | 77 | 0 | 19 |
| SWE | SWEDEN (Fascist)_ The Ultimate Nordic Empire! — Difficulty_⭐⭐⭐ (NEW).docx | 62 | 7 | 7 | 48 | 0 | 9 |
| GER | GERMANY (Fascist)_ BEST BUILD! — Difficulty_⭐ (Work In Progress).docx | 111 | 9 | 8 | 94 | 0 | 21 |
| ENG | UK (Fascist)_ Fascist Empire-No Civil War Path — Difficulty_⭐.docx | 111 | 4 | 3 | 102 | 2 | 21 |
| USA | USA (Fascist) — Difficulty_⭐ (OLD VERSION).docx | 146 | 3 | 10 | 130 | 3 | 20 |
| USA | USA (Fascist)_ Broken USA — Difficulty_⭐.docx | 78 | 7 | 16 | 54 | 1 | 8 |
| USA | USA (Democratic) — Difficulty_⭐ (NCNS NOTES).docx | 119 | 4 | 8 | 105 | 2 | 14 |
| ITA | ITALY (Fascist)_ Historical — Difficulty_⭐⭐.docx | 95 | 10 | 9 | 76 | 0 | 13 |
| TUR | TURKEY (Fascist) — Difficulty_⭐⭐⭐⭐.docx | 122 | 12 | 1 | 109 | 0 | 18 |
| GER | GERMANY (Fascist)_ Paratroopers Only Challenge!  — Difficulty_⭐⭐⭐ (INDICATIVE ONLY).docx | 37 | 8 | 6 | 22 | 1 | 3 |
| POR | PORTUGAL (Communist)_ The Better Spain!  — Difficulty_⭐⭐⭐⭐.docx | 53 | 9 | 4 | 39 | 1 | 5 |
| HUN | HUNGARY (Non-Aligned)_ BROKEN EU Path! — Difficulty_⭐⭐⭐⭐ (NEW & UPDATED FOR 1.19.4).docx | 101 | 12 | 23 | 64 | 2 | 11 |
| ITA | ITALY (Fascist)_ Historical — Difficulty_⭐⭐ (OLD VERSION).docx | 103 | 12 | 8 | 83 | 0 | 14 |
| SOV | RUSSIA (Communist)_ Historical — Difficulty_⭐⭐.docx | 100 | 11 | 5 | 84 | 0 | 16 |
| FRA | FRANCE (Non-Aligned)_ Napoleonic Empire — Difficulty_⭐⭐ (NEW VERSION + NCNS NOTES).docx | 92 | 9 | 9 | 74 | 0 | 9 |
| FRA | FRANCE (Non-Aligned)_ Napoleonic Empire — Difficulty_⭐⭐  (OLD VERSION)_.docx | 199 | 11 | 11 | 171 | 6 | 24 |
| GER | GERMANY (Fascist)_ Heavy Only Challenge! — Difficulty_⭐ (INDICATIVE ONLY).docx | 87 | 11 | 6 | 70 | 0 | 16 |
| SPR | SPAIN (Non Aligned)_ One King, Two Crowns Achievement — Difficulty_⭐⭐⭐⭐⭐.docx | 57 | 7 | 7 | 42 | 1 | 6 |
| POR | PORTUGAL (Non-Aligned)_ The Portuguese Empire! + Macau My Day Achievement — Difficulty_⭐⭐⭐ (NCNS UPDATED).docx | 81 | 18 | 6 | 57 | 0 | 10 |
| NOR | NORWAY (Communist)_ Scandinavia Turns RED! — Difficulty_⭐⭐⭐⭐⭐ (NEW).docx | 82 | 7 | 10 | 64 | 1 | 10 |
| SOV | RUSSIA (Communist)_ Reverse Barbarossa — Difficulty_⭐.docx | 93 | 9 | 6 | 78 | 0 | 17 |
| YUG | YUGOSLAVIA (Fascist)_ Slavic Supremacy! — Difficulty_⭐⭐⭐⭐⭐ (NCNS UPDATED).docx | 71 | 10 | 2 | 59 | 0 | 15 |
| SOV | RUSSIA (Non-Aligned)_ Third Rome — Difficulty_⭐⭐.docx | 92 | 5 | 5 | 81 | 1 | 18 |
| AUS | AUSTRIA (Non-Aligned)_ Secret Path! — Difficulty_⭐⭐⭐⭐ (Most Optimized).docx | 83 | 15 | 3 | 65 | 0 | 14 |
| GER | GERMANY (Non-Aligned)_ Secret Kaiserin Path! — Difficulty_⭐⭐.docx | 97 | 4 | 10 | 82 | 1 | 14 |
| SPR | SPAIN (Fascist)_ Falangist — Difficulty_⭐⭐⭐⭐⭐⭐.docx | 74 | 2 | 5 | 67 | 0 | 14 |
| GER | GERMANY (Fascist)_ Unstoppable Blitzkrieg!  — Difficulty_⭐.docx | 105 | 12 | 11 | 81 | 1 | 14 |
| BUL | BULGARIA (Non-Aligned)_ Master Of the Balkans! — Difficulty_⭐⭐⭐⭐⭐.docx | 70 | 6 | 5 | 59 | 0 | 14 |
| AUS | AUSTRIA (Fascist)_ The Better Germany! — Difficulty_⭐⭐⭐⭐.docx | 85 | 10 | 10 | 64 | 1 | 10 |
| Reference | VIDEO LINK.docx | 0 | 0 | 0 | 0 | 0 | 0 |
| ITA | ITALY (Fascist)_ Roman Empire (NEW Strategy) — Difficulty_⭐⭐.docx | 86 | 10 | 6 | 69 | 1 | 14 |
| ENG | UK (Fascist)_ Broken UK-Civil War Path — Difficulty_⭐ (NCNS UPDATED).docx | 83 | 4 | 6 | 72 | 1 | 14 |
| GRE | GREECE (Fascist)_ Byzantium The Eternal Empire Is Back! — Difficulty_⭐⭐⭐⭐.docx | 86 | 12 | 9 | 64 | 1 | 18 |
| POL | POLAND (Non-Aligned)_ Romanov — Difficulty_⭐⭐⭐⭐ (OLD VERSION).docx | 132 | 9 | 7 | 113 | 3 | 18 |
| ITA | ITALY (Fascist)_ Roman Empire — Difficulty_⭐ (OLD VERSION).docx | 114 | 7 | 0 | 107 | 0 | 18 |
| ENG | UK (Democratic)  — Difficulty_⭐.docx | 86 | 3 | 3 | 78 | 2 | 16 |
| GER | GERMANY (Historical)_ Can The World Stop US_ — Difficulty_⭐⭐⭐.docx | 81 | 11 | 5 | 65 | 0 | 10 |
| AUS | AUSTRIA (Non-Aligned)_ Habsburg Empire — Difficulty_⭐⭐⭐⭐.docx | 87 | 12 | 5 | 69 | 1 | 13 |
| PRC | CHINA (Communist) — Difficulty_⭐⭐⭐⭐⭐ (OLD GUIDE_NCNS DISABLED).docx | 115 | 7 | 6 | 97 | 5 | 19 |
| FIN | FINLAND (Fascist)_ Unlimited Power! — Difficulty_⭐⭐⭐ (NEW).docx | 96 | 10 | 13 | 72 | 1 | 9 |
| MEX | MEXICO (Fascist) — Difficulty_⭐⭐⭐⭐⭐ (PARTIALLY OUTDATED).docx | 86 | 17 | 2 | 66 | 1 | 11 |
| FRA | FRANCE (Democratic) — Difficulty_⭐⭐.docx | 101 | 8 | 7 | 83 | 3 | 11 |
| BEL | BELGIUM Achievement Guide_ Tour De France+Forge Of Victory Rush — Difficulty_⭐ (UPDATED).docx | 16 | 2 | 1 | 13 | 0 | 1 |
| GER | GERMANY (Communist) — Difficulty_⭐⭐.docx | 89 | 14 | 4 | 71 | 0 | 12 |
| AUS | AUSTRIA (Non-Aligned)_ European Union — Difficulty_⭐⭐⭐⭐.docx | 38 | 11 | 4 | 23 | 0 | 2 |
| Reference | How to Import My Equipment Designs.docx | 0 | 0 | 0 | 0 | 0 | 0 |
| JAP | JAPAN (Fascist)_ More Powerful Than Ever! — Difficulty_⭐⭐⭐.docx | 67 | 11 | 5 | 51 | 0 | 9 |
| ROM | ROMANIA (Fascist)_ The Glory of Romania is Eternal! — Difficulty_⭐⭐.docx | 62 | 7 | 4 | 51 | 0 | 13 |
