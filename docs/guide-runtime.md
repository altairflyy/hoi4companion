# Guide Runtime v1

Il runtime interpreta una guida dichiarata: non genera strategie e non esegue
azioni nel gioco. `guide_runtime.evaluate(guide, state, progress)` e
`HoiGuideRuntime.evaluate(guide, state, progress)` sono funzioni pure, senza
orologio, I/O, DOM o dipendenze dal lettore nativo. Gli input non vengono mutati.

## Tre contratti separati

- `Guide`: definizione v1, azioni, condizioni, dipendenze e fonti editoriali.
- `LiveState`: osservazioni correnti adattate al contratto v1. Un campione
  inutilizzabile non fornisce fatti al runtime.
- `GuideProgress`: `{schema_version:1, guide_id, run_id, session_id?, checks?}`.
  Contiene solo conferme manuali della singola partita. Non memorizza
  completamenti ricavati dal gioco. `false` è una conferma negativa esplicita;
  una chiave assente è sconosciuta. Solo chiavi dichiarate dalla guida sono usate.

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
| `active` | Azione selezionata nel gioco; non prova il completamento |
| `ready` | Prerequisiti dichiarati soddisfatti; disponibilità dinamica nel gioco non verificata |
| `waiting` | Condizione falsa o dipendenza non completata |
| `not_applicable` | Scope incompatibile oppure `skip_when` vero |
| `unverifiable` | Informazioni necessarie mancanti o non qualificate |

Ogni riga espone `reason`, `blocked_by`, fatti di `completion`, `selection` e
`availability`, `completion_basis`, priorità e `priority_known`. Appartenenza e
condizioni producono fatti **calcolati**, mai osservati. `availability` resta
sconosciuta: non viene interpretato l'intero linguaggio dei trigger di HOI4.
Le evidenze editoriali e `action.params` non sono prove runtime né codice.

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
- Le vecchie `/api/guide`, `/api/guide/checks` e gli alias nello stato restano
  compatibili. La risposta checklist resta esattamente `checks` + `revision`.
  Il vecchio endpoint non offre la protezione del nuovo ID partita per chi scrive.
- SSE aggiunge `guide_runs` (identità, senza duplicare le checklist), conservando
  `guide_states` e `guide_revision`. Nessuna modifica all'acquisizione nativa.

`RunStore` assegna gli ID nella composizione, separata dal motore puro. Importa
le checklist precedenti una volta, conserva lo storico, protegge gli snapshot e
ruota le partite al cambio di sessione osservata. Nuova checklist/Annulla nella
vista usa questi snapshot. I progressi e lo storico restano **in memoria**:
riavviare il server li perde, come avveniva per le checklist precedenti.

La sessione nativa non è un'identità affidabile del salvataggio/campagna: un
caricamento che riusa processo, paese e indirizzi può non essere distinto.
Usare Nuova checklist per una nuova campagna; il controller separa anche i
rollback osservabili, ma nessuna euristica certifica tutti i caricamenti.

## Verifica e limiti

Fixture condivise coprono stati, dati mancanti, scope, dipendenze incrociate e
in avanti, priorità, checkpoint, fasi, condizioni esplicite, sessioni e slot.
Test dedicati coprono catene di 2000 step, rollback, isolamento delle checklist,
HTTP, protezioni delle richieste e controller DOM in memoria. Quest'ultimo non
costituisce una verifica grafica. Il runtime non comprende ancora nuovi tipi
di azione per costruzioni, esercito o produzione.

La diagnosi separata di `stale`, le osservazioni reali e i limiti della causa
storica sono in [stale-diagnosis.md](stale-diagnosis.md). Risultati completi
dell'integrazione reale in [guide-runtime-verification.md](guide-runtime-verification.md).
