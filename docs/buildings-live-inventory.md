# Live buildings: capability inventory (2026-11-16)

## Current implementation milestone

Runtime construction text binding now recognizes fully understood `Build: A+B
in X,Y` checkpoints, verifies building identities/scopes and locations against
the actual catalog, and retains original keys and text. Compound completion
requires all targets. Fixed non-shared caps are sourced from definitions;
shared-slot factory maxima stay unqualified, with no inferred completion.
The existing Rust construction projection exposes each target's built level,
location, declared target and queue membership; the guide card displays them.
The real installed SOV regression resolves the Moscow infrastructure/facility
instruction to state 219 / province 6380 and levels 5 / 1, and the six-state
infrastructure/military-factory instruction to 12 separate targets.

Validation: backend lib 150 passed / 2 ignored, contracts 10, guide evaluation
44, real-catalog construction regression 1, JS presentation/model/plan 27.
New code is not installed in the running user app; no new live HUD rendering
test or construction/cancellation transition was performed in this milestone.
Historical limitations below retain their original observation dates.

This is an inventory, not a claim that every native layout or mod is qualified.
The 8766 listener was verified as the existing Companion backend (PID 35348),
with HOI4 PID 64832 and the Companion overlay running. Only the existing
read-only state API was queried; the game remained in the background. The
installed game executable SHA-256 was
`709f916afbebcfe64a26678702691aeb40945d206074c2a80de8e9d601fce148`.

| Family | Implemented | Observed in this session | Unknown / limitation |
|---|---|---|---|
| Regional buildings: infrastructure, civilian/military factories, dockyards and other state-scoped types | `country::states` joins the native definition registry and catalog scopes; validates indices, templates and levels; legacy fields retained. | API reports 152 SOV owned/controlled states and 15 building keys in the first state. | Values were not independently compared with the game UI; completeness and active-mod agreement are not established. Catalog scope inference depends on `level_cap`. |
| Provincial buildings: forts, naval bases, facilities and catalog-scoped types | `country_forts` validates province/state and building membership; emits levels per province and state; section has its own watch budget. | Section unavailable: `Native observer: invalid packet or bounds`. | Cause not established; no provincial levels or absent-building zeros can be claimed live. Prior qualification limitations in [construction-live.md](construction-live.md) still apply. |
| Construction orders | Native queue validates production owner, line, template and location; reports remaining amount, built level, active factories, repairs and conversion separately. | API reports 19 SOV lines: 15 military-factory and 4 infrastructure lines, all with section status observed. | No independent UI comparison or live cancellation/completion transition; construction progress is not exposed. A vanished line is not completion. |
| Special-project facilities | Provincial building reader and separate project/facility/scientist readers exist. | Projects section observed 45 project records and one `land_facility` assignment with a scientist and `busy: false`. | Facility building level/location unavailable with provincial section; `busy` is a native selection flag, not proof of full operational status. No active project association verified in this sample. |
| Other or mod-defined buildings | Catalog derives scope from loaded definitions, and native registry supplies template identities. | Several additional state-scoped IDs appeared in the API. | Exact loaded-mod catalog parity, definitions without a recognized scope, replacements and modded examples are not qualified. Unknown scope must not be interpreted as an observed zero. |

The API labels the overall sample `experimental`; observed means reported by the
currently running binary, not independently verified against the game display.
The source has a separate issue to investigate: `construction_binding` treats a
building without `location_scope` as non-provincial when constructing a
state-specific queue predicate. Do not infer a state scope from missing metadata.
No guide parsing or evaluation was changed in this inventory.

Next: reproduce the provincial packet failure with bounded instrumentation and
the existing observer limits, identify whether it is a packet limit or another
cause, then fix and test without relaxing qualification. Qualify catalog/mod
agreement and individual live families before reporting coverage. Keep built
levels, queue quantities, progress, project selection and operational state
separate; do not infer completion from an absent order.

User clarification (2026-11-16): an instruction without a level means the
maximum currently available; a land facility is a single building. A static
definition's numeric cap alone does not establish the currently available
maximum for a shared-slot or conditional building. A uniquely resolved,
provincial, single-level specialized facility can target built level 1; other
unspecified maxima remain unverified until their effective limits are observed.

Follow-up: an isolated backend built from the changed source, on port 8775,
reported the provincial section observed for 2,414 provinces in the same SOV
session; land facility level 1 was reported in province 6380 / state 219.
Three further API reads retained the section and count. This is runtime API
evidence, not an independent game UI comparison or a cancellation/completion
test. Full API acquisition timings were approximately 710–798 ms in these
reads, so HUD responsiveness under this larger watch remains unverified.
The isolated backend was stopped; the user's existing app was not restarted.
