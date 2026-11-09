# Live construction monitoring

Construction queue entries provide evidence that an order was started; they do not prove completion. For “build two more factories”, capture a qualified initial built count when the step becomes available, before the order, and compare the current built count against baseline plus two.

Removing a queue entry can mean cancellation. Never complete a construction step from disappearance alone. State and province identity, building type, requested quantity, and observation qualification must agree. Unknown counts remain unknown, not zero.

## Provincial fortifications

Land (`bunker`) and coastal (`coastal_bunker`) forts support an explicit province,
a documented province set, or every current land-border province against a
country. A total-level goal completes only when every selected province has an
observed built level at least equal to the target. An empty/unknown border never
completes vacuously. A state-wide sum cannot satisfy a per-province target.

For example, `1 fort in each tile at YUG border` becomes a construction order
with `province_selector:{kind:"border",country:"YUG"}` and `target_level:1`.
The generic authoring equivalent is:

```json
{"type":"build","building":{"id":"bunker"},"border":"YUG","quantity":{"mode":"total","value":1}}
```

Use `provinces:[...]` for an explicit set, or the existing `province`/city
reference for one tile. For an additional level count in one province, campaign
baselines use the existing run-relative construction mechanism. A baseline
cannot be reconstructed from queue disappearance or captured after the order.
For a whole border, specify the required final level, not an invented increment.

Static neighbors come from the installed province bitmap/definitions, including
map-mod overrides and impassable-edge removals. Sea crossings are not land
frontiers. Static topology does not provide ownership: it is joined with fully
qualified native controllers, never initial historical owners. The target is
the current border; a changed border is reevaluated. Missing topology or
controller evidence leaves the target unverified.

An eligible new-building queue entry means **in progress**. Repairs, conversions,
wrong provinces and wrong fort types do not count as starting the order.
Completion uses built levels even after the queue is empty. The walkthrough
reports, for example, `3 / 8 provinces at level 1`, separately from factory
baselines. Existing source keys, dependencies and conditions are preserved when
adapting old Word imports; leftover instructions remain separate reminders.

The provincial observer is read-only and bounded. Full audits qualify province,
state, building definition and reciprocal status ownership. Cheap probes watch
own/adjacent controllers, state transfers and own building levels/membership.
Topology reload creates a fresh observer context. If its probe budget cannot
fit, only the fortification section becomes unavailable; other live sections
remain usable. Native build capability gates are not relaxed.

**Current verification limit (3 November 2026):** the installed executable
`cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f`
does not yet have provincial building acquisition qualified. Its fortification
goals remain **unverifiable** until this native layout is qualified on a real
session. Implementation/fixtures/browser proof are not evidence of working live
fort counts on that build. No construction HUD geometry or automatic clicks are
added by this feature.

Regression evidence is recorded in
[the fortification audit](audits/2026-11-03-fortifications.md).

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/construction-live.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
