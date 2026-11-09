# Military experience and war state

The country observer exposes `country.experience.army`, `.navy`, `.air` and
`country.diplomacy.at_war` through the existing qualified LiveState. Unknown,
partial, foreign-country and stale observations cannot authorize expenditure
or satisfy an after-war condition. Experience balances are independent; zero
is an observed value. The existing action readiness and HUD resource checks
consume these balances without a second acquisition loop.

## Native layout and evidence boundary

The executable inspected on 31 October 2026 has SHA-256
`cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f`.
REA identified the installed PE x86-64. Its Ghidra string query timed out during
analysis; no Ghidra decompilation result is claimed. The findings below come
from read-only PE/RTTI inspection and Capstone disassembly of that same file.
These are static findings, not proof of a loaded campaign or visual alignment.

| Field | Static evidence (RVAs) |
| --- | --- |
| Experience manager | `717170` returns country +`1588`; RTTI is `CCountryExperienceStatus`. Constructor `1335EC0` sets owner +`8`. |
| Army / navy / air balances | Script bindings `158A011`, `158A101`, `158A1F1` select getters `1590B00`, `1590B30`, `1590B60`. They read signed 64-bit values +`10`, +`28`, +`40`. Serializer `1339040` independently pairs them with tokens `3392`, `3393`, `3394`. |
| Experience scale | Conversion `2572C70` splits the raw value at 15 fractional bits before converting to the script scale 100000. Native balances use divisor **32768**, not 100000. |
| Diplomacy owner | `CDiplomacyStatus` is country +`F88`; constructor `D95F90` stores the country pointer at +`2D0` (`D96155`). |
| War predicate | `CHasWarTrigger` evaluator `3F78C0` calls `DA4470` with the secondary flag zero. This returns true if the war container +`98` is nonempty, or relations +`20` contain token `380A` (`CWarRelation`). An empty war container alone does not establish peace. |

The exact hash, seven field signatures, RTTI, country identity, owner pointers,
bounded containers and repeated byte witnesses guard these reads. Mutable
balances, headers and relation tokens join the existing change watcher.
Unsupported executable profiles leave the new sections unavailable. No game
function is called and no file, memory, input or game choice is modified.

The new exact profile also uses independently inspected core world/research/
focus/campaign layout and production offsets. Country sections are explicitly
limited to production, experience and diplomacy on this profile. This does not
certify other native domains or the native HUD on this executable.

## Guide behavior

Military spirits with AE/NE/AFE costs use the matching balance. Insufficient
or unknown balances suppress actionable HUD recommendations. Installed idea
membership remains the completion evidence; spending experience is not proof
that the intended spirit was selected.

Generic “after the war” conditions require a qualified war observation in the
same guide run followed by current peace. The war observation persists in the
existing campaign progress. Initial peace, missing evidence, another campaign
and a timeline rewind cannot substitute for this transition. Named wars and
extra prose qualifiers remain unresolved unless the guide declares a predicate.

Synthetic fault tests cover scale, ownership, invalid values, signatures,
transition membership, change witnesses and Python/JavaScript adapter parity.
A real campaign comparison of experience and war state is still required.

## Runtime integration correction, 31 October 2026

A real Hungary session exposed two integration omissions: the exact profile
was not routed through the campaign/country adapters, and joint national focuses
were rejected as ordinary focuses. Both now have regression tests. The joint
focus exception is limited to the qualified vtable RVA `27B7AE8` and exact RTTI
`CJointNationalFocus`, registered in the selected country's witnessed tree array.
Ordinary focuses still require their owning tree. Joint focuses require a null
base owner, consistent with the shipped constructor: `2F0E68` clears EDX before
calling the base constructor `2E3CD0`; `2E3E17` stores that argument at +`160`.
The RTTI hierarchy independently includes `CNationalFocus`. Unknown subclasses,
foreign ordinary owners and active/completed pointers outside the selected
tree remain rejected. The primary player selector checks are unchanged.

The corrected reader obtained a coherent Hungary sample with 273 tree entries
(238 ordinary and 35 joint), active focus `wuw_HUN_crack_down_on_radicals`,
three research slots, a campaign UUID/seed, production, experience and peace.
This is real read-only process evidence; the experience/war values have not been
compared with the game's displayed panels by the user. PP and other disabled
country sections remain unavailable on this exact build. Native GUI geometry
is explicitly unqualified on both newer profiles rather than using old offsets;
this change does not claim visual HUD alignment.

REA identified the same exact PE. Its Windows process-capture adapter reports
`capability_unavailable`; no REA runtime capture or Ghidra decompilation is
claimed. Local bounded PE/RTTI disassembly and the existing VM_READ observer
provided the static and runtime evidence recorded in `.local/joint-focus-static.log`,
`.local/joint-focus-base-constructor.log` and
`.local/focus-type-diagnosis-20261031.json`.

## Native tree HUD restored on the current build

The `cf781ed...` profile now carries an explicit GUI layout rather than falling
back to OCR: root RVA `35010D8`, registry headers `E8/100`, view container
`5F8`, container child/button/icon arrays `1270/12A0/12D8`, clip `1794`,
position anchors `17F0/17F8`. Focus view tree bindings are `3A48/3A50`, optional
country `3A58`, and required country bindings `3A60/3A68`. Standard button
sprite/anchor are `F8/1B8`, drag sprite/anchor `120/118`, icon sprite/anchor
`F0/F8`. These values are admitted only for the exact immutable executable;
RTTI, country/tree ownership, widget IDs, parent anchors and reread witnesses
remain mandatory. The other newer profile remains disabled.

The current PE's `CGameGui` vtable was located at `29C7550` through its RTTI;
the read-only process observation found the GUI singleton references at
`3500FC0/35010D8`. Real Hungary focus observations at scale 0.51024 decoded
both background and symbol rectangles with repeated snapshots around 2 ms.
The research background/icon observation decoded `[515,595,72,72]` with
independent byte witnesses. The final live comparison also observed the research panel, current folder,
visible radio research, all tab identities and an active research HUD rectangle
using the native engine. This does not certify every widget, scale, mod, event
or political panel. Initial registry discovery
remains about 350 ms and is cached; mutable geometry is read every snapshot.

Active focuses and research regain blue tracking rectangles. Active research
does not request a category-tab navigation marker. New choices continue to use
the runtime's qualified slot capacity. Original guide definitions are unchanged.
Popup choice and action-panel layouts are not qualified for this build; an
observed open popup suspends tree annotations. Those unavailable domains are
not claimed as restored by tree compatibility. No game memory or file is written.
