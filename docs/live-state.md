# Qualified LiveState

## Patch-tolerant executable qualification

The reader no longer requires a known executable hash for every small patch.
Known hashes retain their original verified layouts. Other PE64 builds are
qualified once when opened using the field-bearing code signatures in
`live-abi-v1.json`, unique primary RTTI and relocated global references.
Only instruction displacements explicitly marked in the signatures may vary;
field offsets and writable-global constraints must still agree. Ambiguous or
missing core evidence rejects acquisition. Optional domains are qualified
independently and report unavailable rather than borrowing unverified offsets.

The immutable profile records structural qualification separately from a
successful live sample. Country identity, ownership, bounds, executable bytes,
campaign identity and repeated-read coherence checks remain mandatory. No
version string, release date or similarity heuristic admits a layout. This
tolerates compatible patches; an actual ABI change still requires new evidence.

Country domains are independently witnessed observations for the same verified
process, world, country and campaign. Each observed domain carries
`observed_native_tick` and `observation_scope: verified_domain`; the enclosing
sample declares `country_observation_scope: independently_verified_domains`.
The reader does not claim that all counters were frozen at one simulation tick.
Changed domains are refreshed once and withheld if their own repeated-read
verification fails. A ticking clock or a changing production counter does not
invalidate otherwise qualified focus/research observations. Core identity and
ownership checks remain mandatory across the complete acquisition.

The 6 November 2026 addition observes foreign diplomatic relations and operative
assignments on the separately pinned current profile. See
[foreign interactions](foreign-interactions-live.md) for contracts and the
positive cases still awaiting a real-session comparison.

The exact executable inspected on 31 October 2026 additionally has an experimental
core/campaign and production/experience/diplomacy profile. See
[military live observations](military-live.md) for static evidence, field guards
and the outstanding real-campaign validation. This is not a blanket compatibility
claim for every native domain on the newer executable.

The live reader observes supported process structures passively. Adapters expose a versioned LiveState with identity, session/freshness, typed facts, sources, and reasons. Observed, calculated, and unknown values must remain distinct.

The guide runtime and HUD consume qualified state rather than arbitrary raw offsets. Invalid identity, stale samples, partial reads, and unsupported layouts must not be bypassed. A process being open is not proof of a valid campaign. Keep save/snapshot support separate from memory-derived live evidence.

## Executable updated on 27 October 2026

The PE with SHA-256 `b6650c85002374218e60598bd04aaa26cbad996fedce77126fcf92fc6bd6d888`
has a separate experimental profile: the world pointer is at RVA
`0x33dd6b0` and the technology-template vtable at RVA `0x29b9ee0`. These values
are specific to that exact executable. An offline RTTI search and a read-only
process observation identified the world and technology types. The user confirmed
one paused Poland sample: `POL_assemble_the_regency_council` and the three active
researches `electronic_mechanical_engineering`, `basic_machine_tools`, and
`interwar_artillery`. A subsequent coherent sample and a no-change watch probe
also passed. This does not validate every focus, completed technology, transition,
mod, country, or future game version.

Additional read-only Poland probes matched the user's confirmation of 40
divisions, zero available manpower, economy/trade/conscription laws, the
Katowice building levels and the first production assignments including
requested versus assigned factories. Those domains are enabled individually;
political advisors and political power remain unknown. The campaign serializer
signature and token are at `0x209022` and `0x34d0ad0` for this build; one valid
UUID/seed pair was observed, but switching campaigns and persisted progress
have not been verified on this build.

Native GUI geometry, construction, equipment stocks, resources, intelligence,
events, decisions and projects remain disabled until independently verified.
Missing fields remain unknown; HUD alignment and automatic progress depending
on those fields have not been validated. No game files or game memory were modified. REA identified the PE;
its Ghidra overview did not complete within the provider timeout, so no
decompilation result is claimed.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/live-state.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
