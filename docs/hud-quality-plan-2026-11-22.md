# HUD quality plan (2026-11-22)

Follows the responsiveness plans `docs/hud-performance-plan-2026-11-20.md` and `-2026-11-21.md` (both archived). Items F1 and F2 come
from the first plan's section "Follow-up after step 7". Same invariants: read-only toward the game, unknown stays unknown, highlights
only with a verified witness, Rust decides guide status. Never raise the 750 ms geometry deadline or the 3 s delivery budget, and
never draw a marker at a position that no fresh camera read supports.
Each item is measured (trace or a user screenshot) before any qualified geometry changes. Agents never start or control HOI4; live
checks are batched into one session with the user at the PC.

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
- The trace does not record map markers: `marks` counts every highlight, `map_unavailable` is not written.

## Steps (one per session; `/clear` between them; the Resume prompt in `docs/AGENT_HANDOFF.md` says which is next)
F1. Map pan and zoom: consistent markers while the map moves, no misalignment or flicker.
1. [ ] Diagnostics, no behavior change. Trace per iteration: `map` (drawn `construction_map` rows), `map_err` (short
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
2. [ ] Live check with the user: a guide step with construction targets (the HUD shows map markers), ~5 min of pan and zoom at
   several zoom levels, plus 2-3 user screenshots of markers over the map (still camera) to judge H4. Read the "Map markers"
   section; decide which of H1-H4 hold.
3. [ ] Fix by the numbers only. Candidates: for H1 retry the camera read in the same request when verify sees a change (the read is
   ~120 bytes), and treat a sample whose map read failed as unverified for `construction_map` rows instead of a verified absence;
   for H2 never hold a map marker's old rectangle across a camera change (hide it until a fresh projection); for H4 only with
   screenshots that show the offset. Rust tests; rebuild; then a short live check.

F2. Native qualification of edge cases (after F1). Each sub-item starts with a measurement or a screenshot; no geometry change
without one.
4. [ ] Variable DPI and UI scale: list which geometry paths assume 100 % (overlay window scale, GUI matrix scale, map viewport
   check), then a live check at one other Windows scale or HOI4 UI scale chosen by the user.
5. [ ] Inactive projects and the commander roster: list what the native observers report today and what the guide proposes for
   them, then decide with the user which to qualify.

## Measured
Nothing yet.
