# Universal Guide Runtime

`guide_runtime.evaluate(guide, state, progress)` and `HoiGuideRuntime.evaluate(guide, state, progress)` interpret a declared guide deterministically without generating strategy or performing game actions. They evaluate dependencies, completion predicates, conditional branches, priority, and parallel actions.

Step states distinguish completed, active, ready, waiting, not applicable, and unverifiable. `depends_on` requires completion; `depends_on_started` permits a qualified start or later completion. Do not reinterpret existing completion edges as start edges. Unknown facts never complete a step.

Political sequences retain the first unfinished action even if a later action is cheaper. Resource readiness is separate from observed execution. Manual declarations remain identified as manual. Construction baselines and guide/campaign identity must be isolated across runs.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/guide-runtime.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
