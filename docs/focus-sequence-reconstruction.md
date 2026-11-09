# Focus source sequence reconstruction

The importer combines its existing name/route resolver with a bounded constraint
solver. It never generates a strategy, observes the game, changes source evidence,
marks a focus completed, or inserts new runtime dependencies.

## Coverage and baseline

Guide Schema v1 and authoring v1 accept the additive `focus_sequence` field:

```json
{"schema_version":1,"coverage":"complete","initial_focuses":[]}
```

The default is `partial`: omitted prerequisites are legal, and a missing source
slot cannot be inferred by counting. `complete` explicitly asserts one ordered
source slot per focus instruction and requires an initial list (native IDs in the
selected tree). No campaign start date, Word heading, nation or active save implies
this assertion. The importer preview exposes this policy without editing JSON.

## Proof and uncertainty

For a complete list the solver retains AND/OR prerequisites, mutual exclusions,
initial focuses, source ordering, and distinct non-repeatable identities. It can
propagate information forward and backward across interleaved branches. Ten or
more unreadable slots are supported when the constraints identify their positions.

For each proposed binding it searches for a counterexample with a different ID at
that position. Only exhaustive failure to find a counterexample confirms the
binding. Text similarity is not proof. OR alternatives and independent AND
parents can remain ambiguous even when their membership is known.

Possible or unknown bypasses over-approximate the admissible solutions. Missing
relations, graph cycles, independent focus tracks,
focus-tree reset effects and deterministic work/size limits are explicitly
reported. On a search limit **no new sequence binding** is confirmed. Dynamic
availability, scripted effects, events and geopolitical outcomes are not simulated;
`validated` only means the structural ordering constraints admit a solution.

Conditional slots are retained and may contribute their named focus or no focus;
the solver never guesses a predicate's live truth. Known fallback identities are
retained as alternatives. Joint focuses may be completed outside the player's
instructions and cannot be forced into a source hole merely as missing ancestors.
Free-choice entries retain their source position and may contribute a focus or
none. They never acquire a generated recommendation, even if a structural witness
would need a particular focus there. A free choice can therefore prevent another
unreadable slot from being falsely confirmed.

Exact source names, explicit native IDs and manual overrides are retained. Weak
automatic name matches may be revisited using other plausible textual candidates;
an exact-name or explicit-ID contradiction requires review instead of silently
replacing the source's instruction.
When a conflicting exact name admits a one-slot repair, the preview can propose
a compatible candidate explicitly as **probable**, without applying it. This
could also indicate a mistaken completeness assertion; source evidence decides.

## Provenance and consumers

Each imported focus retains `focus_resolution` v1: original `query`, immutable
source `position`, current `entity`, `classification` (`direct`, `reconstructed`,
`probable`, `unresolved`) and optional manual `locked` flag. Revalidating an
automatic binding starts from its original query; an assigned ID is not new
independent evidence. Reordering preserved positions is rejected. Manual sidecar
corrections remain locked and do not rewrite the original guide or campaign.

The runtime and HUD receive only the ordinary validated focus IDs. They continue
to use qualified live facts for selection/completion. Reconstruction is identity
evidence only. Installed guide files and campaign progress are never rewritten by
previewing a source sequence. Python and JavaScript validate the same metadata.
