# HOI4 Companion · Piano di miglioramento UI/UX (2026-11-22)

**Obiettivo.** Rendere immediato riprendere una campagna, capire cosa Companion può verificare e trovare i riferimenti giusti.

**Da dove parte.** `docs/ux-analysis-2026-11-19.md` ha già risolto i problemi di contenuto P1-P10 con gli step R1-R15: categorie,
badge stato/verifica separati, meno rumore, niente sostituzioni a metà frase, rapporto di interpretazione all'import, titoli composti,
modalità guidata «Ora / Poi / In attesa», home senza gioco, pulizia della libreria. Quasi tutti sono marcati «non ricompilato né visto
dal vivo». Questo piano lavora sulla **cornice**: navigazione, gerarchia della home, stati della partita, stati vuoti, tastiera, HUD.
Restano aperti e qui affrontati: le due interfacce (P8, step 7) e la verifica nella finestra desktop degli step R già fatti (step 1 e 8).
Il piano di verifica live dell'HUD (`docs/hud-quality-plan-2026-11-22.md`) resta sospeso.

## Regole per ogni sessione

- Uno step per sessione. Prima di intervenire leggere questo piano e `docs/AGENT_HANDOFF.md`, poi `git status` e i diff pertinenti.
  Eseguire soltanto lo step indicato nel prompt.
- Conservare guide, importazioni, campagne e modifiche concorrenti. Le decisioni di avanzamento restano in Rust. La UI presenta i
  risultati e distingue sempre i dati sconosciuti da quelli verificati. **Uno stato che la UI non sa distinguere dai dati ricevuti si
  distingue nel backend, con un test Rust**; la UI traduce solo un codice in una frase.
- I testi nuovi passano dalla skill `add-translation` (inglese chiave, italiano testo). I testi di questo piano sono bozze: si possono
  accorciare, ma il significato e le negazioni restano.
- Verificare prima il caso mirato, poi le regressioni pertinenti. Fixture esistenti per gli stati di gioco; finestra desktop (suite su
  `http://127.0.0.1:8766/suite.html` dopo `scripts/avvia-tutto.ps1`) per layout e interazioni. Nessuna nuova dipendenza. L'agente non avvia
  né controlla HOI4.
- Catture prima/dopo in `.local/ux-captures/<step>/` (mai nel repository: possono mostrare contenuti di guide di terzi).
- Chiusura di ogni step: test verdi, `invariant-reviewer`, commit locale, rebuild e riavvio se serve, stato ed evidenze qui, handoff
  aggiornato, un solo prompt copiabile per lo step successivo. Se lo step è bloccato, il prompt riprende **lo stesso step** con il blocco
  e la prova mancante. Il report distingue: visto in desktop, coperto da test, richiede una partita reale.

## Esecuzione con Codex

Codex legge `CLAUDE.md` tramite `.codex/config.toml` (`project_doc_fallback_filenames`). Gli strumenti di Claude Code non esistono in
Codex; usare questi equivalenti:

- **Testi (`add-translation`):** `node .claude/skills/add-translation/add.cjs '[["English","Italiano"]]'`, oppure un file `.json` con le
  coppie. Regole in `.claude/skills/add-translation/SKILL.md`.
- **Rebuild e riavvio (`deliver-build`):** solo dopo il commit locale, `node .claude/skills/deliver-build/deliver.cjs` (timeout lungo).
  Le modifiche solo web (JS/HTML/CSS) non richiedono rebuild: basta ricaricare la pagina.
- **Revisione invarianti (`invariant-reviewer`):** prima del commit, leggere `.claude/agents/invariant-reviewer.md` e applicarne la
  checklist a `git diff`, riportando una riga per violazione. Oppure `/review` di Codex con quel file come istruzioni.
- **Hook assenti:** in Codex non girano `.claude/hooks/guard-game.cjs` né `web-test.cjs`. Nessun comando che avvii, chiuda o porti in
  primo piano HOI4; dopo ogni modifica web eseguire a mano i test `tests/test_*.cjs` pertinenti.
- **Catture desktop:** Codex non vede la finestra. Usare Edge headless, già presente in Windows, senza nuove dipendenze:
  `& "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe" --headless --disable-gpu --window-size=1282,832 --screenshot=<file>.png <url>`
  con l'app avviata. Le prove di tastiera e i controlli visivi che una cattura statica non copre vanno elencati nel report come
  «da verificare a mano dall'utente», mai dichiarati visti.

## Metriche

Misurate nello step 1 (prima) e nello step 8 (dopo), nella finestra desktop a 1282 × 832:

- **M1 · Ripresa:** clic e scorrimenti dall'apertura della suite al primo passo leggibile con una guida attiva. Obiettivo: 0 clic, 0 scorrimenti.
- **M2 · Azione primaria:** per ciascuno dei tre stati della home, numero di pulsanti con stile primario. Obiettivo: esattamente 1.
- **M3 · Riferimenti:** clic per raggiungere «Template e riferimenti» da Sala operativa e da Importa ZIP. Obiettivo: 1, con lo stesso titolo.
- **M4 · Stati della partita:** stati della campagna con un messaggio e un'azione propri. Obiettivo: tutti quelli che il backend distingue.

## Step 1 · Correzioni rapide e misura di partenza

**Stato:** step 1 implementato e verificato in Edge 158 headless il 2026-11-22. Navigazione e tastiera verificate anche nel launcher WebView2 in una sessione isolata, con HUD disattivato.

- «Template e riferimenti» nella barra laterale (`data-view="templates"`, `web/suite.js`) apre la stessa vista del link dell'importatore
  (`/importer?view=references`) mantenendo il contesto della suite. «Guide salvate» e «Importa ZIP» conservano le proprie destinazioni.
- Dialogo Configurazione: all'apertura il focus entra nel dialogo, Tab resta nei suoi controlli, `Esc`, «Annulla» e «Chiudi» lo riportano
  al pulsante di apertura. Contorno del focus visibile su tutti i controlli della barra laterale e del dialogo, in tema chiaro e scuro.
- Misurare M1-M4 sulla build corrente e annotare quali effetti di R1-R15 si vedono nella finestra (titoli composti, badge doppi,
  modalità guidata, anteprima senza gioco, libreria raggruppata, card `interpretationOverview`). Quelli non visibili diventano voci aperte.

**Completato quando** Importa ZIP → Template e riferimenti → Importa ZIP funziona in desktop senza caricamento persistente e con titoli
coerenti; l'accesso da Sala operativa porta alla stessa vista; il giro da tastiera nel dialogo funziona; M1-M4 e la lista R1-R15 sono scritti qui.
Test: `tests/test_suite.cjs`, `tests/test_importer_view.cjs`.

### Misura di partenza · 2026-11-22

Edge headless a **1282 × 832**, lingua italiana, backend Rust dal checkout con dati isolati e risposte di campagna da fixture.
Il «prima» usa i file web di `c9b927b`. La riprova Codex ha confermato M1–M4 prima e dopo con le stesse fixture.
Le catture sintetiche e i log `before-recheck.log` e `after-keyboard-recheck.log` sono in `.local/ux-captures/step-1/`.
Nella riprova Edge, Invio ha attivato «Template e riferimenti» dalla barra laterale e dall'importatore, «Importa ZIP» al ritorno e
«Guide salvate». Il frame ha mantenuto `suite=1`, con titolo e breadcrumb coerenti e caricamento concluso.

| Metrica | Baseline | Esito dello step 1 |
|---|---|---|
| M1 · Ripresa | **0 clic, 0 scorrimenti** con guida attiva in anteprima. Titolo del primo passo visibile a circa 599–620 px, dopo le metriche. | Baseline offline raggiunta. Lo step 2 deve portare «Ora» sopra le metriche. |
| M2 · Azione primaria | **2 / 2 / 2** con guida attiva, libreria popolata senza guida attiva e libreria vuota. «Importa ZIP» e «Continua il piano» in tutti e tre gli stati. | Obiettivo ancora da raggiungere nello step 2. |
| M3 · Riferimenti | **1 clic / 1 clic**. Dalla Sala operativa il percorso usa il vecchio ancoraggio. Dall'importatore il titolo interno è corretto, ma il breadcrumb resta «Importa ZIP» e il frame perde `suite=1`. | **1 clic / 1 clic** verso `/importer?view=references&suite=1`. Titolo e breadcrumb «Template e riferimenti», ritorno a Importa ZIP e destinazione Guide salvate verificati. Stato di caricamento rimosso a ogni arrivo. |
| M4 · Stati della partita | **0/6** stati della tabella dello step 3 con la coppia messaggio e azione dedicati nella home. Attesa iniziale, gioco assente e build non qualificata mostrano la stessa anteprima. Campagna non verificabile, incompatibilità e campagna collegata producono messaggi distinti. Le azioni della home restano comuni a tutti. | Baseline per lo step 3. Le fixture distinguono anche campione obsoleto (stessa anteprima) e incompatibilità di nazione o albero (stesso avviso). I segnali disponibili sul backend vanno censiti nello step 3. |

**Configurazione.** In Edge, Invio apre con focus su «Chiudi». Tab e Maiusc+Tab scorrono e richiudono il ciclo sui nove controlli.
Esc, Annulla e Chiudi restituiscono il focus al pulsante della suite. I contorni risultano visibili nella barra laterale e nel dialogo
con entrambe le preferenze di sistema `light` e `dark`. Il checkout usa una palette scura fissa. Il test di avvio ritardato conferma
che il modulo riceve la configurazione prima di abilitare il salvataggio. Nella prova già registrata del launcher WebView2 (interfaccia
inglese, finestra 1282 × 832) sono stati verificati Invio, il ciclo fra Chiudi e Salva con Tab e Maiusc+Tab, il contorno del focus e il
ritorno a Configurazione con Esc, Annulla e Chiudi. La stessa sessione ha verificato entrambi gli accessi ai riferimenti, il ritorno a
Importa ZIP e la destinazione Guide salvate, con titoli coerenti a caricamento concluso.

### Riscontro R1–R15

| Intervento | Evidenza o verifica aperta |
|---|---|
| R1 · Categorie | «Fabbriche e infrastrutture» nella vista guidata in Edge. Le nuove categorie e il ripiego sono coperti da `test_action_plan.cjs`. |
| R2 · Badge distinti | «Ordine della fonte» e «Conferma tu» insieme nella voce «Ora» in Edge. |
| R3, R5 · Avvisi e dettagli | Test JS pertinenti verdi. Avvisi bloccanti, nota unica e dettaglio «Perché?» da riscontrare in finestra su casi rappresentativi. |
| R4 · Testo originale e glossario | Test JS verdi. Resa del glossario su una frase parzialmente interpretata da riscontrare in finestra. |
| R6 · Rapporto d'importazione | Card `interpretationOverview`, conteggi e righe non interpretate visibili in Edge con rapporto sintetico. Il collegamento alla correzione è coperto dal test JS. L'esito di un import reale resta da verificare. |
| R7 · Binder costruzione | Il titolo composto mostra i bersagli forniti dalla fixture. Il binding Rust di una riga importata nella build corrente resta da verificare. |
| R8 · Titolo composto | In Edge «Costruisci Infrastrutture in Muntenia, poi costruisci Fabbrica militare in Muntenia», da fixture `reading`. |
| R9 · Vista guidata | «Ora» visibile in Edge. «Poi» e «In attesa» coperti dai test JS e da riscontrare in finestra con una guida che li popola. HUD sospeso. |
| R10 · Motivi di attesa e focus troncati | Test JS pertinenti verdi. Riga di attesa con motivo e risultato del binding da riscontrare in finestra. |
| R11 · Anteprima senza gioco | Titoli e passi visibili in Edge a home appena aperta, con valutazione Rust da fixture. |
| R12 · Correzione dalla card | Test JS pertinenti verdi. Percorso «Non è questo?» e ritorno alla card da riscontrare in finestra. |
| R13 · Lettura del giocatore | «Cosa significa?» visibile in Edge. Salvataggio e badge «Tua lettura» da riscontrare con dati isolati. |
| R14 · Giustificazione e obiettivi di guerra | Verifica live aperta nella tabella dell'analisi precedente. Richiede dati qualificati e resta sospesa. |
| R15 · Libreria raggruppata | Gruppo per nazione e «Altri documenti» visibili in Edge. Doppioni e versioni distinti nei test JS, da riscontrare in finestra. |

Test mirati 68/68 (suite, importatore, caricamento Configurazione e dimensione handoff).
`npm test` 510 passati, 3 saltati, 0 errori. Revisione invarianti senza rilievi.
Modifiche web caricate in Edge e WebView2 senza rebuild. La campagna reale e le verifiche live HUD restano sospese.

## Step 2 · La prossima azione al primo posto

**Stato:** implementato e verificato il 2026-11-22 con fixture isolate in Edge 158 headless. **Si basa su** R9 (modalità guidata)
e R11 (anteprima senza gioco).

Sala operativa, una sola azione primaria per stato:

| Stato | Azione primaria | Bozza EN |
|---|---|---|
| Guida attiva | Continua il piano | Continue the plan |
| Libreria con guide, nessuna attiva | Scegli una guida | Choose a guide |
| Libreria vuota | Importa uno ZIP | Import a ZIP |

Con una proiezione valida o un'anteprima, la voce «Ora» della modalità guidata sta sopra le metriche. In anteprima la home dice
«Anteprima: nessun progresso viene contato finché la partita non è collegata.» (EN «Preview: no progress is counted until the game is
linked.»). Nel Piano della campagna «Ora / Poi» viene prima di scorciatoie e riferimenti; il piano completo resta espandibile.

**Fuori ambito:** cambiare quale passo è «Ora» (decide Rust).

**Completato quando** M1 e M2 raggiungono l'obiettivo e l'anteprima non attribuisce progressi. Test della suite e del piano per i tre
stati; catture della home e del piano in desktop.

### Verifica dello step 2 · 2026-11-22

Edge 158 headless, viewport 1282 × 832, lingua italiana. Backend Companion con dati isolati e discovery disattivato.
Stessa serie di fixture prima e dopo, con proiezioni Rust da `presentation-evaluations.json`.

| Metrica | Prima | Dopo |
|---|---|---|
| M1 · Ripresa in anteprima | 0 clic, 0 scorrimenti. Primo titolo a 599–620 px, sotto le metriche. | **0 clic, 0 scorrimenti.** Primo titolo a 349–370 px, sopra le metriche. |
| M1 · Partita collegata simulata | La prima voce del riepilogo era sotto le metriche. | **0 clic, 0 scorrimenti.** Primo titolo di «Ora» a 308–329 px, sopra le metriche. |
| M2 · Azione primaria | 2 / 2 / 2. | **1 / 1 / 1.** Verificate le destinazioni Piano della campagna, Libreria guide e Importa ZIP. |

«Ora» mostra la chiave `next` indicata da Rust per ciascun percorso che ha un passo da presentare. In anteprima la Sala operativa
mostra l'avviso sui progressi e quattro metriche sconosciute. Nel Piano, «Ora / Poi» precede le scorciatoie e i riferimenti. La fixture
di anteprima resta a 0/2 e «Piano completo» si apre dal riepilogo.

Catture `before-*.png` / `installed-*.png` e log in `.local/ux-captures/step-2/`. Test mirati 74/74.
`npm test`: 513 passati, 3 saltati, 0 errori. Revisione invarianti senza rilievi.

Dopo il commit, `deliver-build` ha ricompilato backend e overlay per caricare le traduzioni e avviato Companion sulla porta 8766.
La suite e il catalogo aggiornato rispondono HTTP 200. La riprova isolata con il backend ricompilato conferma M1, M2, l'anteprima
e un link di «Ora» con albero e ancora preservati nella suite. La tastiera nella finestra WebView2 resta da verificare a mano.
Il riscontro con una partita reale e i test live HUD restano sospesi.

## Step 3 · Stato della partita e avanzamento

**Stato:** implementato e verificato con fixture isolate il 2026-11-22.

| Stato | Segnali backend prima dello step |
|---|---|
| Gioco assente | `/api/state/events`: `usable=false`, `status=disconnected` o `stale`, uguali agli altri errori. Solo la diagnostica della ricerca del processo distingue il caso. |
| Build non supportata | Stessi campi. L'errore della qualifica distingue il rifiuto del profilo per hash (`live-profiles-v1.json`) o firma ABI (`live-abi-v1*.json`) dagli errori di configurazione e I/O. Manca un codice pubblico. |
| Campagna non verificabile | Con una proiezione live richiesta: `evaluation.mode=preview` se manca lo scope, oppure `evaluation.progress_status=campaign_unverified/session_unverified`. Prima del campione, i fallimenti di lettura espongono solo diagnostica. |
| Incompatibilità accertata | Su campione utilizzabile: `evaluation.mode=wrong_country/wrong_tree` oppure `evaluation.progress_status=wrong_campaign/wrong_session`. Lo scope atteso è in `guide.applies_to`. |
| Campagna collegata | `evaluation.mode=live`, `evaluation.progress_status=accepted`, identità osservata in `sample.campaign_identity` e run corrispondente in `progress` e `guide_runs`. La data viene da `campaign.date` nel LiveState normalizzato. |
| Indeterminato | Stream senza campione, in riconnessione, scaduto o con errore generico. `live=0` forza `preview` anche quando il gioco è disponibile, quindi `preview` da solo non identifica lo stato. |

Servono codici stabili per lo stato della lettura e per la presentazione del collegamento alla guida. Le spunte arrivano da
`progress.checks`; il Piano mostra i conteggi delle righe operative e gli stati prodotti da Rust.

Il backend ora espone `reader_status` nello stream. Distingue processo assente, build rifiutata e lettura senza campagna verificabile.
Senza provider, con un errore generico o durante la riconnessione, lo stato resta indeterminato. La proiezione espone
`campaign_status.code` per campagna collegata, non verificabile o incompatibilità per nazione, albero o checklist.
Il collegamento richiede identità osservata e run corrispondente. La data appare solo se ricavabile dal campione qualificato.

**Seconda parte, messaggi.** Bozze:

| Stato | Messaggio (IT) | Azione |
|---|---|---|
| Gioco assente | HOI4 non è aperto. Puoi consultare il piano; i progressi si verificano quando avvii la partita. | nessuna |
| Build non supportata | Questa versione di HOI4 non è ancora supportata per la lettura: i passi restano da verificare. | nessuna |
| Campagna non verificabile | HOI4 è aperto, ma Companion non riconosce ancora questa partita. I passi restano da verificare. | Attendi dopo il caricamento |
| Incompatibilità accertata | Questa partita non corrisponde alla guida ({nazione della partita}, guida per {nazione della guida}). I progressi non vengono contati. | Scegli un'altra guida |
| Campagna collegata | Partita collegata: {nazione}, {data}. I progressi si aggiornano da soli. | Continua il piano |
| Indeterminato | Controllo della partita in corso… | nessuna |

Bozze EN, nello stesso ordine: «HOI4 is not running. You can browse the plan; progress is checked once you start the game.» ·
«This HOI4 version is not supported for reading yet: steps stay unverified.» · «HOI4 is running, but Companion does not recognize this
game yet. Steps stay unverified.» · «This game does not match the guide ({0}, guide for {1}). Progress is not counted.» ·
«Game linked: {0}, {1}. Progress updates on its own.» · «Checking the game…».

Avanzamento: una spunta salvata si chiama «Confermato da te» (EN «Confirmed by you»); il contatore si chiama «Verificati in gioco: n/m»
(EN «Verified in game: n/m»). Le due cifre non si sommano mai. Conferme, storico e selezione della campagna restano invariati.

**Completato quando** la schermata offline non parla di partita incompatibile e il caso «spunta salvata, avvio non verificabile, 0/98»
si capisce senza tooltip. Fixture per ogni stato dell'inventario, inclusi incompatibilità accertata e campagna collegata; test che
spunte e contatori mantengono i valori originali.

### Verifica dello step 3 · 2026-11-22

Edge 158 headless, 1282 × 832, lingua italiana. Backend senza discovery con dati isolati. Le proiezioni sono prodotte da Rust con la
stessa sequenza di campioni sintetici prima (`dfb3fa5`) e dopo. Log e PNG in `.local/ux-captures/step-3/`.

| Stato | Riscontro in Edge |
|---|---|
| Gioco assente | Messaggio offline e accesso al piano, senza azione dedicata alla partita. |
| Build non supportata | Messaggio di lettura non supportata. |
| Campagna non verificabile | Indicazione di attendere il caricamento, sia quando manca il campione sia quando manca l'identità. |
| Incompatibilità accertata | Nazione e albero distinti. «Scegli un'altra guida» apre la Libreria. Il caso checklist è coperto da Rust e dal test web. |
| Campagna collegata | POL e data 1936-01-01. «Continua il piano» apre il Piano della campagna. |
| Indeterminato | Attesa iniziale e campione obsoleto mostrano il controllo in corso. |

M4 **6/6** rispetto a 0/6 nello step 1. La Sala operativa conserva un solo pulsante primario, con l'azione di navigazione relativa
alla guida. Dove la tabella prevede «nessuna», non compare un comando dedicato alla partita.

Nel Piano offline, la spunta salvata resta selezionata e mostra «Confermato da te». La riga mostra «Avvio non verificabile» e il
contatore «Verificati in gioco: 0/98». Il confronto conserva spunte, stati delle righe e valori originali. Con la stessa conferma in
una campagna collegata, i verificati in gioco restano 0/98 e i passi completati restano 1/98. Quando i valori differiscono, entrambi
sono visibili. Il nuovo numeratore Rust include i completamenti operativi sostenuti da criteri di gioco senza spunte manuali.

Test mirati 35/35. Backend Rust 69 unit test passati, 2 ignorati, e 120 test di integrazione passati. Crate `live` 72/72 con fixture.
`npm test`: 514 passati, 3 saltati. Revisione invarianti senza rilievi. Verifica nella finestra WebView2 da fare a mano.
Verifica in partita reale e HUD sospese.

Dopo il commit `1b3c779`, rebuild di backend e overlay e riavvio di Companion. L'arresto automatico non ha riconosciuto la finestra.
La chiusura diretta della finestra del processo Companion verificato ha permesso alla consegna di proseguire.
Suite, Piano, modulo di stato e traduzioni serviti con HTTP 200 sulla porta 8766. Il test del servizio nativo e la riprova Edge
con il binario installato e dati isolati confermano i risultati. Catture e log `installed-*` nella stessa cartella.

## Step 4 · Importazione e schermate d'attesa

**Stato:** implementato il 2026-11-22 e verificato in Edge headless in italiano e inglese. **Si basa su** R6
(`interpretationOverview`) e R15 (libreria raggruppata).

- I tre selettori ZIP restano; ognuno ha un esempio di una riga e uno solo è indicato come «per iniziare con una guida».
- Dopo un import riuscito, la prima riga è il rapporto R6 in forma onesta: «{a} passi verificati da soli, {b} da confermare, {c} non
  interpretati.» Nessun messaggio di «tutto pronto» se {c} non è zero.
- Archivio rifiutato: cosa è andato storto e cosa fare, in una frase ciascuno; contenuti da correggere con il link a Correggi associazioni.
- Libreria, Stato della nazione ed Eventi senza partita: prima la frase dello step 3 «Gioco assente», poi filtri e contatori; i contatori
  senza dati mostrano «—», mai 0.
- Focus: la conseguenza per il giocatore prima, l'avviso tecnico in un `<details>`.

**Completato quando** si capisce quale ZIP scegliere, cosa fare con un archivio rifiutato e perché mancano i dati della nazione, in
italiano e inglese. Fixture o dati isolati (`HOI4COMPANION_DATA_DIR` in una cartella temporanea) per successo, errore e revisione;
mai la libreria dell'utente, mai preset installati nel gioco.

### Verifica dello step 4 · 2026-11-22

Edge 158 headless, 1282 × 832, suite con backend Rust e profili temporanei separati per italiano e inglese. Baseline
`e40623d`. Catture e log in `.local/ux-captures/step-4/`. Verifica manuale di tastiera e resa nella finestra WebView2 ancora aperta.

| Caso | Evidenza |
|---|---|
| Scelta ZIP | Tre selettori con un esempio ciascuno. Solo «Inizia con una guida» è evidenziato. Provati tre archivi sintetici con guide Word, un'immagine di riferimento e un documento avanzato. |
| Rapporto | Il primo contenuto del risultato è il rapporto Rust della guida salvata. Lo ZIP Word ha prodotto 2 passi verificabili automaticamente e 1 promemoria. Una fixture di guida con conteggi Rust ha verificato il caso con 1 riga non interpretata, senza annuncio di completamento totale. |
| Errore e correzione | ZIP danneggiato e ZIP senza contenuti supportati mostrano causa e azione. Il secondo documento Word apre la bozza da «Correggi associazioni». Scelto il focus corretto, rivalidata e salvata la guida. Dalla fixture con riga non interpretata, aperta la riga esatta in «Correggi associazioni» e salvata una lettura personale. |
| Senza dati | Libreria, Stato della nazione ed Eventi mostrano la stessa frase «Gioco assente» prima dei controlli. I contatori senza dati mostrano «—». Eventi distingue una risposta valida con zero risultati e scarta quelle arrivate dopo la perdita della partita. |
| Focus | Il messaggio spiega l'effetto prima dei dettagli tecnici. Verificati diagnostica dell'albero, indisponibilità delle indicazioni della guida e avviso sulla posizione del nodo. |

Le guide importate e le correzioni sono rimaste nelle cartelle isolate. Le cartelle documenti di prova sono rimaste vuote.
I rapporti delle importazioni precedenti privi dei nuovi dati non mostrano conteggi retroattivi.

Test web mirati 103/103. `npm test`: 514 passati, 3 saltati dal runner e un test del servizio nativo escluso perché prova la
scrittura dei preset MIO. Rust: backend 69 passati e 2 ignorati, model 7 passati, rapporto di interpretazione 1 passato.
Revisione invarianti senza rilievi.

Dopo il commit `663bb41`, backend e overlay ricompilati con `deliver-build`. Il launcher desktop ha avviato Companion con profilo
isolato, cartelle di gioco e documenti sintetiche e HUD disattivato. Suite e importatore hanno risposto HTTP 200. La sessione
isolata è stata chiusa dopo la prova e il profilo abituale è rimasto chiuso. La riprova Edge completa ha usato il binario installato.
Evidenze `before-*`, `installed-*`, `installed.log` e `desktop.log` nella cartella dello step.

## Step 5 · Alberi leggibili

**Stato:** implementato e verificato il 2026-11-22 in Edge headless con dati isolati.

Con un obiettivo, Focus e Tecnologie aprono la porzione dell'albero che lo contiene, centrata e a uno zoom leggibile. «Adatta scheda»
resta per la vista d'insieme. Uno zoom scelto dall'utente non viene più cambiato finché resta nella pagina. Senza obiettivo, vista d'insieme.
I nodi sono raggiungibili da tastiera e il nodo selezionato ha un contorno visibile.

**Completato quando** il nodo obiettivo si legge senza correggere lo zoom a 1282 × 832 e a larghezza ridotta, incluso il caso senza
obiettivo; catture prima e dopo.

### Verifica dello step 5 · 2026-11-22

Suite in italiano, Edge 158 headless, backend Companion con discovery disattivato. Stesse fixture di catalogo prima
(`131c17c`) e dopo. Verificati i collegamenti diretti e l'apertura sull'obiettivo fornito dalla proiezione Rust della guida.
Il nome intero compare sopra l'albero anche per i nodi Tecnologia che sulla scheda hanno solo un'icona.

| Vista | 1282 × 832, prima → dopo | 820 × 832, prima → dopo |
|---|---|---|
| Focus con obiettivo | 78% → 120% | 78% → 120% |
| Focus senza obiettivo | 78% con scorrimento → panoramica al 19% | 78% con scorrimento → panoramica al 18% |
| Tecnologie con obiettivo | 30% → 120% | 25% → 120% |
| Tecnologie senza obiettivo | 43% → panoramica al 43% | 25% → panoramica al 24% |

Obiettivo centrato e contorno visibile nelle quattro prove con obiettivo. In Edge, Tab e Maiusc+Tab raggiungono i nodi con
contorno di tastiera e Invio cambia la selezione. Le frecce regolano lo zoom e spostano il focus tra le schede Tecnologia.
Il 119% scelto da tastiera resta dopo il ridimensionamento e il cambio di albero, scheda, ramo o pannello dettagli.
Invio su «Adatta scheda» ripristina la panoramica. «Mostra focus» e «Mostra tecnologia» riportano all'obiettivo della guida.

Catture `before-*`, `after-*` e log in `.local/ux-captures/step-5/`. Test JS mirati 58/58. Regressione web 515 passati,
3 saltati dal runner e 1 test nativo escluso perché esercita la scrittura di preset MIO. Revisione invarianti senza rilievi.
Il backend installato serve i file web aggiornati al ricaricamento. Verifica manuale nella finestra WebView2 ancora aperta.
La prova non ha richiesto una partita reale. Verifiche HUD in partita sospese.

## Step 6 · L'HUD spiegato dalla suite

**Stato:** implementato e verificato il 2026-11-22 in Edge headless con fixture isolate.

In «Sessione e HUD»: una frase sulle evidenziazioni e una sulla scheda del passo; `Ctrl+Alt+H` visibile come comando di richiamo.
Stato di attesa e gioco collegato distinti con i messaggi dello step 3. Solo segnali già disponibili: senza conferma della visibilità
della scheda, mostrare collegamento e comando senza dire che l'HUD è visibile. Bozza a gioco chiuso: «L'HUD compare sopra HOI4 quando la
partita è collegata. Durante la partita premi Ctrl+Alt+H per mostrare o nascondere la scheda del passo.»

**Completato quando** la schermata a gioco chiuso spiega perché non ci sono indicazioni e come richiamare la scheda. Fixture per ogni
stato esposto. Geometria, acquisizione e tempi di validità dell'HUD invariati.

### Verifica dello step 6 · 2026-11-22

Prima (`d388ab4`) e dopo in Edge headless a 1282 × 832, in italiano e inglese. Stesse dieci fixture basate sui dati dello step 3.
Catture e log in `.local/ux-captures/step-6/`. Prima il pannello mostrava un’attesa generica o un messaggio dell’overlay.
Ora mostra lo stesso stato della Sala operativa. Con HOI4 chiuso spiega perché mancano le indicazioni e come richiamare la scheda.

| Fixture | Messaggio nel pannello |
|---|---|
| Gioco assente, build rifiutata | Stato specifico per ciascun caso. |
| Campagna non verificabile, con o senza campione | Indicazione di attendere il caricamento. |
| Nazione, albero o checklist incompatibili | Motivo specifico e progressi sospesi. |
| Campagna collegata | POL e data 1936-01-01 dalla proiezione, senza attestare la visibilità della scheda. |
| Indeterminato, campione obsoleto | Controllo della partita in corso. |

`Ctrl+Alt+H` compare in tutte le catture dopo la modifica. I test coprono anche l’attesa iniziale e l’invalidazione della
proiezione al cambio di sessione. Geometria, acquisizione e validità dell’HUD invariate.
Test mirati 21/21. Regressione web 515 passati, 3 saltati dal runner. Escluso il test nativo che esercita la scrittura dei preset MIO.
Revisione invarianti senza rilievi. Resa nella finestra WebView2 e richiamo della scheda in partita da verificare a mano dall’utente.
La verifica live dell’HUD resta sospesa.

## Step 7 · Una sola interfaccia (decisione dell'utente)

**Stato:** da fare.

La barra laterale offre ancora «Apri interfaccia classica» (`/walkthrough`). Inventariare cosa fa la vista classica che la suite non fa,
con il percorso che lo mostra. Proporre all'utente una scelta: portare quelle funzioni nella suite e togliere il link, oppure tenerla con
un'etichetta che dica a cosa serve. **Nessuna rimozione senza il sì dell'utente in chat.** Se l'utente decide, la migrazione è uno step a sé.

**Completato quando** l'inventario e la decisione dell'utente sono scritti qui.

## Step 8 · Verifica finale desktop

**Stato:** da fare.

Con la build corrente ripercorrere avvio, Sala operativa, ricerca in Libreria, Piano, Focus, Tecnologie, riferimenti, importazione e
stato HUD, con una guida attiva o una fixture isolata. Rimisurare M1-M4 e confrontarle con lo step 1. Eseguire `npm test` e
`tests/test_agent_handoff_size.cjs`. Ogni regressione con il percorso che la riproduce.

**Completato quando** ogni criterio degli step 1-7 ha un esito esplicito, il report distingue prove visive e automatiche e il handoff
conserva la sospensione dei test HUD in partita. Senza riapertura dei test live, il piano desktop si chiude con la verifica live sospesa.

**Prompt di ripresa, solo quando l'utente riapre i test live:** «Leggi il handoff e la sezione live del piano UX. Esegui solo la verifica
HUD con una partita compatibile già aperta dall'utente. Osserva leggibilità, ingombro, allineamento e richiamo della scheda. Registra le
evidenze senza controllare il gioco o modificare la geometria.»

## Prompt iniziale

```text
Leggi docs/ux-implementation-plan.md e docs/AGENT_HANDOFF.md.
Esegui solo lo step 1 (correzioni rapide e misura di partenza). Verifica nell'app desktop la navigazione ai riferimenti e il dialogo
Configurazione da tastiera, misura M1-M4, aggiorna piano e handoff e lasciami il prompt copiabile per lo step 2.
```
