# Contextual operational summary

The summary explains existing runtime output and qualified LiveState. It does not introduce another rule engine, generate strategy, or change completion state. It compares the guide goal with current observations and links to the relevant instruction/tree.

Examples include requested versus assigned factories, construction in progress, waiting dependencies, and explicitly unobservable resources. A line requesting ten factories while only three are assigned is different from requesting three. Unknown requirements should explain what is missing rather than claim failure or success.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/operational-summary.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
