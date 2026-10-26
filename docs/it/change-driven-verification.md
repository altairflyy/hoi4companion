# Verifica delle migliorie — 20 ottobre 2026

## Aggiornamento live

Il companion resta un osservatore esterno in sola lettura, vincolato allo stesso
SHA-256 HOI4 documentato in [live-state.md](live-state.md). Il browser usa ora
`/api/state/events` (SSE), non richiede più `/api/state` a intervalli regolari.
Gli stati sono deduplicati escludendo misure e tick; i heartbeat sono separati.
Il produttore condiviso effettua controlli leggeri ogni 250 ms e verifica completa
ogni 5 s o quando cambiano i campi monitorati. Non è un feed di eventi del motore.

La nuova prova in partita ROM, PID 12988 durante l'osservazione, ha rilevato il
passaggio in pausa da `continuous_restrict_freedom` al focus nazionale
`ROM_fortify_the_borders`, tick rimasto 60806040. La cattura SSE di circa 182 s
contiene **2 eventi di stato** (iniziale e cambio) e **35 heartbeat**. Il tempo
del clic nel gioco non è stato strumentato: non dichiariamo una latenza misurata
clic→browser. Nel browser il nodo `ROM_fortify_the_borders` è attivo; rimangono
24 completamenti e gli stessi cinque slot di ricerca.

In un confronto locale, la mediana di 8 letture complete secondo il metodo
precedente è 44,218 ms; la mediana di 26 nuovi controlli leggeri è 1,641 ms,
massimo 2,294 ms. Nel ciclo di 28 acquisizioni ci sono anche 2 letture complete.
Sono tempi delle operazioni, non percentuali CPU o risultati FPS. Il tempo CPU
dei singoli controlli risultava sotto la risoluzione utile del contatore e non
viene usato come prova di consumo nullo.

Letture vicine sono raggruppate entro pagine da 4 KiB, senza interpretare byte
intermedi. Limite totale del controllo: 4 MiB sulle due passate. Eventi/validità
sono gestiti con un solo produttore e ultimo stato condiviso, massimo 8 client,
senza una coda crescente. Senza client il reader viene rilasciato quando il server
osserva la chiusura del flusso. La riconnessione attende un campione fresco.

## Schede tecnologiche

Fonti e hash sono in [technology-layout.md](technology-layout.md), con evidenza
statica riproducibile in [technology-layout-evidence.json](technology-layout-evidence.json).
I nomi, ordine e icone delle schede derivano da `countrytechtreeview.gui`, mentre
il filtro DLC deriva dai descrittori e dai tag già supportati.

Nel browser sono state verificate:

- 9 schede visibili con icone nell'ordine della GUI; cambio Fanteria → Industria
  → Fanteria → Aviazione.
- Industria: 43 schede tecnologiche, 4 ricerche attive, 3 rami selezionabili.
  Il ramo Costruzione mostra 10 tecnologie e 2 delle ricerche attive.
- Cercando Industrie decentrate IV mentre è selezionato il ramo Costruzione,
  la vista ritorna al contesto che contiene il risultato e l'ispettore lo indica
  come in ricerca.
- Fanteria: le 7 tecnologie senza mappa GUI sono accessibili in Altre tecnologie.
- Aviazione: 41 tecnologie con 9 rami dichiarati dalla GUI.
- L'API espone tutte le 326 appartenenze (324 identificatori distinti); nessuna
  delle 317 posizioni ricostruite coincide esattamente con un'altra della stessa
  scheda. Questo controllo non prova assenza di ogni sovrapposizione visiva.

La seconda fila è un filtro dei rami aggiunto dal companion. Non affermiamo che
ogni ramo abbia un proprio pulsante nel gioco. Distanze, dimensioni e centratura
sono adattate; non è una replica pixel per pixel del renderer HOI4. Le dottrine
del nuovo sistema restano fuori dal catalogo delle tecnologie standard.

## Verifiche automatiche e prove residue

93 test Python e 7 test JavaScript superati; sintassi di tutti i JS verificata.
Copertura aggiunta: controlli mutabili, completamenti e livelli ripetibili,
proprietà errate, campioni incoerenti, budget di lettura, deduplicazione,
riconnessione, perdita di freschezza con lettore bloccato, ripresa con stato
identico, client lenti/limite/cleanup, endpoint SSE e rifiuto cross-origin,
parsing e disposizione delle schede/rami.

Restano da qualificare completamenti durante l'avanzamento della partita,
percentuali/ETA, altre build e nazioni, casi avanzati di layout del motore e
caricamento di un'altra campagna che riutilizzi indirizzi nativi. Nessun file di
gioco modificato, nessuna mod/iniezione o comando di gioco utilizzato.

Gli output della chat conservano misure e catture: `live-change-probe-benchmark.json`,
`live-sse-observation.json`, `technology-tabs-api-check.json` e schermate finali.
Non sono dump della memoria del processo o prove universali per tutte le versioni.
