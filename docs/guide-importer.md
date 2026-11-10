# Guide import workflow

Catalog resolution accepts documented research-family abbreviations followed by
an explicit level (numeric or Roman). Native IDs remain exact, and original
catalog names take precedence over expansions. Scripted localisation alternatives
are read from the installed game/mod definitions as aliases for the same entity;
this does not assert that any particular name or trigger is active in the campaign.
Word editorial headings and explicit free-choice markers do not become native IDs.

For a collection audit, use the Guide ZIP slot in Import guide. The native
background worker checks every country-guide document against the installation
and preserves errors per guide. Review does not install content until you confirm
the batch. The template ZIP has a separate slot and shares the same persistent
library. Catalog validation is separate from live verification.

Use Import guide to export the creation prompt and relevant thematic catalogs. Supply them with accessible video/web material to an external AI, then upload its complete JSON. The optional integrated adapter remains available but is not required.

Import proceeds through parsing, identity resolution, schema/dependency/evidence validation, editable preview, and explicit installation. Resolved associations stay collapsed; genuine ambiguities remain actionable. Local association corrections do not rewrite source evidence. A technically valid file does not certify faithful extraction of the entire video.

Guide definitions, installed guide selection, and campaign progress remain separate. Import remains in permanent navigation so users can load another guide. See [authoring](guide-authoring.md), [catalogs](thematic-catalogs.md), and [video preparation](guide-video-workflow.md).

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/guide-importer.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.
# Word documents and ZIP collections

The importer accepts a `.docx` file or a `.zip` collection up to 200 MB. Select a
document and prepare a draft; the archive is read locally and individual entries
are never extracted to their paths. Recognized three-column priority tables produce
candidate focus and research steps in their stated order. Complete standalone
law names, explicit agency creation, single-state construction with an explicit
quantity, and named event/choice pairs become structured candidates; other action
text remains a reminder. The catalog must still resolve each candidate, and live
tracking depends on qualified available game data. An event appearing is not
proof of the selected answer. Separate table rows are kept in the acquired source, but do not establish
simultaneity or implicit completion dependencies. Conditions in prose are not
automatically turned into runtime rules. Run ID resolution and review the draft
before installation. A document without a recognized priority table remains
available as source text with a review reminder.

Other document sections are not automatically converted into steps. For new
imports, the extracted text of declared local file sources is preserved in the
installed guide and can be opened under “Guide sources, bindings and limitations”.
It is reference material, not automatically monitored instructions. Older installed
guides cannot recover text that was not saved; keep or reimport their source.
A glossary or other Word reference from the same archive can be added
as a separate source; a uniquely named glossary is preselected and can be changed.
Some documented abbreviations are expanded as contextual hints; the glossary is
not fully interpreted as executable logic. Images and standalone templates
in the archive are not interpreted or automatically attached. For a fuller guide use the source
and appropriate reference files with the creation prompt and review the result.
The application does not validate a strategy or claim that the draft covers the
complete document. The source files and existing guides are not modified.

For a relative `UP` without an explicit anchor, import resolves a unique catalog
branch, not a campaign level. The active guide binds the first unfinished level
from qualified research evidence and stores that choice in campaign progress.
Further steps in the same branch wait for earlier bound steps to complete. If
the branch is ambiguous or live evidence is unavailable, no level is assumed.
Existing guides whose imported metadata preserves the original UP phrase are
adapted in memory using the current catalog; their source files are not changed.
If that branch cannot be identified, it stays unresolved. Steps without such
metadata retain their previous meaning. For a new relative UP, an explicit
anchor identifies the branch without fixing the campaign level. An initial
scenario is not live evidence and must not override a campaign binding.

Reloading an earlier save of the same qualified campaign must not rebase an
existing `UP` onto today's first unfinished level. On a timeline rewind the new
run retains the previous target identities in `research_history`; it does not
copy checklist confirmations, construction baselines or completed actions.
Only relative steps whose stored target is still in their declared catalog
chain can recover that target. Current qualified research membership still
determines whether it is active or complete. Pre-fix rewind runs recover the
first recorded target for each matching step from earlier runs of the same
guide, campaign and nation. Other campaigns and deliberate fresh progress runs
are isolated. Branch choices are selected again from current evidence.

Later levels wait for lower levels in the same progression. This wait does not
hide independent technology families that can use other free slots. An unbound
`UP` whose entire chain is observed complete is not applicable (no further
upgrade exists), not a newly confirmed historical action. Missing or stale
research evidence remains unknown. Python, JavaScript and HUD projection share
the selected runtime technology ID, including while a guide view is cached.
# DLC availability in guide lists

Focus spelling recovery can use the mandatory ancestors of a separately resolved
focus declared elsewhere in the guide. It requires a substantial name, one
plausible textual candidate across all compatible branches, and an unambiguous
required-ancestor path. A possible parallel candidate prevents recovery; list
adjacency alone never selects an identity. OR alternatives, conditional anchors,
broken paths, native IDs, numbered levels and opposing directions are not guessed.
The preview records `guide_required_ancestor`, the matching label and supporting
focus IDs. This resolves an identity only: no live completion, guide dependency,
new step or strategy is generated from the association.

An additional focus-only fallback compares equal word counts in the same order,
allowing one changed word when the country or tree is identified and the authored
route leaves one compatible candidate. Exact names, IDs and minor typos take
precedence. Levels, directions and gameplay qualifiers are protected; unknown
national context or missing/cyclic prerequisites cannot qualify this fallback.
Its preview basis is `focus_one_word_variant`, with the matching label and changed
word recorded for inspection. This does not mark a focus completed.

ZIP collection choices and installed-guide selectors retain guides with unmet
DLC requirements as grey, disabled entries. Each entry names the required DLC
settings; OR alternatives and legacy guides requiring a DLC to be disabled stay
distinct. Unknown DLC state or an unresolved name does not disable a guide.

Preflight uses the source's focus/technology identities and the installed game
scripts. It reuses the DLC condition evaluator, preserves dynamic predicates as
unknown, and never infers requirements solely from a country name. This checks
observable scripted DLC gates, not every mechanic described in prose or every
mod/version requirement. Results are cached per catalog generation and archive
content; a settings/catalog refresh invalidates the cache. Collection scanning
is bounded to 128 documents and 64 MB of document members; unscanned entries
remain unknown and selectable.

The association report lists these unavailable guides separately from names
requiring correction. An ID outside the active catalog without a proven DLC gate
remains a compatibility diagnostic and is not automatically called a missing DLC.

## Focus sequence reconstruction

The editable preview includes **Focus sequence reconstruction**. Keep partial
coverage for ordinary priority lists. A complete ordered list and an explicit
initial-focus baseline enable bidirectional constraint proofs, preserving source
slots even for unreadable names. The preview distinguishes direct identities,
structurally reconstructed identities, probable candidates and unresolved slots.
See [the algorithm and safety boundaries](focus-sequence-reconstruction.md).
This never marks game progress or changes the active guide during preview.
