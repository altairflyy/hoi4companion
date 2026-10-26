# Qualified LiveState

The live reader observes supported process structures passively. Adapters expose a versioned LiveState with identity, session/freshness, typed facts, sources, and reasons. Observed, calculated, and unknown values must remain distinct.

The guide runtime and HUD consume qualified state rather than arbitrary raw offsets. Invalid identity, stale samples, partial reads, and unsupported layouts must not be bypassed. A process being open is not proof of a valid campaign. Keep save/snapshot support separate from memory-derived live evidence.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/live-state.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
