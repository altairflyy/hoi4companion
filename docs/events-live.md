# Events, answers, and HUD targets

Catalog events and options have explicit parent-child identity. The guide selects an event and an answer belonging to that event. Multiple-choice export filtering is based on scripted options; the actual visible choices may be fewer because of conditions.

HUD highlighting requires a qualified visible event/option and matching guide/campaign context. Event popups take priority over tree recommendations underneath them. Event disappearance, a click, or a closed panel does not prove which answer was executed. Static event chains represent possible script relationships, not a prediction of the campaign.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/events-live.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.

## Author event answers in guides (2026-11-16)
Imported guides name event answers in shorthand. `native/backend/src/reference_candidates.rs` recognizes them for every checklist (in `runtime_definition::prepare`, and again after decomposition for new definitions) and binds a single answer through `political_binding`, so the HUD highlights it when the event popup is visible.
- Line shapes (`references`): "Mobilize The…", "- Recall Kocs…(25 CP)", "They will not… event (50PP)", "Which By God…: CW setup" (answer, then other work), "On Expand…done: We will acc…(15PP)" (timing lead-in, then the answer), several answers ("Absolutely Not… + There is a…", "A… / B…", "A…, B…"), and an answer written out in full as the whole line (3+ words, exact match).
- Matching (`AnswerNames`): the abbreviation is a prefix of the answer text, cut anywhere, including mid-word ("A New Bismarck…" = "A New Bismarckjugend…"); case, accents ("Osterr…" = "Österreichische…") and punctuation ("Which By God…" = "...which, by God, …") are ignored. The catalog's own matcher adds other kinds of entries for display.
- Choice (`choose`), in order: the author's "(NNPP)" note equals the answer's political power cost; the nation provably receives the event (`event_reach`); a single answer whose event namespace names the nation. Otherwise nothing is bound and the candidates are shown on the card.
- Several answers on one line: the first is bound; the others are listed in `reference_resolution.also` and shown on the card. Only the bound one is highlighted by the HUD.
- Real catalog (50 installed guides, existing checklists): 36 answers recognized (was 16), bases: 6 cost, 17 received by the nation, 5 namespace. Regressions in `native/backend/tests/real_catalog_contract.rs`; survey: `event_reference_survey` (opt-in `HOI4_REAL_CATALOG=1`).
