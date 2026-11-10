# Live country data

The country view presents qualified divisions, manpower, states, industry, production, equipment, resources, politics, events, and decisions when supported. Every section reports its evidence quality and acquisition limitations.

Observed process values differ from calculated totals. A national division total does not prove template, front assignment, or training state. Requested, assigned, active, and damaged factories have different meanings. Unsupported or stale fields must not be displayed as current zeroes.

The Rust reader also exposes these independently qualified extensions:

- `states.items.<id>.buildings` contains catalog-classified state building levels. Province-only buildings use `fortifications.items.<province>.buildings`; the historical fort fields and four regional industry fields remain compatible. Generic construction goals use the same initial-count/queue/completed-count rules as factories. A missing building definition is unknown, not zero.
- `politics.characters` contains native character role status (country leader, advisor, unit leader, scientist). Role inventory is separate from `politics.advisors`, which remains the appointed-advisor membership. Neither inventory nor a role flag proves army/fleet assignment, affordability or eligibility for appointment.
- `diplomacy.outgoing` and `diplomacy.incoming` contain `relation_kind:COUNTRY` memberships for qualified existing relations. Native primary RTTI must prove the shared nonvirtual `CRelation` base; direction, owner, endpoints and cancellation are checked. Guarantees, embargoes, attachés and lend-lease orders can use the common foreign-task binding. These records do not prove a pending offer was accepted or every prerequisite for a new diplomatic action.
- `decisions.executed` means native evidence of activation/taking; `decisions.active` contains ongoing timed decisions; `decisions.completed` requires a native completed timed state. The serialized `decisions_taken` list alone does not prove a delayed outcome finished. Failure, abortion and cooldown remain distinct. Random counters never prove execution. Targeted outcomes remain unavailable until their exact target can be qualified, and repeat counts/history before observation are not invented.
- Event discovery derives the owner from the actual secondary RTTI displacement and then rechecks the owner, GUI root and event/options. No window found remains unknown, rather than a verified empty event list. Closing a window does not prove a selected response.

Existing explicit guide conditions are preserved. Static runtime adaptation refreshes supported missing bindings in memory; it does not rewrite imported guide files. Training queues, net logistics demand/balance and universal dynamic eligibility remain outside qualified acquisition. Tests of synthetic native memory and shared Rust/JavaScript contracts are separate from actual-game visual validation.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/country-live.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
