# Lettura live sperimentale — Romania

Verifica del 20 ottobre 2026. Il companion ora legge lo stato corrente dalla
memoria di `hoi4.exe` su Windows: non attende autosave o snapshot.

## Uso

1. Avvia HOI4 e carica la partita. Il gioco può rimanere in pausa.
2. Avvia `avvia.bat` e apri <http://127.0.0.1:8766/> oppure
   <http://127.0.0.1:8766/technologies>.
3. Lascia selezionato **Leggi lo stato corrente · sperimentale**.
   Il pannello mostra la nazione rilevata, il focus nazionale o continuo e gli
   slot di ricerca. **Apri l’albero della nazione rilevata** sceglie l’albero
   identificato, quando presente nel catalogo locale compatibile con i DLC.
4. I focus nell’elenco nativo dei completamenti sono evidenziati come completati
   o saltati; il focus nazionale corrente è attivo. Gli altri restano non
   verificati. Le tecnologie distinguono livelli ricercati, ricerca attiva,
   non ricercate e stato non verificato.

Il browser mantiene un flusso **Server-Sent Events** e riceve uno stato completo
solo all'apertura, alla riconnessione o quando cambia il contenuto/validità.
Un unico produttore condiviso controlla i campi mutabili ogni **250 ms**;
legge e valida tutto il campione quando rileva una differenza. Ogni **5 secondi**
effettua comunque una verifica completa di definizioni e strutture. Rimane un
controllo periodico della memoria: non è una sottoscrizione a eventi del motore.
La cadenza non garantisce una latenza massima: si aggiungono lettura, rete locale
e scheduling. I cambi temporanei fra due controlli possono non essere osservati.

I heartbeat ogni 5 secondi confermano che il produttore ha ancora completato
una lettura recente; non costituiscono modifiche della partita e non generano
ricostruzioni del grafo. Il solo avanzamento del tick viene escluso dagli eventi.
Una scheda nascosta o con casella disattivata chiude il flusso; senza client il
produttore termina e chiude l'handle in lettura. Il server può impiegare fino a
circa due heartbeat per accorgersi della disconnessione TCP. Massimo 8 flussi,
un solo stato più recente, nessuna coda crescente per client lenti.
Se il processo non è leggibile o cambia durante il campione, i colori live
vengono rimossi e il pannello segnala il problema. L’ultimo campione può restare
nel payload come obsoleto, con `usable=false`; non viene applicato al grafo.

Disattivando la casella si torna alla modalità manuale/snapshot esplicita già
esistente. In modalità live `/api/tree?...&source=live` ignora anche uno snapshot
eventualmente configurato. `/api/state` non legge mai salvataggi. La lettura
live non scrive completamenti manuali né li combina con dati di altre sessioni.

## Risultati concretamente verificati

| Verifica | Risultato e confine |
| --- | --- |
| Processo | `hoi4.exe`, PID 37868 durante la prova; percorso installazione Steam su F: |
| Eseguibile | SHA-256 `7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5` |
| Build | Log locale: Operation Postern v1.19.3.0.c01a (42c3) |
| Nazione | ROM, confermata dall’utente nel gioco |
| Albero | `romanian_focus`, ID nativo e appartenenza dei 92 focus confrontati con il catalogo locale |
| Ricerca attiva | `tech_military_police2`, `dispersed_industry4`, `rubber_processing`, `excavation2`, `construction4`: cinque slot confermati dall’utente |
| Focus continuo iniziale | `continuous_restrict_freedom`, confermato dall’utente come soppressione dei subalterni |
| Cambio senza salvataggio | Dopo il cambio effettuato dall’utente a gioco in pausa, letto `continuous_reduce_training_time` (localizzazione: Addestramento militare); tick invariato 60806040 |
| Dati di completamento | 24 focus nell’elenco nativo e 73 tecnologie con livello > 0; coerenza strutturale verificata, confronto completo con la GUI del gioco ancora da effettuare |
| Prestazioni a gioco in pausa | 12/12 campioni coerenti; prima lettura 84,864 ms, mediana delle successive 43,783 ms, massimo 84,864 ms. Include apertura/chiusura handle, hash solo al primo campione |
| Test automatici dopo la miglioria | 93 test Python e 7 test Node superati; controllata sintassi di tutti gli script frontend |
| Browser reale | Focus Explorer: albero rumeno e stati applicati, 92/92 percorsi icona; Tecnologie: 43 schede industriali, 4 attive e 15 con livelli ricercati; Costruzione IV indicata in ricerca nell’ispettore |
| Cambio sorgente nella GUI | Disattivando la casella live spariscono gli stati osservati; riattivandola tornano. Nessun errore/warning nella console della vista tecnologica durante la verifica |
| Isolamento per albero nella GUI | Con ROM letta, scegliendo Austria si ottengono 215 stati non verificati e 0 completamenti; tornando con il pulsante della nazione rilevata si ottengono 92 nodi rumeni, 24 completamenti e 68 non verificati. Nessun errore/warning console nel Focus Explorer |

### Aggiornamento su cambiamento rilevato

Misure successive sul processo ROM riaperto (PID 12988 durante la prova):

- Lettura completa di confronto: mediana **44,218 ms** su 8 campioni,
  profilo PE già caricato e handle riaperto per campione, come nella prima versione.
- Nuovo controllo: mediana **1,641 ms**, massimo **2,294 ms** su 26 controlli
  leggeri; 2 letture complete nello stesso ciclo di prova di 28 acquisizioni.
  Non sono percentuali CPU/FPS o misure di latenza clic→interfaccia.
- 1.142 intervalli selezionati, 28.408 byte confrontati per passata,
  raggruppati in 88 intervalli di pagine per ridurre le chiamate di sistema.
  Il controllo stabile effettua due passate (176 letture, 573.688 byte trasferiti
  in totale), confrontando solo i campi selezionati: il padding non causa eventi.
- Il controllo include slot, liste di completamenti e livelli di **tutti** i
  modelli tecnologici, anche quelli ripetibili. Non si limita a quantità o tick.
- Durante la sottoscrizione SSE, l'utente ha cambiato la selezione in pausa:
  da `continuous_restrict_freedom` a focus nazionale `ROM_fortify_the_borders`
  (**Fortificare i confini**). Tick rimasto 60806040. È stato osservato un evento
  di stato per il cambio; le conferme della connessione restano eventi separati.

Evidenze nella cartella outputs della chat: `live-change-probe-benchmark.json`,
`live-sse-observation.json` e rapporto della verifica delle schede.
Il tempo CPU dei singoli controlli non è quantificabile da questi campioni:
il contatore locale ha risoluzione insufficiente. Non è stato misurato l'impatto
sui frame del gioco.

La misura riguarda il tempo del reader, non il ritardo dal clic nel gioco alla
visualizzazione nel browser e non l’impatto FPS. La prova del cambio è
un’osservazione prima/dopo, non una misura strumentata di latenza.

## Implementazione e vincoli

`src/hoi4companion/live.py` implementa un osservatore esterno indipendente.
Richiede esclusivamente `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ` (`0x410`)
e usa `ReadProcessMemory`. Non usa `WriteProcessMemory`, iniezione, debugger,
hook, comandi console, invio di tasti, mod o elevazione di privilegi. I file del
gioco sono aperti in lettura; le scritture riguardano solo il companion, cache,
test e rapporti. Il servizio è locale su 127.0.0.1.

Il profilo rifiuta ogni hash eseguibile diverso da quello sopra. Verifica
percorso, creazione del processo, dimensione e timestamp PE del modulo caricato,
RTTI, proprietà degli oggetti, limiti dei contenitori e identificatori. Ogni
blocco di memoria utilizzato viene riletto prima di pubblicare il campione.
Questo riduce letture strappate; non ferma il motore e non costituisce una
transazione atomica, né elimina cambiamenti ABA fra le due letture.

Ogni risultato sostituisce interamente il precedente. L’identità tecnica include
PID, creazione del processo e indirizzi world/country: non è un identificatore
persistente della campagna e non certifica il caricamento di un’altra partita
che riutilizzi gli stessi indirizzi. Non viene effettuato un merge incrementale.

### Campi corroborati sul PE e sul processo

RVA riferite esclusivamente all’hash supportato, non a versioni generiche HOI4:

- Root world `+0x332F260`; selettore nazione primaria `world+0x520`;
  tabella stringhe tag `+0x358/+0x364`; mappa tag→indice `+0x340` e paesi `+0x310`.
  Le sequenze PE `0xBB4F23–0xBB4F31` corroborano il recupero del tag; non usiamo
  il selettore alternativo `+0x524`, la cui semantica non è qualificata.
- `country+0x1370` è `CNationalFocusTree`; `+0x1380` è
  `CNationalFocusProgress`, con proprietà `+8`, focus corrente `+0x10`,
  focus continuo `+0x18` ed elenco completamenti `+0x40`.
  `+0x18` **non è un tempo trascorso**: setter/serializzazione del PE
  (`0x2DACA0`, `0x2D8BD9`, `0x2DCAFB`) e RTTI del puntatore runtime lo
  identificano come `CContinuousNationalFocus`.
- `country+0xF60` è `CTechnologyStatus`; modelli `+0x88`, completati `+0x40`,
  slot `+0xA0`. Controlliamo proprietà dello slot `+0x10`, bersaglio `+0x18`,
  proprietà modello `+0x158`, definizione `+0x160`, livello `+0x174`,
  indice definizione `+0x38` e massimo `+0x3DC`. La vtable della definizione
  deve corrispondere alla RVA `0x293C2E0`.

Il piano dei controlli leggeri viene costruito solo dopo una lettura completa
coerente. Tiene sotto osservazione identità, puntatori, proprietà, membri delle
liste, bersagli degli slot e livelli. Le letture vicine sono raggruppate entro
pagine da 4 KiB; eventuali byte intermedi non vengono interpretati né confrontati.
Errori e differenze invalidano la cache: si rilegge e si valida, oppure si pubblica
stato non utilizzabile. Nomi e definizioni sono riconvalidati dalla verifica
completa periodica, quindi una loro modifica può attendere fino al prossimo audit.

Gli offset sono osservazioni di layout; la corrispondenza con lo stato scelto
dal giocatore richiede anche la prova runtime. Le cinque ricerche e il focus
continuo sono stati confrontati dall’utente. Le fixture sintetiche verificano
rifiuto di proprietà errate, completamenti incoerenti, campioni cambiati e build
sconosciute; non sono prova di semantica del gioco.

### Fonti e provenienza

La skill REA ha guidato identificazione del target e separazione fra evidenza
statica e runtime. Il target nativo è stato aperto con REA/Ghidra per l’identità;
la verifica mirata PE/RTTI è stata effettuata con letture offline e Capstone.
Non attribuiamo a Ghidra una decompilazione completa o osservazioni runtime.

Sono state consultate come indizi di layout, senza eseguire moduli o copiare
codice nel progetto:

- [saslifat-gif/hoi4-mcp](https://github.com/saslifat-gif/hoi4-mcp), commit
  `93849c027941af8384a7a9c0d9b2cd4a8c29a9f1`: riferimenti di ricerca per lo
  stesso hash. Il progetto include anche meccanismi di iniezione che qui
  **non sono stati usati**. Non è stata trovata una licenza per riuso del codice.
- [klimPaskov/HOI4-Live-Tools](https://github.com/klimPaskov/HOI4-Live-Tools),
  commit `6b91a2a43edfeab60a415ec2ddd9b71d215b8a56`: confronto di architettura,
  offset relativi a un’altra build, non usati come profilo universale.

L’output misurato è conservato nella cartella `outputs` della chat come
`live-memory-verification.json`; comprende solo dati di gioco selezionati,
non un dump di memoria, credenziali o command line del processo.

## Restante qualificazione

- Percentuali, ETA, bonus di ricerca e durata effettiva dei focus non sono
  esposti. Le durate statiche dell’Explorer rimangono stime base.
- Non viene valutata la ricercabilità effettiva, `available`, `allow_branch`,
  `bypass` o visibilità dinamica. L’elenco focus non distingue completato da
  saltato. Livello > 0 di una tecnologia ripetibile non significa che tutti
  i livelli possibili siano completati.
- Il cambio da focus continuo a nazionale è stato osservato in pausa; la
  schermata del companion mostra il focus nazionale attivo. Completamento
  dei focus e delle tecnologie durante l'avanzamento della partita restano
  da confrontare con la GUI del gioco.
- Puntatori nulli, strutture incoerenti ed errori di accesso invalidano il
  campione, con test sintetici anche per il cambio sessione. Arresto/riavvio,
  menu principale e caricamento di un’altra campagna o nazione non sono stati
  esercitati sulla partita dell’utente: la sola presenza di strutture residue
  leggibili non prova che la GUI sia ancora nella stessa partita.
- La prova runtime è sulla Romania di questa build. Gli oggetti
  `CJointNationalFocus` non sono ancora supportati: un albero che li contiene
  viene rifiutato, non letto in modo approssimativo. Austria e altre nazioni
  non sono qualificate come reader live da questa prova.
- I warning di parsing già presenti per alcuni file cinesi e una `.gfx`
  rimangono visibili; il catalogo rumeno risolve 92/92 percorsi icona.
- Non c’è ancora un motore che confronta automaticamente la guida con la
  partita e suggerisce la prossima scelta. La sorgente live fornisce ora la
  base osservabile per implementarlo.
