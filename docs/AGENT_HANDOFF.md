# Shared agent handoff
Updated: 2026-11-23. Cap 6 KB, lines <= 300 chars (`tests/test_agent_handoff_size.cjs`). History: `docs/handoff-archive-2026-11-19.md`, `-2026-11-20.md`, `-2026-11-21.md`, `-2026-11-22.md`, `git log`.

## Next actions
Piano UX chiuso. F1 riaperta su SOV. In attesa delle catture H3.
Resume prompt:
> Riprendi F1 in docs/hud-quality-plan-2026-11-22.md. H3: confronta catture al bordo e traccia. H4: verifica a due zoom solo un edificio identificato. L'utente controlla HOI4. Correggi solo scostamenti misurati, testa, riprova e aggiorna piano e handoff.
Optional HUD responsiveness work (full read, HUD `project`): plan 2026-11-21 step 4b.

## In progress
- 2026-11-20 PURGE (user order): third-party guide data (benchmark/review/probe fixtures, Wojtek guide, `docs/it/walkthrough.md`) is
  LOCAL ONLY in `.local/guide-corpus` (`scripts/guide-corpus.cjs`; tests skip without it). History rewritten with filter-branch on all
  branches and tags, force-pushed (main `f43c37a`, 4 remote branches, tags v0.1.0-v0.2.0): ALL commit hashes before it changed.
  Backup of the old history: `.local/repo-backup-before-purge-2026-11-20.bundle` (contains the files; never push it).
  Release ZIPs (+ .sha256) of v0.1.0-v0.1.3 deleted on user request (v0.2.0 kept). Still exposed: merged PR refs #1 #2 and cached
  old commits stay reachable by SHA until GitHub Support purges them. Never commit or push third-party guide content.

## Recently verified
- 23 novembre, riapertura F1: i sei bersagli «Reverse Barbarossa» hanno coordinate di punti vittoria. Artefatti in `.local/hud-quality-f1/h3-h4/`. Nessuna misura al bordo o su un edificio. Vedi piano HUD F1.
- 2026-11-23 F2 passo 5: progetti inattivi e comandanti in Stato della nazione. WebView2 157 a 1282 × 832 e 740 × 560, DPR 1, tastiera e ritorno alla home. Fixture osservate, vuote, indisponibili e obsolete. Test web 517 passati, 3 saltati.
- Catture prima/dopo e `installed-replay.log` in `.local/ux-captures/f2-step-5/`. Riprova riuscita sul backend installato, profilo isolato chiuso. Acquisizione live e comportamento in gioco fuori prova.
- 2026-11-23 F2 passo 4: WebView2 157, 1282 × 832 e 740 × 560, DPR 1. Corretti focus e visibilità del primo titolo di «Ora». Test web 89/89.
- HUD fittizio a 96 DPI: 360 × 283 e 311 × 304 px, Produzione leggibile, prova 1/1. Catture e log in `.local/ux-captures/f2-step-4/`. Dettagli nei piani UX e HUD.
- 2026-11-23 F1 scheda JAP: 360 × 466–483 px nel client 2560 × 1440, leggibile e libera dai controlli visibili in Produzione.
  L'utente conferma nascondi/richiama con `Ctrl+Alt+H`. Evidenze `.local/hud-quality-f1/card-proof.png`, `card-windows.jsonl`, `card-trace.jsonl`.
- F1 scheda SOV: posizione salvata a y=43 px, sovrapposizione alla fila notifiche quando è popolata. `.local/hud-quality-f1/map-proof-1.png`.
- 2026-11-23 F1 marker SOV: baseline 300 s, H2 in 9 cambi di camera; con `0a6a820` 0 su 7 comparabili (60 s). Overlay 80/80.
  H1: cinque lampeggi da lettura incoerente; con `d82c1f6` 0 errori camera in 1386 iterazioni in movimento (60 s), live 73/73.
  `.local/hud-quality-f1/map-baseline-report.txt`, `map-retest-report.txt`, `map-final-report.txt`, `map-proof-1.png`. H3–H4 pendenti.
- 2026-11-20 `3ac001a` HUD card: a production step is skipped as current step once a later step of its track is completed (cause:
  layouts stop matching when lines change or read briefly unknown). Production list behind a toggle (`production_open` in overlay-card.json).
  Rust unit tests green, rebuilt, app restarted. NOT seen in game.
- 2026-11-20 `8bfbdc5` reload bug: a save reload (tick < bound tick) made a `timeline_rewind` run with no set_aside and no checks
  (seen: run `fa0f389e` 3 aside/21 checks -> `f58745bb` empty). Checks now get `checked_at` (campaign tick, bound runs only, new
  confirmations only); rewind keeps set_aside + checks dated <= loaded tick. Storage/backend/guides tests green; rebuilt, restarted,
  API loads. NOT checked with a real reload. Old undated checks are not recoverable automatically.

## Open questions / hypotheses
- HUD production highlights look narrow (user, 2026-11-20). Box = row container size from memory (`gui_production.rs`, row+0x17AC/0x17B0
  x matrix scale), e.g. Artillery [8,872,508,983]; a scaled screenshot suggests it misses the right column and some height. NOT measured:
  the overlay window cannot be captured; needs a user screenshot with borders before touching qualified geometry.
- R7 prefixes use game-start ownership only: a guide for a later bookmark could bind a prefix unique at start but not in its campaign.
- Research "UP" steps (`98bc02a`) and abbreviated/written-out references: unit and catalog checks pass, NOT verified live in game.
- Allineamento ai singoli edifici e al bordo per i marker pendente (H3/H4). Unit stacks unqualified.
- F2 passo 4: DPI di sistema diversi, scorciatoia e interazioni native, sovrapposizione ai controlli in partita da verificare.
- Intermittent startup/import JS test seen once; no root cause claimed.

## Working boundaries
- Preserve active guide, campaigns, imports and concurrent edits. Never close, foreground or control HOI4; closing/installing/restarting Companion/HUD is allowed.
- Commit finished work locally in focused commits, then rebuild/restart the app. Never push without asking.
- End of objective: rewrite `Resume prompt` for the next objective (Italian: scope, files, preconditions, closing routine) and show it in a fenced block.
- System WebView2 Runtime is a prerequisite of the desktop host.

## Where things are
- Subsystem docs: `docs/README.md`. HUD plans (archived): `docs/hud-performance-plan-2026-11-20.md`, `-2026-11-21.md`; quality plan `docs/hud-quality-plan-2026-11-22.md`. Fast Rust loop: `docs/fast-local-rust.md`. Guide identity: `docs/guide-definition-identity.md`.
- Automation: skills `deliver-build`, `add-translation`, `improve-loop`; hooks `guard-game.cjs`, `web-test.cjs`; agent `invariant-reviewer`.
