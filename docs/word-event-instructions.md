# Event instructions in Word guides

The source's `RELEVANT EVENTS` section can be a historical calendar (Japan's
war in China, the invasion of Poland, etc.). Such entries are not a list of
popup answers and their dates do not prove an event happened. The Word importer
does not manufacture choices or executions from that calendar.

The existing operation compiler now recognizes explicit event/button instructions
in the Actions column, including `Event: title; choose: answer`,
`On title event: choose answer`, `On event: answer`, `answer (event)` and
`answer… (event x2)`. It resolves the English/local labels through the existing
country-scoped index and requires a unique option **and its catalogued parent**.
Explicit costs, source evidence, order, dependencies and authored conditions
are preserved. Existing installed Word guides receive the same cached in-memory
adaptation; no source document or saved guide needs rewriting.

Bare abbreviated answers require at least four whole words, or a positive
national identity hint declared by the event's top-level trigger conjunction.
Only direct `tag`/`original_tag` tests (including AND) provide these hints.
Other scopes, OR/NOT, missing triggers and contradictory tags do not. These hints
assist identity matching, never live eligibility or campaign identity. Short
generic labels such as `Welcome back…` must not silently bind an unrelated
country's similarly named answer. Outcome instructions such as "always increase
war support" and king-selection policies remain reminders until they specify
an unambiguous answer or a separately supported rule. No fuzzy choice or AI
interpretation is introduced.

Resolved pairs feed the existing guide event plan, runtime conditions and native
HUD watch list. The plan uses the same country constraints as import. It is
compiled on guide/catalog changes, with no new polling or OCR loop. No click is
automated. Visible choices can establish that an event is present; disappearance,
a click or spending political power does not prove which answer was executed.
The text `x2` remains source context, not evidence of two completed executions.

This importer work does **not** qualify previously unsupported native fields.
On executable profile `MILITARY_HASH`, event-response GUI fields remain
unavailable and popup occlusion continues to suppress underlying tree highlights.
Real event-button highlighting on that build requires separate native-layout
qualification and a real popup comparison. Unit/fixture tests of an older
qualified layout are not proof that it works on the current game build.

Guide Schema v1, the authoring prompt and existing explicit event/option fields
are unchanged. Use the existing authoring `event` action for an exact title and
answer when the Word source is too vague.
