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

## Metriche

Misurate nello step 1 (prima) e nello step 8 (dopo), nella finestra desktop a 1282 × 832:

- **M1 · Ripresa:** clic e scorrimenti dall'apertura della suite al primo passo leggibile con una guida attiva. Obiettivo: 0 clic, 0 scorrimenti.
- **M2 · Azione primaria:** per ciascuno dei tre stati della home, numero di pulsanti con stile primario. Obiettivo: esattamente 1.
- **M3 · Riferimenti:** clic per raggiungere «Template e riferimenti» da Sala operativa e da Importa ZIP. Obiettivo: 1, con lo stesso titolo.
- **M4 · Stati della partita:** stati della campagna con un messaggio e un'azione propri. Obiettivo: tutti quelli che il backend distingue.

## Step 1 · Correzioni rapide e misura di partenza

**Stato:** da fare.

- «Template e riferimenti» nella barra laterale (`data-view="templates"`, `web/suite.js`) apre la stessa vista del link dell'importatore
  (`/importer?view=references`) mantenendo il contesto della suite. «Guide salvate» e «Importa ZIP» conservano le proprie destinazioni.
- Dialogo Configurazione: all'apertura il focus entra nel dialogo, Tab resta nei suoi controlli, `Esc`, «Annulla» e «Chiudi» lo riportano
  al pulsante di apertura. Contorno del focus visibile su tutti i controlli della barra laterale e del dialogo, in tema chiaro e scuro.
- Misurare M1-M4 sulla build corrente e annotare quali effetti di R1-R15 si vedono nella finestra (titoli composti, badge doppi,
  modalità guidata, anteprima senza gioco, libreria raggruppata, card `interpretationOverview`). Quelli non visibili diventano voci aperte.

**Completato quando** Importa ZIP → Template e riferimenti → Importa ZIP funziona in desktop senza caricamento persistente e con titoli
coerenti; l'accesso da Sala operativa porta alla stessa vista; il giro da tastiera nel dialogo funziona; M1-M4 e la lista R1-R15 sono scritti qui.
Test: `tests/test_suite.cjs`, `tests/test_importer_view.cjs`.

## Step 2 · La prossima azione al primo posto

**Stato:** da fare. **Si basa su** R9 (modalità guidata) e R11 (anteprima senza gioco).

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

## Step 3 · Stato della partita e avanzamento

**Stato:** da fare.

**Prima parte, inventario.** Elencare qui quali stati la UI riceve oggi dal backend e da quale campo (modalità `preview`/live della
proiezione, collegamento della run, campagna non corrispondente, build non qualificata in `live-profiles-v1.json`). Se uno stato della
tabella non è distinguibile, aggiungere il codice nel backend con un test Rust prima di toccare la UI. Senza questo inventario lo step si ferma.

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

## Step 4 · Importazione e schermate d'attesa

**Stato:** da fare. **Si basa su** R6 (`interpretationOverview`) e R15 (libreria raggruppata).

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

## Step 5 · Alberi leggibili

**Stato:** da fare.

Con un obiettivo, Focus e Tecnologie aprono la porzione dell'albero che lo contiene, centrata e a uno zoom leggibile. «Adatta scheda»
resta per la vista d'insieme. Uno zoom scelto dall'utente non viene più cambiato finché resta nella pagina. Senza obiettivo, vista d'insieme.
I nodi sono raggiungibili da tastiera e il nodo selezionato ha un contorno visibile.

**Completato quando** il nodo obiettivo si legge senza correggere lo zoom a 1282 × 832 e a larghezza ridotta, incluso il caso senza
obiettivo; catture prima e dopo.

## Step 6 · L'HUD spiegato dalla suite

**Stato:** da fare.

In «Sessione e HUD»: una frase sulle evidenziazioni e una sulla scheda del passo; `Ctrl+Alt+H` visibile come comando di richiamo.
Stato di attesa e gioco collegato distinti con i messaggi dello step 3. Solo segnali già disponibili: senza conferma della visibilità
della scheda, mostrare collegamento e comando senza dire che l'HUD è visibile. Bozza a gioco chiuso: «L'HUD compare sopra HOI4 quando la
partita è collegata. Durante la partita premi Ctrl+Alt+H per mostrare o nascondere la scheda del passo.»

**Completato quando** la schermata a gioco chiuso spiega perché non ci sono indicazioni e come richiamare la scheda. Fixture per ogni
stato esposto. Geometria, acquisizione e tempi di validità dell'HUD invariati.

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
