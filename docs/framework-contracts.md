# Contratti del framework, versione 1

Il framework separa cataloghi statici, osservazioni della partita e priorità
delle guide. Il nuovo confine è in `contracts.py` e `web/contracts.js`, con tipi
Python `TypedDict` e dichiarazioni JavaScript in `web/contracts.d.ts`.
Non sono state aggiunte dipendenze di esecuzione.

## Dipendenze e confini

| Modulo | Responsabilità | Dipendenze rilevanti |
| --- | --- | --- |
| `live.py`, `country.py` | Acquisizione e qualificazione native esistenti | Profilo della build, Windows; invariati |
| `stream.py` | Produttore condiviso e consegna SSE per cambiamento | Sorgente fornita da `Store`; invariato |
| `focus.py`, `technologies.py`, `eligibility.py` | Cataloghi, posizioni e filtro dell'installazione | Parser e dati installati; invariati |
| `contracts.py` | Tipi, validazione e adattatore dello stato, condizioni pure | Solo libreria standard; nessun import da acquisizione, HTTP o cataloghi |
| `guide.py` | Catalogo guide, migrazione, validazione, arricchimento | Contratti e localizzazione; nessuna acquisizione |
| `api.py` / `Store` | Composizione e compatibilità dei servizi | Adattatori, cataloghi, acquisizione e trasporto |
| `web/contracts.js` | Contratti e adattatori per il consumatore browser | Nessuna dipendenza da DOM o rete |
| `web/guide-model.js` | Stato e prossime priorità della guida | Solo contratti; nessuna nazione o guida incorporata |
| `web/guide.js` | Presentazione, selezione guida e conferme | Modello puro, DOM e API |

Prima il caricatore sceglieva un file polacco fisso; il modello applicava una
sola eccezione di priorità e la vista aveva controlli specifici di quel percorso.
Ora file JSON, ambito della guida e condizioni dichiarative forniscono quelle
informazioni. Il loader scopre i file, valida identità uniche e usa `default:true`
oppure il primo ID nel catalogo ordinato. Più default sono un errore.
La guida polacca conserva il ruolo di contenuto predefinito.
Il servizio fissa la guida predefinita all'avvio, così i consumatori legacy e
il flusso live mantengono la stessa identità fino al riavvio.

I dati delle guide sono memorizzati in cache per percorso, modifica e dimensione;
ogni chiamante riceve una copia. Nessun accesso ai file guida viene aggiunto al
percorso di acquisizione live. L'assenza di guide non impedisce l'avvio del lettore.

## Contratti

Gli envelope `Entity`, `LiveState` e `Guide` hanno `schema_version:1`.
Riferimenti, azioni e condizioni ereditano la versione dell'envelope.
Una versione non supportata della guida viene rifiutata; uno stato con versione
non supportata non può produrre fatti utilizzabili. Le estensioni additive
possono restare nella versione 1; cambiamenti di significato richiedono una nuova
versione e un adattatore esplicito.

| Contratto | Campi e invarianti |
| --- | --- |
| `EntityRef` | `kind`, `id`, `scope?`; l'identità comprende tutti e tre |
| `Entity` | `schema_version`, `ref`, `name`, `properties`; descrizione statica, non stato osservato |
| `Fact` | `status`, `value`, `source`, `reason?`; stato `observed`, `calculated` o `unknown` |
| `LiveState` | `schema_version`, `usable`, `session_id`, `facts` indicizzati per chiave |
| `Condition` | AST limitato: `all`, `any`, `not`, `fact`, `check` |
| `Action` | `select_focus`, `research`, `special_project`, `manual`, `free`; riferimento di tipo compatibile quando necessario |
| `Guide` | `id`, `applies_to`, `tracks`; ogni step ha `key` univoca nella guida e `action` |
| Policy dello step | `when`, `priority_when`, `priority`, `skip_when`; tutte facoltative |

Un focus può avere `scope` uguale all'ID dell'albero. Se una guida dichiara un
albero, gli scope dei suoi focus devono concordare. Senza albero prefissato,
l'arricchimento risolve soltanto un risultato univoco; un ID ambiguo richiede
uno scope esplicito. Le tecnologie usano gli ID del catalogo tecnologico.
Gli step arricchiti espongono anche un'entità versionata. La compatibilità con il
catalogo non costituisce prova di disponibilità nella partita.

Le azioni sono proposte descrittive: nessuna esegue comandi nel gioco.

## Osservato, calcolato, sconosciuto

Un `Fact` sconosciuto ha sempre `value:null`; zero, `false` e lista vuota possono
essere osservazioni valide. Un dato noto richiede provenienza non vuota.
Un campione `usable:false` non trasmette i suoi vecchi valori al dominio.
Gli indicatori di elenco incompleto (`focus_complete:false`,
`research_complete:false`) impediscono di inferire completamenti o assenze.

| Esempio | Classificazione |
| --- | --- |
| Nazione, ID completati, slot letti, livelli edifici, contatori nativi | `observed`, sorgente del trasporto |
| Albero associato tramite catalogo, somme/raggruppamenti divisioni, manpower disponibile | `calculated`, `legacy_adapter` |
| Fabbriche assegnate per differenza, output giornaliero per rapporto, saldo risorse, scorte aggregate | `calculated`, `legacy_adapter` |
| Nome localizzato, proprietà/controllo ottenuti per confronto, conteggio di una lista | `calculated` |
| Addestramento e fabbisogni non qualificati, sezioni indisponibili, campo assente | `unknown` |

La conversione di unità dei singoli contatori nativi mantiene la provenienza
osservata; un'aggregazione o una formula è calcolata. Le conferme dell'utente
rimangono nel dizionario separato `checks`: non diventano osservazioni.

Il modello espone `completion` e `selection` come fatti calcolati dall'appartenenza
agli elenchi noti; se l'elenco o lo scope non sono validi, rimangono sconosciuti.
`status` e `next` sono interpretazioni del percorso, non proprietà del gioco.
`availability` rimane sconosciuta finché manca una valutazione qualificata dei
trigger dinamici.

Chiavi stabili principali: `country`, `focus.tree`, `focus.completed`,
`focus.active`, `focus.continuous`, `focus.ids`, `research.completed`,
`research.active`, `research.slots`, `research.ids`.
Le sezioni nazionali usano `country.<sezione>.<contenitore>.<id>.<campo>`;
per liste senza identità stabile l'adattatore usa l'indice. Gli ID di linea
produzione sono attualmente ordinali, quindi non identificano una linea tra
riordinamenti o sessioni. `session_id` mantiene il significato del lettore:
non prova da solo l'identità persistente di una campagna.

## Condizioni e priorità

`fact` supporta `eq`, `contains` su liste, `gte` numerico; `check` consulta
esclusivamente conferme booleane. I confronti sono tipizzati, senza coercizione
fra stringhe, booleani e numeri. Nessun `eval` o esecuzione di script.
Profondità massima 16, al massimo 64 operandi per `all`/`any`.

La valutazione è a tre valori: vero, falso, sconosciuto (`None`/`null`).
`not unknown` resta sconosciuto; `all` è falso se un operando è falso e
`any` è vero se un operando è vero. Negli altri casi un operando sconosciuto
impedisce una conclusione. Una policy si attiva soltanto con risultato vero.

Le priorità condizionate sono ordinate per `priority` decrescente, con ordine
originale a parità. Il resto conserva l'ordine della guida. Lo stesso meccanismo
vale per focus e ricerca. Le attese restano visibili e fermano i suggerimenti
paralleli; una priorità non dichiara ricercabile il suo bersaglio.

Esempio di guida v1 senza vincoli nazionali:

```json
{
  "schema_version": 1,
  "id": "example",
  "title": "Industria iniziale",
  "applies_to": {},
  "tracks": {
    "research": [{
      "key": "tools",
      "action": {"kind": "research", "entity": {"kind": "technology", "id": "basic_machine_tools"}},
      "when": {"op": "fact", "key": "research.completed", "test": "contains", "value": "electronic_mechanical_engineering"}
    }]
  }
}
```

Questo esempio illustra una dipendenza editoriale: non simula un requisito HOI4.

## Compatibilità e API

- `GET /api/framework/state`: stato di dominio v1 dopo la normale acquisizione.
- `GET /api/guides`: catalogo delle guide e relativo ambito.
- `GET /api/guide?id=<id>`: guida validata, migrata e arricchita.
- `GET/POST /api/guide/checks?id=<id>`: checklist isolata per guida.
- `/walkthrough?guide=<id>` e `/hud?guide=<id>` selezionano lo stesso contenuto.
- Le richieste senza ID usano la guida predefinita, come prima.
- `/api/state`, SSE, alberi, tecnologie e panoramica nazionale mantengono i DTO
  esistenti. SSE aggiunge `guide_id` e `guide_states`; conserva `guide_checks`
  della guida predefinita e `guide_revision` globale. I nuovi consumatori devono
  selezionare `guide_states[guide.id]`.

L'adattatore accetta ancora guide legacy `schema:1`, `country`, `tree_id`,
`kind`/`target`, `gate`, `available_gate`, `priority_after`, `optional_before`.
Li converte in azioni e condizioni; gli alias necessari alle viste restano.
La guida attuale conserva 34 step focus, 48 step ricerca, checkpoint e fonti.
Le checklist restano in memoria nel processo; riavviarlo non le rende persistenti.

## Estensioni previste e limiti

Un Rules Engine potrà consumare `LiveState` e restituire fatti con provenienza,
senza conoscere lettore nativo, HTTP o DOM. Un Planning Engine potrà consumare
azioni, condizioni e cataloghi senza vincoli nazionali incorporati.
Gli operatori attuali sono un nucleo dichiarativo: non sono un interprete
completo dei trigger Clausewitz né un pianificatore con ricerca di strategie,
simulazione di costi o ETA.

Le tracce del contratto sono generiche; la vista esistente presenta ancora i
due pannelli focus/ricerca. Nuovi domini e tipi di azione richiederanno il loro
adattatore e una vista. I DTO dei parser e delle altre viste non sono stati
riscritti: il confine di compatibilità evita modifiche al sottosistema nativo.
La provenienza dei campi legacy resta una mappatura esplicita delle derivazioni
attuali: aggiungere campi al lettore richiede verificarla e aggiornare i fixture.

## Verifica

```powershell
$env:PYTHONPATH = 'src'
py -3 -m unittest discover -s tests -p 'test_*.py'
node --test 'tests/*.cjs'
```

I fixture JSON condivisi fra Python e JavaScript verificano il contratto wire e
la provenienza per campo. Test dedicati coprono migrazione, più nazioni e guide,
scope ambiguo, priorità multiple e tecnologiche, dati parziali, rollback e
checklist isolate. Test HTTP esercitano i servizi reali con cataloghi sintetici;
il controller della guida viene eseguito anche con un DOM in memoria.
Quest'ultima prova non sostituisce una verifica grafica nel browser.

Verifica del 21 ottobre 2026: **136 test Python e 30 JavaScript superati**.
Confrontando la guida precedente con quella migrata, azioni, condizioni, ordine
e metadati normalizzati risultano identici. Un'istanza con il codice finale ha
servito correttamente pagine, script, guida e contratto v1; HTTP e SSE hanno
confermato la partita live della Romania. La guida ha 34 focus, 48 ricerche e
nessun avviso di corrispondenza nel catalogo. La verifica grafica nel browser
non è stata eseguita perché il suo utilizzo è disabilitato nelle impostazioni
di Work. Questi risultati non misurano FPS o latenza di aggiornamento.
