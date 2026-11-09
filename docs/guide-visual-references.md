# Walkthrough source references

Operational steps can expose multiple references from the guide and its source inventory. Open **Show source references** (or the existing frame control) to inspect them. The list distinguishes an author-declared visual observation from a spoken timestamp and from an explicitly supplied frame. Timestamps refer to the original video, not to the beginning of an extracted clip. References from different videos remain separate, and an ambiguous default is not selected automatically.

The frame and contextual clip are available on request when the corresponding source video has been prepared locally. The contextual clip is at most 15 seconds around the reference, can start earlier or end sooner near the source boundaries, and may not show the action. Playback is never automatic. The player remains open through live updates and closes when the guide changes. If the local video is unavailable, the original source link remains available when supplied.

These references are evidence supplied by the guide, not a live observation of the game and not proof that an instruction has been completed. A declared visual observation has not necessarily been independently verified. Large reference sets may be truncated; the interface reports this.

## Inline document references

In operational instructions, hover over an underlined reference (for example,
`Initial Setup*` or `ROM setup*`) to preview that part of the imported source.
Click or use Enter/Space to keep the panel open for reading; Close or Escape
dismisses it. The same links appear in the original action quote. Linked template
names also show their already associated local reference images.
If the operational description replaces the source wording, its links remain
available in the card's **Guide references** line.

The index uses Word table sections and Markdown/plain-text headings. Setup
references prefer matching subsections under setup/war-plan headings, preserving
their source text. Ambiguous names offer the matching sections with their source
and context; no fuzzy association is chosen. An explicit reference without a
matching imported section offers the original document instead. External sources
and embedded Word pictures are not fetched or reconstructed automatically.

This is a presentation feature: it neither rewrites imported guides nor changes
runtime conditions, dependencies or campaign progress. A pinned preview survives
live updates and closes when the guide definition changes.
