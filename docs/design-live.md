# Live equipment design configuration

Guide names retrieve the existing library's canonical recipe; live names do not
decide equivalence. `design_matching` compares type/category, exact occupied
slots and normalized nonzero upgrades. The runtime, browser and HUD use this
same rule, including the equipment picker. Module/upgrade differences are
reported per production line. Unknown compatible configurations cannot prove a
factory total. Missing recipe/observation data is never replaced with a guessed
match based on name, statistics or another nation's model.

`design_native.read_design(reader, variant)` observes the complete selected
module and upgrade configuration of a `CEquipmentVariant`. Its result is either
`observed` with both maps, or `unavailable` with neither map. It reads through the
existing external read-only process handle; it does not call native game code,
send input, open a designer, inspect saves, or modify game files.

## Qualified executable and static evidence

Only SHA-256
`cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f`
is admitted. The investigated executable is
`F:\SteamLibrary\steamapps\common\Hearts of Iron IV\hoi4.exe`.
REA `open_binary` independently reported that path, digest, PE format and x86-64
architecture. Its native session was closed without running a new full Ghidra
analysis. The focused evidence below comes from Capstone disassembly of the
same file's PE sections and RTTI; static disassembly does not establish runtime
execution of the serializers.

| Object / operation | RVA or layout | Observation |
| --- | --- | --- |
| `CEquipmentVariant` RTTI vtable | `0x29CE220` | Its serializer is vtable slot 2 at `0xC39420`. |
| Upgrade serializer dispatch | `0xC39763` | Variant embeds `CEquipmentUpgradesInstance` at `+0x68`; serializer field token `0x3069` is `upgrades`. |
| Module serializer dispatch | `0xC39781` | Variant container at `+0xB8`; count at `+0xC4`; field token `0x3B6B` is `modules`; dispatch to `0x19BC870`. |
| Module container serializer | `0x19BC893` onward | Header contains pointer, capacity, count. Reads every 16-byte entry as slot token at `+0`, module pointer at `+8`. Null pointers are omitted; initialized definitions have byte `+0x10` set and identifier token at `+8`. |
| `CEquipmentModule` RTTI vtable | `0x29B5DB0` | Verifies each nonnull selected module pointer's actual definition type. |
| `CEquipmentUpgradesInstance` RTTI vtable | `0x283F190` | Serializer at `0x1013E00`. |
| Complete upgrade iteration | `0x1013E47`–`0x1013EE3` | Embedded instance header at `+8` (variant `+0x70`), count at `+0x14`. Every entry is 16 bytes: upgrade pointer at `+0`, unsigned byte level at `+8`. Definition identifier is token at `+8`. |
| `CEquipmentUpgrade` RTTI vtable | `0x2A4DE78` | Verifies selected upgrade definition type. |
| Native token lookup | `0x253EB90` | Initialized byte RVA `0x368F570`, maximum token `0x368F574`, string table pointer `0x368F5A0`, stride 32. This build uses a different cache address from older country readers. |

The checked-in reader pins instruction byte sequences at these dispatch,
iteration and token lookup sites. Local reproducible investigation artifacts
are `.local/design-native-vtables.txt`, `.local/design-native-variant.txt`,
`.local/design-native-containers.txt`, `.local/design-native-upgrades.txt`, and
`.local/design-native-tail.txt`, with their same-prefix scripts.

The PE unwind record at `0x1013E00` covers only the first function chunk through
`0x1013E1A`; `.local/design-native-tail.txt` explicitly disassembles the remaining
serializer branch through its return at `0x1013F06`. It must not be mistaken for
an empty serializer from a truncated function listing.

## Completeness, ownership and change detection

The parent production or UI reader must establish the current country, validate
the owner of the production status/line or picker, and witness the pointer that
selects the variant. This function intentionally requires no world/tag context
so it can share the UI reader's bounded sample. A licensed foreign design may
retain its original creator; creator equality is not an ownership substitute.

The function checks the variant, base equipment definition, embedded upgrades
instance and each selected module/upgrade definition with RTTI. Both headers
must satisfy `0 <= count <= capacity <= 256`, and populated containers require
a data pointer. 256 is a conservative reader admission bound, **not** a claimed
engine maximum. Every entry is read, identifiers must resolve through the
initialized token cache, and duplicate module slots or upgrade IDs reject the
entire design. An invalid late entry never publishes the earlier entries.

Both headers and full occupied entry spans enter the change watch. Token cache
identity, label headers/data, and definition identity/initialization fields are
also watched. All consumed memory is part of the enclosing sample's reread
witnesses. The caller must complete `NativeSample.verify()` (or the equivalent
UI sample verification) before publishing the result. External rereads establish
sample coherence; they are not an atomic engine transaction.

An observed module map contains all occupied slot IDs, with null slots omitted.
Repeated identical modules in different slots remain distinct. Upgrade values
are native unsigned integers, including explicit zero levels. An empty observed
container proves no selected entries; unavailable extraction proves nothing
about absence. Consumers may normalize omitted upgrade IDs to zero only after
requiring `status == observed` for the complete design. Names, cosmetic choices,
MIO modifiers and final computed combat statistics are not this contract.

## Runtime evidence and limits

At `2026-11-02T10:11:27Z`, the already-running PID 13524 was sampled twice through
the new function, with the exact executable digest above. The production owner
and each line's parent were checked before reading designs. Both complete
samples passed witness verification, each with 125 watched ranges. All eight
lines yielded observed configurations. The retained evidence is
`.local/design-native-live.json`; its reproducer is
`.local/design-native-live.py`.

The `T-I 1 (0/4/SL)` line (`medium_tank_chassis_1`) contained nine occupied slots:

| Slot | Selected module |
| --- | --- |
| `main_armament_slot` | `tank_medium_howitzer` |
| `engine_type_slot` | `tank_diesel_engine` |
| `special_type_slot_1` | `tank_radio_2` |
| `special_type_slot_2` | `secondary_turret_hmg` |
| `special_type_slot_3` | `secondary_turret_hmg` |
| `special_type_slot_4` | `smoke_launchers` |
| `turret_type_slot` | `tank_medium_three_man_tank_turret` |
| `suspension_type_slot` | `tank_bogie_suspension` |
| `armor_type_slot` | `tank_riveted_armor` |

Its upgrade levels were `tank_nsb_engine_upgrade = 0` and
`tank_nsb_armor_upgrade = 4`. `SPAA` supplied a separate five-module sample with
both levels zero. The other six lines supplied complete empty configuration
containers for equipment without these selections.

These are live memory observations, not UI visual confirmation or a controlled
edit transition. No design was changed to create the evidence. Naval and air
configurations use the same native representation but were not present in these
production samples. Fixture tests cover malformed/oversized containers,
duplicates, unsupported signatures/builds, bad definition types, uninitialized
modules/tokens, late read failure, explicit zeros, empty complete containers and
post-sample mutations. They do not extend live runtime coverage.
