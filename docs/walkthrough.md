# Operational walkthrough

The walkthrough presents operational instructions: construction, production, projects, laws, appointments, events, templates, army orders, and reminders. National focus and research recommendations are highlighted in their corresponding trees instead of repeated as long walkthrough lists.

## Source frames

Frames and their video links use one selection policy in the UI and API: an
explicit `source_seconds` frame choice takes precedence, then direct visual
evidence, then visual inventory items linked through `step_keys`, followed by
timed spoken evidence or inventory. Excluded and unrelated inventory items do
not supply frames. The source URL and timestamp stay paired; conflicting video
identities suppress the frame rather than selecting a source arbitrarily.
Duplicate URLs for the same YouTube video are canonicalized.

Visual frame selection does not rewrite the guide, change its completion rules,
or move a spoken chronological reference to a later screenshot. The frame's
displayed time, video link and browser cache use the selected visual reference.
Without a visual reference, a spoken-time frame remains a contextual fallback;
the application does not claim that it depicts the action or infer times from prose notes.

Show starts/in-progress states separately from completion. Qualified observations can recognize actions automatically; manual controls are reserved for genuinely unobservable declarations. Source frames and campaign dates provide context without becoming completion conditions. The contextual summary explains goals, observed state, and remaining uncertainty.

## Instruction order

The operational list uses a deterministic display order: declared completion and
start dependencies take precedence, followed by documented campaign dates and
comparable source timestamps. A missing campaign date does not move an otherwise
timed instruction behind every dated instruction. Equal-time items keep their
authored order. Distinct source clocks are not compared as one video timeline.
Dependencies through focuses or research remain respected even though those
actions appear in their separate trees.

Instructions without a comparable time reference retain their relative guide
order after timed instructions, unless needed as prerequisites. They are marked
as chronologically unverified. Conflicting references are reported; dates and
timestamps are never invented or rewritten. Dates alone do not become deadlines.

This changes presentation only. Runtime priorities, parallel actions, political
sequences, completion checks, and saved progress remain unchanged. Attention
summaries and a currently visible event can intentionally take priority over the
chronological list. The next actionable suggestion still requires the runtime's
conditions; it is not simply the first displayed card.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/walkthrough.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.

## Actions blocks

See [Reading the Actions column](action-plan.md) for source-numbered blocks,
operational instructions, compound-command milestones and per-action HUD status.
