# Video e istruzioni operative

Nella pagina **Importa guida**, accanto al prompt per l'IA esterna, il link
YouTube avvia una preparazione asincrona. Non chiama un'IA e non installa guide.
`yt-dlp` e `imageio-ffmpeg` sono dipendenze del pacchetto; sono supportati anche
gli eseguibili configurati con `HOI4_YTDLP` e `HOI4_FFMPEG`.

Il programma scarica il miglior sorgente fino a 1080p, preferendo l'audio
inglese quando disponibile, e conserva la descrizione. Non usa login, cookie,
configurazioni o plugin esterni. Le limitazioni di accesso di YouTube vengono
segnalate. Un sorgente già presente nella cache viene riutilizzato.

Il preset conserva la risoluzione fino a 1080p senza ingrandire un 720p:
15 FPS, H.265, AAC mono 64 kbps. Prima di usare NVENC, QSV o AMF viene eseguita
una piccola codifica reale. Il preset v4 confronta poi l'encoder hardware
disponibile e x265 su due campioni di 10 secondi al 20% e 60% del sorgente.
Preferisce l'hardware se il risultato è entro il 25% dalla dimensione minima;
altrimenti usa il candidato più piccolo. Questo criterio bilancia velocità e
dimensione, non certifica equivalenza percettiva. L'encoder CPU è x265 medium CRF 22; i parametri
hardware di qualità non sono numericamente equivalenti a CRF e possono produrre
file più grandi. Un errore hardware o tagli non allineati comportano un nuovo
tentativo con x265. La presenza dell'encoder in una lista non basta a provarne
l'utilizzabilità.

Ogni parte viene verificata contro **150.000.000 byte**, senza contare su una
stima di bitrate. Il preset v4 produce segmenti da **5 minuti**, con l'ultima
parte più breve; se necessario li accorcia per rispettare il limite di dimensione.
Punti IDR ogni 10 secondi consentono il taglio senza ricodificare le parti;
anche i tagli adattivi vengono allineati a questa griglia. NVENC usa tre B-frame.
Un controllo
della durata rifiuta tagli che partirebbero da un keyframe precedente. Le parti
condividono **10 secondi** sulla timeline originale, con la normale granularità di
timestamp audio/video. L'ultima parte può essere più piccola. Il manifest riporta
gli intervalli originali: l'IA deve aggiungere `start_seconds` al minuto locale
e deduplicare gli step nella sovrapposizione. Il prompt scaricabile lo richiede.

I risultati sono in `.local/guide-videos/<job-id>/`; una preparazione completata
con lo stesso preset viene riaperta senza un altro download o una nuova codifica.
SHA-256 e dimensioni verificano i file. Il master è salvato una sola volta in
`_masters/`, con chiave basata sul contenuto del sorgente e sul profilo di
codifica, indipendente da durata, sovrapposizione e limite delle parti. Cambiare
soltanto i tagli riutilizza quel master. Un master completato sopravvive a un
errore di suddivisione; una codifica interrotta a metà viene invece rifatta,
senza tentare di riaprire un MP4 incompleto.
La cache mantiene il sorgente registrato per i fotogrammi; non è inclusa nel repository. Il server
espone soltanto i file finali dichiarati dal job. Gli endpoint di avvio sono
protetti come le altre scritture locali contro richieste cross-origin. La chiusura
normale del programma termina tutti i processi posseduti, inclusi probe,
calibrazione, lettura metadati e taglio. Su Windows viene terminato anche
l'albero del processo yt-dlp per includere l'eventuale merger.

Dopo il successo vengono eliminati i file generati delle preparazioni
precedenti dello stesso URL e i master superati dello stesso video. La pulizia
controlla directory gestite, nomi generati e sorgenti ancora registrati;
file estranei e sorgenti protetti restano. I video di altri URL non sono rimossi.
Le cartelle che conservano solo un sorgente mantengono un piccolo `job.json`
per essere riconosciute in una futura pulizia. I file di calibrazione e i
master parziali vengono rimossi al termine della lavorazione.

La pagina mostra la risoluzione reale del sorgente. Per cercare una versione
più alta si può selezionare **Riscarica il miglior sorgente fino a 1080p**:
`refresh_source:true` ignora la cache di download, senza promettere che il
video disponga effettivamente di 1080p. Il valore predefinito conserva il riuso
locale ed evita traffico superfluo. I retry azzerano il vecchio avanzamento.

## Traccia audio separata

La preparazione include anche la traccia audio completa del sorgente, con
download distinto dalle parti video. AAC viene copiato in `audio.m4a`, Opus
in `audio.opus` e MP3 in `audio.mp3`, senza ricodifica. Gli altri codec vengono
convertiti in AAC 128 kbps stereo per un file M4A. Il manifest include `audio`
con nome, dimensione, SHA-256, codec, indicatore `transcoded` e URL locale.
Il file conserva la timeline del sorgente completo, senza ripetere le
sovrapposizioni delle parti video. L'assenza di una traccia audio o un errore
di estrazione viene segnalato e non invalida le parti video già preparate.

Una preparazione precedente può ricevere il solo audio premendo nuovamente
Prepara sullo stesso link: l'estrazione avviene in background, senza encoder
video o nuovo download. `audio_status` permette al client di seguire questa
fase anche mentre il video è già `completed`. I formati audio serviti sono
limitati ai tre nomi dichiarati e rientrano nella pulizia dei job superati.

Verifica reale sul video Germania: AAC originale copiato senza ricodifica,
38.755.868 byte, durata 2.396,35 secondi, decodifica completa riuscita.

Il preset v3 invalida il riuso delle parti v2, che avevano durate determinate
dalla dimensione e 15 secondi di sovrapposizione. I sorgenti locali restano
riutilizzabili. Il manifest v3 dichiara `segment_seconds: 300` e
`overlap_seconds: 10`; il prompt usa gli intervalli effettivi del manifest.

Verifica del cambio v3: 535 test Python e 245 JavaScript superati. Una prova
FFmpeg su un video sintetico di 620 secondi ha prodotto gli intervalli nominali
0–300, 290–590 e 580–620, tutti decodificati integralmente senza errori. Le
durate dei contenitori sono 300,15, 300,28 e 40,14 secondi per la granularità
audio/video. Nessun nuovo download YouTube è stato eseguito per questa verifica.
Il servizio già aperto richiede un riavvio per caricare il nuovo modulo Python.

## Verifica e pulizia preset v4

Ricodificato integralmente il sorgente locale AV1 1280×720, senza nuovo download.
NVENC: campioni 5.956.093 byte in 2,56 s; CPU x265: 5.502.006 byte in 5,23 s.
La regola ha scelto NVENC. Master da 589.616.239 byte, contro i precedenti
942.515.595 byte (circa il 37,4% in meno). Il sorgente AV1 resta più piccolo:
il requisito H.265/qualità 22 non garantisce di migliorare ogni sorgente già
compresso. Non sono state dichiarate equivalenza visiva o accuratezza dell'IA.

Prodotte 9 parti; la maggiore pesa 90.244.870 byte. Tutte le parti audio/video
sono state decodificate integralmente senza errori. Verificati sovrapposizione
nominale di 10 secondi, durate nominali massime di 300 secondi e tetto di
150.000.000 byte. Prova separata con tagli da 4 minuti: stesso master riusato,
11 parti in 2,75 s, senza invocare encoder o calibrazione; output di prova rimossi.
Riapertura della preparazione completa via HTTP in circa 0,61 s senza ricodifica.

Rimossi automaticamente 5.509.831.850 byte di preparazioni obsolete. Il master
interrotto e i residui delle immagini (328.945.128 byte) sono stati trasferiti
al Cestino dopo che la revisione automatica aveva rifiutato l'eliminazione
definitiva; non sono conteggiati come spazio disco liberato finché il Cestino
non viene svuotato. Sorgente registrato, guide e avanzamento preservati.
Artefatti di verifica in `.local/video-preparation-audit/`.

## Fotogrammi, date e produzione

`action.params.source_seconds` identifica il fotogramma più utile. In mancanza,
la vista usa un'evidenza visiva temporizzata, poi una citazione temporizzata.
**Mostra fotogramma della guida** estrae l'immagine dal sorgente locale e la
mantiene aperta durante gli aggiornamenti live. Non è una cattura della partita
corrente né un riconoscimento automatico degli ordini militari. Una fonte senza
minutaggio o con video ambiguo resta priva di immagine: non si sceglie un frame
arbitrario. Per le guide future il prompt richiede immagini temporizzate per
istruzioni militari/produzione non riproducibili chiaramente dal solo testo.

`action.params.game_date`, per esempio `1936.1.1`, è la data letta nel video.
Il calendario è validato in Python e JavaScript. La data ordina le istruzioni
nella vista e viene mostrata come riferimento; non completa step, non impone una
scadenza e non modifica le dipendenze. Le date mancanti non vengono inventate.
Le istruzioni datate precedono quelle senza data, poi si usa il minuto fonte.

Per la produzione `requested_factories` indica il totale impostato, incluse le
fabbriche non disponibili. `assigned_factories` mantiene il precedente significato
di fabbriche effettivamente assegnate. I due campi si escludono. Un contatore
15/150 indica 150 richieste: non si devono leggere i consumi delle risorse nella
colonna destra come quantità di fabbriche. `production_targets` raggruppa più
prodotti nella stessa schermata; il completamento richiede tutti i totali, non la
sola somma nazionale. Per incrementi relativi usare istruzioni singole con baseline.
Un catalogo esteso legge anche gli ID espliciti in `duplicate_archetypes` e
`variant_name`, come i telai CAS distinti dai caccia. Non inventa modelli dai nomi.

Le costruzioni entrano in **In corso** alla presenza in coda, anche senza fabbriche
attive. Annullare la coda non dimostra il completamento: si controlla il totale
costruito. Se il conteggio precedente non è stato osservato e non è documentato
nella guida, rimane sconosciuto. La guida locale Kaiserin è stata corretta usando
il frame 01:58 (fabbriche militari 6/5/3) e i livelli iniziali verificati nei file
di storia effettivi dello scenario 1936 (Hessen 1, Westfalen 3, Hannover 2). Questi
ultimi sono dati derivati dallo scenario, distinti dai conteggi osservati live.
Il file originale ricevuto dall'IA resta invariato.

## Verifiche del 24 ottobre 2026

- Progetto Military Engineering Vehicles rilevato attivo sulla partita GER,
  coerente con il nome e l'impianto terrestre confermati dall'utente.
- Costruzioni iniziali riconosciute in corso; produzione iniziale confrontata per
  prodotto con il numero richiesto, compresi 150 fucili con meno fabbriche disponibili.
- Contratti completi Python/JavaScript confrontati sullo stesso campione reale.
- Simulazione yt-dlp sul link originale riuscita con selezione video/audio inglese;
  la preparazione completa usa il sorgente locale, senza riscaricarlo.
- Codifica completa con NVENC sul sorgente locale: otto parti, la maggiore
  148.580.440 byte. Verificata la decodifica completa di audio e video di tutte
  le parti e la sovrapposizione di 15 secondi sulla timeline nominale. Il contenitore
  aggiunge 0,20–0,42 secondi circa per timestamp/padding audio-video; non si dichiarano
  tagli audio precisi al campione. La prima prova con keyframe aperti è stata
  rifiutata e sostituita dal preset v2 con IDR regolari.
- Automazione browser locale disabilitata nelle impostazioni Work: controlli HTTP,
  rendering DOM con fixture e ispezione dei fotogrammi eseguiti; nessuna prova
  visiva interattiva del nuovo layout dichiarata.

Suite finali: **457 test Python e 243 JavaScript**, senza errori o test saltati.
Sul servizio di produzione 8766 verificati via HTTP: stesso run, conferme,
avvii, baseline, campagna, configurazione, DLC e mod dopo il riavvio; progetto
attivo e non completato, tre costruzioni attive e non completate, produzione
iniziale completata sulle richieste; immagini JPEG di produzione e difesa;
riapertura dello stesso video completato senza download/codifica aggiuntivi.
Nessuna verifica reale del completamento di un progetto o di una costruzione
è stata provocata: questi passaggi restano coperti dalle fixture.

Riferimenti degli strumenti: [yt-dlp](https://github.com/yt-dlp/yt-dlp),
[NVENC/FFmpeg](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/ffmpeg-with-nvidia-gpu/).
