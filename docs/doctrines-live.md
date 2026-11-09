# Live doctrine selection

The external observer reads selected grand doctrines and subdoctrines on the
exact executable profile `MILITARY_HASH` (`cf781ed1…f10476f`). Other builds remain
unavailable. It never invokes native functions or writes to the game.

## Native evidence

The pinned PE's `CHasDoctrineTrigger@NDoctrines` evaluator at RVA `572A30`
iterates country folders (stride `50`), compares the selected grand definition
at `+10`, then selected subdoctrine definitions at track `+8` (stride `60`).
The country getter at `1460C71` uses the current world's system at `+400`;
`DE4EB0` selects the country status from the system array at `+8`, stride `A0`,
using the world's tag-to-country mapping. Status construction at `DE2099`
establishes the owner tag at `+8`. The observer rejects an absent mapping or
record instead of reproducing the native fallback to the first record.

Definitions carry atom identifiers at `+8`. Resolver `253EB90` validates the
initialized atom table (`368F570`), its maximum index (`368F574`), and resolves
strings from `368F5A0` with stride `20`. Function fingerprints, RTTI types,
country/session ownership, bounded containers, unique identities and witnessed
reads qualify the complete selection. No partial list is published on failure.
Unused progress fields are not added to the change watch.

## Contract and runtime

`country_overview.doctrines = {status: "observed", items: [{id: "…"}]}` is native
selection membership, not availability, mastery level, or historical adoption.
Python and JavaScript adapters publish the derived `doctrines.selected` fact
only for complete valid observations belonging to the selected country.
Missing, partial, malformed and foreign observations remain unknown.

The existing runtime already supports this fact and campaign-bound milestone
history. Combined with qualified active advisor membership, it restores
production phase admission without changing guides or treating unknown
prerequisites as satisfied. Completed admitted quotas remain HUD targets with
`goal_satisfied: true`; production matching and rendering continue to use the
shared existing engine.

## Verification and limits

On the running Hungarian campaign, read-only samples resolved active advisors
and `grand_battleplan`, `grand_assault`; a later coherent sample also observed
`assault_infantry` after the player progressed. A full state evaluated against
the active guide admitted six satisfied production quotas and the pending tank
recipe. The later production phase conditioned on Italy remained blocked.
Evidence is retained locally in `.local/production-readiness-live.json`.

Synthetic tests cover profile/signature rejection, ownership and mapping,
container/type/token corruption, empty selections, duplicates, watch
invalidation, Python/JavaScript contract parity and production/HUD projection.
These tests are not visual proof of alignment with production widgets. This
change does not qualify political power, doctrine availability/costs, historical
adoptions before the observer began, or other executable versions.
