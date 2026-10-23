# Parser Jomini: integrazione e sostituzione del parser Python

## Esito al 23 ottobre 2026

**Jomini 0.37.2 è ora il solo parser degli script del programma.** Il parser
Python precedente è stato spostato in `scripts/legacy_clausewitz.py`, congelato
come riferimento per i confronti e assente dalla wheel. Server, motore guide,
lettore live e overlay restano implementati in Python: questa modifica completa
la sostituzione del parser, non la riscrittura dell'intera applicazione.

La libreria nativa viene compilata dal sistema di packaging `setuptools-rust`,
inclusa nelle wheel di piattaforma e individuata automaticamente nel pacchetto.
All'avvio viene verificata prima di leggere cataloghi o inizializzare la
persistenza. `/api/config` espone motore, versione, schema, `document_model: "TextTape"`,
`typed_models: 2` e le capacità effettivamente caricate (`windows-1252`,
`jomini-json`, `typed-focus-relations`).

Dipendenza fissata a [Jomini 0.37.2](https://docs.rs/crate/jomini/0.37.2), licenza
MIT, con `Cargo.lock`. Fonti: [repository ufficiale](https://github.com/rakaly/jomini)
e [README](https://github.com/rakaly/jomini/blob/master/README.md).

## Cosa è stato implementato

Il documento è ora costruito da **`jomini::TextTape`**, che determina tipi dei
contenitori e collegamenti di apertura/chiusura. La precedente funzione Rust che
ricostruiva i blocchi a partire dai token grezzi è stata rimossa. Rimane una
traduzione del DOM della libreria nel contratto `Node` usato dal framework:

- lista ordinata di coppie, con chiavi duplicate e blocchi anonimi;
- scalari stringa: `001`, date e `yes` non diventano numeri o booleani JSON;
- operatori distinti, Unicode, percorsi, escape e spazi finali;
- campi sconosciuti ed espressioni dinamiche interi e opachi, senza valutarli;
- errore per blocchi non chiusi/valori mancanti e profondità oltre 180;
- limite di 80 MB prima e dopo la decodifica, UTF-8/Windows-1252, contratto JSON
  e ABI C `schema_version: 1`.

Prima di `TextTape`, `TokenReader` effettua soltanto un controllo di bilanciamento
e una normalizzazione lessicale **in memoria**: racchiude tra virgolette i meta-ID
con parentesi quadre (senza cambiarne il valore), avvolge le liste anonime alla
radice e mantiene i blocchi anonimi vuoti che Jomini altrimenti normalizzerebbe
via. Il marcatore interno evita collisioni con qualsiasi testo sorgente. Non c'è
un fallback a un secondo parser, né una modifica a Jomini o ai file di HOI4.
L'adattatore percorre gli intervalli del documento già riconosciuti da TextTape;
contenitori misti, header e operatori rimangono nel contratto generico precedente.

**`JominiDeserialize` viene usato davvero dagli estrattori**, non soltanto nei
test. I modelli Rust versionati come `typed_models: 2` estraggono:

- focus: `id`, `x`, `y`, `cost`, `relative_position_id`, `icon`, gruppi di
  `prerequisite` e riferimenti `mutually_exclusive`;
- tecnologie: `research_cost`, `start_year`, `is_special_project_tech`.

I caricatori Python consumano questi campi nativi. Restano in Python localizzazione,
filtri, valutazione dei prerequisiti, tooltip, layout relativo e risoluzione delle variabili:
questa è una migrazione incrementale degli estrattori, non un loro port completo.
Le proiezioni distinguono stringa sorgente e campo mancante/non scalare. I valori
come `@cost` restano espressioni; il parser non li trasforma in numeri inventati.
Il primo campo con operatore `=` mantiene la precedenza originaria, un primo
blocco maschera gli scalari successivi e `x >= 3` non diventa la coordinata `x`.

Ogni nodo conserva tutti i campi originali, anche quelli esclusi dal modello
tipizzato. I contenitori misti fuori dal modello conservano il DOM generico e
continuano a essere letti dai caricatori esistenti. Un decoder fornito tramite
l'interfaccia `Encoding` di Jomini mantiene escape e spazi del contratto: il
decoder predefinito della libreria li normalizzerebbe diversamente.

La conversione JSON generica di Jomini, Windows-1252, salvataggi binari e scrittura
non vengono attivati da questo intervento. Il formato JSON ordinato serve ancora
per preservare il contratto con il framework. I file UTF-8 non vengono reinterpretati
come altre codifiche e le funzionalità di acquisizione live restano invariate.

Il bridge `jomini_parser.py` usa la libreria nello stesso processo, senza avviare
un eseguibile per ciascun file. DLL mancante o ABI incompatibile causano errore:
non esiste un downgrade silenzioso. Gli errori di installazione generano
`ParserUnavailableError`, distinto dai `ValueError` degli script malformati:
non possono essere assorbiti dai caricatori e trasformati in cataloghi vuoti.

## Prove eseguite

Corpus locale in sola lettura: gioco installato e sovrapposizione delle cinque
mod attive; `common`, `history`, `events`, `interface/*.gui` e `interface/*.gfx`.
Non vengono distribuiti file o asset del gioco.

| Risultato | Numero |
| --- | ---: |
| File confrontati | 5.020 |
| Byte sorgente | 91.402.816 |
| AST identici | 4.980 |
| File rifiutati da entrambi | 20 |
| File prima rifiutati, ora accettati | 1 |
| AST diversi per correzioni sintattiche esaminate | 19 |
| File prima accettati, ora rifiutati | 0 |
| Differenze non spiegate | 0 |

Le 19 differenze riguardano espressioni come `var?50` mantenute intere e
separatori `;`, che il parser precedente includeva nei valori o spezzava. Il gate
richiede l'allineamento dell'intero AST dopo queste sole correzioni: non accetta
la perdita di altri campi, né usa eccezioni per paese o file. I 20 rifiuti comuni
restano un limite: non si dichiara copertura universale di qualsiasi script/mod.

Tempo di parsing in una singola esecuzione locale: Python 7,002 s, Rust tramite
bridge 4,655 s, circa **1,50×** di throughput. Il dato Rust include FFI,
serializzazione JSON e ricostruzione dei `Node` Python; esclude lettura disco e
confronto degli AST. Non è una misura delle prestazioni dell'intera applicazione.

Confronto indipendente delle API del server legacy con uno Store offline Rust:
**40.774 entità identiche**, nessuna aggiunta/rimozione/modifica, **43 alberi
nazionali identici** e **9 schede tecnologiche identiche**. Uguali anche filtri
DLC, mod, alberi esclusi e catalogo tecnologico. Questo confronto usa gli stessi
file, impostazioni e localizzazioni; non richiede letture di memoria o modifiche
all'avanzamento della campagna.

Suite di sostituzione: **372 test Python con il parser nativo predefinito,
nessun test nativo saltato**, **232 test JavaScript**, **7 test Rust** (6 unitari
e un test dell'eseguibile JSONL), Clippy senza warning. Sono inclusi test di
individuazione senza variabili d'ambiente, ABI incompatibile, arresto prima
della persistenza e gate di confronto che rifiuta perdite di campi e riordinamenti.

Provati inoltre: build sdist e wheel Windows CPython 3.13; installazione della
wheel in un ambiente virtuale separato; avvio del catalogo demo e parsing da
una directory esterna con `PYTHONPATH`, override nativi e Rust assenti dal PATH.
La wheel contiene libreria, risorse demo e avvisi di licenza delle dipendenze
Rust, ma non parser legacy, dati locali o asset HOI4.

Verifica reale sulla partita polacca aperta: vecchio server e nuovo server
concordano su identità della campagna/nazione, focus, tecnologie completate e tre
slot (Macchine utensili di base, Ingegneria elettro-meccanica, uno libero).
Identici anche runtime della guida, run persistente e checklist. Il confronto
usa un database di avanzamento copiato tramite backup SQLite; nessun test
scrive nel database della partita attiva. Verificate nel browser le viste
Focus/Tecnologie e la guida localizzata. Nessun cambiamento di ricerca, campagna,
file o memoria del gioco viene eseguito per questa prova.

L'istanza abituale sulla porta **8772 è stata riavviata con Jomini**. Dopo il
passaggio sono stati ricontrollati stato `observed`, identità POL, evento SSE,
runtime della guida e persistenza: stessi run ID, campagna, checklist, avvii e
baseline. Prima del passaggio è stato creato un backup SQLite locale. Il processo
overlay esistente è rimasto attivo; non si dichiara una nuova prova visiva delle
cornici nel gioco, che non è stato portato in primo piano per questa modifica.

Report locali, esclusi da Git: `.local/jomini-replacement-corpus.json`,
`.local/jomini-replacement-catalogs.json` e `.local/jomini-replacement-live.json`.
Il controllo dopo il riavvio è in `.local/jomini-production-verification.json`.
Il primo confronto temporizzato (7,002/4,655 s) precede la sostituzione; una
ripetizione concorrente con altri controlli ha misurato 7,328/5,992 s. Questi
campioni non costituiscono un benchmark controllato dell'intera applicazione.
I comandi sotto permettono di rigenerare i gate.

## Installazione e controlli

Per chi usa una wheel della propria piattaforma/versione Python:

```powershell
py -3 -m pip install percorso/hoi4companion-0.1.0-cp313-cp313-win_amd64.whl
py -3 -m hoi4companion --no-browser --port 8780
```

Non serve Rust per installare la wheel. **Su questo PC è stata verificata Windows
x64 con CPython 3.13**; non si dichiarano provate wheel Linux, macOS o altre
versioni Python. Le wheel restano artefatti locali, non pubblicati su GitHub/PyPI.

Per sviluppare dal sorgente occorrono Rust con toolchain MSVC su Windows e i
C++ Build Tools (oppure il linker della propria piattaforma):

```powershell
py -3 -m pip install -e .
```

Il sistema PEP 517 compila automaticamente il parser con `Cargo.lock`. In un
checkout Windows è disponibile anche il percorso diretto:

```powershell
powershell -NoProfile -File scripts/build-parser.ps1
$env:PYTHONPATH = "$PWD\src"
py -3 -m unittest discover -s tests
py -3 -m hoi4companion --no-browser --port 8780
```

Lo script usa la toolchain isolata `.local/toolchains` se presente, altrimenti
`cargo` dal PATH; compila in release e copia la libreria nel pacchetto sorgente.
Chiudere le istanze che la usano prima di ricompilarla su Windows. La toolchain
locale non viene inclusa nel repository né aggiunta al PATH dell'utente.

Non occorre impostare `HOI4_PARSER` o `HOI4_JOMINI_LIBRARY`. Il primo accetta solo
`jomini`: una vecchia selezione `legacy` genera un errore esplicito. Il secondo
resta un override per sviluppatori, non un requisito d'avvio. Non viene mai
selezionata automaticamente una libreria dalla directory corrente o dal PATH.
Una libreria mancante/incompatibile arresta l'avvio con istruzioni di ripristino.

L'eseguibile `.local/rust-target/release/hoi4-parser-cli.exe` accetta una richiesta
JSON per riga su stdin e restituisce una risposta per riga su stdout:

```json
{"schema_version":1,"source":"prerequisite={focus=A focus=B} prerequisite={focus=C}"}
```

Il CLI funziona senza Python; non è un nuovo server Companion. Il limite di
80 MB riguarda lo script, non qualifica il trasporto JSONL contro input ostili.

### Codifiche dei file e JSON nativo di Jomini

`parse_file(path, encoding="auto")` e `parse_bytes(data, encoding="auto")`
accettano byte originali: UTF-8 valido ha precedenza; altrimenti viene usata la
tabella **Windows-1252 di Jomini**. Il BOM UTF-8 è vincolante: un file con quel BOM
e byte UTF-8 errati viene rifiutato. È possibile forzare `utf-8` o `windows-1252`
quando l'origine è nota; una stringa già decodificata resta UTF-8. Auto non può
disambiguare un file Windows-1252 che forma anche una sequenza UTF-8 valida.
`Node.source_encoding` e le risposte native espongono la codifica usata.

Accenti, simboli, backslash e spazi finali vengono conservati senza sostituzioni
silenziose. I byte Windows-1252 non definiti e i contenuti riconosciuti come
binari vengono rifiutati. Un carattere di controllo presente in un commento
UTF-8 originale di HOI4 è invece accettato come prima. La lettura byte per byte
è ora usata dai caricatori di focus, tecnologie, GUI/GFX, politica, produzioni,
storia iniziale, descrittori DLC e mod. Le localizzazioni YAML e i file JSON
mantengono i loro lettori; non si introduce un nuovo decoder YAML.

`export_json(source, duplicate_keys="key_value_pairs")` chiama direttamente il
serializzatore di **Jomini**, con `TypeNarrowing::None`. Non converte il nostro
`Node` in un JSON di imitazione. L'esportazione è separata dal contratto AST delle
API: i consumatori esistenti mantengono il loro formato. Le modalità sono:

- `key_value_pairs` (predefinita): contenitori marcati con `type` e `val`; nei
  normali oggetti, coppie ordinate preservano duplicati e interleaving. `a=001 b=yes a=002`
  diventa `{"type":"obj","val":[["a","001"],["b","yes"],["a","002"]]}`.
- `group`: oggetti JSON comuni, duplicati raggruppati in liste; non conserva
  l'interleaving globale tra chiavi diverse.
- `preserve`: oggetti con chiavi JSON ripetute; richiede un lettore consapevole
  dei duplicati, altrimenti molti client conservano soltanto l'ultimo valore.

Il documento vuoto viene esportato da Jomini come array vuoto. Gli operatori
seguono la rappresentazione di Jomini, distinta dalle chiavi AST del Companion.
I wrapper interni del parser non compaiono nell'esportazione. Non si dichiara
round trip completo di commenti, impaginazione o byte sorgente.

Esportazione dal programma, senza inizializzare Store o avviare il server:

```powershell
py -3 -m hoi4companion --export-script "C:\mod\focus.txt" --output "C:\mod\focus-export.json"
py -3 -m hoi4companion --export-script "C:\mod\focus.txt" --script-encoding windows-1252 --json-duplicates group
```

Il primo comando crea esclusivamente un file `.json` nuovo: rifiuta destinazioni
già esistenti. Il secondo scrive UTF-8 su stdout. Il file sorgente è sempre letto
in sola lettura. Anche il CLI Rust autonomo supporta il formato:

```json
{"schema_version":1,"source":"a=001 a=002","format":"jomini-json","duplicate_keys":"key_value_pairs"}
{"schema_version":1,"source_bytes":[120,61,34,224,34],"encoding":"windows-1252","format":"jomini-json"}
```

Il nuovo punto di ingresso C `hoi4_parser_process` accetta codifica e modalità;
`hoi4_parser_parse` conserva l'ingresso UTF-8 AST precedente. L'ABI ha numero 1;
il bridge richiede esplicitamente modelli v2 e le tre capacità, così una vecchia
DLL non viene scambiata per un'installazione aggiornata.

Gate riproducibili dopo la compilazione, con `PYTHONPATH=src`:

```powershell
py -3 scripts/compare-parsers.py --game-dir "D:\SteamLibrary\steamapps\common\Hearts of Iron IV" --docs-dir "$env:USERPROFILE\Documents\Paradox Interactive\Hearts of Iron IV" --report .local/jomini-replacement-corpus.json
py -3 scripts/compare-parser-catalogs.py --reference-url http://127.0.0.1:8772 --report .local/jomini-replacement-catalogs.json
```

Il secondo comando confronta il nuovo Store con un server di riferimento: nella
qualificazione è stato usato il server precedente alla sostituzione, ancora
avviato con il vecchio codice. Dopo il riavvio il server corrente usa anch'esso
Jomini, quindi ripetere il comando contro quello non prova di nuovo la parità
con il vecchio parser. Il primo comando usa sempre la copia legacy congelata.
Entrambi i gate terminano con errore in caso di regressioni; il primo blocca
anche differenze AST non spiegate. Il secondo usa un archivio avanzamento
temporaneo e non chiama endpoint live.

## Come eliminare Python senza perdere funzionalità

| Componente attuale | Coperto da Jomini? | Passaggio necessario |
| --- | --- | --- |
| Parsing script testuali | Sì, sostituzione completata | Qualificare altre piattaforme e profili |
| Cataloghi, DLC/mod, localizzazioni, sprite e layout | DOM TextTape e primi campi tipizzati di focus/tecnologie | Completare estrattori e risoluzione risorse mantenendo output API |
| Guide Schema, importazione, ID, Rules/Guide Runtime | No | Portare motore deterministico con fixture condivise Python/JS |
| Checklist, baseline e campagne SQLite | No | Conservare formato e migrare persistenza con prove di ritorno campagna |
| HTTP, SSE e interfaccia web | No | Server Rust con gli stessi contratti; frontend riutilizzabile |
| Lettura memoria Windows e identità/build | No | Portare lettore e controlli senza ridurne i vincoli; verificare partita reale |
| Overlay, OCR e riconoscimento sprite | No | Sostituire componenti Python/OpenCV e verificare visivamente il gioco |
| Snapshot, acquisizione web/YouTube e supporto IA opzionale | No | Preservare formati, invocazioni e funzionalità opzionali |
| Salvataggi binari HOI4 | Non automaticamente | Token dictionary specifico della build, formato e semantica HOI4 |

Jomini supporta flussi binari Paradox, ma ciò non equivale a supportare già un
salvataggio binario HOI4: servono dizionari e adattamento specifici. Il nuovo
backend non estende la copertura dei salvataggi binari e non cambia l'acquisizione
live. Le informazioni osservate, calcolate o sconosciute restano responsabilità
del framework; il parser non inferisce completamenti o strategie.

La migrazione successiva resta per componenti, con confronto dei contratti a ogni
passaggio. Il parser Rust ora usa la libreria esistente; rimuovere adesso Python eliminerebbe funzionalità ancora implementate
in Python. Nessun file o memoria di HOI4 è stato modificato. La partita reale,
l'overlay e gli eventi live con un futuro server interamente Rust restano da
qualificare prima di dichiarare completata la migrazione.

## Qualificazione del passaggio a TextTape e modelli tipizzati

Report: `.local/jomini-texttape-corpus.json` (5.020 file, zero regressioni,
zero differenze non spiegate), `.local/jomini-texttape-catalogs-final.json`
(parità di 40.774 entità, 43 alberi nazionali e 9 schede tecnologiche).
Il riferimento API è il server con Jomini/TokenReader precedente, quindi questo
confronto verifica anche il passaggio intermedio, non soltanto la copia Python.

La profilazione ha identificato un costo indipendente dal parser: oltre 25.000
texture aperte in `load_technology_visuals`, con 111,8 s su 130 s del caricamento
profilato. I metadati delle texture vengono ora letti soltanto quando una GUI
tecnologica li richiede, con cache limitata alla singola ricarica. I controlli dei
percorsi e la precedenza delle mod rimangono applicati; cambiare un file e premere
Ricarica invalida la cache. La parità degli alberi tecnologici controlla anche
questi metadati. Questi tempi profilati non sono benchmark controllati del parser.

La libreria viene distribuita nel sottopacchetto `_native`, così un vecchio
binario rimasto nella radice del pacchetto non può essere caricato per errore.
La verifica d'avvio rifiuta esplicitamente librerie antecedenti a TextTape/modelli
v1, anche se la loro ABI C ha ancora numero 1.

Risultati finali di questa fase: **379 test Python, 232 JavaScript e 9 Rust**,
Clippy senza warning; wheel Windows CPython 3.13 installata e provata in ambiente
separato con Rust e override assenti dal PATH/configurazione. Il caricamento
cataloghi dopo la lettura delle texture su richiesta ha impiegato 12,93 s nella
prova locale; le API confrontate sono identiche al riferimento precedente.

Il server **8772 ora esegue TextTape e modelli v1**. Il controllo successivo al
riavvio (`.local/jomini-texttape-production-verification.json`) conferma stato
live `observed`/utilizzabile per POL, identità campagna e ricerche coerenti,
stessi run ID, checklist, baseline, avvii e runtime, persistenza senza errori e
un evento SSE con campione osservato. Focus, Tecnologie, Walkthrough e Importer
rispondono HTTP 200. Backup SQLite precedente al passaggio disponibile localmente.
Nessun file o memoria del gioco è stato modificato; il lettore nativo non cambia.

Il controllo visivo nel browser dopo questo ultimo riavvio non è stato possibile:
l'accesso browser locale risulta disabilitato nelle impostazioni Work. Le prove
di questa fase sulle pagine sono HTTP/API e suite JavaScript, non una nuova
qualificazione visiva dell'overlay. Restano i 20 file rifiutati da entrambi i
parser; non si dichiara copertura di qualsiasi sintassi di ogni mod.

## Qualificazione di modelli v2, Windows-1252 e JSON

Superati **392 test Python**, con individuazione della libreria predefinita e
nessun test nativo saltato, **232 JavaScript** e **12 Rust** (10 unitari, due del
CLI), oltre a Clippy con warning trattati come errori. I test coprono anche
UTF-8/BOM errati, file Windows-1252 con accenti e percorsi, relazioni focus
consumate dai caricatori, duplicati JSON e rifiuto delle sovrascritture all'export.

Il gate del corpus ora passa al parser nativo i **byte originali** dei 5.020
file. Confermati 4.980 AST identici, 20 rifiuti comuni, un nuovo file accettato,
19 correzioni sintattiche già esaminate, zero regressioni e zero differenze non
spiegate. Il confronto API con il precedente server TextTape/v1 conferma le
stesse **40.774 entità, 43 alberi nazionali e 9 schede tecnologiche**. Report:
`.local/jomini-features-corpus.json` e `.local/jomini-features-catalogs.json`.

Verificati sdist, wheel Windows CPython 3.13 e installazione separata: parser,
Windows-1252, duplicati JSON e demo funzionano senza Rust nel PATH e senza
`PYTHONPATH` o override della libreria. Gli artefatti restano locali.

Il server 8772 carica ora i modelli v2 e le tre capacità. La verifica iniziale
con gioco chiuso ha confermato runtime e avanzamento identici in uno staging con
database copiato. Durante il passaggio è stato riaperto HOI4 (PID 60176) sulla
Polonia a una data precedente: tick 60.760.777 contro 60.761.050 archiviato.
Il comportamento esistente ha creato una run con motivo `timeline_rewind`;
**la precedente run è conservata integralmente nel database**, verificata contro
il backup. Non si dichiara quindi lo stesso run ID attivo dopo questo caricamento.

Confermati stato live `observed`/utilizzabile, identità POL/campagna qualificata,
concordanza dei campi focus/ricerca tra staging e produzione, stessa semantica
del runtime, persistenza senza errori, evento SSE osservato e HTTP 200 per Focus,
Tecnologie, Walkthrough e Importer. Evidenze:
`.local/jomini-features-verification-8780.json`,
`.local/jomini-features-production-verification.json` e
`.local/jomini-features-package-smoke.json`.

L'accesso browser Work è disabilitato: queste prove non costituiscono una nuova
verifica visiva dell'interfaccia o dell'overlay. Nessun file o memoria di HOI4,
né il codice del lettore live, è stato modificato. Restano i 20 rifiuti comuni
del corpus e la migrazione parziale degli estrattori Rust descritta sopra.
