# Analisi statica con CWTools e Rust/Jomini

La pagina `/analysis` usa ora **il motore CWTools completo**, revisione
`b377453dee803f9258be92cfc49896d09039702d`, attraverso il componente nativo Rust.
Non usa più l’adattatore Python parziale per analizzare l’installazione.

**Aggiornamento parsing del 24 ottobre:** corretti i disaccordi lessicali fra
Jomini e CWTools e la classificazione dei documenti non script. L’installazione
passa da 46 a 31 errori, tutti rifiutati da entrambi i parser. I risultati iniziali
riportati sotto restano lo storico della prima integrazione.
[Correzioni, prove e casi residui](parsing-compatibility.md).

## Architettura e confini

- Jomini 0.37.2 resta il parser Rust dei cataloghi, con TextTape, modelli tipizzati,
  Windows-1252 e JSON. Le API di focus, tecnologie, guide e acquisizione live restano indipendenti.
- Il facade Rust chiama direttamente `Companion.Cwtools.exe`, senza shell o console
  visibili. Il processo locale usa la libreria upstream .NET e il suo parser interno;
  il motore CWT non è stato riscritto in Rust o tradotto in un sottoinsieme.
- Il manifesto dei file applica prima l’ordine delle mod attive, le sostituzioni
  per percorso e `replace_path` del Companion. Il motore riceve una sola vista
  effettiva tramite `ZippedDirectory`, senza copiare o modificare file del gioco.
- Il pacchetto contiene tutti i **111 file CWT** della directory Config upstream,
  più le risorse ausiliarie, revisione delle regole
  `297dd69c42aabe10078a8b7ab7f2d07ee631d4ef`.
- Provenienze e SHA-256 verificano sorgenti vendorizzati, regole e host distribuito.
  Le licenze CWTools MIT e gli avvisi delle dipendenze .NET sono inclusi.

Fonti: [motore](https://github.com/cwtools/cwtools/tree/b377453dee803f9258be92cfc49896d09039702d),
[regole HOI4](https://github.com/cwtools/cwtools-hoi4-config/tree/297dd69c42aabe10078a8b7ab7f2d07ee631d4ef).

## Controlli e contratto

CWTools interpreta alias, cardinalità, tipi e sottotipi, enum, contesti, riferimenti,
scripted trigger/effect e localizzazioni secondo le regole incluse. Le diagnosi
mantengono codice upstream, severità, file, righe e colonne. La classificazione
nei quattro gruppi dell’interfaccia è presentazione; non cambia la diagnosi.

`GET /api/script-analysis` mantiene `schema_version: 1`, `source: static_analysis`,
`runtime_evaluated: false`, `diagnostics`, `summary`, `sources` e `file_warnings`.
`rules.adapter` vale `cwtools-engine-v1`; `rules.engine_revision` identifica il motore.
Il risultato è memorizzato in cache e invalidato al ricaricamento dei cataloghi.
L’analisi costosa non tiene il lock delle API live/SSE.

I vecchi `coverage.recognized`, `unsupported` e `unknown_scope` valgono **null**:
CWTools non espone quelle misure. `metrics_available: false` impedisce di mostrarle
come zeri. Sono invece disponibili documenti caricati, definizioni tipizzate,
nomi dei tipi, sorgenti di localizzazione ed errori di parsing effettivi.
`status: analyzed` indica che l’esecuzione è terminata senza errori di lettura/parsing,
non che tutti gli script o le condizioni della partita siano validi.

L’elenco è limitato a 2.000 diagnosi, ma i totali comprendono tutte le severità e
le diagnosi omesse. `truncated` segnala il limite. Un host mancante o incompatibile,
una regola CWT sintatticamente malformata o un timeout producono un errore esplicito
(HTTP 503 per il motore), senza tornare silenziosamente al vecchio adattatore.

`GET /api/script-rules` conserva la consultazione degli schemi e delle descrizioni.
Il vecchio `analyze_documents` rimane soltanto per la dimostrazione senza gioco e i
suoi test storici: è esplicitamente parziale e non è il backend dell’installazione.

## Build e distribuzione

Il wheel Windows x64 include l’host **self-contained** con runtime .NET 9.0.21.
L’utente finale non deve installare .NET, Rust, VS Code o CWTools. Non sono previsti
scaricamenti a runtime.

Per compilare i sorgenti servono Rust/MSVC e .NET SDK 9.0.319 (patch compatibili
ammesse da `global.json`):

```powershell
powershell -NoProfile -File scripts/build-parser.ps1
powershell -NoProfile -File scripts/build-cwtools.ps1
```

`pip install -e .` e la creazione del wheel compilano entrambi i componenti.
Il build .NET ripristina dipendenze fissate da `packages.lock.json`; il build Rust
usa `Cargo.lock`. NuGet/rete servono solo durante la compilazione.
L’ABI aggiunta `hoi4_cwtools_analyze` non modifica l’ABI del parser.
Il CLI Rust accetta anche `action: analyze`, un percorso assoluto dell’host e
`request: {schema_version: 1, rules_dir, parser_library, files: {percorso_logico: file_assoluto}}`.
`parser_library` è il percorso assoluto della libreria Jomini del pacchetto,
usata dall’host per la compatibilità lessicale e il confronto degli errori.

Limiti del bridge: manifesto 16 MB, risposta 64 MB, file testuali 80 MB, esecuzione
300 secondi. Il processo viene terminato al timeout. I file grafici/binari sono
indicizzati per verificarne i riferimenti, senza decodificarne il contenuto.

## Verifiche eseguite

- **418 test Python e 236 JavaScript superati**, compresi test reali attraverso
  l’ABI Rust e l’host: riferimenti a shared focus e sottotipi evento, alias,
  cardinalità, tipo booleano, contesto errato, localizzazione e parsing fallito.
- **14 test Rust superati**, Clippy con `-D warnings` superato.
- Build da sdist a wheel Windows CPython 3.13 x64, installazione isolata e chiamata
  reale al motore senza Rust/.NET nel PATH: verificate.
- Installazione locale con cinque mod attive: **38.608 file indicizzati,
  5.591 documenti elaborati, 150.893 definizioni**, circa 48 secondi per il motore
  nella prima misura. **46 errori di parsing** esposti, inclusi testi di licenze
  con estensione `.txt` e alcuni file non accettati dal parser upstream.
- Il motore ha restituito 42.247 diagnosi: sono controlli sull’intera installazione,
  comprendono suggerimenti e informazioni e **non rappresentano 42.247 bug confermati**.
  Le misure del vecchio adattatore non sono confrontabili: l’ambito ora è più ampio.
- Lettura della partita reale in background: `observed`, `usable: true`, Polonia,
  stessa identità di campagna. Il lettore resta sperimentale; questa prova non
  certifica tutti i campi rispetto ai pannelli del gioco.
- Dopo il riavvio del server, il payload SQLite dell’avanzamento coincideva con
  il backup precedente. Verificati identità di campagna/run e guida, sette risposte
  HTTP 200, flusso SSE utilizzabile e parità di 55 API dei cataloghi fra server
  precedente e candidato. La risposta dell’analisi in cache ha richiesto 0,392 s.

## Limiti residui

- La compatibilità del pacchetto di regole con ogni build/DLC/mod non è certificata.
  Nuove istruzioni o interpretazioni diverse possono generare falsi positivi.
- File non interpretati rendono incompleto l’indice: un riferimento segnalato come
  mancante può dipendere da quel parsing fallito. Questi errori restano visibili.
- CWTools non esegue le condizioni del gioco né risolve ogni espressione dinamica;
  non restituisce una misura esaustiva della copertura. L’assenza di diagnosi non
  prova che una condizione sia vera o che una guida sia completata.
- L’analisi delle localizzazioni è in inglese, lingua di riferimento supportata dal
  motore; la lingua delle diciture del Companion e le traduzioni mod restano gestite
  dal caricatore esistente. Non certifica la completezza della traduzione italiana.
- Il processo viene ricreato quando si ripete l’analisi. Il costo iniziale è evitato
  nelle normali richieste grazie alla cache; non è un controllo periodico live.
  In un campionamento sull’installazione locale il working set del processo
  CWTools era circa 3,2 GB; il processo termina e libera quella memoria dopo
  l’analisi. Non è una misura del picco né un benchmark generalizzabile.
- Distribuzione verificata soltanto su Windows x64/CPython 3.13. Il codice .NET
  upstream presenta due avvisi di compilazione FS0104, non errori del bridge.
- Nessun test visivo del browser o dell’overlay è stato eseguito in questa sessione.
  Le verifiche web usano API HTTP, SSE e test del DOM.
