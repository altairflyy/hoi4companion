# Inline guide references — 2026-11-06

Implemented a passive source index and shared preview controller in
`web/guide-references.js`, loaded by the walkthrough and its existing web HUD
route. The native in-game overlay is unchanged. Word table sections, text after
tables, Markdown headings and associated local template images supply previews.
Hover/focus previews; click pins; Close/Escape dismisses. Ambiguities expose all
matching sections; missing explicit/starred references retain a source-text
fallback. Source content is rendered as text, never executable HTML.

The walkthrough links titles, instructions, timing and original quotations.
References omitted by a structured operational description remain available in
a separate Guide references line. The source index is rebuilt only for a changed
guide definition, with bounded caching for associated images. Pinned content
survives ordinary card replacements during live updates. No runtime predicates,
progress, native acquisition, game files or imported guide definitions changed.
The AI creation prompt requires no new fields.

Verification:

- 1,163 Python tests passed, including actual HTTP delivery of the new module.
- 444 JavaScript tests passed, including nine new reference tests and the
  existing walkthrough integration test extended to cover inline links.
- The running local service returned HTTP 200 for the module, walkthrough and
  Hungary guide API. Seven distinct probes resolved uniquely to the intended
  source sections: Initial, YUG, ROM, Axis, Allies, Navy Build and UK invasion.
  The last two resolve to the imported Abbreviations & Tools source.
- The Hungary guide hash remained
  `287d7d5f3188ae96dd0f70ff63eb5067994b64ff7cc7c12d964e98b6c1726f8e`.
- JavaScript syntax and scoped whitespace checks passed.

Evidence logs are in `.local/guide-references-{python,js}.log` and
`.local/guide-references-http-probe.json`. This is source implementation and HTTP/
fixture verification, not visual browser proof: the local browser tool returned
`Local browser use is disabled in Work settings`. No screenshot or real-browser
alignment is claimed. Embedded Word pictures and absent external references
remain unavailable unless separately imported into the existing reference
library. No automatic external fetch or fabricated reference is introduced.
