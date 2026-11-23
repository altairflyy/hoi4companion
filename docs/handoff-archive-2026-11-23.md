# Handoff archive 2026-11-23

Finished history moved out of `AGENT_HANDOFF.md`. Details per step: `docs/ux-implementation-plan.md` and `docs/hud-quality-plan-2026-11-22.md`.

## Recently verified (moved)
- 2026-11-23 F2 passo 5: progetti inattivi e comandanti in Stato della nazione. WebView2 157 a 1282 × 832 e 740 × 560, DPR 1, tastiera e ritorno alla home. Fixture osservate, vuote, indisponibili e obsolete. Test web 517 passati, 3 saltati.
- Catture prima/dopo e `installed-replay.log` in `.local/ux-captures/f2-step-5/`. Riprova riuscita sul backend installato, profilo isolato chiuso. Acquisizione live e comportamento in gioco fuori prova.
- 2026-11-23 F2 passo 4: WebView2 157, 1282 × 832 e 740 × 560, DPR 1. Corretti focus e visibilità del primo titolo di «Ora». Test web 89/89.
- HUD fittizio a 96 DPI: 360 × 283 e 311 × 304 px, Produzione leggibile, prova 1/1. Catture e log in `.local/ux-captures/f2-step-4/`. Dettagli nei piani UX e HUD.
- 2026-11-23 F1 scheda JAP: 360 × 466–483 px nel client 2560 × 1440, leggibile e libera dai controlli visibili in Produzione. L'utente conferma nascondi/richiama con `Ctrl+Alt+H`. Evidenze `.local/hud-quality-f1/card-proof.png`, `card-windows.jsonl`, `card-trace.jsonl`.
- F1 scheda SOV: posizione salvata a y=43 px, sovrapposizione alla fila notifiche quando è popolata. `.local/hud-quality-f1/map-proof-1.png`.
- 2026-11-23 F1 marker SOV: baseline 300 s, H2 in 9 cambi di camera; con `0a6a820` 0 su 7 comparabili (60 s). Overlay 80/80. H1: cinque lampeggi da lettura incoerente; con `d82c1f6` 0 errori camera in 1386 iterazioni in movimento (60 s), live 73/73. `.local/hud-quality-f1/map-baseline-report.txt`, `map-retest-report.txt`, `map-final-report.txt`, `map-proof-1.png`. H3–H4 pendenti.
- 2026-11-20 `3ac001a` HUD card: a production step is skipped as current step once a later step of its track is completed (cause: layouts stop matching when lines change or read briefly unknown). Production list behind a toggle (`production_open` in overlay-card.json). Rust unit tests green, rebuilt, app restarted. NOT seen in game.
- 2026-11-20 `8bfbdc5` reload bug: a save reload (tick < bound tick) made a `timeline_rewind` run with no set_aside and no checks (seen: run `fa0f389e` 3 aside/21 checks -> `f58745bb` empty). Checks now get `checked_at` (campaign tick, bound runs only, new confirmations only); rewind keeps set_aside + checks dated <= loaded tick. Storage/backend/guides tests green; rebuilt, restarted, API loads. NOT checked with a real reload. Old undated checks are not recoverable automatically.
