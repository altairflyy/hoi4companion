# Guide Runtime v1

Il runtime interpreta una guida dichiarata: non genera strategie e non esegue
azioni nel gioco. `guide_runtime.evaluate(guide, state, progress)` e
`HoiGuideRuntime.evaluate(guide, state, progress)` sono funzioni pure, senza
orologio, I/O, DOM o dipendenze dal lettore nativo. Gli input non vengono mutati.

## Tre contratti separati

- `Guide`: definizione v1, azioni, condizioni, dipendenze e fonti editoriali.
- `LiveState`: osservazioni correnti adattate al contratto v1. Un campione
  inutilizzabile non fornisce fatti al runtime.
- `GuideProgress`: `{schema_version:1, guide_id, run_id, session_id?, checks?, started?}`.
  Contiene conferme manuali e avvii dichiarati della singola partita. Non memorizza
  completamenti ricavati dal gioco. `false` è una conferma negativa esplicita;
  una chiave assente è sconosciuta. Solo chiavi dichiarate dalla guida sono usate.
  `started` è una mappa facoltativa di booleani per le chiavi di step `manual`
  e `special_project`. Non modifica `checks` né la condizione di completamento.
  L'assenza del campo conserva la compatibilità con i client v1 precedenti.

I tipi Python sono in `contracts.py`, quelli TypeScript in `web/contracts.d.ts`;
la firma JavaScript è in `web/guide-runtime.d.ts`.

Una guida o sessione diversa sospende le conferme (`wrong_guide`,
`wrong_session`, `session_unverified`). Le osservazioni della partita corrente
restano indipendenti dalle conferme. Uno scope nazionale/albero incompatibile
non applica alcun completamento; un'identità richiesta ma sconosciuta resta
anteprima, senza inventare una corrispondenza.

## Valutazione

| Stato | Significato |
|---|---|
| `completed` | Condizione di completamento vera o appartenenza osservata ai completamenti |
| `active` | Selezione nel gioco oppure avvio manuale dichiarato; non prova il completamento |
| `ready` | Prerequisiti dichiarati soddisfatti; disponibilità dinamica nel gioco non verificata |
| `waiting` | Condizione falsa o dipendenza non completata |
| `not_applicable` | Scope incompatibile oppure `skip_when` vero |
| `unverifiable` | Informazioni necessarie mancanti o non qualificate |

Ogni riga espone `reason`, `blocked_by`, fatti di `completion`, `selection` e
`availability`, `completion_basis`, priorità e `priority_known`. Appartenenza e
condizioni producono fatti **calcolati**, mai osservati. `availability` resta
sconosciuta: non viene interpretato l'intero linguaggio dei trigger di HOI4.
Le evidenze editoriali e `action.params` non sono prove runtime né codice.
Per un avvio manuale, `selection` è calcolata con sorgente
`manual_started:<step>`; non è un'osservazione del gioco. Un avvio non sblocca
dipendenze di completamento e non prevale su un'identità incompatibile.
La condizione finale resta indipendente: se vera, lo stato diventa `completed`;
se torna falsa, un avvio ancora dichiarato torna `active`. Non esistono timer,
percentuali o deduzioni basate sull'assenza di dati.

Gli step `manual` e `special_project` possono dichiarare `started_when`, una
condizione v1 valutata sugli stessi fatti e conferme delle altre condizioni.
Il risultato vero indica `active`, con selezione calcolata e sorgente
`declared_start`; falso e sconosciuto non applicano l'avvio manuale eventualmente
conservato in `GuideProgress.started`. La condizione finale rimane indipendente
e prevale quando vera. Questo campo non è ammesso per focus, tecnologie o step
liberi, e non modifica le osservazioni native né sblocca dipendenze di completamento.

La valutazione segue queste regole:

1. Validare schema e grafo, quindi valutare le dipendenze in ordine topologico,
   anche tra tracce e verso step dichiarati più avanti. I cicli sono rifiutati.
2. Una prova di completamento prevale sulle attese successive; una selezione
   osservata resta attiva. Non si cancellano fatti storici perché una condizione
   di ingresso non è più vera. I vincoli d'identità precedono queste prove.
3. `completion`, se presente, sostituisce il criterio implicito. Altrimenti
   focus/ricerche usano le appartenenze native, manuali/progetti la conferma
   omonima. Nessuna assenza di dati viene convertita in completamento.
4. Per gli step non completati/attivi: `skip_when`, fase `when`, step `when`,
   catalogo e dipendenze determinano applicabilità e attesa. Gli operatori sono
   trivalenti: anche la negazione di un fatto sconosciuto è sconosciuta.
5. `depends_on` richiede **completed**; uno step saltato non soddisfa una
   dipendenza di completamento. Una conferma manuale assente richiede verifica,
   una conferma esplicitamente falsa rimane non completata.
6. Per compatibilità, senza `depends_on` gli step manuali precedenti nella stessa
   traccia costituiscono checkpoint. `depends_on:[]` dichiara indipendenza.
   I progetti speciali non creano un checkpoint implicito per la ricerca.

Le priorità numeriche sono decrescenti; `priority_when` falsa o sconosciuta non
promuove lo step (nel secondo caso `priority_known=false`). A parità, le tracce
sono ordinate per ID Unicode e gli step conservano l'ordine della lista.
L'ordine degli oggetti JSON, inclusi nomi di traccia numerici, non decide i pareggi.

`tracks.next` indica il primo passaggio non concluso/non attivo, anche se in
attesa: non è una promessa che sia eseguibile. `ready` elenca tutti i passaggi
con prerequisiti soddisfatti. `parallel` propone solo focus e ricerche, nei
limiti della capacità **osservata** (0/1 focus e slot ricerca vuoti). Capacità
sconosciuta significa nessuna proposta parallela. Le attese della sequenza
restano barriere; i dipendenti bloccati vengono attraversati per poter proporre
un prerequisito dichiarato più avanti. Le proposte non sono comandi al gioco.

## API e compatibilità

`guide-model.js` è un adattatore di presentazione del runtime. Mantiene alias
legacy, checkpoint, link, ricerche parallele e progetti della guida polacca;
espone anche `runtime_status`, `runtime_reason` e il risultato completo.
La vista e l'HUD mostrano i sei stati e la provenienza del completamento.

- `GET /api/guide/runtime?id=...&run=...`: valutazione con lo stato corrente;
  `run` è facoltativo, una lettura storica non cambia la partita attiva.
- `GET /api/guide/progress?id=...&run=...`: snapshot dell'avanzamento.
- `POST /api/guide/progress?id=...` con `{}`: nuova checklist, storico conservato.
- Stesso POST con `{"run_id":"..."}`: riattiva una checklist della stessa guida.
- Stesso POST con `{"run_id":"...","checks":{"chiave":false}}`: sostituisce
  le conferme della partita attiva. Rifiuta un ID non attivo, chiavi non dichiarate
  e valori non booleani. Le checkbox inviano anche le negazioni esplicite.
- Stesso POST con `{"run_id":"...","started":{"costruzione":true}}`:
  sostituisce gli avvii manuali, conservando le conferme. Sono ammessi anche
  entrambi i campi; un aggiornamento non valido non scrive nessuna delle due
  mappe. Non si possono avviare manualmente focus o tecnologie native.
  Omettere una mappa la conserva. Nuova checklist e reset legacy le azzerano
  entrambe; Annulla recupera entrambe dallo storico.
- Le vecchie `/api/guide`, `/api/guide/checks` e gli alias nello stato restano
  compatibili. La risposta checklist resta esattamente `checks` + `revision`.
  Il vecchio endpoint non offre la protezione del nuovo ID partita per chi scrive.
- SSE aggiunge `guide_runs` (identità, senza duplicare le checklist), conservando
  `guide_states` e `guide_revision`; `guide_started` trasporta separatamente gli
  avvii per guida. Nessuna modifica all'acquisizione nativa.

`RunStore` assegna gli ID nella composizione, separata dal motore puro. Importa
le checklist precedenti una volta, conserva lo storico, protegge gli snapshot e
associa le partite alla campagna nativa e alla nazione. Nuova checklist/Annulla
nella vista usa questi snapshot. Dal 23 ottobre progressi e storico sono
persistiti in SQLite e ripristinati dopo il riavvio del server.

UUID e seed nativi distinguono la campagna dalla sessione tecnica del processo.
Il server riconcilia cambi e rewind osservati, conservando le checklist precedenti;
il browser non effettua reset autonomi. Copie di salvataggi con la stessa identità
nativa mantengono i limiti descritti in [campaign-progress.md](campaign-progress.md).

## Verifica e limiti

### Conteggi iniziali per costruzioni aggiuntive

L'importatore compila un ordine strutturato `quantity_mode: additional` con
edificio e regione risolti in una condizione v1 sul fatto calcolato riservato
`guide.construction.<step>.added`. Il runtime ricava quel fatto dal contatore
costruito osservato meno il conteggio iniziale della singola partita; ignora
eventuali valori esterni dello stesso fatto. Le condizioni già esplicite della
guida sono conservate. Nessuna nuova lettura nativa viene introdotta.

`RunStore.observe` registra una sola volta il contatore prima dell'ordine, quando
dipendenze e condizioni permettono l'istruzione e la coda qualificata non contiene
già quell'edificio nella regione. Non acquisisce dati autonomamente. Il conteggio
deve essere osservato, con nazione e sessione compatibili; dati mancanti, stale o
calcolati non fissano una baseline. Ordini successivi sullo stesso contatore non
condividono lo stesso incremento: il secondo parte dopo il completamento del primo.

`GuideProgress.baselines` è un'estensione opzionale v1, con voci
`{fact_key,value,country,quantity,source}` per step. La genera solo il server:
non è scrivibile dall'API delle checklist. È separata dalla definizione della guida
e dai fatti live. SSE la trasporta in `guide_baselines`; Python e JavaScript
calcolano lo stesso risultato. La vista mostra conteggio corrente, iniziale e target.

Esempio: 5 iniziali + 2 richieste → target 7. Coda presente: in corso; coda
annullata e totale 5 o 6: incompleta; totale osservato almeno 7: completata.
La baseline non cambia quando l'ordine viene annullato e ripreso. La creazione di
una nuova checklist o il cambio campagna/nazione la separa dalla partita precedente.
Il criterio misura il totale netto nella regione, non la provenienza causale
di ogni nuovo edificio.

Limiti: se il primo campione arriva dopo l'avvio della costruzione, il conteggio
originario resta sconosciuto; non viene ricostruito dai lavori rimasti in coda.
Dal 23 ottobre le baseline e gli altri progressi sono persistiti su disco e
associati alla campagna nativa e alla nazione. Vedi
[avanzamento per campagna](campaign-progress.md) per riconciliazione, storico,
prove reali e limiti dei rami di salvataggio.

Verifica 22 ottobre 2026: fixture condivise Python/JavaScript, API HTTP reali con
acquisizione simulata e browser reale su server fixture isolato hanno verificato
5→coda→annullamento, completamento parziale, 7→completata, rilettura pagina,
isolamento tra run e assenza di pulsanti manuali. Queste prove non certificano
una nuova transizione di costruzione nella partita reale.

Fixture condivise coprono stati, dati mancanti, scope, dipendenze incrociate e
in avanti, priorità, checkpoint, fasi, condizioni esplicite, sessioni e slot.
Test dedicati coprono catene di 2000 step, rollback, isolamento delle checklist,
HTTP, protezioni delle richieste e controller DOM in memoria. Quest'ultimo non
costituisce una verifica grafica. Il runtime non comprende ancora nuovi tipi
di azione per costruzioni, esercito o produzione.

La diagnosi separata di `stale`, le osservazioni reali e i limiti della causa
storica sono in [stale-diagnosis.md](stale-diagnosis.md). Risultati completi
dell'integrazione reale in [guide-runtime-verification.md](guide-runtime-verification.md).
