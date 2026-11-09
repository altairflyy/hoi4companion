# Simplified guide authoring v1

External AIs write `hoi4-guide-authoring` v1, with sources and typed `guide.steps`. The importer resolves references and compiles this simpler format into internal Guide Schema v1. Existing `hoi4-guide-import` files remain supported.

Supported types include focus, research, build, production, special_project, law, idea, appoint, event, decision, intelligence actions, MIOs, templates/designs, army, diplomacy, instructions, reminders, and explicit free choices. Type support structures intent; it does not imply every action is observable live.

Diplomacy and known operative assignments now have qualified live bindings;
see [foreign interaction monitoring](foreign-interactions-live.md). Existing
`diplomacy.task`/`country` and `intelligence_operation.operation`/`country`
fields suffice. Assignment, network strength and covert-operation outcome are
distinct goals; only supported observed criteria may complete an instruction.

The complete English creation contract, examples, limits, and evidence rules are in the [packaged creation prompt](../src/hoi4companion/guide-import-prompt.md). Keep source_inventory separate from runtime progression. Never infer a completion dependency from narrative proximity.

New authoring documents may declare `guide.dependency_policy: "explicit"`.
Only a declared `depends_on` or `depends_on_started` then gates a step:
the former requires completion, the latter accepts an observed start or
completion. Conditions still need qualifying evidence; missing live data never
confirms them. Record a real dependency from the source rather than assuming
that adjacent actions are sequential. Existing documents without this marker
keep their legacy implicit manual-action barriers. The import preview reports
potential barriers; it does not change the original document or claim that they
are currently blocking a campaign. Choosing `explicit` cannot reconstruct
conditions or dependencies missing from a source.

Import compatibility accepts a single evidence object as a one-item list and
maps known descriptive categories to the existing UI groups (for example,
`mio` to `production`, `intelligence` to `projects`, `doctrine` to `army`). The
original label remains in `category_hint` and is displayed with its UI group.
Category labels are optional, case/spacing tolerant descriptors. Unknown labels
are retained under general instructions, with a nonblocking explanation in the
preview warnings. A typed action owns its group; a supplied category cannot
change its identity or monitoring. This does not turn an instruction into a typed action.
Both a single instruction sentence and a list are accepted. Empty or nontext
instructions/categories, invalid identities, conflicting values and declarative
conditions remain errors. Multiple independent step-shape errors are reported
together (up to 32 messages), without returning a partially compiled guide.
Army `details.quantity` and design `details.role` become descriptive
`division_count` and `design_role`, avoiding collisions with construction and
appointment bindings. Conflicting values are rejected.

An explicitly labelled video range in a phase's textual `when` is retained as
`timing`, accepting capitalization, spacing and dash variants without requiring
a special disclaimer. Invalid/reversed ranges and other textual conditions
remain errors. Real declarative conditions
are unchanged. Inventory evidence may support several distinct actions, while
duplicate links to the same action and unsupported repeated untimed labels
remain review issues. Long catalog names may omit a purely numeric model suffix;
multiple matching models remain ambiguous and explicit IDs are never guessed.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/guide-authoring.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
