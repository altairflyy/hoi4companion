# HUD quality plan (2026-11-22)

Follows the responsiveness plans `docs/hud-performance-plan-2026-11-20.md` and `-2026-11-21.md` (both archived). Items F1 and F2 come
from the first plan's section "Follow-up after step 7". Same invariants: read-only toward the game, unknown stays unknown, highlights
only with a verified witness, Rust decides guide status. Never raise the 750 ms geometry deadline or the 3 s delivery budget, and
never draw a marker at a position that no fresh camera read supports.
Each item is measured (trace or a user screenshot) before any qualified geometry changes. Agents never start or control HOI4; live
checks that use the same game setup can be batched when the user is at the PC. Live work remains paused until the user reopens it.

## What the code does today (read 2026-11-22, not measured)
- Construction map markers (`native/live/src/map_markers.rs`, doc `docs/native-overlay.md` "Construction map markers"): each HUD
  geometry request (`provider.rs` `GuiLane::geometry`, ~every 16 ms) reads the camera matrix, center X, viewport and map width, projects
  each `construction_map` proposal's static `world_point`, then `Sample::verify` reads every range again. A 12 px square per point.
- Any failure (camera changed between read and verify, viewport mismatch, unqualified build) only sets `diagnostics.map_unavailable`;
  the geometry reply stays usable and lists `construction_map` in `handled_panels`. The overlay then treats that sample as a VERIFIED
  sample without markers (`main.rs` `Observation::Verified`), so `stability::MarkHold` drops a marker at the 2nd such sample in a row
  (`ABSENT_CONFIRMATIONS`) or once its witness is older than `HOLD_GRACE` (150 ms), and meanwhile draws it at its previous rectangle.
- Hypotheses for F1 (none measured): H1 while the camera moves, verify fails often, so markers blink off and on; H2 a held marker
  is drawn at its old position for one more sample, so it trails the map; H3 the 8 px edge margin hides markers near the window edge;
  H4 the anchor is projected without icon alignment, so a marker may sit off the building it names at some zoom levels.
- Before F1 step 1, the trace only had the aggregate `marks` counter. The current map-specific fields are listed below.

## Objectives and session boundaries

Use one chat per objective. Within an objective, keep measurement, any evidence-backed fix, and the retest together.
The Resume prompt in `docs/AGENT_HANDOFF.md` carries the next objective and its prerequisites.

### F1 · HUD card and map markers in a compatible game

Pair the pending live UX check from `docs/ux-implementation-plan.md` step 8 with F1 steps 2–3. Start with the HUD card, then
use a guide step that proposes construction markers for the map trial. Keep separate outcomes for the card and the markers.

1. [x] Diagnostics, no behavior change. Trace per iteration: `map` (drawn `construction_map` rows), `map_err` (short
   `diagnostics.map_unavailable`, once per change like `native_error`) and the camera state from `map_markers::geometry` diagnostics
   (center X and the matrix scale, rounded) so the report can split still and moving camera. `hud-trace-report.cjs` section
   "Map markers": foreground iterations with map proposals, share with markers drawn, `map_err` reasons, blink episodes (markers go
   to 0 and back within 1 s), each split by still/moving camera. Rust (live, overlay) + report tests; rebuild.
   Done 2026-11-22 (code): `map_markers::geometry` returns `camera` = [center X, m0, m5, m12, m13] at 5 significant digits; the
   provider adds `map_points`, `map_camera`, and `map_skipped` (`gui_unavailable`, `resolution`, `central_panel`) when no projection
   ran. Overlay trace, only while the delivery proposes map markers: `map_n`, `map` (drawn, held rows included), `map_pts`,
   `map_cam`, `map_err`, `map_skip`. Report section "Map markers": per camera state (moving = a signature change within 300 ms,
   unknown = no signature within 300 ms) iterations, drawn, projected, held-only (drawn without a projection), read failures,
   skips; read errors and skip reasons; blinks (drawn, none, drawn within 1 s, reset by a skip). No behavior change.
2. [ ] Live check with the user. Assess card readability, footprint, alignment and whether the shortcut shows or hides it.
   For a guide step with construction targets (the HUD shows map markers), collect ~5 min of pan and zoom at
   several zoom levels, plus 2-3 user screenshots of markers over the map (still camera) to judge H4. Read the "Map markers"
   section; decide which of H1-H4 hold.
3. [ ] Fix by the numbers only. Candidates: for H1 retry the camera read in the same request when verify sees a change (the read is
   ~120 bytes), and treat a sample whose map read failed as unverified for `construction_map` rows instead of a verified absence;
   for H2 never hold a map marker's old rectangle across a camera change (hide it until a fresh projection); for H4 only with
   screenshots that show the offset. Rust tests; rebuild Companion; then a short live check under the same conditions.
   Record the findings and evidence paths. If evidence is missing, leave the affected check pending. If no fix is warranted,
   record that outcome for step 3.

### F2 · Variable DPI and UI scale (step 4, after F1)

- [ ] List which geometry paths assume 100 % (overlay window scale, GUI matrix scale, map viewport check). Measure at one other
  Windows scale or HOI4 UI scale chosen by the user. Use the measurement or a screenshot to decide whether geometry needs a change.

### F2 · Inactive projects and commander roster (step 5, after F1)

- [ ] List what the native observers report and what the guide proposes. Choose with the user which case to qualify, then start
  its live check with a measurement or screenshot.

## Measured
Nothing yet.
