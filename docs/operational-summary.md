# Riepilogo operativo contestuale

La pagina Walkthrough include «Situazione attuale della partita»: fino a sei
schede confrontano gli obiettivi della guida con i dati disponibili. Il piano
completo resta consultabile sotto il riepilogo.

## Comportamento

- **Produzione:** distingue fabbriche richieste e assegnate. Un obiettivo di
  dieci fabbriche richieste può essere soddisfatto anche con tre assegnate:
  il riepilogo segnala la differenza senza revocare il completamento deciso
  dal runtime. I prodotti di un piano multiplo restano distinti.
- **Costruzioni e progetti:** spiega l'avvio e mantiene separato il criterio
  finale. La scomparsa di un ordine dalla coda non dimostra il completamento.
- **Dipendenze:** spiega l'attesa delle ricerche e rimanda alla relativa scheda.
  Focus e tecnologie continuano a essere indicati nei rispettivi alberi;
  il riepilogo non introduce una seconda lista di ricerche.
- **Dati mancanti:** non deduce esecuzione, disponibilità o completamento. Il
  potere politico viene menzionato soltanto quando un criterio della guida
  lo richiede esplicitamente; non viene inventato il costo di una legge.
- **Avvio manuale:** resta riconoscibile come segnalazione del giocatore.
- **Stato non utilizzabile, nazione o guida diverse:** sospende il confronto;
  non presenta i conteggi precedenti come dati attuali.

Le schede sono ordinate per attenzione, attività in corso e spiegazioni del
piano, mantenendo la priorità del runtime a parità di categoria. Questa è
soltanto un'organizzazione visiva: non cambia l'ordine strategico della guida.
I promemoria sul metodo e le operazioni future bloccate da dipendenze restano
nel piano completo. Le schede rimandano all'istruzione originale; «Dati della
partita» distingue valori osservati, calcolati e non disponibili.

## Contratto e dipendenze

`web/operational-summary.js` espone la funzione pura:

```javascript
HoiOperationalSummary.project(guide, runtimeEvaluation, liveState, {limit: 6})
```

La proiezione restituisce `schema_version: 1`, modalità, nazione, avviso,
schede e conteggio delle attività ulteriori. Ogni scheda conserva la chiave
dello step, stato del runtime, motivo, dipendenze e provenienza dei dati.
Usa l'adattatore dei contratti esistente; non valuta condizioni, non decide
completamenti e non modifica gli input.

`guide-session.js` pubblica lo stesso stato già passato al runtime. La vista
proietta `view.model.runtime` e quello stato, usando la sottoscrizione SSE
esistente. Non aggiunge richieste, polling, accesso alla memoria o persistenza.
Python serve il nuovo asset statico; il formato delle API di guida e
avanzamento rimane invariato.

## Verifiche e limiti

I test mirati coprono produzione richiesta/assegnata, costruzione avviata e
annullata, dipendenze, slot occupati, stato obsoleto, nazione diversa, avvio
manuale, dati mancanti e limiti delle schede. Fixture condivise confrontano
la proiezione dell'output dei runtime Python e JavaScript. I test della vista
verificano anche l'integrazione e la sospensione con dati obsoleti.

Il controllo visivo del riepilogo è stato eseguito con una guida e dati
sintetici, separati dal profilo dell'utente. Questo non costituisce una nuova
verifica su una partita reale. La qualità del riepilogo resta vincolata ai
criteri dichiarati dalla guida e alle capacità del lettore esistente.

Questo intervento non modifica HUD, renderer degli alberi, acquisizione live
o file di HOI4. Anteprima/grafo degli eventi, confronto grafico mirato e una
diagnostica generale più leggibile restano interventi successivi.
