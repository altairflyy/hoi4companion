# Guida contestuale negli alberi e piano operativo

Incremento locale del 21 ottobre 2026, successivo al Guide Runtime v1.

Aggiornamento: l'avvio delle costruzioni è ora automatico dalla coda live.
Il pulsante manuale descritto sotto resta per le attività non osservabili.
Prova sulla partita reale e confini in [construction-live.md](construction-live.md).

## Uso

- **Focus nazionali:** il prossimo nodo si evidenzia nel grafo del companion.
  **Mostra focus** apre l'albero della guida e centra il nodo, senza cambiare il
  gioco. L'ispettore e il tooltip spiegano il ruolo del nodo nel percorso.
- **Tecnologie:** i nodi consigliati si evidenziano nella scheda originale.
  La categoria che li contiene è contrassegnata; **Mostra ricerca** cambia
  categoria, seleziona il nodo e lo centra. Le conferme sugli sconti sono nei
  dettagli del nodo, non in una lunga lista separata.
- **Walkthrough:** solo attività operative, filtrabili per costruzioni,
  produzione, progetti, politica, personaggi, forze armate, template e condizioni.
  Ogni scheda riporta **Quando**, istruzioni, eventuali requisiti, conferma e fonte.
  Il minutaggio è del video, non una data della campagna.
- **HUD:** istruzione operativa corrente e collegamenti ai due alberi del companion.
  Non viene introdotto un aggancio ai nodi nell'interfaccia nativa di HOI4.

Oro pieno significa proposta corrente del runtime, azzurro azione attiva.
Tratteggio significa passo successivo senza capacità libera, attesa oppure
anteprima; la dicitura sul nodo distingue questi casi senza affidarsi al colore.
Una partita di un'altra nazione/albero sospende le evidenziazioni. In mancanza
di un campione valido, nessun vecchio completamento è applicato: viene indicata
l'anteprima dall'inizio della guida. Le condizioni dinamiche finali del gioco
rimangono da verificare.

## Implementazione e dati

`guide-presentation.js` è una proiezione pura del modello/runtime esistente:
separa annotazioni dei nodi da attività operative. Non genera strategie,
non acquisisce memoria e non modifica la semantica del runtime.
`guide-session.js` condivide una definizione, una progressione e la sottoscrizione
live della pagina; non aggiunge polling. Riusa le API e gli eventi SSE esistenti.
`guide-assist.js/.css` applica le annotazioni ai nodi, offre navigazione e
condizioni nell'ispettore. Il posizionamento dei nodi non viene ricalcolato.

La guida polacca conserva le tracce 34/48 e i milestone legacy; aggiunge 15
step manuali nella traccia `operations`, usando esclusivamente Guide Schema v1.
Insieme al checkpoint Lituania e ai due progetti speciali, la vista operativa
presenta 18 attività. Le scelte libere finali non sono attività da spuntare.
Il runtime restituisce 97 step totali; i milestone conservati non sono duplicati
nelle schede. Nessuna nuova tipologia di comando al gioco.

Metadati facoltativi di `action.params`, descrittivi e validati come JSON:

| Campo | Uso nella presentazione |
|---|---|
| `category` | Categoria dell'attività; non attiva logiche nazionali |
| `timing` | Momento/condizione descritti dalla fonte, non un timer |
| `instructions` | Testi dei singoli passi |
| `source_seconds` | Riferimento al video |
| `legacy_key` | Conferma cumulativa precedente, sostituita da attività distinte |

Le condizioni operative restano AST dichiarativi v1; completamenti espliciti
usano `completion`, senza dedurre lavori completati dai dati mancanti.
Gli step indipendenti dichiarano `depends_on:[]`, così una conferma su politica
non diventa una barriera implicita per costruzioni o ricerche.

La preparazione iniziale separa costruzioni, agenzia, organizzazioni industriali
e addestramento. Una vecchia conferma cumulativa resta compatibile. Alla prima
modifica di una delle attività, le altre conferme già note vengono conservate
separatamente: togliere la spunta a una costruzione non annulla l'addestramento.
Nuova checklist e Annulla continuano a usare i run separati del server.

## Attività avviate e completate

Il pulsante **Ho impartito l'ordine** registra l'avvio manuale: la scheda diventa
azzurra e mostra **Avviata · in corso (manuale)**. Il contatore dei completamenti
resta invariato; il totale in corso è separato. La scheda resta visibile quando
si nascondono le completate e non è riproposta come prossima attività da avviare.
**Annulla avvio segnalato** ritira la segnalazione, senza cambiare le conferme.

La checkbox del criterio finale rimane distinta. Per costruzioni e progetti va
confermata quando il lavoro richiesto è terminato nel gioco, non alla messa in
coda. Le condizioni dichiarative osservabili continuano a essere valutate dal
runtime. Le attività senza un criterio osservabile richiedono ancora conferma
manuale. Gli incrementi successivi rilevano l’avvio delle costruzioni dalla
coda ([copertura](construction-live.md)) e sia avvio sia completamento della
creazione dell’agenzia dai rispettivi flag ([copertura](intelligence-live.md)).
La sola scomparsa di un ordine dalla coda non dimostra la fine dei lavori.

`GuideProgress.started` usa le chiavi generiche degli step manuali/progetti e
riutilizza lo stato runtime `active`. Non sblocca dipendenze, non attribuisce
percentuali e non genera strategie. Checklist precedenti, storico, reset e
aggiornamenti SSE mantengono separate le due mappe. Come le conferme esistenti,
gli avvii sopravvivono al ricaricamento della pagina, ma non al riavvio del server.

## Copertura della fonte

Le istruzioni sono ricavate dai contenuti già curati, con evidenze editoriali
esplicite. Non è stata eseguita una nuova analisi di ogni fotogramma.
Il percorso indica impianto terrestre, poi infrastrutture e fabbriche militari
a Katowice; non viene inventata una quota di fabbriche civili o un calendario
di conversione. I nomi non determinati dei MIO/politici e i moduli esatti dei
mezzi restano da verificare nella fonte. I personaggi documentati includono
Anastasia e Wojtek; le modifiche ai template includono il battaglione SPG.
Queste attività rimangono conferme manuali, anche quando il lettore offre
statistiche aggregate della nazione.

## Verifica

- Browser reale sulla porta 8769: piano operativo privo delle liste native,
  filtro costruzioni, collegamenti agli alberi, nodo focus e tecnologia in
  anteprima, categoria Ingegneria corretta e controllo Mostra ricerca.
- Stato di gioco al momento della prova grafica: **HOI4 chiuso**. La verifica
  del nuovo livello grafico in live su una partita reale resta da eseguire;
  la precedente qualificazione del lettore/runtime non viene spacciata per
  prova di questi nuovi indicatori.
- Fixture pure e DOM isolato: avanzamento dell'illuminazione al completamento,
  attivo/successivo distinti, capacità slot, identità nazionale/albero, dati
  obsoleti, navigazione della ricerca, separazione istruzioni/nodi, alias legacy,
  conferme negative, rollback di scritture rifiutate e Nuova checklist/Annulla.
- Suite complete Python e JavaScript, inclusi test HTTP delle nuove risorse
  statiche e assenza delle liste focus/ricerca nella pagina operativa.
  Risultato: **179 test Python e 116 JavaScript superati**, nessun test saltato.

Nessuna modifica al lettore, a offset o memoria/file del gioco in questo incremento.

Verifica dell'incremento **Avviata / in corso**: **180 test Python e 125 JavaScript
superati**, nessun test saltato; parità dell'intero output Python/JavaScript su
42 fixture condivise. Coperti avvio senza completamento, dipendenze bloccate,
criterio finale falso/vero, annullamento, isolamento per guida/sessione/nazione,
API legacy, aggiornamenti atomici, rigetto di avvii di tecnologie native e SSE.
Nel browser sulla porta 8769, con il lettore collegato a una Polonia valida,
una checklist temporanea verifica 0/18 + 1 in corso, persistenza al reload,
1/18 dopo conferma finale e ritorno a 0/18 + 1 in corso togliendola. La checklist
originale viene riattivata dopo la prova. È una verifica del flusso manuale:
nessun ordine o completamento di costruzione è stato verificato nel gioco.
