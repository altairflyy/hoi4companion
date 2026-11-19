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
- `–`/`+` collapses the card to its header line; `×` hides it. `Ctrl+Alt+H` (a HUD-owned global hotkey) or the control panel's "Show/hide step card" button shows it again. The position and collapse are saved; a hide lasts only until the HUD restarts (the desktop app's HUD has no tray icon to bring a hidden card back).
- `✓ Mark done` appears only when the backend marks the card `checkable`: the step takes a manual confirmation (`application::manual_keys`, the same rule as the suite's checkbox) and is not completed. The HUD sends overlay kind `check` (`guide_id`, `key`, `checked`); the backend checks the key only if it is the current card's checkable step in a current delivery (`hud_check_allowed` in `http.rs`), then writes it through `app_checks`. A stale card is refused ("The current step changed"). The card shows the result for 8 s with an `Undo` button, which unchecks that manual confirmation. Observed steps (focus, research, ...) still complete from the game only.
- Captions of highlighted controls avoid the card wherever it is placed.
- A lost backend connection is now reported as unknown delivery (`live_delivery_fresh: false`, `connection_lost: true`) so the card is kept briefly; the guide list is still cleared on a lost connection.

## Step card production plan (2026-11-16)
The card lists the guide's current production targets under a "Production" line, so the player sees how many factories to assign without opening the production panel: one line per product, `✓`/`•` + "product · Target N (or ALL remaining factories) · requested R / assigned A", plus "Target met" and the line priority.
- Backend: `step_card.production` (`hud_service::card_production`, at most 8) copies the HUD's own production proposals (`hud_production::proposals`: production steps the Rust evaluation has ready, active, completed or awaiting manual confirmation, superseded targets excluded), in guide order, with the same field names (`title`, `quantity`, `quantity_mode`, `requested`, `assigned`, `goal_satisfied`, `production_priority`, `guide_step`). Live only: when the guide does not apply (no live evaluation) there are no proposals and no list. Unknown counts are shown as `?`, never as met.
- Overlay: `production_line` in `main.rs` words both these lines and the production-line highlight captions. Hidden when the card is collapsed.

## Step card guided line (2026-11-19, UX R9)
The card speaks like the suite's guided mode. `step_card` adds `category` (`guides::display_category`, the Rust port of `category()` in `web/action-plan.js`, both checked against `tests/fixtures/display-category-contract.json`), `verification` (`import_reports::verification`), `started_when`, `reading_title` and `original` (the R8 composed title, only for lines understood in full), and `then`/`waiting` (`{key,heading,reading_title}` of the next open step that is not waiting, and of the nearest waiting one, after the card's step in the same track, from the Rust evaluation statuses).
- Overlay: `guided_line` words "Category · When · Verification" with the suite's labels, drawn above the heading; `reading_title` translates the templates (`i18n::format`) and replaces the heading, the author's text staying once as "In the guide: …" (never for a recognized event answer); "Then: …" and "Waiting: …" are drawn after the body. Presentation only.
- UX R10: `waiting.reason` says why, in the suite's order (`guidedReason` in `web/guide.js`): `{kind:"step",step}` (first `blocked_by` of the evaluation), `{kind:"entity",entity,name,relation}` (compiled `trigger_resolution`), `{kind:"text",text}` (unrecognized trigger), `{kind:"reason",code}` (evaluation reason), else null. The overlay's `waiting_reason` words it as "Waiting: X · reason"; a code is read through the `runtime_reasons` table of `translations.json` (`i18n::reason`) and an unlisted code shows nothing.

## Step card reference images (2026-11-16)
The card shows the same design/template pictures as the walkthrough page (`guide.js`), e.g. the F1ca design for "Change I-16 to F1ca … production to 15".
- Backend: `step_card.images` = `[{id,name}]` from the step's `action.params.reference_images`, then `design_reference.image_id` and each `production_targets[].design_reference.image_id`; deduplicated by id, at most 3 (`hud_service::card_images`). Presentation only; no evaluation change.
- Overlay (`native/overlay/src/card_images.rs`): fetches `GET /api/reference-library/image?id=` over loopback on a short-lived thread, decodes PNG or JPEG (the library's stored formats; `image` crate `png` + `jpeg`), scales to at most 1024 px, converts to premultiplied BGRA and caches by id (at most 16 entries kept beyond the current card). The HUD loop never waits: `current_card` attaches the pictures decoded so far.
- Drawing (`draw_card`): under the text, each image has a muted "Design reference · name" line and a thumbnail fitted to the card width at the DPI scale, at most a third of the card's maximum height (80% of the client height). A picture that does not fit is left out; collapsing hides all of them.
- Failure: until the picture arrives, or if fetch/decode/bitmap creation fails, only the name line is shown; a failed id is retried after 60 s. The card redraws once when a picture arrives (`CardImage` equality compares the shared decoded picture, so later frames do not redraw).

## Construction map markers (2026-11-16)
The current step's catalog-bound `construction_targets` propose one small (12 px)
marker per state ID. Names are captions only. Installed `map/buildings.txt` supplies
static graphical anchors (factory/airbase positions), respecting the existing mod
file replacement rules. These positions never supply building levels or completion.
Live country, observed ownership/control, guide/session/campaign and current-card
identity gate the proposals. Completed steps stop proposing markers.

For qualified PE SHA-256
`709f916afbebcfe64a26678702691aeb40945d206074c2a80de8e9d601fce148`,
`map_markers.rs` reads the current camera via the existing read-only qualified
handle. Globals: idler RVA `33ddb08` -> camera at `+760`; graphics RVA `3500ed0`;
map RVA `33e7700`. Camera matrix at `+90`, center X at `+190`, viewport at
`+6c8/+6cc`; graphics viewport at `+180/+184`; world width at map `+40`.
The projection uses column-major clip coordinates, nearest horizontal world wrap,
positive W, depth [0,1], and viewport clipping. All reads are revalidated; a changed
camera, mismatching viewport or unknown build yields no marker. Central panels
recognized by native GUI observation suppress map markers. Existing foreground,
window, guide revision and 750 ms freshness gates still apply. No map OCR or screen
capture fallback starts when native map evidence is unavailable.

Evidence: static disassembly of projection VA `14121d5c0` and a separate external
RPM-only probe on the installed build/current game. 39 native VictoryPoint icons
matched the formula plus native alignment (0,15) with maximum rounding residual
0.499 px; eight airbases independently matched alignment (16,16). Unit-stack
interpolation was excluded. The static anchor is projected directly without icon
alignment. At the sampled 1280x720 view, Minsk, Vinnytsia and Odessa were inside;
Moscow, Kharkov and Krasnodar were outside. No game interaction or screenshot was
used. Pan/zoom/resize transition and final visual appearance need gameplay review;
the numerical probe does not establish those results.

Regression checks cover wrapping/clipping/camera translation, exact-build and
viewport rejection, current-step state ID deduplication/country rejection, and
native renderer routing without capture fallback.
Installed release verification: current SOV card2 active; six state proposals and
three native map rectangles (Odessa/Vinnytsia/Minsk), native_available/usable true.
Observed geometry2.48ms, witness age1.29ms, map read120bytes. Installed backend and
HUD hashes match optimized builds; Companion restarted, HOI4PID64832 unchanged.
This is API/native-coordinate verification; final visual gameplay was not inspected.

HUD requests now renew a two-second demand lease on the existing shared reader.
The observer runs while either SSE clients or a live HUD need it; closing the
browser no longer disconnects the HUD. This does not extend the separate data
(2 s) or geometric (750 ms) evidence budgets. With neither client, the original
reader shutdown remains. tests/test_native_hud_demand.cjs exercises HUD-only
requests, missing-game unknowns, expiration and resumption through real HTTP.
