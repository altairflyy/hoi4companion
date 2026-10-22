# HOI4 Companion — Focus Explorer (MVP 0.1.0)

Applicazione **gratuita e open source** per esplorare i focus nazionali di *Hearts of Iron IV* da un'interfaccia esterna, senza installare una mod, alterare il checksum o modificare file del gioco.

> **Stato:** prototipo funzionante, non versione definitiva. Non è affiliato a Paradox Interactive. Non include file o asset del gioco.

## Installazione

1. Clona o scarica il repository in una cartella a scelta.
2. Assicurati di avere Python **3.11 o successivo**.
3. Installa l'unica dipendenza (Pillow, per la conversione delle texture del gioco):
   ```bash
   pip install "Pillow>=12.3,<13"
   ```
   oppure installa in modalità modificabile:
   ```bash
   pip install -e .
   ```
4. Su Windows puoi avviare direttamente con un doppio clic su **`avvia.bat`**. Verrà aperta l'interfaccia nel browser su `http://127.0.0.1:8766`.
5. Apri **Configurazione** dall'interfaccia e imposta:
   - Percorso della cartella principale di Hearts of Iron IV (dove risiede `hoi4.exe`).
   - (Opzionale) Percorso della cartella documenti di HOI4 (per leggere mod attive da `dlc_load.json`).

**Primo avvio:** se non viene trovata l'installazione di HOI4 appare un **albero dimostrativo INVENTATO**. Non rappresenta l'albero originale dell'Austria.

### Esecuzione dalla console

Windows PowerShell, dalla cartella del progetto:

```powershell
$env:PYTHONPATH = "$PWD\src"
py -3 -m hoi4companion --game-dir "D:\SteamLibrary\steamapps\common\Hearts of Iron IV"
```

Su Linux/macOS:

```bash
PYTHONPATH=src python3 -m hoi4companion --game-dir "/percorso/Hearts of Iron IV"
```

Altri argomenti: `--docs-dir`, `--snapshot`, `--country AUS`, `--port 8766`, `--no-browser`, `--test`.

## Funzionalità disponibili

- **Guide Importer MVP** (`/importer`): prompt completo da dare alla propria
  IA insieme alla guida; importazione diretta del file restituito, risoluzione ID, citazioni,
  anteprima modificabile ed esportazione/installazione di guide v1.
  Acquisizione YouTube/web e API restano opzioni aggiuntive.
  [Istruzioni e limiti](docs/guide-importer.md).

- **Guide Runtime universale v1**: interpreta dipendenze, condizioni di
  completamento, fasi, priorità e azioni parallele senza generare strategie.
  Checklist separate per partita e stati non verificabili espliciti.
  Semantica, API e limiti in [docs/guide-runtime.md](docs/guide-runtime.md).

- **Avvio automatico delle costruzioni**: ordini letti live anche in pausa,
  con edificio, regione, quantità in coda e fabbriche attive. La guida usa
  condizioni dichiarative `started_when`; l'avvio resta distinto dal
  completamento. Copertura e prove in [docs/construction-live.md](docs/construction-live.md).

- **Agenzia di intelligence live**: creazione avviata e agenzia operativa
  sono rilevate separatamente, anche in pausa; completamento automatico
  soltanto quando l’agenzia è creata. [Copertura e prove](docs/intelligence-live.md).

- **Contratti del framework v1**: entità, stato live, condizioni, azioni e guide
  tipizzati; catalogo guide indipendente dalle nazioni, priorità dichiarative e
  checklist isolate. Dati osservati, calcolati e sconosciuti distinti.
  Architettura, compatibilità e limiti in [docs/framework-contracts.md](docs/framework-contracts.md).

- **Stato della nazione** su `/country`: divisioni totali/per tipo e modello,
  manpower, fabbriche e infrastrutture dei territori, produzioni con fabbriche
  richieste/assegnate, magazzini e saldi delle materie prime. Lettura live della
  build Windows supportata; sezioni parziali esplicite per addestramento e
  fabbisogni logistici. Uso, evidenze e limiti in [docs/country-live.md](docs/country-live.md).

- Parser degli script Clausewitz `.txt`, capace di preservare campi duplicati e blocchi annidati.
- Estrazione `focus_tree`, `focus`, `shared_focus` e `joint_focus` (importazione basilare).
- Prerequisiti con **OR all'interno dello stesso blocco** e **AND tra blocchi differenti**; focus mutuamente esclusivi.
- Coordinate `x`, `y`, `relative_position_id`, costo stimato, icona indicata per nome, ID e testo localizzato.
- Grafo navigabile con ricerca, zoom, dettaglio, evidenziazione di tutti i possibili prerequisiti.
- Lettura memoria **live sperimentale su Windows**: nazione giocata, albero nazionale, completamenti, focus nazionale/continuo e slot di ricerca. Eventi SSE al browser solo quando cambia lo stato; controlli leggeri dei campi mutabili ogni 250 ms e verifica completa ogni 5 secondi. Profilo vincolato all'hash della build locale 1.19.3; una build diversa viene rifiutata. Dettagli e prove in [docs/live-state.md](docs/live-state.md).
- Sovrapposizione dei file dalle mod attive (cartelle) e rilevamento dei file che sostituiscono file originali per percorso.
- Snapshot JSON importabili; parser **sperimentale** per salvataggi `.hoi4` testuali, solo per lo stato dei focus.
- Contrassegni di completamento **manuali e locali nel browser** quando non è disponibile uno snapshot.
- Filtro DLC automatico del menu: legge i descrittori e i relativi contenuti in `dlc` e `integrated_dlc`, quindi applica `disabled_dlcs` del launcher. Nessuna interrogazione Steam e nessuna modifica al gioco.
- Selezione conservativa degli alberi iniziali: valuta `has_dlc`, tag nazionali, AND/OR/NOT singolo, trigger statici richiamati e priorità `add`/`factor`. Le versioni con priorità inferiore non vengono proposte per lo stesso paese; un albero condiviso resta visibile se serve ancora ad almeno un paese.
- Pannello **Filtro DLC attivo** con inventario, alberi esclusi e motivi. **Ricarica** ricontrolla DLC, launcher e script dal disco.
- Icone e cornici originali lette da `.gfx` e texture DDS/PNG/TGA installate, convertite in PNG solo nella cache locale `.local/sprite-cache`. Include atlas a frame orizzontali e DDS DX10 BGRA sRGB.
- Tooltip al passaggio o con tastiera, descrizioni complete con colori e paragrafi, simboli inline e testi diretti di requisiti/bypass/effetti. Le chiavi `$KEY$` statiche vengono risolte con limiti e protezione dai cicli; i valori dinamici restano distinguibili.
- Navigazione con trascinamento, ricerca che centra i risultati, pulsante **Centra focus**, riferimenti ai prerequisiti cliccabili e linee tratteggiate per i percorsi esclusivi.

Il filtro verifica la compatibilità degli **alberi iniziali** con l'installazione locale. Non legge licenze Steam, DLC prestati dall'host multiplayer, eventi o sostituzioni dell'albero avvenute durante una partita. Quando attivazione o condizioni non sono verificabili, l'albero è riportato tra quelli non mostrati, senza dichiararlo disponibile. Il generico rimane disponibile per i paesi senza albero specifico. La demo non viene filtrata.

### Lettura dello stato live sperimentale (Windows)

Carica la partita in HOI4 e lascia attiva la casella **Leggi lo stato corrente · sperimentale** nelle viste Focus e Tecnologie (o visita `/country` per la panoramica della nazione):

- **Architettura e sicurezza:**
  - Richiede esclusivamente i privilegi `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ` (0x410) tramite `ReadProcessMemory` delle API Windows.
  - Nessuna scrittura in memoria (`WriteProcessMemory`), nessun debugger, nessun hook, nessuna iniezione di codice DLL e nessuna modifica ai file di gioco. Checksum e anticheat rimangono inalterati.
- **Validazione rigorosa della build:**
  - Il profilo di memoria è vincolato all'hash SHA-256 esatto dell'eseguibile testato (`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`, build `Operation Postern v1.19.3.0.c01a (42c3)`).
  - Versioni diverse o hash non riconosciuti vengono **rifiutati esplicitamente** per evitare letture corrotte o puntatori errati.
- **Dati osservati in tempo reale:**
  - Nazione giocata (tag) e identificazione dell'albero focus associato.
  - Focus nazionale corrente e focus continuo attivo.
  - Elenco dei focus completati o saltati.
  - 5 slot di ricerca tecnologica attiva e livelli tecnologici ricercati.
  - Panoramica nazionale: divisioni totali/per tipo, manpower disponibile, fabbriche civili/militari/cantieri, linee di produzione e saldi risorse.
  - Trasmissione al browser tramite flusso **Server-Sent Events (SSE)** con controlli leggeri a intervalli di 250 ms e verifica strutturale completa ogni 5 s.
- **Limiti e comportamenti definiti:**
  - **Nessuna valutazione di trigger dinamici:** condizioni complesse come `available`, `allow_branch`, `visible` o `bypass` non vengono simulate. I focus non completati rimangono non verificati; il companion non attribuisce disponibilità arbitraria.
  - **Nessun calcolo di percentuali o ETA:** non vengono stimate velocità di ricerca/produzione né date di completamento.
  - **Focus congiunti non supportati:** gli oggetti `CJointNationalFocus` non sono attualmente supportati e gli alberi che li contengono vengono rifiutati in modo sicuro.
  - **Invalidazione immediata:** chiudendo il gioco, tornando al menu o passando a dati non coerenti, lo stato live viene revocato (`usable=false`) per prevenire la persistenza di dati fantasma o obsoleti.
  - I contrassegni manuali dell'utente rimangono sempre separati dallo stato live.
  - Dettagli architetturali completi e benchmark in [docs/live-state.md](docs/live-state.md) e [docs/country-live.md](docs/country-live.md).

**Apri l’albero della nazione rilevata** porta all’albero identificato se disponibile nel catalogo locale.

### Come collegare uno snapshot (modalità alternativa)

Disattiva la lettura live, apri Configurazione e imposta `Percorso snapshot` su un file JSON:

```json
{
  "tag": "AUS",
  "completed_focuses": ["ID_focus_1", "ID_focus_2"],
  "current_focus": "ID_focus_3",
  "focus_progress": 0.5
}
```

Il file `examples/demo_snapshot.json` funziona con l'albero dimostrativo. Selezionandolo puoi verificare subito il tracker.

Per un file `.hoi4` testuale occorre disporre di un salvataggio in formato testuale **non Ironman**. Il riconoscimento non è garantito su tutte le versioni; quando il formato è sconosciuto l'applicazione evita di inventare completamenti.

L'aggiornamento automatico ogni 15 secondi **rilegge soltanto il file indicato**: non legge il gioco in tempo reale. Se il salvataggio non cambia, non cambiano nemmeno i dati.

## Limitazioni dichiarate

1. **Selezione nazionale:** il menu mostra gli alberi iniziali verificabili con i DLC locali, considerando i pesi per ciascun tag. Gli alberi assegnati da eventi, i pareggi e le condizioni dinamiche non risolte sono esclusi dal menu e motivati nel pannello DLC. Il reader live identifica l'albero della partita caricata solo se la lettura è valida e ID/appartenenza dei focus trovano riscontro nel catalogo: non amplia automaticamente il menu ad alberi esclusi.
2. **DLC:** il filtro riguarda la selezione dell'albero intero. Le condizioni dei singoli focus, la visibilità dei rami e i DLC disponibili soltanto durante una sessione multiplayer non vengono simulati. I file installati non costituiscono una verifica della licenza Steam.
3. **Mod:** funzionano normalmente le mod con file di focus in cartelle. Mod archiviate e combinazioni avanzate di `replace_path`, DLC e script dinamici necessitano di ulteriori verifiche. Nessuna modifica ai file originali.
4. **Disponibilità:** `available`, `allow_branch`, `visible`, `bypass` e `completion_reward` non vengono eseguiti: lo stato `Da verificare` è intenzionale.
5. **Grafica:** usa asset proprietari soltanto dall'installazione locale e dalle mod configurate. Nessun asset viene incluso nel codice o distribuito. Precedenza scelta dal companion: gioco, DLC installati/non disattivati, mod nell'ordine configurato; non è una verifica del montaggio effettivo dell'engine. Le icone condizionali mostrano un'anteprima e le varianti senza affermare quale sia attiva. Gli shader/shine dell'engine non sono riprodotti. Se una `.gfx` dichiara una `.png` inesistente ma esiste una `.dds` dello stesso nome, il companion usa quest'ultima: questo non prova il fallback interno di HOI4.
6. **Durata:** i giorni mostrati sono `cost × 7` come stima standard, non una previsione precisa con modificatori attivi.
7. **Save:** parser dei salvataggi testuali sperimentale; i formati binari e Ironman non sono supportati, e non si tenta di decodificarli.
8. **Rendering:** alberi con layout condizionale/offset dinamici e focus condivisi complessi potrebbero differire dalla posizione reale.
9. **Guide:** disponibile il walkthrough curato Polonia Romanov → Wojtek dal video FBPdhlwqMwM. Non è un importatore automatico di video arbitrari e non contiene un’IA che decide mosse nel gioco.
10. **Tooltip:** non sono tooltip runtime estratti dal motore. I testi e gli effetti diretti vengono letti dagli script; IF/ELSE, effetti nascosti o scriptati, parametri del motore e riferimenti `[ROOT...]` non vengono simulati. I rami condizionali non vengono presentati come ricompense simultanee. I frame inline con indice 0 o le texture mancanti restano segnalati. La copertura indicata nell'interfaccia misura i percorsi icona risolti; una conversione fallita mostra un segnaposto.

## Struttura

```text
hoi4companion/
├── avvia.bat, test.bat
├── src/hoi4companion/
│   ├── clausewitz.py      # parser formato Paradox
│   ├── focus.py           # alberi, prerequisiti, stati
│   ├── installation.py    # paths, mod, localizzazioni
│   ├── state.py           # snapshot JSON e .hoi4 testuali
│   ├── live.py            # reader esterno VM_READ vincolato alla build
│   ├── api.py             # server HTTP locale
│   ├── __main__.py        # CLI
│   └── web/               # frontend HTML/CSS/JS
├── tests/                 # parser, DLC, tooltip e conversione sprite
├── examples/              # dati dimostrativi INVENTATI
└── .local/                # impostazioni, generata al primo salvataggio
```

## Verifiche

### Walkthrough e HUD

Focus nazionali e ricerche vengono **evidenziati direttamente nei rispettivi
alberi**, mantenendo icone, coordinate e schede del gioco. La barra della guida
offre **Mostra focus / Mostra ricerca**; anche la scheda tecnologica che contiene
il prossimo elemento è contrassegnata. Oro significa prossimo passo, azzurro
in corso, tratteggio attesa o anteprima.

Apri `http://127.0.0.1:8766/walkthrough` per le **istruzioni operative**:
fabbriche e infrastrutture, produzione, politica, personaggi, eserciti, template
e progetti speciali, con indicazione di quando intervenire e riferimenti alla
fonte. I 34 passaggi focus e le 48 priorità ricerca sono conservati nei dati
della guida, senza riproporre le loro liste nella pagina operativa.
Dal 1936 il primo focus è **Bloccare Danzica**; le prime tre priorità di ricerca
sono **Ingegneria elettro-meccanica**, **Attrezzature di base** e **Artiglieria Interguerra**.

Gli eventi del lettore aggiornano completati, attività e prossimi passi quando la
partita verificata è polacca e l’albero corrisponde. A gioco chiuso resta l’anteprima;
non vengono applicati campioni obsoleti. Lituania, sconti, progetti speciali e
disponibilità dell’unità slava richiedono conferma. Le conferme sono condivise fra
guida e HUD nella memoria del server e si azzerano riavviandolo. **Nuova checklist**
le azzera con possibilità di annullare. I livelli indicati come UP sono interpretazioni
dei collegamenti locali, distinti dalle voci esplicite dell’autore.

Per l’HUD avvia **`avvia-hud.vbs`** (senza finestra console) oppure `avvia-hud.bat`.
Avvia il companion se necessario e crea un piccolo pannello Edge separato, sempre
in primo piano sopra HOI4, con l'istruzione operativa da verificare e i collegamenti
agli alberi guidati. Si nasconde quando passi
a un’altra applicazione. Usa un profilo locale dedicato `.local/hud-browser`, nessun
accesso all’account del browser, mod o codice iniettato. Chiudi la finestra HUD con X.
Richiede modalità finestra o finestra senza bordi per una sovrapposizione affidabile:
la modalità schermo intero esclusiva non è qualificata. Il pannello evidenzia le
raccomandazioni nell’HUD; **non traccia direttamente il nodo dentro l’albero del gioco**.
Il lancio `powershell -File scripts/hud.ps1 -Preview` mostra l’anteprima anche a gioco chiuso.
Lo script di spegnimento non parte con l’HUD: viene eseguito solo su richiesta esplicita.

Dettagli e limiti in [docs/walkthrough.md](docs/walkthrough.md).

### Ricerca tecnologica

Apri **Tecnologie** nella navigazione oppure `http://127.0.0.1:8766/technologies`.
La vista legge `common/technologies` e `common/technology_tags`, applica le condizioni DLC
alle categorie e ai rami e conserva le tecnologie condivise fra più categorie.
Mostra icone nazionali con fallback generico, descrizioni, percorsi dichiarati,
dipendenze aggiuntive, esclusioni reciproche, sottotecnologie e sblocchi.
Le categorie sono schede con le icone e nell'ordine dichiarato dalla GUI di HOI4.
Le schede mostrano le griglie native complete, con gli stessi gruppi e orientamenti,
riquadri compatti o larghi secondo il template, sfondi, cornici e intestazioni degli anni
letti dall'installazione locale. **Adatta scheda** scala uniformemente la vista;
zoom e trascinamento consentono di ingrandire i particolari. **Dettagli** apre il pannello
della tecnologia selezionata; i tooltip restano disponibili al passaggio o con tastiera.
**Rami** rende disponibili filtri opzionali e i dati fuori dalla scheda del gioco:
i discendenti di radici escluse dai DLC non vengono accodati alla vista principale.
Non è una replica pixel per pixel, né una simulazione delle condizioni del motore.
Vedi [docs/technology-fidelity.md](docs/technology-fidelity.md) e
[verifica nel browser](docs/technology-fidelity-verification.md) per prove e limiti.
Anno e coefficiente di costo non indicano durata effettiva o ricercabilità corrente.
Condizioni dinamiche e progetti speciali sono segnalati; lo snapshot dei focus non è usato
per attribuire ricerche completate. Le dottrine del nuovo sistema restano fuori da questa vista.

## Esecuzione dei test

Il repository dispone di una suite completa di test automatizzati per il backend Python e per i modelli frontend:

### Test Python (309 test)
Tramite lo script batch (Windows):
```cmd
test.bat
```
oppure da riga di comando:
```powershell
$env:PYTHONPATH = "$PWD\src"
py -3 -m hoi4companion --test
```
(in alternativa: `python -m unittest discover -s tests -p "test_*.py"`).

### Test frontend JavaScript / Node.js (203 test)
I modelli logici del client sono verificati tramite il test runner nativo di Node.js (`node:test`):
```powershell
Get-ChildItem -Path "tests" -Filter "*.cjs" | ForEach-Object { node $_.FullName }
```

## Sicurezza e rispetto del gioco

Il server locale è vincolato all'interfaccia di loopback `127.0.0.1` e non espone accesso remoto. Le richieste di configurazione provengono dalla medesima origine e persistono esclusivamente nella cartella locale ignorata dal versionamento (`.local/settings.json`). I percorsi di Hearts of Iron IV, i file mod e gli eventuali save sono letti strettamente in sola lettura, mai alterati. Nessuna iniezione nel processo, nessun anticheat bypass e nessuna alterazione del checksum di gioco.

## Licenza e vincoli delle dipendenze

- **Licenza del codice:** [MIT License](LICENSE).
- **Verifica delle dipendenze di terze parti:**
  - L'unica dipendenza esterna di terze parti a runtime è **Pillow** (`Pillow>=12.3,<13`), concessa con licenza open source **HPND / PIL Software License**, licenza ampiamente permissiva e pienamente compatibile con la licenza MIT.
  - Il resto del software adotta esclusivamente la libreria standard di Python 3.11+, standard web ECMAScript / CSS nativo, e Node.js built-in (`node:test` / `node:assert`) per i test di modello.
  - Nessuna dipendenza impone vincoli copyleft (GPL) o clausole proprietarie incompatibili.
- **Nessun asset proprietario ridistribuito:** Il repository non contiene file `.hoi4`, texture DDS/TGA, audio o script proprietari di Paradox Interactive. Tutte le risorse grafiche originali vengono estratte dinamicamente e memorizzate solo in locale nella cache dell'utente.
- **Dichiarazione sui marchi:** *Hearts of Iron IV* è un marchio registrato di Paradox Interactive AB. Questo progetto è indipendente, gratuito e open source, e non è sponsorizzato né affiliato a Paradox Interactive.

## Evoluzione prevista

- Migliorare la fedeltà al formato Clausewitz, la gestione del playset e dei DLC, testando file HOI4 reali.
- Valutare la lettura dei salvataggi con parser Jomini/Rakaly e supporto al formato binario nel rispetto delle limitazioni dei token resolver.
- Estendere il walkthrough curato ad altri video e percorsi, con verifica esplicita delle varianti UP e degli eventi non osservati dal reader.
- Qualificare il reader live su avanzamento della partita, cambi campagna e altre nazioni/build; aggiungere progressi solo dopo riscontro nel motore e nella GUI.

## Fonti tecniche

- https://hoi4.paradoxwikis.com/National_focus_modding
- https://github.com/rakaly/hoi4save (archiviato, incorporato in Jomini)
- https://github.com/DRThornton/HOI4Advisor (ispirazione, codice non copiato)
