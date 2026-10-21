# Guide Schema v1 — estensioni additive

Scopo: descrivere **una strategia esistente** proveniente da un video o da una
pagina web; non generare strategie nuove né comandare il gioco. Lo schema
convive con gli attuali `tracks`, `when`, `priority_when` e `skip_when`
del contratto v1. Tutti i nuovi campi sono facoltativi.

## Campi della guida

- `sources[]`: fonti identificate da `id` univoco, tipo
  `youtube | web | file | manual`, ed eventuale URL.
- `phases[]`: sezioni logiche con ID e titolo, eventuale `when` dichiarativo.
- `tracks`: le tracce esistenti restano valide. Le nuove azioni devono
  mantenere un `kind` riconosciuto dall'attuale runtime.

## Campi opzionali di uno step

- `phase`: ID di una fase dichiarata.
- `depends_on`: lista di ID `key` degli step precedenti, anche di altre
  tracce. Sono validate unicità, esistenza e assenza di cicli.
- `completion`: condizione nel linguaggio v1. La validazione **non**
  implica che il tracker sappia ancora valutarla per tutti i tipi di azione.
- `action.params`: parametri descrittivi JSON con limiti di profondità.
  Non sono comandi e non ampliano i tipi di azione disponibili.
- `evidence[]`: `source_id`, `basis` e timestamp opzionali. `basis`
  vale `explicit | visual | inferred | game_derived | unverified`.
  Timestamp in secondi **del video**, mai data interna a HOI4.

## Cosa fa questo incremento

`normalise_guide` verifica i riferimenti fra fasi, fonti e step, rileva i
cicli delle dipendenze e rifiuta metadati malformati. I campi sono preservati
nel DTO della guida e gli input legacy restano validi.

`HoiContracts.guide` applica gli stessi controlli nel consumatore JavaScript,
compresa la normalizzazione legacy. I fixture condivisi in
`tests/fixtures/guide-schema.json` definiscono input accettati/rifiutati e DTO
normalizzati attesi per entrambi i runtime. Non è stato aggiunto un secondo
formato di guida o un nuovo pacchetto di validazione.

### Limiti e provenienza

- Massimo 32 fonti, 128 fasi, 32 tracce e 4096 step per traccia; chiavi degli
  step uniche nell'intera guida. Per step: 64 dipendenze e 32 evidenze.
- ID di fonte/fase: 1–160 caratteri Unicode; conteggio per code point anche
  in JavaScript. Le dipendenze possono riferire step di qualsiasi traccia o
  posizione nel file: è il grafo, non l'ordine di dichiarazione, a dover essere
  aciclico. La verifica è iterativa, con costo O(step + dipendenze).
- `action.params`, se presente, deve essere un oggetto JSON, mai `null`.
  Massimo 8 livelli, 512 elementi per lista, 128 campi per oggetto,
  nomi di campo di 1–128 caratteri e stringhe fino a 4096 caratteri.
- I numeri nei parametri, nelle condizioni e nei timestamp devono essere
  finiti. Gli interi devono rientrare nell'intervallo esatto condiviso da
  Python e JavaScript: da `-(2^53-1)` a `2^53-1`. Booleani e stringhe non
  sono timestamp; gli intervalli con entrambi gli estremi non possono essere
  invertiti. Il solo `end_seconds` è consentito come riferimento puntuale.
  Nei campi interi, `1` e `1.0` hanno lo stesso significato JSON; Python
  normalizza la versione e la priorità senza accettare booleani al loro posto.
- Gli URL opzionali delle fonti devono essere HTTP(S), con host e senza
  spazi o caratteri di controllo. Il validatore non visita le fonti, non
  certifica il contenuto e non esegue i parametri descrittivi.
- `basis: explicit` o `game_derived` è una dichiarazione editoriale, non
  una lettura qualificata del gioco. Le evidenze non vengono convertite in
  fatti osservati o disponibilità. Le conferme restano manuali.

Nel DTO della **definizione** `completion` rimane la condizione dichiarativa.
Le righe del modello **valutato** mantengono il campo preesistente
`completion: Fact<boolean>` e conservano la definizione separatamente in
`completion_condition`. Il documento originale non viene modificato. Questo
evita di confondere una condizione del percorso con una prova di completamento.

## Cosa NON fa ancora

Il frontend attuale non risolve `depends_on` o `completion` come criterio
operativo per nuove azioni: questi campi sono **contratti validati**.
Le priorità `when`, `priority_when` e `skip_when` mantengono il
comportamento corrente. Nuovi tipi (costruzione, produzione, consiglieri,
esercito) richiederanno modelli di azione validati e successivi adapter, senza
eseguire script delle guide.

## Esempio minimo

```json
{
  "schema_version": 1,
  "id": "generic-industry",
  "title": "Apertura industriale",
  "applies_to": {},
  "sources": [{"id": "s1", "type": "web", "url": "https://example.com/guide"}],
  "phases": [{"id": "start", "title": "Preparazione"}],
  "tracks": {
    "research": [
      {
        "key": "tools", "phase": "start",
        "action": {"kind": "research", "entity": {"kind": "technology", "id": "basic_machine_tools"}},
        "evidence": [{"source_id": "s1", "basis": "explicit"}]
      },
      {
        "key": "check", "phase": "start",
        "action": {"kind": "manual"},
        "depends_on": ["tools"],
        "completion": {"op": "check", "key": "check"},
        "evidence": [{"source_id": "s1", "basis": "unverified"}]
      }
    ]
  }
}
```

La fase successiva sarà validare le azioni specifiche per dominio e applicare
le dipendenze in un Guide Runtime indipendente dall'interfaccia.

## Verifica dell'integrazione

Eseguire dalla radice del progetto:

```powershell
$env:PYTHONPATH = 'src'
py -3 -m unittest discover -s tests -p 'test_*.py'
node --test 'tests/*.cjs'
```

Oltre ai fixture di parità, i test verificano catene e cicli di 2000 step tra
tracce, conservazione dei metadati nelle API, risposta JSON 400 per definizioni
non valide, separazione tra evidenze e completamenti e compatibilità della
guida polacca. Le suite native, SSE e del controller della vista restano
parte della verifica completa; i test DOM simulati non costituiscono prova
grafica o prova su una partita reale.

### Risultati del 21 ottobre 2026

- 145 test Python e 69 JavaScript superati.
- Parità esatta di accettazione e normalizzazione verificata su 38 casi,
  inclusa la guida polacca completa. Conservati 34 focus e 48 ricerche,
  senza avvisi di corrispondenza nel catalogo installato.
- Verificati HTTP, metadati delle API, SSE e rendering del controller con
  la guida restituita dal servizio reale; avanzamento polacco provato con
  eventi simulati in un DOM in memoria.
- Browser grafico non verificato: utilizzo locale disabilitato in Work.
- Durante il controllo il lettore ha segnalato `stale`, con errore
  “Paese giocato non identificato con certezza”, anche nelle istanze precedenti.
  È stata verificata l'invalidazione dei fatti obsoleti, non l'aggiornamento
  su una partita reale valida. Il lettore nativo e il trasporto SSE sono
  rimasti invariati.
