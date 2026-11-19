# Analisi UX: dalla guida importata al companion di partita (2026-11-19)

Analisi senza modifiche al codice. Base: branch `auto/improve-2026-11-18` (HEAD `18a213c`), app in esecuzione
su `http://127.0.0.1:8766`, HOI4 chiuso (modalità anteprima, nessun dato live).

Etichette: **CONFIRMED** = osservato nel codice, nell'app o nei dati; **INFERRED** = dedotto da evidenze
parziali; **NOT VERIFIED** = ipotesi da controllare.

Metodo:
- lettura di codice e documentazione (`docs/`, `native/guides/src`, `src/hoi4companion/web`);
- ispezione dell'app reale nel browser integrato (walkthrough, suite, importer) sulla guida attiva
  "ROMANIA (Fascist) The Glory of Romania is Eternal!";
- statistiche su tutte le 50 guide installate tramite `GET /api/guides` e `GET /api/guide` (sola lettura).

Non sono stati usati agenti di revisione aggiuntivi: le evidenze dirette (app e corpus) bastavano e
un secondo passaggio avrebbe ripetuto la stessa analisi.

---

## A. Stato attuale

### Flusso reale dell'utente

| Fase | Cosa succede | Dove | Stato |
|---|---|---|---|
| Importazione | ZIP/Word, oppure JSON prodotto da un'IA esterna con il prompt esportato | `importer.html`, `/api/import/*`, `import_document.rs` | CONFIRMED |
| Interpretazione | Tabelle a tre colonne: focus e ricerche diventano passi nativi; la colonna Actions viene scomposta (`decomposition.rs`, `operation_language.rs`, `command_families.rs`) | `native/guides` | CONFIRMED |
| Validazione | Risoluzione delle identità sul catalogo, DLC, sequenza; anteprima modificabile | `import_preview.rs`, `import_reports.rs` | CONFIRMED |
| Presentazione | Walkthrough operativo, alberi focus/ricerca, suite con "Centro comando", HUD con card del passo | `guide.js`, `suite.js`, `hud_service::step_card` | CONFIRMED |
| Esecuzione | L'utente agisce nel gioco; l'app non clicca mai | invariante | CONFIRMED |
| Monitoraggio | Valutazione Rust (`guide-core`) su stato live qualificato | `/api/guide/projection` | CONFIRMED |
| Completamento | Automatico se esiste un criterio osservabile; altrimenti checkbox o "✓ Mark done" sulla card HUD | `app_checks`, `hud_check_allowed` | CONFIRMED |

### Quanto viene davvero interpretato (corpus: 50 guide installate)

| Traccia | Passi | Verifica |
|---|---:|---|
| Focus | 1.672 | nativa (albero focus) |
| Ricerca | 2.155 | nativa (albero tecnologie) |
| Operazioni | 4.415 | 885 (20%) con criterio di completamento |

Categorie delle 4.415 operazioni: `checkpoints` 3.483 (**78,9%**), politics 434, characters 248,
production 131, projects 90, army 18, construction 11. CONFIRMED.

`checkpoints` è mostrato all'utente come **"Eventi e condizioni"**, ma è il ripiego per tutto ciò che non è
stato classificato (`native/guides/src/authoring.rs:118` e `:728`, `web/guide.js:4`). CONFIRMED.

Una classificazione a parole chiave dei 3.483 ripieghi (prova approssimativa, multi-etichetta) trova:
esercito/marina/aviazione 863, guerra e conquista 641, politica e PP 574, costruzione 382,
produzione 335, diplomazia 288, commercio 128; 802 righe senza famiglia riconoscibile. INFERRED: circa
3 righe su 4 potrebbero ricevere almeno una categoria di **presentazione** corretta in modo deterministico.

### Giudizio di sintesi

Il motore è rigoroso: identità, dipendenze, stati "unknown" e confini di verifica sono curati.
La comprensibilità per il giocatore è molto più debole del motore. L'app comunica soprattutto *ciò che non
può verificare*, mentre spiega poco *cosa fare* e *perché*. Per focus e ricerche l'esperienza è buona
(evidenziazione nei rispettivi alberi). Per la colonna Actions, che contiene le decisioni difficili
(costruzione, guerra, esercito, commercio), quattro righe su cinque arrivano al giocatore come testo
grezzo con un'etichetta di categoria sbagliata. CONFIRMED sul corpus e sulla guida Romania.

---

## B. Problemi prioritari

**P1 · Categoria di ripiego fuorviante.** "INFRAs, then MILs in Mun" (costruzione), "Fix Trade"
(commercio), "Attache to NSPA" (diplomazia), "design & produce SI-0" (produzione) e la legge
d'occupazione appaiono tutti come "Eventi e condizioni". Il filtro per categoria diventa inutile proprio
dove servirebbe. CONFIRMED (walkthrough Romania, passaggi 1, 2, 4, 7, 8).

**P2 · Il titolo della card è l'abbreviazione grezza.** Il "Passaggio attuale 1" mostra
`INFRAs, then MILs in Mun (future in best)`. Il vocabolario per `INFRA` e `MIL` esiste
(`guide_language.json:81-86`), ma il binder di costruzione accetta solo righe che iniziano con
`build|construct` (`native/guides/src/construction_instructions.rs:8`). CONFIRMED. Il fatto che "Mun"
(Muntenia) non venga risolto come stato abbreviato è INFERRED.

**P3 · Espansioni bilingue a metà frase.** Esempi visti: "Never spend potere politico (PP) on events",
"Esercito: setup\*, rename & Addestra", "appena i requisiti lo consentono (ASAP): Attache to NSPA
(100 potere politico)". L'aiuto alla lettura sostituisce singoli termini dentro una frase inglese.
Il risultato è meno leggibile dell'originale. CONFIRMED.

**P4 · Un solo badge per due informazioni diverse.** `badge()` in `web/guide.js:90` sceglie fra circa
13 etichette che mescolano lo *stato* (in attesa, in corso, completato) con il *metodo di verifica*
(live, criterio, manuale). "Da verificare nel gioco" significa in realtà "conferma tu quando l'hai fatto";
"Momento da verificare" e "Avvio non verificabile" non dicono al giocatore cosa fare. CONFIRMED.

**P5 · Rumore tecnico ripetuto.** In ogni card personaggio compaiono tre righe come "Costo politico
effettivo non qualificato; la sola soglia della guida non conferma la disponibilità dei punti" e
"Requisiti dinamici non verificabili". Ogni blocco ripete "Colonna Actions · ordine della fonte...".
Prima del piano c'è un paragrafo di nove frasi sulle regole di rilevamento. Il messaggio di stato
"la campagna salvata non corrisponde a un'identità live verificata" è gergo interno. CONFIRMED.

**P6 · Nessun riepilogo della qualità d'importazione.** L'importer riporta identità non risolte, DLC e
sequenza (`import_reports.rs`). Non dice mai al giocatore quante istruzioni saranno verificate da sole,
quante richiedono conferma e quante non sono state interpretate. Le note di convalida parlano di "zero
errori di identità in 47 guide", ma il 79% delle operazioni resta non interpretato. L'utente riceve
un segnale di "tutto pronto" che sovrastima l'assistenza reale. CONFIRMED (stringhe in `importer.js`,
`docs/action-plan.md`).

**P7 · Condizioni mostrate come testo.** "Quando: On Revise…" e "On Strengthen…: Liturgical…(70PP)"
sono nomi di focus troncati dalla fonte. Restano testo; il giocatore non sa a quale focus si riferiscono.
CONFIRMED che sono mostrati grezzi. La risoluzione univoca per prefisso nell'albero della nazione è
INFERRED fattibile.

**P8 · Due interfacce e una home vuota senza gioco.** La suite (barra laterale, "Centro comando") e lo
"spazio di lavoro classico" hanno navigazioni diverse; il walkthrough ha una propria barra in alto. Senza
HOI4 la home mostra solo "Apri HOI4 per qualificare la campagna" e nessun passo della guida. CONFIRMED.
Non è possibile prepararsi alla partita dalla home.

**P9 · Libreria rumorosa.** Il selettore della guida elenca 50 titoli di file grezzi con doppioni
("HUNGARY BROKEN EU Path!", "FRANCE Napoleonic Empire", "ROMANIA ... Eternal!" due volte ciascuno) e voci
che non sono guide nazionali ("VIDEO LINK", "How to Import My Equipment Designs"). CONFIRMED. Che i
doppioni siano file identici è NOT VERIFIED.

**P10 · "Cosa posso fare in parallelo" non è visibile.** Il runtime calcola le azioni parallele
(`runtime.parallel`) per focus e ricerca, ma il walkthrough mostra un solo "Passaggio attuale" e una lista
"In attesa". Le azioni dello stesso blocco Actions, normalmente eseguibili insieme, non sono presentate come
tali. CONFIRMED per la struttura della pagina; il beneficio è INFERRED.

---

## C. Proposta UX

### Principio: tre informazioni separate per ogni istruzione

Ogni card mostra sempre tre elementi distinti, al posto del badge unico attuale:

1. **Categoria** (dove si agisce nel gioco): Costruzione, Produzione, Esercito, Guerra e diplomazia,
   Politica, Commercio, Altro.
2. **Quando**: *Ora* · *Dopo* · *In attesa di…* (con la condizione in parole) · *Fatto*.
3. **Come si verifica**: *Automatica* · *Conferma tu* · *Promemoria* · *Non interpretata*.

I quattro valori del punto 3 si derivano da dati che esistono già: `completion`, `tracking: reminder`,
`application::manual_keys`, assenza di parametri risolti.

### Esempio 1 · costruzione abbreviata (guida Romania, passaggio 1)

Prima (CONFIRMED, schermata attuale):

```
Passaggio attuale 1
Eventi e condizioni                         [Da verificare nel gioco]
INFRAs, then MILs in Mun (future in best)
▸ Dettagli del monitoraggio e dell'HUD
▸ Istruzione originale e riferimenti
☐ Completato
```

Dopo (proposta, se "Mun" si risolve in modo univoco):

```
COSTRUZIONE · Ora · Automatica
Costruisci infrastrutture, poi fabbriche militari in Muntenia
  1. Infrastrutture → Muntenia (livello massimo)
  2. Fabbriche militari → Muntenia
Nota della guida: anche le future costruzioni in "best" (stato migliore).
Testo originale: INFRAs, then MILs in Mun (future in best)
```

Se "Mun" non si risolve, la card resta onesta:

```
COSTRUZIONE · Ora · Conferma tu
Infrastrutture, poi fabbriche militari in «Mun»
«Mun» non corrisponde con certezza a uno stato. Spunta quando hai messo in coda.
Testo originale: INFRAs, then MILs in Mun (future in best)
```

### Esempio 2 · "Justify HUN, YUG — ASAP"

Le azioni justify sono oggi solo riconosciute, non verificate (handoff, "recognized only"); IL-04 è stato
scartato per mancanza di un campo live qualificato per gli obiettivi di guerra. Quindi:

```
GUERRA E DIPLOMAZIA · Ora (appena possibile) · Conferma tu
Giustifica obiettivi di guerra
  ⚑ Ungheria (HUN)    ☐ avviata
  ⚑ Jugoslavia (YUG)  ☐ avviata
Il gioco non espone ancora questo dato: spunta ogni giustificazione quando la avvii.
Testo originale: Justify HUN, YUG — ASAP
```

I nomi delle nazioni vengono dal catalogo dei tag (deterministico). Le bandiere sono un'aggiunta da
valutare: richiedono di leggere gli asset grafici del gioco in sola lettura, come già si fa per le icone
delle tecnologie (`docs/tech-visual-assets.md`). Fattibilità per le bandiere NOT VERIFIED.

### Esempio 3 · promemoria di metodo

Prima: `Eventi e condizioni · Da verificare nel gioco · Never spend potere politico (PP) on events` con
checkbox "Completato".

Dopo:

```
PROMEMORIA · per tutta la partita
Non spendere potere politico (PP) sugli eventi.
Testo originale: Never spend PP on events
```

Nessuna checkbox e nessun conteggio nell'avanzamento. Esiste già la classe `reminder`; manca il
riconoscimento dei verbi di metodo (`never`, `always`, `remember`, `optional`...).

### Esempio 4 · nomina di un personaggio (passaggio 6)

Prima: nome, "Quando: On Revise…", istruzione generica e tre righe "non verificabile/non qualificato".

Dopo:

```
POLITICA · In attesa del focus «Revise …» · Automatica
Nomina Gheorghe Argeseanu (consigliere politico) · 112 PP secondo la guida
▸ Perché in attesa?  Il focus indicato dalla guida non risulta completato.
▸ Dettagli tecnici   (le righe attuali sul costo e sui requisiti, chiuse per default)
```

### Modalità guidata e modalità avanzata

- **Guidata (predefinita).** Una sezione "Ora" con al massimo 3–5 azioni eseguibili insieme, prese dal
  runtime (`runtime.parallel`, blocco Actions corrente, prossimo focus, prossima ricerca); una riga "Poi";
  una riga "In attesa di…" con la condizione più vicina. Ogni azione con categoria, verifica e un solo
  pulsante: "Fatto" se serve una conferma, nessun pulsante se la verifica è automatica.
- **Avanzata.** Il "Piano completo" attuale, con filtri, dipendenze, fonti, fotogrammi video e dettagli
  tecnici. Non serve una nuova schermata: è la sezione esistente, con un interruttore che ricorda la scelta.

La card HUD mostra già "il passo da fare ora" (`docs/native-overlay.md`): deve usare la stessa riga
"Categoria · Quando · Verifica" e lo stesso titolo leggibile, così suite e gioco parlano la stessa lingua.

### Panoramica dopo l'importazione

Una card nella libreria o nell'importer, senza un nuovo pannello:

```
ROMANIA · The Glory of Romania is Eternal!       ★★  · DLC: ok
62 istruzioni operative
  ● 14 verificate automaticamente
  ● 31 da confermare tu
  ● 6 promemoria
  ● 11 non interpretate  → vedi quali
+ 38 focus e 52 ricerche evidenziati nei rispettivi alberi
```

I numeri sono un esempio di formato; i valori reali vanno calcolati dal backend.

### Linguaggio dei messaggi di limite

| Oggi | Proposta |
|---|---|
| Avanzamento sospeso: la campagna salvata non corrisponde a un'identità live verificata. | Questa partita non corrisponde alla checklist salvata. Apri la partita giusta o inizia una nuova checklist. |
| Costo politico effettivo non qualificato; la sola soglia della guida non conferma… | (nei dettagli tecnici) PP attuali non leggibili: controlla di avere 112 PP. |
| Requisiti dinamici non verificabili. | Il gioco potrebbe richiedere altre condizioni: controlla che il pulsante sia attivo. |
| Da verificare nel gioco | Conferma tu quando l'hai fatto. |
| Momento da verificare | La guida non indica quando: segui l'ordine della fonte. |

---

## D. Evoluzione tecnica

Vincoli rispettati: la categoria e il testo leggibile sono **presentazione**, non valutazione. Non
cambiano mai stato, completamento o progressi. Il calcolo avviene in Rust (crate `guides`), perché serve
sia alla suite sia all'HUD; il JS si limita a mostrarlo.

1. **Categoria di presentazione deterministica.** Nuovo campo `display_category` con `category_basis`
   (`declared` | `keyword` | `none`) calcolato da `guide_language.json` e dal catalogo. Il ripiego diventa
   "Altre istruzioni" e non più "Eventi e condizioni". Rischio basso.
2. **Asse di verifica esplicito.** Campo `verification`: `auto` | `manual` | `reminder` | `uninterpreted`,
   derivato dagli stessi dati di `manual_keys`. Il badge JS si divide in due etichette.
3. **Strato di lettura (`reading`).** Per ogni passo: verbo, obiettivi risolti (tag con nome localizzato,
   stati, edifici), qualificatori (`ASAP`, `ALL`, quantità) e testo originale. Il titolo localizzato
   viene composto da modelli di frase **solo** se l'intera riga è stata compresa; altrimenti resta il testo
   originale con "chip" di glossario a fianco, mai sostituzioni a metà frase (risolve P3).
4. **Binder di costruzione più ampio.** Accettare righe senza verbo ("INFRAs, then MILs in …"), plurali,
   separatore `then` (che diventa ordine, non barriera di completamento) e abbreviazioni di stato risolte
   per prefisso univoco nella nazione della guida. Ambiguità: resta non risolto. Corpus di prova: le 382
   righe di costruzione ricadute nel ripiego.
5. **Condizioni da focus troncati.** "On Revise…" → focus il cui nome inizia con "Revise" nell'albero
   della guida, solo se unico. Il risultato alimenta il testo "In attesa del focus …"; diventa una
   dipendenza di runtime solo se la regola d'import lo dichiara già oggi.
6. **Rapporto di interpretazione all'import.** `import_reports.rs` aggiunge i conteggi per `verification`
   e per `display_category`, e l'elenco delle righe `uninterpreted` con il collegamento a "Correggi
   associazioni".
7. **Motivi di attesa leggibili.** Una tabella unica `runtime_reason` → frase per il giocatore (in
   `translations.json`), usata dalla suite e dall'HUD.
8. **Libreria.** Raggruppare per nazione, segnalare i doppioni per hash della definizione, separare le
   voci che non sono guide nazionali.
9. **IA locale facoltativa (lungo periodo).** Solo per le righe `uninterpreted`: un modello locale
   propone una categoria o un'interpretazione; l'utente la accetta; il risultato diventa una correzione di
   associazione salvata, mai un criterio di completamento. Nessuna API a pagamento. Prima vanno esauriti i
   punti 1–5, che sono deterministici e verificabili.

---

## E. Roadmap

Ordine per impatto sul giocatore, costo e rischio. Ogni voce è verificabile senza avviare HOI4.

### Immediati (piccoli, basso rischio)

| # | Intervento | Problema | Verifica senza HOI4 |
|---|---|---|---|
| R1 | Ripiego "Altre istruzioni" + `display_category` a parole chiave | P1 | Test Rust su un fixture di righe reali; metrica sul corpus (quota di `checkpoints`) |
| R2 | Badge diviso in Stato + Verifica | P4 | Test browser con `presentation-evaluations.json` |
| R3 | Dettagli tecnici ripetuti chiusi in "Perché?"; nota del blocco Actions mostrata una volta | P5 | Test DOM: numero di righe visibili per card |
| R4 | Stop alle sostituzioni a metà frase; chip di glossario a fianco | P3 | Test sulle tre frasi citate in P3 |
| R5 | Riscrittura dei 5 messaggi di limite più frequenti | P5 | Revisione delle stringhe con la skill `add-translation` |

### Medio periodo

| # | Intervento | Problema | Verifica |
|---|---|---|---|
| R6 | Rapporto di interpretazione all'import e card panoramica | P6 | Test d'integrazione backend sui conteggi; vista importer su una bozza reale |
| R7 | Binder di costruzione esteso con abbreviazioni di stato | P2 | Corpus di 382 righe: quota risolta e zero risoluzioni ambigue |
| R8 | Strato `reading` con nomi delle nazioni e titoli composti | P2, P3 | Fixture di contratto in `tests/fixtures/` |
| R9 | Modalità guidata "Ora / Poi / In attesa" su suite e card HUD | P10, P8 | Test browser con evaluazioni simulate; screenshot a larghezza ridotta |
| R10 | Motivi di attesa leggibili e focus troncati risolti | P7 | Test Rust sulla risoluzione per prefisso univoco e ambiguo |
| R11 | Home della suite utile anche senza gioco (anteprima dei primi passi) | P8 | Test browser in modalità anteprima |

### Lungo periodo

| # | Intervento | Note |
|---|---|---|
| R12 | Correzione guidata delle interpretazioni dalla card ("Non è questo") | Riusa la pagina associazioni |
| R13 | Suggerimenti da IA locale per le righe non interpretate | Facoltativo; solo proposte approvate dall'utente |
| R14 | Verifica live di justify e obiettivi di guerra | Bloccato finché non esiste un campo qualificato (IL-04) |
| R15 | Pulizia e raggruppamento della libreria | P9 |

## Execution order

Un passo per sessione: l'utente lancia `/clear` dopo ognuno. Ogni passo deve finire con test verdi,
revisione `invariant-reviewer`, commit locale, rebuild e riavvio (`deliver-build`), e questa lista aggiornata.
L'ordine è stato rivisto dopo R1: R3 e R5 toccano gli stessi testi e vanno insieme.

- [x] **R1 · Categorie di presentazione** (2026-11-19). Fatto in JS, non in Rust: la categoria resta solo
  presentazione e il punto naturale è già `category()` in `web/action-plan.js`. Il ripiego "Eventi e
  condizioni" diventa "Altre istruzioni"; nuove categorie "Guerra e diplomazia" e "Commercio e risorse".
  Corpus di 50 guide: ripiego da 3.483 a 633 righe su 4.415. `params.category` e i binding non cambiano.
  Test: `tests/test_action_plan.cjs`.
- [ ] **R2 · Stato e verifica separati.** Dividere `badge()` in `web/guide.js:90` in due etichette:
  Quando (Ora / In attesa / In corso / Fatto) e Verifica (Automatica / Conferma tu / Promemoria /
  Non interpretata). Derivare la verifica da `completion`, `tracking`, `started_when` e
  `runtime_reason` già presenti nella riga; nessuna nuova valutazione in JS. Test browser con
  `tests/fixtures/presentation-evaluations.json`.
- [ ] **R3+R5 · Meno rumore, messaggi chiari.** Chiudere in un `<details>` "Perché?" le righe ripetute
  su PP e requisiti dinamici; mostrare una sola volta la nota "Colonna Actions"; riscrivere i messaggi della
  tabella "Linguaggio dei messaggi di limite" (sezione C) con la skill `add-translation`.
- [ ] **R4 · Niente sostituzioni a metà frase.** In `web/action-reading.js`, se la riga non è compresa per
  intero, mostrare il testo originale e le espansioni come glossario a fianco. Casi: le tre frasi di P3.
- [ ] **R6 · Rapporto di interpretazione all'import** (Rust, `import_reports.rs`) e card panoramica.
- [ ] **R7 · Binder di costruzione esteso** (Rust, `construction_instructions.rs`).
- Poi R8–R11, quindi il lungo periodo, nell'ordine delle tabelle sopra.

### Metrica guida proposta

Misurare a ogni passo, sul corpus installato, la quota di operazioni in ciascuna classe di verifica e la
quota del ripiego. Oggi: 78,9% `checkpoints`, 20% con criterio di completamento. Lo script di misura usato
per questa analisi legge solo le API locali e può diventare un test o uno strumento in `scripts/`.

### Strumenti consigliati per le fasi successive

- `invariant-reviewer` prima di ogni commit che tocca presentazione o importer.
- Test browser esistenti con evaluazioni da fixture (mai valutazione in JS).
- Una verifica di accessibilità (contrasto, ordine del focus da tastiera, `aria-live` sugli stati) sulla
  pagina walkthrough: NOT VERIFIED finora, nessuno strumento automatico è stato eseguito.
