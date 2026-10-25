# HOI4 Companion

Un assistente esterno per **Hearts of Iron IV**: consulta gli alberi del gioco,
segui una guida e visualizza le prossime azioni, anche con evidenziazioni sopra
la finestra di HOI4. Gratuito, open source e ancora sperimentale.

**Non gioca al tuo posto e non genera strategie autonome.** Legge i file locali
del gioco e, sulle build supportate, osserva lo stato della partita in memoria.
Non installa mod, non modifica i file o la memoria di HOI4 e non preme pulsanti.
Non è un prodotto Paradox Interactive e non include gli asset del gioco.

## Installazione Windows: percorso semplice

1. Apri [Releases](https://github.com/altairflyy/hoi4companion/releases) e scarica
   `HOI4Companion-<versione>-windows-x64.zip`, quando disponibile.
   I collegamenti **Source code** contengono il sorgente, non il programma pronto.
2. Estrai **tutta la cartella** ZIP in una cartella a scelta.
3. Fai doppio clic su **HOI4 Companion.exe**. Non separare l'eseguibile dalla
   cartella `_internal` o da `Companion Runtime.exe`.
4. Si aprono il browser su <http://127.0.0.1:8766/walkthrough> e il controllo HUD.
   Il caricamento iniziale dei file del gioco può richiedere qualche minuto.
5. In **Configurazione**, verifica la cartella del gioco che contiene `hoi4.exe`
   e la cartella Documenti di HOI4, soprattutto se usi un profilo personalizzato.

Il pacchetto include Python, parser Rust/Jomini, host CWTools con runtime .NET,
HUD Rust, OpenCV, yt-dlp, Node.js e FFmpeg: **non devi installarli separatamente**.
Occorrono Windows x64, un browser e un'installazione propria di HOI4. Non sono
richiesti privilegi di amministratore. Il pacchetto non è firmato digitalmente;
Windows può mostrare una richiesta di verifica. Confronta il file `.sha256`
pubblicato insieme allo ZIP.

Se non viene trovata un'installazione valida, compare una **demo inventata**:
serve a provare l'interfaccia, non rappresenta un albero storico del gioco.

Per avviare solo l'interfaccia web:

```powershell
& '.\HOI4 Companion.exe' --no-hud
```

Il servizio rimane attivo chiudendo il browser. Il controllo HUD permette di
mettere in pausa o chiudere le evidenziazioni; il processo del servizio è
`Companion Runtime.exe`. Nessuna impostazione personale dello sviluppatore è
inclusa nella distribuzione.

## Come si usa una guida

1. Apri **Importa guida**, scegli la nazione/albero e scarica il prompt completo.
2. Fornisci alla tua IA il prompt, le fonti della guida e i cataloghi tematici
   utili. L'IA restituisce un file JSON; non deve accedere al tuo PC.
3. Carica il JSON nell'importatore. Il programma risolve nomi e ID usando il
   catalogo locale, controlla dipendenze/evidenze e mostra l'anteprima modificabile.
4. Correggi eventuali ambiguità reali e aggiungi la guida. Le associazioni già
   risolte sono raccolte nei dettagli; non richiedono conferme individuali.
5. Apri la guida nel **Walkthrough** e selezionala anche nel controllo HUD.
   Puoi aggiungerne altre: **Importa guida resta sempre disponibile**.

La guida della Polonia inclusa nel repository è un esempio facoltativo.
Il framework e il formato delle nuove guide sono indipendenti dalla nazione.
Un'API IA integrata è disponibile come opzione; **non serve una chiave API** per
usare un'IA esterna e importare il suo file.

### Cosa mostra ciascuna schermata

| Schermata | Funzione |
| --- | --- |
| Focus nazionali | Albero ricavato dai file installati, icone, collegamenti, descrizioni e prossimo focus previsto dalla guida |
| Tecnologie | Schede e disposizione ricavate dal gioco, livelli ricercati/attivi e prossime ricerche della guida |
| Walkthrough | Costruzioni, produzioni, politica, eventi e altre istruzioni operative, con eventuali fotogrammi della fonte |
| Stato nazione | Dati live esposti dal lettore: divisioni, manpower, territori, fabbriche, produzioni e materiali |
| Importa guida | Prompt, cataloghi, preparazione video, importazione e correzione delle associazioni |
| Analisi script | Diagnostica statica facoltativa con CWTools; non decide se uno step è stato eseguito |

### Come avanza il walkthrough

Il runtime segue le condizioni **dichiarate dalla guida**. Non inventa azioni,
quantità o completamenti quando manca un dato.

- **Pronto / in attesa:** dipendenze e condizioni determinano quando agire.
- **In corso:** l'avvio è osservato, ma l'obiettivo finale non è ancora raggiunto.
- **Completato:** il criterio finale è effettivamente soddisfatto.
- **Non applicabile / non verificabile:** il ramo non si applica oppure mancano
  informazioni sufficienti; i due casi restano distinti.
- **Promemoria:** un consiglio sul metodo rimane consultabile senza richiedere
  una conferma di completamento.

Esempio: «aggiungi due fabbriche» con un conteggio iniziale di cinque diventa
in corso quando rileviamo l'ordine; si completa al raggiungimento di sette.
La scomparsa dalla coda, da sola, non dimostra il completamento: potrebbe essere
un annullamento. La baseline viene acquisita quando possibile prima dell'ordine.

Non tutte le azioni hanno un riscontro live disponibile. Un evento visibile
può dimostrare che è il momento della scelta, **non quale risposta hai premuto**.
Alcune istruzioni richiedono ancora conferma o rimangono promemoria.
Checklist e avanzamento sono separati per guida/campagna.

## HUD sopra HOI4

L'HUD disegna cornici trasparenti ai clic sui bersagli suggeriti dalla guida.
Focus, tecnologie, categorie ed eventi hanno percorsi di riconoscimento dedicati;
gli altri pannelli usano anche OCR/sprite locali. La copertura varia: non è
garantita un'evidenziazione per ogni tipo di istruzione.

Le indicazioni richiedono una partita compatibile con la guida, dati validi e
HOI4 in primo piano. Il rilevamento visivo non viene usato come prova automatica
di completamento. [Dettagli e limiti HUD](docs/native-overlay.md).

## Compatibilità: file locali e lettura live

**Cataloghi, importazione guide e interfaccia non sono vincolati all'hash di una
specifica build del gioco.** Leggono gli script installati con Rust/Jomini.
Questo non garantisce che qualsiasi futura sintassi o mod sia già supportata.

**La lettura di memoria live è invece sperimentale e vincolata alla build
Windows verificata**: `Operation Postern v1.19.3.0.c01a (42c3)`, SHA-256
`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`.
Una build sconosciuta viene rifiutata dal lettore, perché gli indirizzi e le
strutture interne possono cambiare. Il pacchetto Windows non elimina questo limite.

Su una build supportata, il monitoraggio legge la memoria anche a gioco in pausa:
**non dipende dai salvataggi**. Il browser riceve aggiornamenti SSE quando cambia
lo stato. Dati incoerenti, menu principale o gioco chiuso invalidano la lettura;
il programma non continua a presentare vecchi dati come attuali.

Gli snapshot JSON e i salvataggi testuali `.hoi4` costituiscono una modalità
alternativa e parziale, principalmente per i focus; non equivalgono al monitoraggio
live. I salvataggi binari/Ironman non sono supportati da questa modalità.
Non è dichiarata una certificazione per multiplayer o sistemi anticheat.

## DLC, mod e lingua

- Il programma legge contenuti installati e configurazione del launcher,
  applicando i filtri statici che riesce a verificare. **Non verifica la licenza
  Steam** e non simula tutte le condizioni dinamiche del gioco.
- Le mod in cartelle sono sovrapposte nell'ordine rilevato, con `replace_path`.
  Mod archiviate, percorsi mancanti e casi ambigui vengono segnalati.
- Dopo aver cambiato playset o DLC, avvia HOI4 e premi **Ricarica** nel Companion.
  La configurazione del launcher non prova da sola quali mod siano già nel processo.
- Il riferimento dei nomi è l'inglese; le diciture visualizzate usano le
  localizzazioni installate, comprese quelle delle mod supportate.
- Città, province e regioni vengono collegate localmente quando i dati lo
  consentono. Un nome ambiguo non viene associato al primo risultato.

## Video da fornire all'IA

In **Importa guida** puoi preparare un video YouTube: fino a 1080p, 15 FPS,
H.265, segmenti da **5 minuti con 10 secondi di sovrapposizione**, ciascuno entro
**150.000.000 byte**. Se necessario i segmenti vengono accorciati. Sono disponibili
anche audio separato, descrizione e manifest degli intervalli originali.

L'accelerazione hardware viene usata dopo una prova reale; in caso di errore
si usa la CPU. La preparazione del video **non analizza la strategia e non chiama
un'IA**. Accessibilità del video e restrizioni di YouTube possono impedirne il
download; non vengono usati login, cookie o aggiramenti dei blocchi.
[Workflow completo](docs/guide-video-workflow.md).

## Dati personali, aggiornamenti e disinstallazione

La distribuzione Windows salva impostazioni, guide importate, avanzamento,
associazioni, log e cache in **`%LOCALAPPDATA%\HOI4Companion`**.
Il sorgente continua a usare **`.local/`** nella cartella del repository.
I pacchetti non contengono la cartella dati dello sviluppatore.

Per aggiornare, chiudi HUD/servizio ed estrai la nuova distribuzione in una
cartella nuova: i dati nel profilo Windows rimangono disponibili. Per un backup
copia la cartella dati a programma chiuso. Non è prevista un'importazione
automatica dei dati di un precedente checkout sorgente.

Per disinstallare il pacchetto ZIP elimina la cartella estratta. La cartella dati
rimane finché non la elimini esplicitamente; può contenere video di grandi dimensioni.
Il servizio ascolta soltanto su `127.0.0.1`. Le funzioni YouTube/web e l'API IA
opzionale contattano i rispettivi servizi quando vengono richieste.

## Sviluppo e compilazione

Per lavorare dal sorgente servono Python 3.11+, Rust con linker MSVC su Windows
e .NET SDK 9 (versione fissata in `native/cwtools/global.json`).

```powershell
py -3 -m pip install -e '.[overlay]'
```

Dal checkout, `AVVIA COMPANION E HUD.bat` avvia servizio e HUD;
`avvia.bat` avvia il servizio. Non sono i launcher della distribuzione ZIP.

Per costruire il pacchetto Windows x64 con Python 3.13:

```powershell
py -3.13 -m venv .venv-build
& .venv-build\Scripts\python.exe -m pip install -r packaging\requirements-windows.lock
& .venv-build\Scripts\python.exe scripts\build-windows.py
& .venv-build\Scripts\python.exe scripts\smoke-windows.py dist\HOI4Companion
```

Il workflow GitHub **Windows distribution** costruisce e verifica il pacchetto.
Su tag `v*` prepara una **bozza di release**, che va controllata e pubblicata
per rendere lo ZIP scaricabile pubblicamente. [Procedura](docs/windows-distribution.md).

Test backend e frontend:

```powershell
$env:PYTHONPATH = "$PWD\src"
py -3 -m unittest discover -s tests -p 'test_*.py'
node --test tests/*.cjs
```

## Approfondimenti

- [Formato semplice da fornire all'IA](docs/guide-authoring.md)
- [Importatore](docs/guide-importer.md) e [runtime delle guide](docs/guide-runtime.md)
- [Persistenza delle campagne](docs/campaign-progress.md)
- [Lettura live](docs/live-state.md) e [stato della nazione](docs/country-live.md)
- [Parser Jomini](docs/jomini-migration.md) e [analisi CWTools](docs/script-analysis.md)

Il codice del progetto è distribuito con [licenza MIT](LICENSE). I componenti
di terze parti conservano le proprie licenze, incluse nel pacchetto Windows.
I file e gli asset di Hearts of Iron IV rimangono di proprietà dei rispettivi titolari.
