# Loop di miglioramento autonomo — design

Data: 2026-11-18. Stato: in attesa di approvazione.

## Intento
L'utente lancia il loop e lo lascia lavorare senza supervisione fino al completamento.
Il loop migliora HOI4 Companion (funzioni, comprensione delle guide, runtime, architettura,
prestazioni, UX) con interventi piccoli e verificati offline, ognuno in un commit locale.
Al ritorno l'utente rivede i commit uno per uno.

## Obiettivi
- Lavoro continuo senza domande intermedie.
- Ogni intervento ha un criterio di verifica oggettivo, scritto prima di implementarlo.
- Lo stato vive su file, non nella conversazione: regge alla compattazione del contesto.
- Riusa l'harness esistente invece di riscoprirlo a ogni giro.

## Non obiettivi
- Nessuna infrastruttura nuova (niente orchestratori, server, routine cloud).
- Nessuna verifica in gioco: la compatibilità live resta "NOT VERIFIED" se testata solo offline.

## Anti-obiettivi (fallimento anche se "funziona")
- Toccare il gioco, i dati utente in `.local/` o i binari installati in `src/hoi4companion/_native/`.
- Fermarsi per una domanda e sprecare la notte.
- Commit che mescolano lavori diversi, o modifiche UX approvate dal loop stesso.
- Aumentare la copertura delle guide a scapito della precisione.

## Approcci considerati
1. Prompt migliorato incollato in una sessione: semplice, ma lo stato vive solo nel contesto. Scartato.
2. **Skill di progetto + backlog su file + branch dedicato: scelto dall'utente.**
3. Come la 2 ritmato da `/loop`: parte in più da gestire, nessun vantaggio concreto per cicli di pochi minuti. Scartato.
Routine cloud esclusa: niente Windows/MSVC, non compila né testa il progetto.

## Design
**File**
- `.claude/skills/improve-loop/SKILL.md`: istruzioni del loop, corte, in inglese come le altre skill.
  Rimanda a `CLAUDE.md` per invarianti, build e test invece di ripeterli.
- `docs/improvement-backlog.md`: la lista dei lavori. Ogni voce ha area, priorità, criterio di verifica,
  stato (`todo` / `doing` / `done <commit>` / `needs-user: <motivo>` / `dropped: <motivo>`).
  In fondo, il report finale della corsa.

**Avvio**
1. Legge `CLAUDE.md`, `docs/AGENT_HANDOFF.md`, `git status`. Se l'albero non è pulito si ferma subito.
2. Crea il branch `auto/improve-<data>` dal commit corrente, nello stesso checkout.
3. Se il backlog è vuoto o assente, lo semina da fonti misurabili: "Not done" e "Residual"
   dell'handoff, `event_reference_survey`, `docs/guide-interpretation-benchmark.md`, tempi di avvio,
   più un giro UX con screenshot di `suite.html` (pannello browser). Massimo ~15 voci, ordinate per
   impatto/rischio/costo.

**Ciclo (per ogni voce, massimo 3 poi si rivaluta l'ordine del backlog)**
1. Segna la voce `doing`, scrive il criterio di accettazione se manca.
2. Implementa la soluzione più semplice. Le regole di valutazione vanno in Rust (invariante del progetto).
3. Verifica: test mirati, poi `npm test` e `node scripts/test-native.cjs <crate>` per i crate toccati.
   Rigenera le fixture di presentazione solo secondo `CLAUDE.md` e ne controlla il diff.
4. Se tocca valutazione, stato live, HUD, pagine web, importer o packaging: agente `invariant-reviewer`
   sul diff, e correzione di ciò che segnala.
5. Per un bug: annota nella voce perché i test esistenti non l'avevano preso, e aggiunge il test mancante.
6. Commit locale, uno per voce, messaggio Conventional Commits. Le modifiche UX hanno `[ux-review]`
   nel messaggio e screenshot prima/dopo nella scratchpad, citati nella voce.
7. Aggiorna backlog e, se il traguardo è significativo, `docs/AGENT_HANDOFF.md`.

**Voci bloccate**
- Serve una decisione dell'utente: `needs-user: <domanda>`, si passa alla voce successiva.
- Due tentativi falliti sulla stessa voce: `dropped` con il motivo, modifiche annullate solo per quella voce.

**Arresto**
- Backlog senza voci `todo` eseguibili, oppure 15 voci completate, oppure 8 ore dall'avvio.
- Subito, se resta una regressione che non riesce a risolvere: lascia l'albero all'ultimo commit verde.
- Alla fine: suite completa (JS + tutti i crate Rust), report in fondo al backlog e in chat:
  miglioramenti, strumenti usati, migliorie all'harness, test PASS/FAIL/NOT VERIFIED, commit,
  voci `needs-user`, prossima priorità.

**Risorse**
- File temporanei solo nella scratchpad della sessione; cache di build esistenti in `.local/rust-dev-*` riusate.
- Nessun push, merge, installazione (`deliver-build`) o riavvio dell'app durante la corsa:
  la consegna avviene dopo la revisione dell'utente.

## Scelte mie (da controllare)
- Stesso checkout, non worktree: un worktree partirebbe con cache Rust fredde e senza i dati di
  `.local/` necessari ai survey sul catalogo reale. Durante la corsa l'utente non deve usare il checkout.
- Limiti 15 voci / 8 ore / 2 tentativi: valori iniziali, modificabili nella skill.
- Backlog in `docs/` e committato, così resta sul branch insieme ai lavori.

## Lasciato a chi costruisce
- Formato esatto delle voci del backlog e testo della skill.
- Comandi precisi per i survey e la cattura degli screenshot.
