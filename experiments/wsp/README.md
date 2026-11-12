# WSP semantic parsing experiment for HOI4 Companion

**Not production-ready. No runtime integration. No pretrained weights bundled.**

This directory is a reproducible research harness for using genuine
**Weight-Space Programming (WSP)** to improve natural-language interpretation
of HOI4 country guides without extending country-specific rule tables.
It is separate from the application's native Rust importer and native HUD.

The semantic objective is to transform source-grounded prose into structured
**intent IR**, including multiple operations, targets, quantities, temporal
conditions, and unresolved ambiguity. Native game entity IDs and live facts
remain owned by the existing Rust binder and game reader.

The foundation must be a language-capable open-weight model such as
[Qwen3.5-0.8B](https://huggingface.co/Qwen/Qwen3.5-0.8B). The already-existing
seven-label character n-gram classifier does not possess the required language
semantics; adding weight deltas to that classifier is insufficient.

## Files

- `semantic_gold.jsonl`: 32 human-annotated examples, 16 development and 16
  separately held out; **smoke coverage only**, not statistical proof.
- `semantic_spec.txt`: typed source-to-IR requirements and anti-hallucination
  constraints. The model has no authority to resolve IDs or execute actions.
- `run_model.py`: local, deterministic baseline-vs-patched evaluation;
  no network calls, no dependency on the application runtime.
- `inspect_model.py`: inspect actual named neural weights, magnitude statistics,
  and optional MLP input activations before writing a proposed edit.
- `wsp_weights.py`: explicit numerical sparse and rank-one weight writes to
  selected 2D tensors; no gradient, optimizer or training loop. Validates all
  requested coordinates and enforces a weight-change budget.
- `semantic_eval.py`: strict semantic IR grader that detects invented fields,
  unresolved cases, incomplete output and exact-match regressions.
- `tests/test_wsp.py`: zero-model unit tests on synthetic tensors and IR.

## Test without a language model

```sh
python -m unittest discover -s experiments/wsp/tests -v
```

These tests verify that code writes **actual tensor entries** and correctly
rejects unsafe proposals. They do not validate an effective semantic model.

## Run real WSP on a pinned local pretrained model

Only in the developer environment, install a current PyTorch/Transformers
version supporting Qwen3.5, download the pinned official open-weight model,
and use your local checkpoint directory:

```sh
pip install -r experiments/wsp/requirements_research.txt
hf download Qwen/Qwen3.5-0.8B --local-dir models/qwen3.5-0.8b
python experiments/wsp/inspect_model.py --model models/qwen3.5-0.8b \
    --probe 'Inf->25' \
    --probe 'Put 25 factories on infantry equipment' \
    --out wsp-inspection.json
python experiments/wsp/run_model.py --model models/qwen3.5-0.8b \
    --split development --out-dir runs/wsp-base
```

The WSP editor must **reason about the actual inspected parameters and
measured failure examples**, then propose a concrete, sparse edit JSON:

```json
{
  "schema_version": 1,
  "max_changed_fraction": 0.0002,
  "edits": [
    {"tensor": "CHECK_ACTUAL_NAME", "op": "sparse_add",
     "entries": [{"index": [0, 0], "delta": 0.01}]}
  ]
}
```

This example is only the interface, NOT a patch that has been tested or should
be applied to Qwen. Do not make arbitrary changes to its weights.

Evaluate the same model before/after with `run_model.py --patch PATCH.json`.
A candidate is accepted only when semantic exact matches improve without
regressions among previously correct cases and structured output validity does
not worsen. Hold out `split=heldout` until the candidate is frozen; then expand
these 16 cases into at least 200 human-annotated examples split by guide file
before deployment decisions.

### Current status

The initial **direct four-parameter WSP editing** experiment was performed on
the previous 7-way classifier; it yielded development domain accuracy 12/15 →
13/15 and held-out domain accuracy 8/14 → 8/14. This tiny no-generalization
result is archived in the separately downloadable model lab, not shipped here.
**No Qwen3.5 language checkpoint was available or modified in this session**.
A successful real-language-model WSP experiment is still required.

No changes to native guide parsing, import APIs, game observation, runtime
or HUD are included in this research branch. Only a tested model producing
reliable semantic IR should be promoted into the existing review flow.

Research context: [Takehi et al.](https://rikiyat.github.io/blog/weight-space-programming/),
[limitations study](https://arxiv.org/abs/2611.09382).
