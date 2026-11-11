# Guide ACTION intent model (experimental)

The repository now provides an **optional, read-only local classifier** for the
ACTION column of imported Word priority tables. It assists review, and does not
replace the current Rust importer or guide runtime.

## Why this integration is intentionally narrow

The 9–10 November 2026 native cutover removed the resident Python runtime.
The existing Rust modules (`word_sources`, `operation_language`,
`informal_targets`, `research_import`, `focus_routes` and
`import_preview`) already implement structured interpretation, catalog matching,
evidence checks, and the critical distinction between an instruction and an
observed game fact. They remain authoritative.

The model is a **seven-class linear classifier**, trained on the user's English
country-guide ACTION lines with *weak, automatic labels*. It predicts only
`AIR_NAVY`, `ARMY`, `CONSTRUCTION`, `DIPLOMACY`, `ESPIONAGE`,
`POLITICS`, or `PRODUCTION`.

Implementation:

- `web/guide-intent-model.json` — 800 character n-grams, per-feature IDF,
  per-class int8 weight rows and seven biases; compact, fixed-size local asset.
- `web/guide-intent.js` — zero-dependency browser/Node inference, TF-IDF
  character 2–5-grams, normalized sparse dot products, no HTTP inference API.
- The Word importer shows a review-only panel for up to 40 reminder lines
  after draft preparation. It never rewrites those lines, changes
  authoring JSON, bypasses ID association, or installs a guide automatically.
- The standalone Rust HTTP server exposes **only this one named JSON asset**,
  not a generic `.json` static-file route.

The model was trained outside the repository on an archive of 55 English guide
documents. Labels are weak supervision and therefore agreement with their
generating heuristics does **not** establish human-level semantic accuracy.
The earlier larger prototype reported 92.69% role accuracy against source
columns and strong intent agreement against weak labels, but those figures
**do not measure this compact classifier end-to-end in Companion**. The
compact classifier has not yet been calibrated against a human-annotated
held-out corpus. Margins are score differences, **not probabilities**. Its
`requires_review` flag is a conservative heuristic, not a safety guarantee.

## Verify

```bash
npm test
# Or only the new tests:
node --test tests/test_guide_intent.cjs
```

For live integration, open **Import guide**, select a Word guide/ZIP, and use
**Prepare selected guide**. Expand **Model suggestions for Word ACTION reminders**.
Compare the predictions with source instructions and preview results. Existing
Rust rules, native catalog IDs, country/DLC constraints, UP progression and
live-evidence checks still govern all actual steps.

## Next development step

Collect corrected `(text, expected ACTION category)` examples independently
from guide authors. Preserve separate guide-level train/test partitions; include
out-of-domain and paraphrase tests. Retrain offline and version the quantized
artifact. Only after independent precision/recall and ambiguity calibration
should predictions influence candidates for **review**, and even then never
directly produce executable or automatically completed steps.

**This is not Weight-Space Programming:** no pretrained LLM weights were edited.
It is compact supervised linear inference, deliberately compatible with the
native Rust architecture and safe to disable.
