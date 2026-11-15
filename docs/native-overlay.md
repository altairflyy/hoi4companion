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
