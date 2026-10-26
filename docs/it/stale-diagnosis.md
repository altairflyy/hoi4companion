# Diagnosi del lettore live: 21 ottobre 2026

## Esito e confini della verifica

L'errore storico `Paese giocato non identificato con certezza` non si è
riprodotto sulla partita polacca valida osservata durante questa verifica.
Il lettore esistente ha restituito campioni coerenti e utilizzabili. Per
questo errore di selettore non è stata introdotta alcuna correzione: una
modifica di offset o un selettore alternativo non sarebbero giustificati.
Una seconda anomalia distinta, riprodotta successivamente e descritta in
fondo, ha invece richiesto una correzione minima dell'interpretazione dei
completamenti.

L'accesso al processo è avvenuto tramite il lettore del progetto,
`PROCESS_QUERY_INFORMATION | PROCESS_VM_READ` (`0x410`). Nessuna scrittura
di memoria, iniezione, debugger, comando, input, salvataggio o modifica dei
file del gioco. Le scritture del test riguardano esclusivamente fixture
sintetiche nel processo Python e il rapporto locale del companion.

## Osservazioni su una partita reale

Identità verificata dal lettore: PID `23556`, creazione del processo
`134370428925982251`, SHA-256 eseguibile
`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`.
Il profilo PE, RTTI, identità del paese, proprietà dei focus e degli slot,
appartenenza alle liste e rilettura dei witness sono rimasti obbligatori.

Il primo campione aveva:

- selettore primario `10`, paese `POL`, tick `60762569`;
- focus nazionale `POL_the_four_year_plan`;
- ricerche `electronic_mechanical_engineering`, `basic_machine_tools`,
  `construction1`;
- nessun focus continuo selezionato.

L'utente ha confermato paese, focus e tre ricerche nella propria interfaccia
e dichiarato la partita in pausa. Questa è una conferma dell'utente, non uno
screenshot acquisito dall'agente. Anche le istanze preesistenti sulle porte
`8766` e `8768`, interrogate su `/api/state`, hanno restituito
`status=observed`, `usable=true` e gli stessi paese, focus e tick.

Tre letture successive del `NativeProvider` (una completa, due controlli
leggeri) hanno mantenuto la stessa identità tecnica e selettore, ma la partita
era nel frattempo avanzata autonomamente: tick `60762949`, focus
`POL_central_region_strategy`, seconda ricerca `concentrated_industry`,
quattro focus completati e 29 tecnologie ricercate. I tre campioni erano
coerenti fra loro. La conferma dell'utente sul primo campione non viene
estesa a queste nuove scelte, né l'uguaglianza di tre tick prova una pausa.
Queste letture dimostrano acquisizione attuale valida, non una misura della
latenza clic→HUD o una prova di tutte le possibili transizioni di partita.

Dettagli ripetibili, orari UTC e metriche sono in
`.local/stale-diagnosis-evidence.json` (rapporto locale ignorato da Git).

## Causa accertata e causa non accertata

Il rapporto storico `.local/schema-runtime-verification.json` e la
documentazione del Guide Schema riportavano lo stesso errore anche nelle
istanze precedenti al nuovo schema. Nel codice questo messaggio ha un unico
ramo: il valore primario `world + 0x520` è minore o uguale a zero.
Non deriva da condizioni della guida, dipendenze, completamenti o API.

`LiveMonitor` espone quel fallimento come `stale` soltanto quando aveva già
un campione valido; conserva quel campione come storico, sempre con
`usable=false`. Senza campione precedente espone `disconnected`.
`stale` non significa quindi che una vecchia nazione sia ancora utilizzabile.

Il valore numerico e lo stato della schermata al momento del fallimento
storico non erano stati registrati. Non è possibile distinguere, con le
evidenze disponibili, menu/caricamento, modalità osservatore o altra fase
transitoria. Attribuire l'errore a una di queste cause sarebbe un'ipotesi.
La causa tecnica immediata è isolata; la causa nel ciclo di vita del gioco
resta non riprodotta e non risolta con una modifica inventata.

Non viene usato il campo alternativo `world + 0x524`: il primo campione lo
osservava pari a `1`, differente dal selettore primario `10`. Non è stato
interpretato come identità del giocatore. Nessuna scelta della guida o
configurazione manuale sostituisce l'identificazione nativa.

## Regressioni coperte

Tre test aggiunti in `tests/test_native_fixture.py` verificano che:

- selettori primari `0` e `-1` restino respinti anche con campo alternativo
  positivo e strutture altrimenti valide;
- il selettore positivo debba corrispondere all'identità dell'oggetto paese;
- dopo un campione valido, il selettore `0` produca uno stato storico non
  utilizzabile e il recupero avvenga solo dopo una nuova lettura completa
  valida, non dopo il solo ripristino del selettore.

Eseguiti: 11 test fixture native, 12 test dei controlli di cambiamento,
10 test del monitor/live/API; tutti superati. Le fixture verificano
invarianti e recupero, non riproducono artificialmente una partita reale.

La skill REA ha guidato separazione fra osservazioni e inferenze.
La sessione REA non aveva target attivo né evidenze conservate per questo
hash. Non è stato aperto un nuovo progetto Ghidra: il ramo del fallimento
era già esplicito nel sorgente e la questione residua richiederebbe
osservazione del ciclo di vita reale, non altra decompilazione.

## Limiti residui

- La precedente anomalia non è stata riprodotta su una partita valida.
- Nessuna verifica grafica automatizzata: browser/computer use disabilitati.
- La validazione reale riguarda la build fissata e la partita confermata,
  non altre build, modalità o caricamenti successivi.
- L'identità tecnica di sessione resta quella già documentata in
  `docs/live-state.md`; non certifica un cambio campagna che riutilizzi gli
  stessi indirizzi.

## Seconda anomalia distinta: completamenti duplicati

Durante la verifica finale della stessa partita in avanzamento, un errore
diverso è stato riprodotto: `Stato focus incoerente`. Le istanze precedenti
`8766` e `8768` esponevano `stale`; la nuova istanza `8769`, senza precedente
campione valido, esponeva `disconnected`. Tutte con `usable=false`.

Due letture dirette, con tutti i witness riletti, alle ore UTC
`15:01:47` e `15:02:19` hanno isolato il ramo esatto:
`len(set(completed_ptrs)) != len(completed_ptrs)`. Il selettore era ancora
`10`, paese `POL`, paese e stato focus con proprietà corrispondenti,
albero `polish_focus` con 301 definizioni. Non c'erano completamenti esterni
all'albero, né focus corrente già completato: il puntatore corrente e quello
continuo erano nulli. La lista conteneva 13 elementi ma 12 puntatori unici.
Il medesimo puntatore `0x1c0cab69f60`, identificato come
`POL_fulfil_fifth_of_november`, compariva due volte. Il fenomeno persisteva
fra tick `60771293` e `60771486`: non è una lettura strappata dimostrata.

Esiste una spiegazione negli script vanilla, letti senza modificarli:
`events/NSB_Poland.txt`, evento `NSB_poland_royal_election.4` (Anastasia),
opzione `.4.a`, prima carica `polish_focus` con `keep_completed=yes`, poi
esegue di nuovo `complete_national_focus = POL_fulfil_fifth_of_november` e
sblocca `POL_romanov`. La lista reale contiene anche `POL_romanov` e
`POL_invite_exiled_nobility`. Questa sequenza corrobora la possibilità di
completamenti ripetuti dello stesso oggetto; non abbiamo tracciato
l'esecuzione dell'evento, quindi la sua responsabilità causale esatta resta
un'inferenza. Il fallimento dell'invariante di unicità è invece osservato.

Il selettore alternativo non è pertinente a questo errore. Rilassare
l'identità della nazione o sostituire i dati live con uno snapshot non lo
risolverebbe correttamente. Il rapporto separato
`.local/stale-focus-diagnosis-evidence.json` conserva valori, puntatori,
campioni, ramo fallito e distinzione fra osservazione e inferenza.

### Correzione minima e nuova prova reale

La lista nativa è trattata come cronologia di riferimenti, mentre il
contratto pubblico dei completamenti esprime appartenenza. Dopo aver
validato che ogni riferimento appartenga all'albero della nazione e che
il focus attivo non compaia nei completamenti, vengono normalizzati
esclusivamente riferimenti identici, conservando l'ordine della prima
occorrenza. Non vengono uniti oggetti distinti per nome o ID.

L'acquisizione resta identica: stessi offset, accessi in sola lettura,
witness, limite dei contenitori e controllo del cambiamento. Il piano
di osservazione continua a contenere l'intero array grezzo, duplicati
inclusi. Selettore primario, identità del paese e degli owner, rifiuto dei
riferimenti esterni, unicità delle definizioni e controllo del focus
attivo completato restano invariati. Il solo cambiamento nel reader
riguarda la semantica della collezione già validata, non il sistema di
acquisizione della memoria.

Un nuovo test riproduce il caso: prima della patch falliva con
`Stato focus incoerente`; dopo restituisce un solo completamento e
verifica che entrambi i riferimenti grezzi restino nei witness.
Due ulteriori test confermano il rifiuto di puntatori esterni duplicati
e focus attivi duplicati nei completamenti. Totale mirato dopo la patch:
14 test fixture, 12 cambiamenti, 10 live/monitor/API, tutti superati.

La patch è stata provata sulla stessa partita reale: tre campioni validi
alle `15:07:27` UTC, con un'acquisizione completa e due controlli leggeri,
identità `POL` confermata dal selettore `10`, 14 elementi grezzi e 13
completamenti unici, una sola occorrenza pubblicata del focus duplicato.
Focus corrente `POL_demand_slovakia_pavel`, ricerche
`basic_medium_tank_chassis`, `artillery1`, `support_weapons`.
Un tentativo precedente era stato respinto con
`Campione incoerente: riprovare` mentre il gioco avanzava: il controllo
delle letture strappate rimane operativo e non è stato indebolito.
Le nuove scelte sono osservazioni native; la conferma UI dell'utente
riguarda esclusivamente il primo campione sopra riportato.

