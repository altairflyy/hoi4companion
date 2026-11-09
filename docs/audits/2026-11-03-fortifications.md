# Provincial fortification goals — 3 November 2026

Previous behavior could recognize a queued fort at a resolved city/province,
but did not expose built province levels or aggregate a complete frontier. The
imported Hungary instruction was a reminder. This change adds a generic goal
to the existing Guide Schema/Runtime, without editing its source guide.

## Changes

- Parse explicit numeric/max fort levels, province references and every-tile
  country-border instructions. Preserve residual text, authored conditions and
  dependencies. The actual imported Hungary instruction now binds to
  `bunker`, target level 1, border `YUG`, with declared start/completion facts.
- Older imports already have substep keys. Retained remainder notes now use
  free note keys instead of colliding with/replacing those existing substeps.
- Resolve static land neighbors from the installed map and active map overrides;
  remove impassable edges. Join current observed controllers, not history-file
  ownership. Unknown or empty scope never proves completion.
- Read built provincial fort levels independently of the construction queue,
  with bounded identity, state registry, building definition and reciprocal
  ownership checks. Existing per-executable capability gates still apply.
- Evaluate total-level province/group goals and existing campaign baselines for
  single-province increments. Reuse queue qualification, run persistence and
  deterministic Python/JavaScript evaluation. No separate strategy engine.
- Add typed fortification progress, per-province UI counters, generic authoring
  location alternatives, corresponding prompt instructions, and correction of
  generated bindings without treating custom predicates as disposable.
- Scope lightweight controller probes to relevant territory/adjacency. A failed
  optional fortification probe budget cannot invalidate other country sections.

## Evidence

- The installed map produced **10,154 land vertices / 53,882 directed neighbor
  entries**, initially in approximately 0.75 seconds with NumPy. This is static
  map parsing, not live territory evidence. It is cached by selected file paths,
  sizes and modification times. Vertices absent from the bitmap remain unknown.
- The imported Hungary file remains unchanged, SHA-256
  `287d7d5f3188ae96dd0f70ff63eb5067994b64ff7cc7c12d964e98b6c1726f8e`.
- Full Python and JavaScript regression results are in
  `.local/fortifications-python.log` and `.local/fortifications-js.log`:
  **1,151 Python tests and 431 JavaScript tests passed**, with none skipped.
- Shared fixtures cover readiness, queue start, partial building, cancellation,
  completion, missing levels, stale state, explicit province sets and increments.
  Additional checks cover source keys, conditions, bounds, ownership, native
  registry faults, correction of generated criteria, repairs/conversions and
  isolation of an oversized optional probe plan.
- A real isolated headless Edge page rendered the actual walkthrough modules:
  the synthetic native transport moved the counter **1/2 → 2/2**, updated the
  completed state through the live stream, and produced **0 JS exceptions** and
  no horizontal overflow. This used owned fixture data, not a game session.
- Archive audit: **61 country guides, 47 ready, 14 DLC-unavailable, 0 requiring
  import revision**. DLC-unavailable is not classified as an import error.

## Remaining qualification

**No real game session was observed.** HOI4 was closed. The installed executable
`cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f`
does not enable the fortification country section: its provincial layout must
still be qualified. On that build the framework deliberately shows unknown
progress, rather than claiming automatic completion from unsupported memory.
The native structural tests qualify synthetic layouts only.

No game files/memory, imported guide definitions or existing campaign progress
were modified. Only owned browser/test-server processes were closed. No release,
commit, game launch, construction HUD or automatic game click was performed.

Intervention counter: **4 areas implemented** (instruction recognition and
legacy adaptation, geographic scope, qualified level/progress criteria,
presentation and authoring integration); **1 live qualification remains** for
the installed executable. This is not a count of all outstanding project work.
