# Simplified guide authoring v1

External AIs write `hoi4-guide-authoring` v1, with sources and typed `guide.steps`. The importer resolves references and compiles this simpler format into internal Guide Schema v1. Existing `hoi4-guide-import` files remain supported.

Supported types include focus, research, build, production, special_project, law, idea, appoint, event, decision, intelligence actions, MIOs, templates/designs, army, diplomacy, instructions, reminders, and explicit free choices. Type support structures intent; it does not imply every action is observable live.

The complete English creation contract, examples, limits, and evidence rules are in the [packaged creation prompt](../src/hoi4companion/guide-import-prompt.md). Keep source_inventory separate from runtime progression. Never infer a completion dependency from narrative proximity.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/guide-authoring.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
