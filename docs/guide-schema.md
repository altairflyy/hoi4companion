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
