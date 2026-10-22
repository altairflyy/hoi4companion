# Avvio delle costruzioni rilevato live

Verifica locale del 21 ottobre 2026. Istanza aggiornata verificata:
<http://127.0.0.1:8771/walkthrough>. Non richiede salvataggi, input automatici,
iniezioni, scritture di memoria o modifiche ai file di HOI4.

## Cosa cambia

`sample.country_overview.construction` espone gli ordini della nazione giocata:
identità nativa, edificio, quantità ancora in coda, fabbriche attive, livello
già costruito, regione/provincia, riparazioni e conversioni. Una coda senza
fabbriche assegnate è comunque una coda: l'avvio non richiede tempo di gioco.
La lettura usa lo stesso flusso SSE e gli stessi controlli leggeri a 250 ms;
nessun nuovo polling nel browser o hook del motore.

L'adattatore produce il fatto calcolato `construction.queued`, con membership
`building:<id>` e `state:<id>:<building>`. Richiede una coda interamente
qualificata: righe incomplete, tipi non supportati, ubicazioni o nazione
incoerenti producono **sconosciuto**, mai una coda vuota inventata.
Riparazioni e conversioni non contano come ordini di nuove costruzioni.

`GuideStep.started_when` è una condizione dichiarativa v1, facoltativa e
indipendente dalla nazione. Per azioni manuali/progetti, se vera determina
`active`; se falsa/sconosciuta non ripiega su `GuideProgress.started`.
`completion` rimane separata e prevale quando vera. Focus e tecnologie
conservano la selezione nativa e rifiutano questa estensione.

Il blocco iniziale della guida polacca rileva almeno un ordine pertinente:
impianto terrestre oppure infrastrutture/fabbriche militari a Katowice.
Mostra gli ordini concreti e **Avviata · rilevata live**, senza pulsante manuale
per l'avvio. Questo non dichiara completato l'intero blocco.
La fonte non stabilisce una quota finale affidabile: il completamento del
blocco rimane una verifica manuale. Una guida con un obiettivo quantitativo
esplicito può già usare i livelli costruiti `country.states.items.<id>.*`
nella condizione `completion`. La scomparsa dalla coda può essere una
cancellazione e non è mai prova sufficiente di completamento.

Le altre attività restano manuali finché non hanno un criterio collegato a
fatti qualificati. Il lavoro non implementa automaticamente ogni decisione,
nomina, addestramento o progetto speciale.

## Identità ed evidenze

REA `open_binary` ha identificato il PE x86-64 installato su F:, SHA-256
`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`.
La risposta di apertura è inline e non contiene un Evidence ID.
`search_strings` con Ghidra è scaduto dopo 300 secondi: non si dichiara una
decompilazione riuscita. La sessione è stata chiusa con `close_binary`.
Le evidenze seguenti provengono dall'analisi offline Capstone dello stesso PE
e dalla lettura runtime passiva, non da risultati Ghidra.

| Riscontro locale | Identità/layout della build supportata | Limite |
|---|---|---|
| CQ-01 | RTTI `CBuildingProductionLine`, vtable RVA297B958; serializzatore RVA F6E660 | Identità statica e RTTI runtime |
| CQ-02 | Serializzatore produzione E71CB0: lista incorporata `production+70`; proprietario `production+290`, ordine `+20` | La sola appartenenza non basta: entrambi i proprietari sono verificati |
| CQ-03 | F6E660: ordine `+70` edificio, `+78` riparazioni, `+8C` conversione; 14B59B0: `+48` amount, `+18` fabbriche attive | Token inizializzati letti passivamente: `amount`, `to_repair`, `conversion` |
| CQ-04 | 10DB880: edificio `+1D8` status, `+1E0` template; template `+270` chiave e `+2E0` indice | Template nel catalogo corrente e edificio nella mappa reciproca |
| CQ-05 | 1178270/11772CB: status `+68` bit provincia, `+6C` ID; provincia `+C0` regione; E81390: provincia `+188` controller e `+190` status incorporato | I flag runtime 5/6 sono interpretati per bit, senza indovinare una classe dalla sola cifra |
| CQ-06 | Regione `+58` ID, `+120` status incorporato; status `+20` mappa e `+38` edifici; edificio `+40` livello i16 | Owner e controller della regione devono entrambi coincidere; override nazionali non qualificati |

I riferimenti Linux locali in `work/hoi4-live-tools-reference` sono stati usati
come ipotesi. Gli offset differenti di provincia/template sono stati corretti
con il PE Windows e riscontri runtime; il codice del riferimento non è eseguito.

## Verifiche effettivamente eseguite

- Partita POL, PID12180, tick60759371 durante il riscontro: 1 impianto terrestre
  a Varsavia/provincia6511 (2 fabbriche attive), 2 livelli di infrastrutture e
  5 fabbriche militari a Katowice/stato762 (0 fabbriche attive per queste righe).
  L'utente ha confermato la coda e la partita in pausa.
- API reali e browser sulla porta8771: nessuna conferma di avvio salvata;
  runtime `active`, selezione calcolata `declared_start`, completamento
  sconosciuto; scheda azzurra, ordini localizzati, contatore0/18 +1 in corso.
- Non è stata richiesta una seconda modifica reale alla coda dopo che l'utente
  l'ha esclusa. Non sono validati sul gioco un nuovo inserimento/cancellazione
  o il completamento effettivo di un edificio. Le relative transizioni sono
  coperte da test sintetici, senza confonderli con una prova live.
- 15 controlli stabili del provider: una lettura completa, mediana circa7,8 ms
  dei controlli successivi. È tempo del reader, non latenza clic-browser o FPS.
- Parità dell'intero output Python/JavaScript: 49 fixture runtime e 27 fixture
  dello stato, comprese ragioni degli sconosciuti e provenienza.
- Suite complete: 193 test Python e 138 JavaScript superati, nessuno saltato.
  Inclusi 12 test delle strutture native e regressioni della UI.

Evidenze nella `.local` del progetto: `construction-api-verification.json`,
`construction-live-before.json` (il runtime del campione nativo grezzo rimane
correttamente in anteprima finché non interviene il mapping del catalogo),
`construction-parity.json`, log delle suite e `guide-context-proof`.
Il mapping di nazione/albero e i controlli d'identità precedenti non sono stati
indeboliti. Sezioni costruzioni non qualificate non invalidano quelle riuscite.

Il riavvio dell'istanza8769 è stato rifiutato dal controllo automatico delle
autorizzazioni, senza una motivazione dettagliata. Quell'istanza resta intatta;
la versione aggiornata è stata avviata separatamente sulla porta8771, con
confronto delle conferme della stessa guida/sessione. Le checklist rimangono
in memoria come prima: riavviare un server non conserva il suo storico.
