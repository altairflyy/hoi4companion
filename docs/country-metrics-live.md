# Effective war support

The current executable has SHA-256
`cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f`.
REA `open_binary` identified the installed file as PE x86-64. The evidence below
comes from local PE/RTTI inspection and Capstone disassembly of that exact file;
no Ghidra decompilation or execution of a game function is claimed.

## Static findings

`CHasWarSupportTrigger` has vtable RVA `280EF20`. Its numeric acquisition slot
23 points to `40BC70`; this resolves the selected country and calls `741BE0`
with its optional explanation string disabled. The shared comparator `143E1A0`
compares the returned signed integer to the trigger's parsed threshold.

The country getter starts with country +`10D8`, but that field is **base war
support**, not the effective support used by `has_war_support`. Publishing that
field divided by 100000 would silently omit requirements-relevant changes.
The getter also includes:

- War-state-dependent offensive/defensive contributions (`741D9D..741E96`).
- World tension obtained through `1F1FB0`, multiplied by a live define and
  divided with truncation at the script fixed-point scale (`741F95..741FD3`).
- Cached signed Q15 adjustments at country +`14C8`, +`14D0`, +`14D8`, +`14E0`,
  individually converted through `2572C70`, including native rounding.
- A history aggregate from country +`FC8` through `15B31D0`, converted and
  multiplied by a live define with its own intermediate bounds
  (`742324..7423AC`). This is not an optional tooltip-only computation.
- Country modifier index `149` from country +`5B8`, through `5A51A0`
  (`742CCE..742CEA`). Modifier lookup includes definition/context flags and
  can apply bounds; reading the raw matching row alone is insufficient.
- The native configured lower and upper bounds (`74312A..74314A`).

`CHasStabilityTrigger` uses vtable `280EDA0`, numeric getter `40BC20`, and
effective country getter `7404C0`. Country +`10D0` is likewise only its base.
Its effective calculation has additional distinct terms and is not qualified
by identifying the war-support layout.

These findings identify the calculation. The bounded search inspected the trigger's RTTI,
numeric getter, effective getter and directly relevant helper bodies. It did
not find a final cached scalar on this path; it does not prove that no cache
exists anywhere else in the executable.

## Guarded external decoder

`country_metrics_native.read_metrics` reconstructs the effective getter,
including all the numerical terms listed above. It does not
call the getter. Registration requires the exact executable hash and an
explicit `country_metrics_layout`; complete body digests for the effective
getter and relevant helpers check the static derivation.

The decoder validates the current world, selected country mapping and native
country identity. War predicates reproduce `DA4470(1)`, `DA3190` and `DA2F50`:
`CWarRelation` +`158` swaps attacker/defender tags +`10`/+`14`; country tag
aliases compare through the world +`340` mapping, as in native `C08580`.
Relations are shared by both participating countries. `DA4470(1)` specifically
consults the fixed +`14` country's diplomacy +`1F0`, even when that end is the
selected country. Offensive and defensive contributions can both apply in
different current wars. A cached war count alone cannot supply this predicate.
All relation endpoints, country identities and diplomacy owners are validated.
`CWorldThreat`
RTTI identifies the tension object. Country +`FC8` is `CLoopHistory`, whose
+`10` selects a `CLoopHistoryContainer`. Constructor `15B1A70` establishes the
container's +`38` owner as the history object passed by its queue. Allocation `15B2E50` gives
each raw row the same owner at +`18`. The container header, capacity, head,
wrap flag, row headers and selected scalar values are all bounded and
witnessed. Native `15B31D0` ordering and per-row saturation are preserved.

Modifier index 149 requires an ordered unique native map. A missing entry
returns zero, matching the native early return. A present entry also requires
the definition/context flags and reproduces the flag-controlled clamp.
Nonzero native context masks are unsupported and return unavailable. All
numerical dependencies and ownership/membership bytes join the existing watch
plan and final sample reread; changing a dependency invalidates the sample.

Stability, nonzero modifier context masks, malformed containers and
extreme arithmetic ranges are explicitly unsupported. They cannot return a
partial value or fall back to base war support. The guarded decoder is
statically derived and covered by synthetic semantic/fault tests; that alone
does not establish its live-game qualification.

## Runtime evidence and limits

An initial read-only acquisition on 1 November 2026 found no game process.
After a separate authorized agent loaded and paused the user's campaign, a
coherent VM_READ sample of HUN in process 15504 at native date 60799489 returned
`war_support = 0.87043`. That sample included two wars, a wrapped 30-row history,
native country modifiers, negative Q15 adjustments, and 184 mutable witnesses.
The loading agent independently observed **87%** war support in the top bar,
consistent with the decoder's **87.043%** at the displayed precision.

The same sample's base support was only **56%**. This directly demonstrates why
base support cannot substitute for the effective trigger value. The read-only
sample and the independent UI observation are retained separately in
`.local/country-metrics-live-sample.json`. Stability was visible as 78% but its
native calculation remains unsupported; no stability fact is published.

The native acquisition did not launch/focus the game, write process memory or
game files, invoke game functions, or advance gameplay. These observations
qualify the inspected build/path and this real sample, not every mod, native
modifier context or future version. Unknown and stale observations retain the
normal unavailable fact handling. Synthetic tests cover mathematical terms,
shared war relations, side swaps, aliases, native predicates, ownership faults,
unsupported contexts and invalidation witnesses.

A later integration check at 13:11 UTC read the same process at native tick
60782022. Direct `read_metrics` and standard `read_native_state` both returned
**0.74091**, with `wuw_HUN_renovato_imperii` current and
`wuw_HUN_integrate_bohemian_and_austrian_industries` completed. The earlier
0.87043 sample was at tick 60799489: the native timeline had moved backwards
between observations. The cause of that state/load change was not observed
by this reader. This is a different gameplay state, not a discrepancy between
the direct and integrated metric acquisitions. Its separate evidence is
`.local/country-metrics-current-comparison.json`; the earlier 87% UI comparison
does not apply to the later state.

The local finding ledger is `.local/country-metrics-qualification.json`.
Disassembly is retained in `.local/country-metrics-evaluators.log`,
`.local/country-metrics-getters.log`, `.local/country-metrics-full-values.log`,
`.local/country-metrics-helpers.log` and the matching scripts. Exact signature
bytes in the ledger preserve the investigation's evidence; the production
profile admission boundary remains explicit.

## Current-build ideas membership

The same exact executable also admits **ideas membership only** through the
existing politics section. Country getter `717370` selects +`F90`;
`CHasIdeaTrigger` (`3F4776..3F47AC`) tests the manager's +`50` array and +`5C`
count. The `CPoliticalStatus` constructor stores its country owner at +`C8`
(`BFA327`). All three byte signatures and the owner are required on this
profile. Advisors, political power and government identity remain unavailable.

A separate coherent read of the current HUN campaign returned ten complete
idea identities, including `HUN_hungarian_gendarmerie`,
`HUN_levente_associations`, `war_economy`, `export_focus` and
`limited_conscription`. This verifies the native membership path, not a
displayed politics panel. Complete membership, rather than a completed focus
or an expenditure, supplies presence/absence evidence. Current-build owner,
membership, signature and ideas-only restriction tests are retained in
`test_politics_current_profile.py`; live evidence is
`.local/country-metrics-ideas-live.json`.
