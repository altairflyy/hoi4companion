# HUD quality plan (2026-11-22)

Follows the responsiveness plans `docs/hud-performance-plan-2026-11-20.md` and `-2026-11-21.md` (both archived). Items F1 and F2 come
from the first plan's section "Follow-up after step 7". Same invariants: read-only toward the game, unknown stays unknown, highlights
only with a verified witness, Rust decides guide status. Never raise the 750 ms geometry deadline or the 3 s delivery budget, and
never draw a marker at a position that no fresh camera read supports.
Each item is measured (trace or a user screenshot) before any qualified geometry changes. Agents never start or control HOI4; live
checks that use the same game setup can be batched when the user is at the PC. Live work remains paused until the user reopens it.

## Baseline before F1 steps 2–3 (read 2026-11-22)
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
3. [x] Fix by the numbers only. Candidates: for H1 retry the camera read in the same request when verify sees a change (the read is
   ~120 bytes), and treat a sample whose map read failed as unverified for `construction_map` rows instead of a verified absence;
   for H2 never hold a map marker's old rectangle across a camera change (hide it until a fresh projection); for H4 only with
   screenshots that show the offset. Rust tests; rebuild Companion; then a short live check under the same conditions.
   Record the findings and evidence paths. If evidence is missing, leave the affected check pending. If no fix is warranted,
   record that outcome for step 3.

### F2 · Offline UI/UX only (from 2026-11-23)

Use an isolated Companion profile with deterministic fixtures. The next sessions focus on what the UI displays and how it responds.
Live qualification of HOI4 readers, camera geometry, UI scaling and HUD placement relative to the game remains pending.

- [ ] Step 4 · DPI and layout. Compare the same Companion views at two scales or viewport sizes. Check readability, footprint,
  clipping, alignment and keyboard focus in WebView2. Cover the HUD card through isolated rendering or existing fixtures where
  available. Record the actual scale or viewport used and keep findings specific to that test surface.
- [ ] Step 5 · Inactive projects and commander roster. Use fixtures for observed, empty and unavailable data. Check labels,
  hierarchy, navigation and whether the UI clearly distinguishes unavailable data from an empty list.
## Measured

### F1 · 2026-11-23

- **Scheda.** Partita giapponese `JAP` / `japan_wtt_focus`, con la guida giapponese attivata dall'utente. Nel client di
  2560 × 1440 px la finestra è rimasta interamente contenuta, con origine `[1783,43]` e dimensioni osservate di 360 × 466–483 px
  (4,55–4,72% del client). Nella cattura nativa con Produzione aperta il titolo, il testo e le sei righe di produzione si leggono
  a risoluzione piena. La scheda copre una porzione di mappa e lascia liberi il pannello Produzione e i controlli visibili.
  L'utente ha confermato che `Ctrl+Alt+H` nasconde la scheda e la seconda pressione la richiama. Evidenze in
  `.local/hud-quality-f1/`: `card-proof.png` e `card-proof.json` (cattura
  verificata), `card-windows.jsonl` (finestra), `card-trace.jsonl` (consegna della scheda e primo piano).
  Su SOV la scheda, alta 635 px con la lista Produzione espansa (6,20% del client), resta entro il client e copre parte
  della fila delle notifiche in alto all'origine salvata `[1783,43]`. Evidenza `map-proof-1.png`. La posizione salvata è
  rimasta quella scelta dall'utente. Il posizionamento predefinito non è stato provato.
- **Marker.** La proiezione giapponese è priva di marker. Su `SOV` / `soviet_focus`, il passo di costruzione di «Reverse
  Barbarossa» risulta completato. Per la prova è stata selezionata temporaneamente «Third Rome», con passo corrente
  `word-0001--line-2` e due marker unici (Chita 563 e Buriazia 564). «Map markers» nel preflight registra 3969 iterazioni
  con proposte, 3968 sospese per `central_panel` (Produzione), zero proiezioni. Evidenze `.local/hud-quality-f1/map-preflight.jsonl`
  e `map-preflight-report.txt`.
- **Baseline marker.** Pan e zoom a più livelli per 300,0 s, client 2560 × 1440, prima della correzione. «Map markers» conta
  14.932 iterazioni in primo piano con proposte, 10.616 con marker disegnati, 3955 cambi di firma camera e 91 sospensioni da
  pannello centrale. Evidenze in `.local/hud-quality-f1/`: `map-baseline.jsonl`, `map-baseline-report.txt`,
  `map-baseline-comparison.json`, `map-baseline-blinks.jsonl`. L'avvicinamento precedente è in `map-approach.jsonl`.
  - **H1, rara e isolata nella riprova.** Nella baseline due letture incoerenti su 5507 iterazioni in movimento (0,036%). Nessuna coincide con i
    10 intervalli senza marker poi ricomparsi entro 1 s: uno ha zero punti proiettati, nove passano per la cattura mentre
    il backend proietta due punti. Il ramo cattura ha priorità quando la geometria del popup evento non è qualificata.
  - **H2, confermata.** 11 iterazioni con più marker disegnati che proiettati, nove su un cambio di camera con meno punti
    proiettati rispetto al campione precedente. Il report conta quattro frame in movimento con soli marker mantenuti.
  - **H3 e H4, pendenti.** Una cattura nativa verificata a camera ferma (`map-proof-1.png` e `map-proof-1.json`) mostra
    Chita e Buriazia nella zona attesa. Entrambi i marker sono interni al viewport. La cattura non individua l'icona dello
    specifico edificio usato come ancora. L'utente ha chiuso la raccolta dopo questa immagine, quindi manca il confronto
    a più zoom e sul bordo.
- **Correzione H2.** `0a6a820`: i marker escono immediatamente quando il campione corrente non li conferma. Test di
  regressione riprodotto prima della modifica, suite overlay 80/80 dopo, report 8/8. Build overlay Dev reinstallata e
  riavviata. Log `overlay-test-before.log`, `overlay-test-after.log`, `overlay-build.log`.
- **Riprova H2.** 60,0 s sulla stessa partita, 3343 iterazioni con proposte, 2497 in movimento. Zero marker mantenuti su sette
  cali di proiezione con cambio camera, contro nove su nove nella baseline. Cinque letture incoerenti causano cinque interruzioni
  singole di 16–19 ms; la sesta interruzione (319 ms) ha campioni verificati senza punti. Evidenze `map-retest.jsonl`,
  `map-retest-report.txt`, `map-retest-comparison.json`, `map-retest-blinks.jsonl`.
- **Correzione H1.** `d82c1f6` rilegge la camera una sola volta quando la verifica segnala un cambio durante il campione.
  Usa solo la nuova proiezione verificata e conserva la soppressione se anche la seconda lettura è incoerente. Test di
  regressione fallito prima, suite `live` 73/73 dopo. Ricompilati e reinstallati backend e DLL Dev, Companion riavviato,
  stesso processo HOI4. Log `map-retry-test-before.log`, `map-retry-test-after.log`, `backend-build.log`, `final-launcher.log`.
- **Riprova finale.** 60,0 s sulla stessa partita, 3050 iterazioni con proposte, 1386 in movimento, 2997 con marker visibili.
  Zero incoerenze camera riportate e zero marker mantenuti, inclusi due cali di proiezione con cambio camera. Quattro
  intervalli brevi senza marker, senza errori camera: uno con zero punti proiettati (222 ms) e tre nel ramo idle (13–20 ms).
  Evidenze `map-final.jsonl`, `map-final-report.txt`, `map-final-comparison.json`, `map-final-blinks.json`. Riattivata la guida
  «Reverse Barbarossa» al termine. Nessuna modifica ai margini o alle ancore. F2 prosegue con prove UI/UX offline e fixture.
