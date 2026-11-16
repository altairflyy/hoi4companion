# Native Windows HUD

Campaign observations are applied once per qualified state and committed progress
revision. Reading progress or an HUD projection reuses that binding; changes to
the live state, selected guide, definitions, or campaign progress invalidate it.
Guides outside the qualified country/tree scope retain their progress without
running action observation; the scope gate is shared with runtime preparation.
`/api/state.pipeline_ms` separates acquisition, progress observation, and packet
preparation so delayed guide delivery can be distinguished from GUI rendering.
Research discovery requires the actual `CCountryTechTreeView`, rather than the
unrelated `CCountryTechnologyView`. The same identity, ownership, freshness,
visibility and overlap checks still qualify every highlighted rectangle.
Focus geometry composes the item's two local position matrices with the shared
container's world transform. The item's own cached world matrices are not read:
live dragging was observed to temporarily expose a different translation there.
Parent links, types, local transforms and the shared transform are all revalidated
before publishing. Native targeting uses no capture session, OCR, motion filter,
retained transform or predicted position; the 750 ms deadline is unchanged.
WGC frames whose surface/window/client bounds disagree during a transition are
discarded. Capture failures clear geometric/OCR evidence and recreate capture
with a bounded retry delay instead of terminating the HUD; native GUI observation
remains independent of that retry. This never extends a highlight's lifetime.

Production highlights use thin, unfilled borders so model names, icons and
factory assignments remain readable. Pending captions use a compact side column
outside the highlighted production controls; completed blue lines keep their
border without a duplicate caption. Captions never overlap another highlighted
control or caption. If there is no room, the border remains and the caption is
omitted rather than painted over the control. Other panels also place captions
outside highlighted controls. Detailed counts and differences remain available
in the walkthrough. This affects drawing only, not target admission or matching.

Foreground native targeting obtains the current guide projection and qualified
GUI geometry in one `observe` IPC round trip, replacing separate snapshot and
geometry requests. A guide/run revision change during observation discards the
coordinates. Legacy requests and the OCR fallback remain supported. The evidence
deadline still starts before observation, with no extension of the 750 ms limit.
An external out-of-context Windows foreground-event subscription wakes idle
recognition when the player returns to HOI4, instead of waiting for the remaining
100 ms idle interval. Guide selection and pause/resume also wake it. A generation
counter prevents signals being lost between inspection and sleep. No event hook
is injected into HOI4; if the subscription is unavailable the bounded idle wait
still runs. `foreground_events` in the status file reports subscription success.

Direct2D brushes are retained for the graphics-device lifetime; text formats are
retained until DPI changes. Unchanged guide/status metadata does not post another
UI update. Fresh geometric evidence still wakes rendering, even when its bounds
match the previous sample, so an expired/hidden layer can reappear immediately.
Stroke paths are inset by half their physical width, with bounded widths/radii
for small clipped controls. The painted border stays within the qualified widget
at fractional coordinates and DPI, without rounding away native zoom or panning.
These optimizations do not predict coordinates or relax visibility/identity gates.

Cold GUI discovery memoizes unreadable candidate pages and rejected immutable
RTTI resolutions for that single scan. Cleared registry slots (null GUI vtable)
are not followed or processed again for production controls. Known candidate
pages also supply identifier bytes, avoiding duplicate small process reads;
cross-page identifiers retain their exact bounded read. These caches disappear
at the end of discovery. The resulting index contains candidate addresses/IDs,
never trusted coordinates. `GuiSample` still rereads and verifies current widget
type, ownership, country, visibility and transforms before any border is admitted.
Diagnostics expose discovery duration, candidate counts and temporary cache size.

Sample revalidation now runs in the small Rust observer DLL, shared with the live
reader. The caller lends its already verified query/VM_READ handle; the DLL has
no game offsets, process-opening or writing APIs. Each sample supplies current
expected bytes, including the existing visibility-bit masks. No coordinates or
observed states are cached. Partial/unreadable ranges and malformed requests
remain failures. GUI diagnostics include `verification_engine` and
`verification_ms`. The observer crate (`native/observer`) is linked into the backend; `scripts/build-backend.ps1` builds it.

The live change detector prepares an immutable comparison packet only after a
fully qualified sample, then compares exactly the selected bytes twice in Rust.
Unselected gaps are not evidence. New samples replace the packet and closing
the process/session discards it. These are still external memory probes, not
game-engine events. There is no alternative reader to fall back to: a failing
observer comparison is reported as a failure, never replaced by another path.

The external Rust host owns capture, OCR where needed, native controls, and Direct2D rendering. The Rust backend shares guide/runtime projections and qualified GUI geometry with it over local HTTP (`/api/overlay`). The overlay performs no automatic clicks and does not write game memory or files.

Focuses, technologies, category navigation, supported events, political appointments/laws, and projects use the shared targeting pipeline. The exact target must match identity, role/category, guide, campaign, visible panel, and freshness. Access points precede entity selection. Animated research sprites and transient visibility need explicit handling without keeping stale borders indefinitely.

Political power and special-project points gate supported action highlights. Insufficient or unknown resources withhold a border; a costly action must not be skipped for a later cheap one. Literal project costs and qualified balances are separate evidence. Dynamic prices and every positive real-screen alignment are not universally validated.

Build with scripts/build-overlay.ps1. English is the default HUD control language; startup reads the Companion ui_language preference. Restart the HUD after a language change. Guide captions retain their authored/game language.

Detailed historical implementation notes and recorded evidence are preserved in the
[Italian reference](it/native-overlay.md). Those records retain their original dates and
verification limits; they are not a claim that every check was rerun for this translation.

## Current step card (2026-11-16)
While HOI4 is in the foreground and the selected guide applies to the observed game, the HUD shows the guide step to do now in a panel on the right edge of the game window, under the top bar and theater selector (`card_origin` in `native/overlay/src/render.rs`). It appears with or without highlighted controls. The backend adds `step_card` to the HUD snapshot (`hud_service::step_card`): the first not-yet-done step of the main `operations` track (else of any track), as chosen by the Rust evaluation, with its numbering (reminders not counted), the author's text, and a recognized event answer shown first with the author's shorthand under it. Captions of highlighted controls are placed outside the card. Mockup: Claude Design canvas "HUD step card". The card is display-only: it never clicks and never changes the evaluation.

## Visual stability (2026-11-16)
Causes of the reported flicker and jumps, found in code and with live `observe` probes (no game screenshots): almost every transient condition called `renderer.hide()`, which hid the whole layer window and reshowed it on the next sample; the step card shared the highlights' 750 ms witness deadline, so it disappeared with them; one rejected or empty native sample cleared every border at once; after a progress write or a slow read the backend's HUD delivery is briefly missing (`live_delivery_fresh: false`, no `step_card`, empty guide list), which blanked the card and the guide list; every redraw re-ran `SetWindowPos`; captions were first-fit each frame, so they could swap sides.

Rules now (`native/overlay/src/stability.rs`, `render.rs`):
- Highlights: a row witnessed by a verified sample stays for at most one further verified sample without it (`ABSENT_CONFIRMATIONS` = 2) and, through unverifiable samples, for at most `HOLD_GRACE` = 150 ms after its witness. It keeps its original witness time, so the 750 ms deadline is unchanged and still counts from the real observation. A change of guide, guide revision, server run or game window drops every held row at once. Nothing is shown without a verified witness.
- Step card: its own lifetime (`CARD_VALID_FOR` = 1.5 s if the main loop stops refreshing it). It stays while the delivery is unknown for at most `CARD_HOLD` = 2 s, never for another guide, and goes away as soon as a current delivery has no card. Expired highlights leave the card on screen.
- Window: hidden only when nothing may be drawn (paused, HOI4 not in the foreground or moved, no layer alive). `SetWindowPos` runs only when the client area moves or the window was hidden; topmost is re-asserted once per second. Transitional WGC frames publish nothing instead of hiding. Only a guide selection still hides immediately.
- Captions keep the side they used in the previous frame while it stays free; otherwise first fit as before.
- Appearance: a 90 ms compositor fade-in when the layer becomes visible. Disappearance stays instantaneous.

## Step card controls (2026-11-16)
The step card now has its own window (`card` in `native/overlay/src/render.rs`, logic in `card.rs`). It takes clicks but never activates: HOI4 keeps the foreground and receives no input from the HUD. The highlight layer stays click-through.
- Move: drag the card anywhere except its buttons. It stays inside the game's client area; the place is saved as fractions of the client area in `overlay-card.json` next to the status file (`.local/`), so it survives restarts and resolution changes.
- `–`/`+` collapses the card to its header line; `×` hides it. `Ctrl+Alt+H` (a HUD-owned global hotkey) or the control panel's "Show/hide step card" button shows it again. Both choices are saved.
- `✓ Mark done` appears only when the backend marks the card `checkable`: the step takes a manual confirmation (`application::manual_keys`, the same rule as the suite's checkbox) and is not completed. The HUD sends overlay kind `check` (`guide_id`, `key`, `checked`); the backend checks the key only if it is the current card's checkable step in a current delivery (`hud_check_allowed` in `http.rs`), then writes it through `app_checks`. A stale card is refused ("The current step changed"). The card shows the result for 8 s with an `Undo` button, which unchecks that manual confirmation. Observed steps (focus, research, ...) still complete from the game only.
- Captions of highlighted controls avoid the card wherever it is placed.
- A lost backend connection is now reported as unknown delivery (`live_delivery_fresh: false`, `connection_lost: true`) so the card is kept briefly; the guide list is still cleared on a lost connection.
