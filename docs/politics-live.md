# Political state and appointments

Laws, ideas, advisors, commanders, chiefs, and high-command roles have distinct identities/categories. Observe actual selections and role assignments; roster membership alone does not prove deployment or appointment.

Political power is exposed only through a qualified observation. A guide-declared cost is not automatically the current dynamic price. Unknown/insufficient resources suppress relevant HUD recommendations while preserving the first unfinished action in guide order. Neither clicking nor a PP decrease proves success.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/politics-live.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.

## Current-build active advisor membership

Active appointment membership is separately qualified on executable SHA-256
`cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f`
(`MILITARY_HASH`). This does not enable the political-power decoder or change
the older `UPDATED_HASH` profile's restrictions.

Static evidence was reconstructed from the exact local PE bytes and Capstone
instruction decoding, without invoking native game functions:

- RVA `734AD0` obtains `CCountryCharacters` through country `+FF0`.
- Constructor `11703D0` assigns the country owner at manager `+8`.
- `CIsHiredAsAdvisor` (RTTI virtual slot 22, entry `4611B0`) calls `14DD120`.
  The latter resolves the advisor's character owner and country, then calls
  `2E9E50`, which returns manager `+58`, and tests exact pointer membership in
  that array. This distinguishes hired appointments from the `+40` available
  advisor list used by named-candidate lookup at `1173000`.
- `CAdvisor` constructor `132DA20` sets character owner `+10`, the raw native
  slot field `+28`, and the identity string `+30`. Accessors `12BB80` and
  `707BD0` corroborate owner and identity. `ABC220` supplies the character's
  country tag field `+68`; accessor `D73120` supplies character identity `+48`.

The reader validates nine exact byte fragments from these paths before reading
current-build appointments. Runtime validation requires manager/role/character
RTTI, country owner/tag equality, bounded complete arrays, valid strings and no
duplicate appointment identities. Every dependency is re-read for coherence and
included in the existing change watch. Unqualified or inconsistent data never
publishes a partial member list. Native slot integers are retained as raw data;
their numeric values are not mapped to role names by this extension.

Read-only background qualification on the real Hungarian campaign produced
three coherent samples with these active identities: `HUN_kalman_kanya`,
`HUN_ivan_hindy`, `HUN_lajos_remenyi_schneller`, `HUN_shvoy_kalman`,
`HUN_hugo_sonyi`, `HUN_ferenc_fischer`. A preceding sample contained five entries
before `HUN_ivan_hindy` appeared; no input was sent to the game. This establishes
current native membership, not a history of past hires or a visual UI comparison.
Fixture tests cover current/legacy profiles, signature failure, foreign owners,
available-versus-active lists, duplicates and same-tick coherence/watch changes.

Local evidence is retained as `.local/politics-production-signatures.json`,
`.local/politics-production-live-qualified.json` and focused static disassemblies
under the same prefix. No game file, save or memory was modified, and no native
function was called.
