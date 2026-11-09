# Foreign interactions and operative assignments

The existing Guide Schema v1 and Guide Runtime now consume qualified foreign
interaction facts. No automatic input, save parsing, game-file edits, memory
writes, separate polling loop or nation-specific strategy is introduced.

## Supported intents

Diplomatic orders cover starting/improving relations, stopping that activity,
and an established non-aggression pact. Improving relations is directional:
another country improving its opinion of the player does not satisfy the
player's order. Pacts are bilateral. Cancelled relations do not count. Merely
sending an offer, spending political power or increasing an opinion value is
not execution evidence.

Operative assignments cover building an intelligence network, maintaining a
quiet network, diplomatic pressure, controlling trade, propaganda, boosting
ideology, counter-intelligence and rooting out resistance. An agent must be
actively performing the specified mission against the specified country.
Captured, cooling-down, disbanding, killed and operation-reserved agents do not
prove a currently active assignment. An assigned mission is not proof of a
network-strength goal or a covert operation's success. Quantified network goals
remain unverified; this reader does not expose network strength or operation
outcomes.

Creation-start and agency-created flags remain separate. They are also enabled
for the currently qualified executable. Agency upgrades are not inferred from
those flags.

## Contracts

`country_overview.diplomacy.interactions` independently qualifies `countries`,
`improving_relations` and `non_aggression_pacts`. The adapter exposes calculated
membership facts `diplomacy.countries`, `diplomacy.improving_relations` and
`diplomacy.non_aggression_pacts`.

`country_overview.intelligence.missions` independently qualifies `countries`
and operative items `{id, mission_id, target_country, state}`. The adapter
exposes `intelligence.countries` and `intelligence.active_missions`, whose
members have the form `diplomatic_pressure:CZE`. Multiple operatives can perform
the same mission; membership is deduplicated only after validating unique
operative identities. The native `no_mission` state is never an active task.

Complete observed empty lists prove absence. Unsupported/partial domains,
malformed rows, duplicated identities, unknown mission types, stale samples and
wrong-country samples remain unknown. Python and JavaScript enforce the same
constraints. The generic legacy flattener does not bypass these adapters.

## Importing and campaign progress

The existing authoring types are sufficient:

```json
{"key":"relations","type":"diplomacy","task":"improve relations","country":"Czechoslovakia"}
```

```json
{"key":"pressure","type":"intelligence_operation","operation":"diplomatic pressure","country":"CZE"}
```

The resolver accepts an exact country tag or a unique installed country name;
it does not guess geopolitical identities with fuzzy matching. Word commands
such as `REL + NA CZE, cancel REL, REL+spy pressure ROM` use the same bindings.
Plus-separated actions remain parallel; comma-separated complete clauses retain
their explicit local order. A cancelled REL clause can inherit its target only
from the preceding clause in that same command. Supported comma-prefixes can
be separated from untouched method notes. A compound source cost is preserved
as source text rather than charged to every child action.

Original Actions block positions, evidence, custom predicates and existing
substep keys are retained. Standalone timing headings apply to all generated
children. Unresolved timing remains a gate, not permission to execute now.
No imported guide file is rewritten.

Execution observations use the existing campaign milestone persistence,
scoped to guide, campaign and qualified session. Reassigning an operative or
stopping relations does not erase an earlier observed assignment. Future gated
steps cannot create history merely because their desired state currently
matches. This does not reconstruct actions that happened before observation.

## Native acquisition and verification

The added capability is pinned to executable SHA-256
`cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f`.
Other builds retain their existing capability gates. Ownership, country map,
relation type/token, agent handle, mission enum/class and reciprocal mission
container are validated. Reused civil-war tag names are accepted as aliases
only if they resolve to the same country object; ambiguous targets are not
guessed. Consumed bytes are revalidated; changed relation membership,
cancellation, targets, agency roster or mission state use the existing native
watch/SSE pipeline.

The 6 November 2026 read-only real-session check loaded the latest save in the
background. The played HUN agency was created, with two active resistance
suppression agents; its improving-relations and pact lists were observed empty.
A diagnostic across 439 world countries additionally found outgoing relation
improvements, active network-building and counter-intelligence missions. Those
foreign-country observations are structural verification, not player progress.
Five unchanged provider probes reused one full sample. No orders were issued.

Positive established pacts and diplomatic-pressure assignments were absent
from this save. Their layouts and behavior have static/synthetic coverage, but
their positive real-session and visible-screen comparisons remain pending.
Other mission types without a positive world sample have the same limitation.
There is no new diplomatic/operative button geometry or HUD alignment claim.

Evidence and test results are recorded in the
[follow-up audit](audits/2026-11-06-foreign-interactions.md).
