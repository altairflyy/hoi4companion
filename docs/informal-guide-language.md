# Informal guide language

See [Reusable guide vocabulary](guide-vocabulary.md) for contextual doctrine
abbreviations, generic character traits, installed-ID resolution and live-evidence
limits. The same versioned vocabulary is used for new and existing imports.

`src/hoi4companion/guide_language.json` is a versioned, local reference derived
from the supplied membership archive's ABBREVIATIONS & TOOLS document. Its
entries are grouped by domain because, for example, MIL can mean a building or
a production assignment. It does not change the original archive or guides.

An UP refers to the next useful technology on the specified branch, including
its first mention; the user's explicit convention takes precedence over the
source document's usual convention that the first level is written out. An
ambiguous branch, unavailable live evidence, or missing game data must remain
unresolved. Phrases and arrows must not be treated as executable conditions
without enough context. The original text remains available for review.

The research glossary explicitly defines `Armor UP` as the tank armor-protection
family and `Guns UP` as the infantry-equipment/weapons family. These definitions
apply to all guides, including their first UP mention; live observations still
determine the next useful level. They do not mean naval armor or artillery.

`Small Caliber…` / `Small-Caliber…` denotes the next light naval-shell level:
`basic_light_shell` then `improved_light_shell`. `Armor Piercing…` (or the same
name explicitly qualified by `(navy)`) denotes `basic_heavy_shell` then
`improved_heavy_shell`. These documented progressions describe shell upgrades;
game paths leading to gun batteries are still prerequisites, not shell levels.
Full native IDs and the full `Armor Piercing Bombs` name keep their fixed meaning.
A missing progression member is unresolved rather than silently skipped.

Focus labels respect the script's `text` localisation override while retaining
the focus's native ID. Same-title focus variants can also use the government's
explicit English label in the guide title, such as `(Communist)`, to exclude a
contradictory `allow_branch` variant. This is declared guide context, not an
observation of the current game's ideology or of dynamic focus availability.
Without a unique declared government or a provably false candidate condition,
the variants remain distinct.

After native IDs, exact labels and bounded spelling errors, focus lookup also accepts
one different word at the same position or one extra/omitted word.
This applies only to multiword focus labels with a meaningful unchanged word;
it does not relax technology or other entity lookup. Alternatives, native IDs,
levels, directions, negations and explicit gameplay qualifiers remain protected.
An identified country or focus tree is required; unavailable national content
must not be replaced with a similar foreign focus. Country, tree, government
and authored-route constraints still apply. Multiple
compatible identities remain unresolved, and missing/cyclic prerequisites cannot
qualify a relaxed match. A known name of another entity (including an optional
English article) blocks this fallback: a named decision is not a focus typo.
The preview records `focus_one_word_variant`, the label
and changed word; this identifies an entity without asserting live completion.

Spelling recovery normally tolerates at most two letter edits across substantial words,
including insertions and transpositions, after Unicode folding. Levels and
directional/gameplay qualifiers remain protected. A one-letter match keeps
precedence over newly admitted two-letter candidates; equal candidates stay
ambiguous. In a qualified focus tree, a secondary spelling pass permits three
edits across at most two words, at most two per word, with an unchanged word of
at least five letters. Exact and closer matches retain precedence; native IDs,
levels and gameplay qualifiers stay protected. Simple singular/plural `s`
differences also work for short nouns such as `Act`/`Acts`.
Confirmed nomenclature is data
in `guide_language.json`, not nation-specific resolver logic. National aircraft
model nicknames are derived from English unlocked-equipment labels: e.g. the
name suffix of a `Fighter` or `CAS` model containing its model number.

Identically named Word focus variants can also be distinguished through an
explicit earlier native prerequisite and a normal-route exclusivity conflict.
This is recorded as `declared_source_branch`, with the supporting and excluded
native IDs. Unrequested bypasses are not invented as source instructions; they
remain possible in the game and their availability/completion is not asserted.
This fallback requires complete ancestor definitions and rejects cycles,
ambiguous predecessors and sources explicitly requesting a bypass. It neither
changes runtime completion conditions nor marks a focus completed.

`Concentrated/dispersed Industry 1` declares one player choice between two
research branches. Both options are purple in the research sheet and native
HUD; they occupy one runtime proposal. Only qualified observed active/completed
research can commit the choice. Stored per-step native targets identify the
branch across restarts and subsequent UPs in the same guide/campaign. Unknown,
conflicting or foreign observations do not choose a branch. This uses the
existing campaign progress and research bindings; no manual confirmation or
automatic clicks are introduced. A new campaign/run starts with an open choice.

Other fully resolved research alternatives separated by `/` use the same choice
engine in `research_choice.mode: "once"`. A named option keeps its exact ID;
an UP option uses its next unfinished level. Older completed levels do not
commit this one-off choice. Qualified, eligible offers are frozen in the run's
`research_options` map, then matched against observed active/completed research;
this also recognizes a completion between samples. An earlier unfinished step
on the same upgrade chain cannot be reused for the new UP. The choice does not
force later independent research onto that branch. Unknown alternatives remain
unresolved instead of silently dropping one side.

`Continuous Focus/Nothing` (or `None`) resolves only to a known continuous
focus. Its internal `focus_optional` flag keeps it out of required focus capacity
and prevents it from holding up later national focuses. When live evidence is
qualified it is highlighted in purple as an optional choice. Selecting nothing
does not complete it; selecting it still uses the normal continuous-focus fact.
Explicit authored dependencies on it are preserved.
An explicit `Until <focus> available` note can also identify a later authored
focus through the catalog. The existing `optional_before` policy stops proposing
the continuous option after that target is observed completed (or its declared
priority condition is met). It does not guess dynamic availability from prose.

Slash-separated adjectives in a script's `[TAG.GetAdjective]` title are a
different case: if every alternative is an English country/cosmetic adjective,
the shared title template can identify one native focus. Country/tree constraints
and ambiguity checks remain in effect; this does not claim its live availability.

Reference names confirmed against this installation include `Inter-war Tanks`
(`gwtank_chassis`), `Pulverized Coal Firing 1`
(`improved_equipment_conversion`), `Eng3` / `Engine 3` (`engines_3`) and
`Improved Small Aircraft` (`improved_small_airframe`). An explicit UP retains
its relative meaning even with a qualifier such as `Improved Infantry Equip UP`.
The confirmed focus label `Proclaim A Greater Italy` resolves to
`ITA_towards_a_greater_italy`; an actual decision reference retains its decision
identity. `Fighter Saetta`, `Fighter Folgore` and `CAS Lince` identify the legacy
`fighter1`, `fighter2` and `CAS2` technologies from the English Italian equipment
names. They remain outside the active research catalog with By Blood Alone;
identifying their names does not make the incompatible Vanilla guide playable.

Continuous national focuses are loaded from `common/continuous_focus` separately
from national-tree layouts. A matched continuous identity carries the internal
`action.params.focus_mode: "continuous"` marker. Both runtimes read its selection
from the qualified `focus.continuous` fact. It remains active while selected and
has no native completion; an explicit completion condition is still respected.
An authored `depends_on_started` can follow its selection, while `depends_on`
must not be silently reinterpreted. No new authoring prompt field is required.

Explicit `Focus A (Bypass) / Focus B (if <condition>)` notation is compiled by
the importer only when both identities resolve uniquely within the same national
tree and country. It becomes one conditional step targeting B, with an existing
Guide v1 `completion` predicate accepting either A or B in the qualified
`focus.completed` list (which includes recorded bypasses). The notation alone
does not establish that the game's bypass conditions are currently true.
Original quotes and notes are retained; no additional focus is inserted.

An existing declarative `when`, `completion`, phase and dependencies retain their
meaning. Without an authored `when`, informal wording such as "late with YUG"
is retained in descriptive `action.params.conditional_focus` metadata and guarded
by an unavailable namespaced `guide.conditions.<digest>` fact. No current live
provider supplies that fact: the runtime therefore reports `condition_unknown`
and the HUD does not recommend B. No checkbox, deadline, diplomatic condition or
in-game availability is fabricated. Supplying a measurable declarative `when`
through the ordinary guide format enables automatic evaluation. An already
observed completion/bypass or active selection is still recognized. Bare slash
alternatives, unresolved names or incompatible trees remain unresolved. This is
an importer extension using existing policies; no new AI prompt field is needed.

The file is a curated starting point, not a complete dictionary and not an AI
model. Terms in unrelated domains are not automatically resolved merely because
they appear here; add executable interpretations only with explicit context and
tests. The original document also includes designs and strategies which are not
automatically translated into actions.

The importer also promotes complete operational commands inside existing Word
reminders. `Create Agency & Setup Done` becomes an agency-creation action plus a
separate reminder retaining `Setup Done`. Only the agency's qualified
`in_creation` / `is_created` flags establish start and completion; its later
configuration is not inferred from creation.

A complete `<advisor name or trait> (N PP)` instruction is promoted only if the
existing political resolver finds one matching adviser or political idea in the
guide's country. Role and native identity come from that catalog entry; the cost
remains explicitly guide-declared. Ambiguous, foreign, unscoped and compound
instructions remain reminders. The existing binder observes `politics.advisors`
or `politics.ideas` for completion, and the runtime/HUD reuse their ordered
political sequence and affordability checks. A power decrease is not completion.

Multiline source cells retain unconverted methods as reminders and copy evidence,
authored policies and dependencies into extracted substeps. Source timing headers
such as `On Civil War:` are retained. In the absence of an authored predicate,
their generated condition stays unknown; no acquisition of a civil-war flag is
claimed. Later political actions cannot skip an earlier pending appointment.
These conversions add no manual confirmation and need no new AI prompt fields.
Supported timing thresholds and focus/research conditions are described in
[Guide timing conditions](guide-conditions.md), including conservative handling
of unknown clauses and the latest archive audit.
Already imported Word documents with explicit dependency policies receive the
same adaptation in memory when loaded by the app and HUD. The cache follows
definition/catalog identity; original guide files and campaign progress are not
rewritten, and legacy implicit-barrier policies are left unchanged.

Complete economic, conscription and trade-law commands inside those reminders
use the same `explicit_action` recognizer as standalone Word instructions. A
unique installed law identity supplies its category; qualified `politics.ideas`
membership supplies completion. Listed PP costs remain guide-declared and an
omitted cost stays unknown. Inline timing such as `After the war: War Economy
(150PP)` is retained as a condition rather than discarded. Unrelated commands
remain reminders, and ambiguous/missing laws are not replaced with advisers or
another similarly named law. No game files or live acquisition paths change.

Complete generic timing labels such as `After the war`, `Once the war is over`
and `Dopo la guerra` use `guide.campaign.after_war`. It becomes true only when
the current run has observed its nation at war and a qualified current sample
reports `country.diplomacy.at_war: false`. Initial peace does not prove a prior
war ended. Missing/stale observations remain unknown; entering a later war
closes the condition again. The small `campaign_conditions` history is stored
with the existing guide/campaign run and cleared on a new run or timeline
branch. Campaign restoration retains that campaign's own observation. A war
that ended before observation cannot be reconstructed from peace alone.
Named wars and added qualifiers remain unresolved. Explicit authored conditions
are preserved. Exact supported timing on existing explicit structured guides is
adapted in memory with the same cache as imported Word documents; no source
guide is rewritten and no new AI prompt fields are required.

The same country-scoped target compiler handles complete appointment aliases
without a stated PP cost and a trailing condition such as `(before event)`.
Ambiguity is checked across adviser, selectable-idea and decision namespaces;
an exact decision name cannot silently override an adviser trait with that name.
`Political Loyalty` / `PL Spirit` identify the same installed spirit. AE, NE and
AFE prices are explicit army, naval and air experience costs, with additive v1
`action.params.cost_currency` metadata validated against the spirit's group and
`resource_costs`. They do not fabricate a PP price. Explicit PP costs, when
provided as well, still apply. The build-qualified native experience reader
publishes the three balances on the exact executable documented in
`military-live.md`; unsupported layouts and invalid observations stay unknown.
Real-session comparison remains pending. Observed `politics.ideas` membership still proves the
spirit selection; clicking or spending experience does not.

Unique country-scoped national decision names become actions with an execution
predicate on `decisions.executed`. The current native provider does not publish
execution history. Imports therefore report the identity as resolved with
unverified execution, not automatic live completion. Visibility, disappearance
and manual checks never substitute for that predicate. Dynamic `[FROM...]` or
`[TARGET...]` decisions without a declared recipient remain reminders.

`Production: Sup:1, Inf:10`, `MIL:` and `NAV:` numeric allocations reuse the
production abbreviation dictionary and existing family selectors. The requested
factory count is checked even when fewer factories are effectively assigned.
Known numeric items form one production layout. An explicitly named final
military `ALL` product uses the dynamic remaining-capacity policy documented in
`production-guide-monitoring.md`; unspecified counts, MIO notes and unresolved
names remain separate reminders. Duplicate or overlapping
equipment targets are not silently combined. These expansions are restricted to
explicit dependency policies so legacy implicit barriers retain their meaning.
No original guide files or progress records are rewritten; existing imported
file guides use the same cached in-memory adaptation as new previews.

Identity lookup now expands declared scripted localisation alternatives (including
country-scoped functions and fixed state names). This lists possible labels; it
does not evaluate their triggers or claim which label is currently displayed.
Country-specific short equipment labels are identity aliases, but canonical
technology family names take precedence when identifying an UP progression.
Detached numbered levels may form a progression without adding prerequisites.
Known DLC-filtered technology branches do not count as broken references; a truly
missing target still prevents automatic progression.

For a complete review of an archive, run `scripts/audit-guide-archive.py` with
`--output .local/archive-associations.json --review-output .local/association-review.md`.
The second report includes every unresolved occurrence, candidate IDs, source
files, and content outside the active catalog. Text-similarity candidates are
review-only. Running the audit does not install or modify guides and does not
enable unavailable DLC content.

Focus identity resolution also excludes individual branches whose `allow_branch`
DLC condition is provably false, not just entire trees. A dependent identity is
excluded only when every alternative of a required prerequisite group is already
excluded. Runtime conditions, missing DLC information, missing parents and OR
alternatives remain unknown. This changes the compatible identity catalog; it
does not remove national-layout nodes, edit game files, or infer live progress.
