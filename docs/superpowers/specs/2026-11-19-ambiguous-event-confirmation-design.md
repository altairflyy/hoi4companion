# Ambiguous event answers: HUD color, confirmation and durable user answers

Date: 2026-11-19. Status: design, awaiting the user's approval.

## Intent and why

Imported guides name event answers in shorthand ("We cannot…"). After IL-01..IL-12 several lines still
match more than one game event answer, and no further rule separates them for certain (precision rule:
never bind on a guess). The user accepts that a clean automatic result is not possible and wants a
workable path in play instead:

1. When an ambiguous line's event pops up, the HUD still shows the guide's answer, but in a color that
   says "ambiguous" (not the color of a proven answer).
2. The HUD colors follow one logical scheme everywhere.
3. The player confirms on the HUD that they chose that answer. The confirmation is recorded, it completes
   the step, and it survives app updates and guide re-preparation, so the line is no longer ambiguous in
   later campaigns with the same guide.

The app is read-only toward the game: it cannot see which option the player clicked, and a popup that
disappears proves nothing (`docs/events-live.md`). An explicit confirmation on the HUD is therefore the
evidence, chosen by the user over inferring the choice from in-game effects.

## Goals

- A distinct HUD color and shape for `visible_candidate` highlights (IL-12), so an ambiguous answer is
  never mistaken for a proven one.
- One palette module in the overlay with named roles; every marker, card border and button uses a role,
  not a literal color.
- A "I chose this" button on the HUD card while a `visible_candidate` highlight is shown.
- Durable storage of confirmed answers in the user's SQLite database (approach A), outside the
  prepared-guide cache and outside per-run progress.
- On every preparation of the guide, a confirmed answer binds its line again (basis `user_confirmed`),
  so later campaigns see it as a resolved line.
- Undo removes the confirmation and the step's check.

## Non-goals

- Inferring the chosen answer from game effects (flags, opinion modifiers) or from the popup closing.
- A learned or weighted model that picks answers (discussed, not chosen).
- Reading the in-game date.
- Restyling the web suite. It only gains the text label for the new states (see Builder's calls).
- Matching confirmations across different guide documents (a re-imported, edited DOCX gets a new
  `guide_id`; its confirmations do not carry over).

## Anti-goals (a failure even if it works)

- A confirmed answer marks anything done that the player did not confirm, or an ambiguous highlight
  looks like a proven one.
- Any click, input, injection or write toward the game.
- Confirmations lost after an app update, a backend rebuild, a catalog regeneration or a guide
  re-preparation.
- Evaluation logic in JS: the Rust guide evaluator stays the only one (`CLAUDE.md`).

## Constraints

- Read-only toward the game; unknown stays unknown; Rust is the only evaluator; no Python (`CLAUDE.md`).
- The prepared-guide cache never stores live observations or campaign progress. Confirmations are user
  data: they must not be baked into a cache entry that is shared or reused without them.
- `guide_id` of a Word guide derives from the DOCX bytes; step keys such as `word-0006` are positional
  and must never be reinterpreted in an existing run (`docs/guide-definition-identity.md`).
- Live HUD highlight needs a qualified visible event/option (`events.status == observed`).
- UI strings go through `translations.json` (English default plus Italian; use the `add-translation`
  skill).

## Approaches considered

- **A. New SQLite table in the user's progress database (chosen by the user).** One row per confirmed
  line; applied after preparation; transactional; lives with the rest of the user data in `.local/` or
  `HOI4COMPANION_DATA_DIR`.
- B. A separate JSON file in the data directory: simpler to read by hand, but no transactions and a
  second persistence path to maintain.
- C. Only in the current run's progress: lost for every new campaign, so the line stays ambiguous;
  rejected because the user wants the answer kept.

## Decided design

### Colors (overlay palette)

A single palette module in `native/overlay` with roles, used by `render.rs` and `card.rs`:

| Role | Color | Used for |
| --- | --- | --- |
| `next` | amber (current `colors[0]`) | proven next action: focus, decision, proven event answer, answer confirmed by the user |
| `active` | cyan (current `colors[1]`) | element in progress / active; Done and Undo buttons |
| `choice` | purple (current `colors[2]`) | a choice between options the guide itself lists (research choices) |
| `ambiguous` | pink, about r 1.0 g 0.45 b 0.75 | the guide's answer on an ambiguous event (`resolution == "visible_candidate"`) |
| `done` | green (current `done`) | completed |

The `ambiguous` marker also has a dashed border and a "?" in its label, so it does not rely on color
alone. Today's mapping in `render.rs` (`tone` 0/1/2 from `mark.choice` / `mark.active`) is replaced by the
role the HUD proposal carries.

### Confirmation flow

1. IL-12's `hud_targets::visible_candidate` proposes the highlight (unchanged conditions). The proposal
   carries what the card needs: guide id, step key, the option's identity (event option id).
2. The overlay card shows a button "I chose this" ("Ho scelto questa") next to the existing ones, only
   for a `visible_candidate` proposal.
3. Click: the overlay sends a new control action to the backend (same bridge as the existing `check`
   action), with guide id, step key and option id.
4. The backend, in one transaction: stores the confirmation (table below) and checks the step in the
   current run (see Completion). Response feeds the existing notice with Undo.
5. Undo: restores the previous confirmation state and unchecks the step.

### Storage (approach A)

Table in the same SQLite database as `progress` (`native/storage`), for example
`confirmed_event_answers(guide_id TEXT, step_key TEXT, quote TEXT, option_id TEXT, confirmed_at TEXT,
PRIMARY KEY (guide_id, step_key, quote))`. `quote` is the line's normalized evidence text, so a positional
key that later points at a different line does not reuse the confirmation. One row per line; a new
confirmation on the same line replaces the old one.

### Applying confirmations (coordinator's call after the builder check)

Confirmations are an **overlay on evaluation input**, not part of the definition. Runs are evaluated
against their pinned, immutable definition archive (`definition_ref`, `application.rs::definition_for_run`);
baking a confirmation into the archive would change the archive id on every confirm or Undo, force run
upgrades, and leave old archives with a binding Undo cannot reach. Instead, the backend passes the guide's
confirmations to the Rust evaluation (guide-core, via the backend) next to the definition. Archives and the
prepared cache stay clean, and Undo takes effect at the next evaluation.

A confirmation applies, in every run of that guide (the current one, other in-progress ones and new ones),
only when:

- the row's `guide_id`, `step_key` and `quote` match the definition's step, and
- the confirmed option is still among the line's compatible candidates in that definition.

Otherwise the row is kept but not applied, and the step shows its candidates as before (unknown stays
unknown).

### Completion (coordinator's call after the builder check)

Ambiguous lines are `tracking: "reminder"` today: guide-core reports them `ready / reminder_available`
and `manual_keys` (`application.rs`) refuses a manual check on them. Event answers also have no automatic
completion fact (a popup closing proves nothing). So:

- In guide-core (Rust evaluator change, presentation fixtures regenerated per `CLAUDE.md`), a reminder line
  with an applied confirmation is evaluated as a bound event-answer step: it gets the confirmed option's
  `started_when` (`events.visible_options`) and becomes manually checkable in the run (added to the keys
  the manual check accepts).
- Completion in a run is always a manual check of that step: the "I chose this" click checks it. In a new
  campaign the line is highlighted with the `next` color and labeled "confirmed by you" when its popup is
  visible, and the card shows the same "I chose this" button, which checks the step again for that run.
  No automatic completion.
- Undo of the notice unchecks the step; if the click also created or replaced the confirmation, Undo
  restores the previous state (no row, or the previous row).

### Removing a wrong confirmation (coordinator's call after the builder check)

A misclick must not bind the line forever. The web suite step card of a confirmed line shows the confirmed
answer and a "Remove confirmation" action (Rust route deletes the row; the JS only calls it and presents
the result). A new confirmation on the same line also replaces the old one.

## Builder's calls (left to the implementer)

- The exact module and HTTP route names, and whether the control action reuses `/api/guide/checks` or
  gets its own route.
- How the confirmations reach guide-core (an extra evaluation input or a field of the request), as long
  as definition archives and cached prepared guides never contain confirmations.
- The exact pink value and dash style, checked against the existing dark card background for contrast.
- Web suite: show the same label words ("ambiguous", "confirmed by you") on the step card; no color work.
- Tests: Rust tests for storage (insert, replace, delete, survives reopen), application (applied only when
  key, quote and candidate match), HUD proposal (button only on `visible_candidate`), overlay palette
  mapping; JS tests only for presentation of the new labels.
- Documentation: `docs/events-live.md` and `docs/campaign-progress.md` updated; `docs/AGENT_HANDOFF.md`
  notes the change. Live behavior NOT VERIFIED in game until the user checks it.
