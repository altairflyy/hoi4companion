# Framework contracts v1

The shared contracts cover entities, qualified facts, declarative conditions, actions, guide definitions, and campaign progress. Python and JavaScript validators must agree on accepted structures, dependency references, cycles, and evidence.

A fact distinguishes observed, calculated, and unknown values and carries source/reason information. Unknown is not false or zero. Guide definitions describe intent; LiveState describes current observations; GuideProgress stores per-run manual declarations and captured baselines. Neither guide text nor missing observations can masquerade as live evidence.

Application-language translation applies to UI messages. Entity IDs, semantic status codes, guide definitions, source quotations, and game localization are separate contracts.

An observed political appointment is identified by its native role `id`. A
character display name must never become an inferred character identifier.
When only that label is available, the appointment carries `character_id: null`,
`character_name`, and `character_identity_status: unavailable`. Qualified role
membership remains usable; character-specific identity remains unknown. Missing
or malformed identity metadata does not qualify membership.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/framework-contracts.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
