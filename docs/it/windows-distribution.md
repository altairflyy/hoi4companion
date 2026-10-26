# Distribuzione Windows x64

Il formato è una cartella autonoma compressa in ZIP. `HOI4 Companion.exe` è il
launcher senza console; `Companion Runtime.exe` è il worker con pipe standard,
avviato senza finestre aggiuntive dal launcher e dall'host HUD. I due eseguibili
condividono `_internal`. Non è un programma di installazione, non registra servizi
di sistema e non richiede modifiche al PATH o privilegi amministrativi.

Il bundle contiene il runtime Python e le dipendenze, parser Rust/Jomini,
host CWTools self-contained .NET, HUD Rust, OpenCV, yt-dlp/EJS, FFmpeg e Node.js.
Node è scaricato da nodejs.org durante la build e verificato contro lo SHA-256
ufficiale della versione fissata; non si copia un'installazione personale.
Il runtime JS incluso è abilitato esplicitamente per le richieste yt-dlp.
Non vengono scaricati runtime al primo avvio dell'applicazione.

## Dati e risorse

- Risorse immutabili: cartella `_internal` della distribuzione.
- Dati utente: `%LOCALAPPDATA%\HOI4Companion` (impostazioni, guide, SQLite, video,
  immagini e log). Il checkout sorgente conserva il percorso `.local/` preesistente.
- `HOI4COMPANION_DATA_DIR` permette un profilo separato, per esempio nei test.
- Componenti impacchettati secondo una lista esplicita: niente `.local`, impostazioni
  con percorsi personali, mod, salvataggi, credenziali o asset di HOI4.
- Manifest con versione, revisione Git, indicazione di checkout modificato e hash
  dei file distribuiti. File `.zip.sha256` esterno per verificare il download.
- Le dipendenze conservano licenze e avvisi in `THIRD-PARTY` e nelle cartelle native.
  La copia FFmpeg inclusa è GPLv3: consultare i relativi avvisi e riferimenti ai
  sorgenti/build. La licenza MIT del Companion non sostituisce quelle delle dipendenze.

## Build locale

Prerequisiti: Windows x64, Python 3.13, Rust/MSVC e .NET SDK fissato nel progetto.
Il maintainer deve disporre di accesso alla rete per pacchetti, restore e Node.

```powershell
py -3.13 -m venv .venv-build
& .venv-build\Scripts\python.exe -m pip install -r packaging\requirements-windows.lock
& .venv-build\Scripts\python.exe scripts\build-windows.py
& .venv-build\Scripts\python.exe scripts\smoke-windows.py dist\HOI4Companion --output dist\windows-smoke-results.json
```

I componenti nativi vengono compilati in `build/windows-native`, senza sovrascrivere
un HUD già in uso dal checkout. `--skip-native` è disponibile per lo sviluppo,
ma le release devono usare il percorso completo. La cartella di output viene
ricreata da PyInstaller: non inserirvi documenti personali.

Artefatti: `dist/HOI4Companion-<versione>-windows-x64.zip`, checksum e rapporto
smoke test. Il numero di versione proviene da `pyproject.toml`.
Lo smoke test accetta anche il percorso dello ZIP: in quel caso verifica anche
estrazione e avvio da una cartella temporanea con spazi, come fa il workflow CI.

## GitHub e pubblicazione

Il workflow `Windows distribution` può essere avviato manualmente dalla scheda
Actions dopo aver caricato questi file nel repository. Produce artefatti anche
senza pubblicare una release.

Un tag `v<versione>` con numero corrispondente al progetto avvia build, test e
creazione di una **bozza di release**. Il maintainer controlla gli allegati,
aggiunge le note pertinenti e pubblica la bozza dalla sezione Releases.
Creare il pacchetto locale non lo rende automaticamente disponibile su GitHub.
La modifica o sostituzione degli allegati di una release già pubblicata non è
prevista dal workflow: usare un nuovo numero di versione.

## Verifiche e limiti

`smoke-windows.py` esegue il programma impacchettato con un profilo temporaneo e
un PATH privo di Python, Node, FFmpeg e .NET. Verifica parser, invocazione reale
CWTools, codifica HEVC CPU, importazione OpenCV, worker yt-dlp, pagine/API locali,
protocollo binario del worker HUD e riuso del servizio da parte del launcher GUI.
I processi di prova sono separati dal servizio dell'utente e vengono chiusi.

Questo controllo non equivale a una prova in una VM Windows appena installata,
a una validazione visiva dell'HUD durante una partita o a una prova di download
YouTube. OCR Windows richiede un componente/lingua OCR presente nel sistema.
Il pacchetto non è firmato digitalmente; SmartScreen può mostrare avvisi.
Il vincolo del lettore live alla build HOI4 verificata rimane invariato.

Riferimenti di implementazione: [PyInstaller runtime](https://pyinstaller.org/en/stable/runtime-information.html),
[pipe e console Windows](https://pyinstaller.org/en/stable/common-issues-and-pitfalls.html),
[runtime JavaScript yt-dlp](https://github.com/yt-dlp/yt-dlp/wiki/EJS).
