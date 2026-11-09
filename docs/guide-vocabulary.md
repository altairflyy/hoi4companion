# Reusable guide vocabulary

The versioned vocabulary lives in
[`guide_language.json`](../src/hoi4companion/guide_language.json). It is shared by
the importers; it does not rewrite source documents, generate strategy or prove
that an action happened. `schema_version` describes the file format and
`vocabulary_revision` identifies the current content revision.

Names expand to English references, then the installed catalog resolves native
IDs and local display names. Keep aliases scoped by domain: a short form is not
a global synonym. National characters are resolved by their English trait names,
country, role and installed definitions rather than a dictionary of national IDs.
Both direct character roles and roles inside `instance` blocks are indexed.

## Doctrine shorthand

The qualifier `Land Doc`, `Air Doc` or `Naval Doc` supplies the domain. Bare `AI`,
`SD` or `TI` never selects a doctrine. Examples:

| Domain | Short forms and English names |
| --- | --- |
| Land | GBP: Grand Battleplan; MW: Mobile Warfare; SF: Superior Firepower; MA: Mass Assault |
| Land | GA: Grand Assault; AI: Assault Infantry; SD: Streamlined Deployment |
| Land | FE: Field Engineering; SA: Siege Artillery; LUT: Large Unit Tactics |
| Land | AS: Armored Spearhead; SPS: Self-Propelled Support; FC: Fire Concentration |
| Naval | FIB: Fleet in Being; TI: Trade Interdiction; BS: Base Strike |
| Air | SD: Strategic Destruction; BS: Battlefield Support; OI: Operational Integrity |

The full English doctrine name is accepted too. Definitions are read from the
installed `common/doctrines/grand_doctrines` and `subdoctrines`, respecting mod
replacement paths. A vocabulary entry cannot resolve an entity absent from the
installation, and multiple matching IDs remain unresolved. This matters when
different game versions use different doctrine systems or names.

`LF Spirit` maps to the generic Logistical Focus spirit. Other appointment traits
such as Military Theorist and Financial Expert come directly from the installed
character definitions; their native IDs vary between nations.

## Conditions and limits

Doctrine actions retain native IDs, domain and track. Their completion predicate
is membership in the qualified `doctrines.selected` fact, not researched
technologies, available experience, a click, or mastery. They can become the
source-ordered milestones used to admit later production instructions.

The current memory reader does not yet publish qualified doctrine selections.
Association is therefore available, but live completion stays unknown without
that observation. Adding an alias does not create a native acquisition feature.
Guide-declared XP costs remain declared costs, not proof of the actual discounted
price. `GBP Land Doc + Tactic (80AE)` identifies the doctrine while retaining an
unspecified tactic as a separate reminder; its combined cost is not assigned to
the doctrine alone.

Complete doctrine commands can also appear before a comma-separated method,
with `ASAP`, a declared discounted XP cost, or after a complete appointment
followed by `ASAP:`. The identified actions retain their source order and quotes;
unresolved follow-up instructions remain separate notes. Conditions, alternatives
and ambiguous conjunctions are not stripped to force an association. A bare
`Land Doctrine ASAP` still needs a named doctrine.

Generated tactic reminders use separate note keys, preserving the existing keys
of subsequent source actions and campaign checks. Generated production milestone
annotations carry a content signature so they can be refreshed after newly
resolved doctrine/appointment predicates are bound. Authored annotations or
edited generated annotations are preserved.

To extend the vocabulary, add a domain-specific English expansion with source
evidence, then verify a unique installed match and a negative ambiguity/context
case. Avoid adding an abbreviation to a broader domain merely to make it resolve.
The existing AI guide format and source files remain compatible.
