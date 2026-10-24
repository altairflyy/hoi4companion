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
una piccola codifica reale. L'encoder CPU è x265 medium CRF 22; i parametri
hardware di qualità non sono numericamente equivalenti a CRF e possono produrre
file più grandi. Un errore hardware o tagli non allineati comportano un nuovo
tentativo con x265. La presenza dell'encoder in una lista non basta a provarne
l'utilizzabilità.

Ogni parte viene verificata contro **150.000.000 byte**, senza contare su una
stima di bitrate. Le durate dei segmenti si adattano alla dimensione effettiva.
Punti IDR regolari consentono il taglio senza ricodificare le parti. Un controllo
della durata rifiuta tagli che partirebbero da un keyframe precedente. Le parti
condividono 15 secondi sulla timeline originale, con la normale granularità di
timestamp audio/video. L'ultima parte può essere più piccola. Il manifest riporta
gli intervalli originali: l'IA deve aggiungere `start_seconds` al minuto locale
e deduplicare gli step nella sovrapposizione. Il prompt scaricabile lo richiede.

I risultati sono in `.local/guide-videos/<job-id>/`; una preparazione completata
con lo stesso preset viene riaperta senza un altro download o una nuova codifica.
La cache mantiene sorgente e master; non è inclusa nel repository. Il server
espone soltanto i file finali dichiarati dal job. Gli endpoint di avvio sono
protetti come le altre scritture locali contro richieste cross-origin. La chiusura
normale del programma termina i processi di download/codifica di sua proprietà.

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
