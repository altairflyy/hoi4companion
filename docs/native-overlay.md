# Overlay Windows nativo (sperimentale)

`avvia-overlay.vbs` avvia un processo Rust esterno che evidenzia le indicazioni
ammesse dal Guide Runtime. Focus, tecnologie, capitoli ed eventi usano un unico lettore
GUI nativo e il renderer Direct2D/DirectWrite, senza cattura o OCR nel loro
percorso attivo. Il riconoscimento visivo degli altri pannelli usa WGC e WinRT;
non usa Tkinter, ImageGrab, PNG/Base64 o PowerShell.
Non invia comandi al gioco e non modifica file o memoria di HOI4.

## Motore comune per focus, tecnologie e capitoli

Per la build HOI4 qualificata dal lettore live, focus e tecnologie sono agganciati
all'ID del widget del gioco e agli sprite effettivamente disegnati (simbolo e
sfondo). La matrice del contenitore corrente e le posizioni locali native dei
figli forniscono posizione e scala, senza dipendere dalla matrice di rendering
potenzialmente vecchia dello sprite: spostare l'albero
o ridurre lo zoom non richiede di rileggere il nome con OCR. La lingua non entra
nella selezione dell'ID. Il modulo `focus_ui_native.py` usa un handle con soli
permessi di interrogazione e lettura; non chiama funzioni del gioco. Focus,
tecnologie e capitoli riusano lo stesso calcolo degli sprite ancorati al parent
corrente, lo stesso contratto di geometria e lo stesso ciclo di disegno.

Le ricerche in corso possono utilizzare `CFrameAnimatedSprite` e
`CFrameAnimatedSpriteType`, oltre alle varianti statiche. Il lettore ammette
queste due classi soltanto se la RTTI del PE qualificato dimostra rispettivamente
una base non virtuale `CSprite` o `CSpriteType` allo stesso indirizzo. Non accetta
genericamente qualsiasi sottoclasse. Gli stessi controlli su parent, proprietario,
identità, dimensioni, visibilità e ritaglio continuano a valere. Il caso reale è
stato riprodotto su sfondi animati delle ricerche: il vecchio controllo sul solo
tipo statico respingeva il campione prima del disegno.

L'albero tecnologico è `CCountryTechTreeView`, distinto dal pannello laterale
`CCountryTechnologyView`. La cornice segue il vero pulsante `Background`, non
l'illustrazione interna o le dimensioni del contenitore. L'ID della cartella è
verificato nella gerarchia nativa e confrontato con la scheda effettivamente
aperta. Per gli altri capitoli si evidenzia la scheda nativa; più ricerche nello
stesso capitolo producono una sola cornice con il relativo numero. Le varianti
DLC nascoste non sono ammesse. In una scheda aperta un nodo fuori dal ritaglio
non viene sostituito da un invito a premere nuovamente quella stessa scheda.

I template compatti, inclusi Armi di supporto e Logistica, chiamano il pulsante
di sfondo `bg` anziché `Background`. Entrambi i nomi vengono riconosciuti sulla
stessa classe `CButtonDrag`, mantenendo l'unicità, il proprietario e l'ancoraggio
qualificati. Il caso `bg` è coperto da un test di regressione, compreso il rifiuto
di un pulsante appartenente a un altro proprietario. Prima della correzione un
solo widget di questo tipo poteva sospendere l'intero campione tecnologico.

L'ammissione verifica processo, nazione corrente, albero, proprietario GUI,
visibilità, ritaglio e corrispondenza della risoluzione fisica. Le letture sono
ripetute per respingere campioni cambiati durante l'osservazione. I candidati
del registro GUI sono memorizzati, le coordinate sono sempre rilette. Una lettura
incoerente viene ritentata una sola volta con nuovi dati e gli stessi controlli;
un secondo errore sospende le cornici senza riutilizzare coordinate vecchie.
Il solo cambiamento della geometria non invalida i candidati del registro;
errori strutturali, di proprietà o di identità li invalidano. Build non
supportate mantengono il riconoscimento visivo esistente.

Gli aggiornamenti che rinnovano soltanto la validità di una cornice non
ridisegnano più la superficie DirectComposition. Lo stato diagnostico e l'elenco
delle guide non impongono più cancellazioni e nuove presentazioni. Il termine
di sicurezza di 750 ms e i controlli sulla finestra in primo piano restano attivi.

Il percorso comune osserva soltanto i widget richiesti e le schede, senza
trasferire pixel. La scoperta iniziale del registro è memorizzata; non viene
ripetuta a ogni fotogramma. Il ciclo è limitato a circa 16 ms per iterazione per
evitare una lettura continua senza attesa: non sono notifiche interne emesse dal
gioco. Le raccomandazioni continuano ad arrivare dal runtime tramite SSE.
Gli eventi usano la stessa geometria nativa quando la guida associa una risposta
e la griglia corrente la contiene: [contratti, prova reale e limiti](events-live.md).
Politica e produzione conservano il riconoscimento visivo precedente.

## Avvio

Avvia il Companion e poi `avvia-overlay.vbs`. Entrambi usano **8766** per
impostazione predefinita. Per un server su un'altra porta:
`wscript.exe avvia-overlay.vbs 8772`.

La finestra di controllo nativa consente di selezionare una guida installata,
ricaricare l'elenco, sospendere/riprendere e chiudere l'overlay. Una guida ancora
in anteprima nell'importer deve essere installata prima. Il controllo si apre
senza sottrarre il focus; puoi raggiungerlo dalla barra applicazioni o Alt-Tab.
Le cornici trasparenti ai clic compaiono solo quando HOI4 è in primo piano.

Il testo del controllo distingue ora i motivi per cui un bersaglio non compare:
categoria diversa (con nome), pannello chiuso, ritaglio fuori schermo, widget
nascosto, non individuato o ambiguo, identità diversa ed errore tecnico. Un popup
evento indica esplicitamente la sospensione degli alberi sottostanti. Le stesse
spiegazioni, con un controllo di freschezza e identità del servizio, sono esposte
nel Walkthrough attraverso gli eventi esistenti. Il launcher passa un percorso
assoluto di `overlay-status.json` nella cartella dati, anche nella distribuzione
Windows. Queste spiegazioni non modificano i criteri di ammissione delle cornici.

### Guida condivisa e ricerche in corso

Il browser e l'HUD condividono la selezione attraverso `/api/guide/active`.
Cambiare guida in una pagina visibile o nel controllo HUD aggiorna la stessa
selezione; le altre viste seguono il cambiamento tramite il flusso SSE esistente.
La preferenza è conservata in `active-guide.json` nella cartella dati del
programma, separata da definizioni delle guide e avanzamento della partita.
L'HUD non sceglie più automaticamente la prima guida installata. Le revisioni
respingono aggiornamenti precedenti e un cambio di guida invalida le vecchie
cornici. Una pagina nascosta non pubblica la guida di un vecchio URL al caricamento.
Con un servizio precedente ancora in esecuzione, il browser mantiene la propria
selezione locale fino all'aggiornamento del servizio.

Le tecnologie e i focus già in corso rimangono evidenziati in azzurro anche
quando non ci sono slot liberi. Le nuove tecnologie proposte restano limitate
agli slot liberi ammessi dal runtime. Non vengono aggiunte ricerche future
solo perché uno slot è occupato. Le schede tecnologiche distinguono un gruppo
di sole ricerche attive da nuove scelte disponibili. Restano invariati i controlli
su nazione, ID, DLC, cartella, visibilità e validità del campione.

### Indicatore di cattura di Windows

Focus, tecnologie e capitoli qualificati continuano a utilizzare la geometria
nativa senza cattura dello schermo. Per gli altri pannelli, il motore WGC
richiede una volta il modo `GraphicsCaptureAccessKind.Borderless` e imposta
`IsBorderRequired=false` prima di avviare la cattura. Non aggiunge cornici
intorno al desktop nel renderer. Lo stato diagnostico include guida attiva,
uso della cattura e autorizzazione Windows alla modalità senza bordo.

L'assenza del bordo deve essere verificata sulla macchina reale: Windows può
ignorare la proprietà se l'accesso non è concesso o mostrare un indicatore
richiesto da un'altra applicazione. Si veda la
[documentazione Microsoft](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired).

Il controllo diagnostico `--capture-access-check --status-file <file>` verifica
l'autorizzazione senza avviare una cattura o un secondo HUD. Nella verifica locale
Windows ha restituito accesso concesso; la cattura WGC effettiva ha riportato
`capture_borderless_allowed=true` e lo screenshot successivo non mostrava il bordo
giallo. Il supporto ai widget animati è stato verificato anche sui widget reali
conservati dal gioco a scheda chiusa: questo verifica la decodifica, non la loro
visibilità o l'allineamento delle cornici dopo l'ultima correzione.

Avvio equivalente:

```powershell
$env:PYTHONPATH = "$PWD\src"
py -3 -m hoi4companion.overlay_native --port 8766 --start
```

L'entrypoint storico `python -m hoi4companion.overlay_windows` avvia ora lo stesso
backend nativo. Non esiste un ripiego automatico su quello vecchio. Il vecchio
codice e l'OCR PowerShell restano disponibili per confronti diagnostici, senza
essere avviati dal launcher normale.

Il wheel Windows x64 include `_native/overlay/hoi4-overlay.exe`. Servono Windows
con Python e Pillow; Windows Graphics Capture, OCR nelle lingue del profilo e
OpenCV sono utilizzati dal percorso visivo degli altri pannelli o delle build
HOI4 prive di un profilo nativo supportato. L'extra `overlay` aggiunge OpenCV:

```powershell
py -3 -m pip install "hoi4companion[overlay]"
```

Dal checkout, `scripts/build-overlay.ps1` compila e installa il binario con Rust
MSVC. Il build del pacchetto richiama `scripts/build-overlay.py` automaticamente.
Gli utenti del wheel non devono installare Rust, .NET SDK o compilatori.
`HOI4_OVERLAY_NATIVE` consente di selezionare un binario di sviluppo esplicito.
Per elencare le guide: `python -m hoi4companion.overlay_native --list-guides`.

## Architettura e limiti preservati

- Il percorso nativo comune precede la cattura: WGC e OCR sono inizializzati
  solo quando serve il riconoscimento visivo. Le cornici identiche rinnovano la
  validità senza una nuova presentazione della superficie.
- Il percorso visivo usa Windows Graphics Capture `FrameArrived` su un HWND verificato come
  appartenente a `hoi4.exe`. Conserva soltanto il frame più recente, con coda
  limitata e lettura CPU su richiesta. Verifica il ritaglio client con i bordi
  DWM; cambio finestra, ridimensionamento e coordinate incoerenti invalidano le
  evidenze. Non installa hook e non si inserisce nel processo del gioco.
- Il preprocessing Rust riproduce grayscale, contrasto 1.4 e bicubica Pillow
  12.3.0 a 2x, per tasselli fino a 1280×720. OCR Windows riceve direttamente BGRA
  tramite SoftwareBitmap. Attribuzione e licenza Pillow sono incluse.
- Le raccomandazioni SSE, la proiezione del runtime e il riconoscimento OpenCV
  qualificato, insieme alla lettura GUI, restano in un worker Python. L'overlay
  non è quindi una rimozione di Python dall'intero programma. L'IPC versionata
  trasporta ID e rettangoli nel percorso comune, BGRA grezzo soltanto quando
  serve il matching visivo: nessuna ricodifica PNG/Base64. Le immagini non sono
  salvate durante l'uso normale.
- DirectComposition, Direct2D e DirectWrite disegnano contorni arrotondati ambra
  o ciano, etichette leggibili e sfondo traslucido. Il thread UI controlla
  indipendentemente PID, foreground, geometria, minimizzazione e scadenza, anche
  durante un riconoscimento lento. Le evidenziazioni scadono entro 750 ms senza
  nuova evidenza; il controllo di sicurezza del renderer avviene ogni 50 ms.
- Nel percorso visivo le scene identiche riusano il riconoscimento; ogni nuova cattura verifica
  l'intera area sotto la barra delle risorse e tutte le patch di supporto. La
  fascia superiore di 82 pixel è esclusa dal confronto globale perché le misure
  WGC locali hanno individuato animazioni fino alla riga 81; eventuali patch
  ammesse in quella fascia vengono comunque ricontrollate. Un duplicato che
  compare altrove invalida la cache e richiede una nuova ricerca globale.
  Altre scale/DPI possono ridurre i riusi, senza rendere ammesse corrispondenze
  ambigue. Dopo il riconoscimento si confrontano ancora bersagli e supporti con
  un frame fresco prima del disegno.
- Guida, nazione del runtime e nazione osservata devono corrispondere. Errori,
  identità mancante e dati live inutilizzabili sospendono le cornici. La scadenza
  del collegamento al runtime rimane 12 secondi; le cornici hanno anche la
  scadenza visiva indipendente più breve descritta sopra.

## Indicazioni e riconoscimento

Le ricerche proposte rispettano gli slot liberi osservati e l'ordine del runtime;
non si riempiono posti con azioni bloccate o sconosciute. Il motore comune non
sceglie altre tecnologie: proietta gli ID del runtime sul nodo visibile oppure
sul capitolo nativo corrispondente, se è aperto un altro capitolo.

Sulle build non supportate, i focus conservano il riscontro testuale esatto e
univoco e le ricerche mantengono il matching visivo precedente. Politica,
personaggi e produzioni usano ancora quel riconoscimento, localizzato o inglese.
Nel percorso visivo le tecnologie senza
nome richiedono il bersaglio univoco, intestazione e due sprite distinti nel
layout; immagini riciclate, occlusioni e barre duplicate non vengono accettate.
La ricerca dei peer delle schede è ora eseguita solo quando necessaria, mantenendo
soglie, ricerca globale dei bersagli e controlli di ambiguità originali.

Per gli eventi nativi restano necessari una finestra qualificata e l'ID esatto
della risposta prevista dalla guida nella griglia corrente. Il percorso visivo
delle build non supportate richiede ancora la finestra osservata dal lettore e
il riscontro del titolo e dell'opzione. L'overlay non provoca eventi e non sceglie
risposte. Nella produzione evidenzia la linea
qualificata con il totale di fabbriche richiesto; non identifica il singolo
pulsante di assegnazione di una fabbrica. Politica e personaggi si basano su ID
associati e nomi visibili univoci; non equivalgono a una mappatura universale di
tutti i widget interni del gioco. Nessun riconoscimento visivo completa uno step.

## Verifiche e prestazioni, 24 ottobre 2026

### Sfarfallio residuo delle schede

Riprodotto in background sul tab nativo `nsb_armour_folder_tab`: il byte di stato
a `CButtonStandard+0x75` alternava `0x48` e `0xE8`, mantenendo sempre il bit di
visibilità `0x08`. Il confronto dell'intero byte respingeva campioni validi e
provocava una breve scomparsa delle cornici. `GuiSample` verifica ora separatamente
quel bit; gli altri campi, ID, proprietari e geometria mantengono il confronto
completo. Un cambiamento del bit di visibilità continua a invalidare il campione.
Il lettore dello stato live e il termine di sicurezza del renderer non cambiano.

Due acquisizioni da 1.001 letture ciascuna, con HOI4 in background e stessa
geometria: prima 51 errori residui e 263 letture riuscite al secondo tentativo;
dopo zero errori e zero secondi tentativi, mediana 5,09 ms. Superati 507 test
Python, inclusa la riproduzione dei due valori del byte e il rifiuto di una vera
variazione della visibilità. Il foreground è rimasto invariato anche al riavvio
dell'overlay. Questa è una verifica dei dati e delle invalidazioni, non una nuova
osservazione visiva prolungata sopra il gioco in primo piano.

Aggancio GUI dei focus: ID e contorno di `GER_oppose_hitler_ww` confrontati con
lo screenshot reale a 2560×1440; rettangolo `[1645,166,1809,278]`. Verificati anche
pannelli aperti/chiusi. Dieci letture successive reali dopo la correzione della
scoperta iniziale sono riuscite: circa 410 ms al primo accesso, poi 0,38–0,61 ms
con pannello chiuso. Sono tempi della sola lettura GUI, non latenza complessiva.
Pan e zoom sono coperti da fixture delle matrici native; una prova visiva
continuativa delle cornici durante il movimento resta da completare. I test del
renderer verificano che il rinnovo della validità non provochi una nuova
presentazione della medesima cornice.

Una cattura composta durante il pan ha mostrato una cornice fuori posizione;
non viene considerata una verifica riuscita. La versione finale compone il
parent corrente con le coordinate locali risolte dal gioco e ha fixture anche
con matrice di rendering deliberatamente obsoleta. La diagnostica `--proof`
attende geometria stabile e richiede letture identiche prima e dopo la cattura;
il file JSON associato contiene `verified`, senza trasformare una cattura durante
il movimento in una prova positiva. L'ultima verifica completa eseguita include
505 test Python, 243 JavaScript e 10 Rust, tutti superati.

Per le tecnologie sono state eseguite 100 letture consecutive del provider con
la raccomandazione reale `basic_medium_tank_chassis`: nessun errore, mediana
3,13 ms e 95° percentile 4,22 ms dopo l'apertura iniziale (~530 ms). Un campione
più ampio, con quattro tecnologie e tutte le schede, ha richiesto 4,8–5,7 ms.
Con il codice finale (pulsante di sfondo e retry coerente), altre 100 letture su
quattro tecnologie e tutte le schede sono riuscite senza errori: mediana 5,59 ms,
95° percentile 6,78 ms, massimo 9,08 ms.
Questi sono tempi della lettura GUI, non latenza complessiva né misura degli FPS.
La composizione reale ha permesso di rilevare la differenza fra icona e pulsante;
il rettangolo finale dello sfondo, confrontato con lo screenshot, è
`[845,528,1028,612]`. Il test del passaggio effettivo fra capitoli e una lunga
osservazione del lampeggio restano distinti dai test automatici.

Le misure seguenti riguardano il precedente percorso OCR, tuttora usato per gli
altri pannelli:

Con gli stessi pixel di ingresso già decodificati, warm-up escluso e tre misure:

| OCR e preprocessing | Percorso precedente | Nativo Rust |
| --- | ---: | ---: |
| Scheda ingegneria | 608 ms | 261 ms |
| Focus nazionali | 1.082 ms | 355 ms |

Testi, parole e coordinate sono identici in tutti i confronti. Otto tasselli
preparati corrispondono byte per byte al percorso Pillow. Queste misure escludono
cattura, matching e disegno: non sono la latenza totale né un dato sugli FPS.
Il matching delle schede, misurato separatamente alternando otto coppie, scende
da 250 a 138 ms sul caso ingegneria; stessi risultati ed evidenze.

Verificata la composizione reale sopra HOI4 DirectX 11 a 2560×1440: ingegneria
elettro-meccanica e navigazione verso artiglieria e industria. Il passaggio reale
dei clic è stato verificato su una finestra sintetica sotto il renderer, senza
impartire ordini nella partita. Verificato inoltre il blocco della guida polacca
su una partita tedesca. Non sono validati tutti i pannelli, i DPI, i monitor,
le modalità video o il comportamento di ogni mod.

Gli artefatti locali `.local/overlay-status.json` e i log descrivono frame,
riconoscimento, errori e contatori della cache. `--proof PERCORSO.png` cattura una
sola immagine realmente composta quando sono presenti evidenziazioni valide;
non genera cornici sintetiche per simulare una prova. Un file di prova va
richiesto esplicitamente e può contenere informazioni visibili della partita.

## Navigazione per azioni politiche e progetti

La proiezione usa action_sequence del runtime e l'identità della guida attiva.
Lo stesso host Rust/Direct2D e lo stesso decoder geometrico osservano controlli
opzionali con contesto access/slot/facility/entity. Il solo OCR di un nome non
ammette più leggi o consiglieri: la scelta richiede token nativo, categoria/ruolo
e rettangolo qualificati. Gli accessi noti usano i nomi delle definizioni GUI
installate (player_flag, technology_button, facilities_tab), mai coordinate fisse.
Il decoder riusa CContainerWindow/CButtonStandard e i testimoni di visibilità,
appartenenza al root GUI, ancoraggio e clipping del lettore comune.

Una finestra evento/albero in primo piano mantiene precedenza. Viste duplicate,
modelli anonimi, categorie sconosciute e specializzazioni non osservate sospendono
la selezione. I progetti richiedono una specializzazione compatibile osservata
per il controllo dell'impianto/progetto; il decoder attuale non qualifica ancora
il payload della specializzazione di un impianto in stato inattivo. Quella
selezione resta pertanto sospesa, pur essendo disponibili progetto, avvio e
completamento nel live di campagna. I comandanti d'unità sono risolti nel
catalogo, ma assegnazione e GUI del roster non sono ancora qualificate.

Questi percorsi sono verificati con fixture; allineamento di nuove schermate
politiche e impianti non verificato su una partita reale in questa estensione.
La disponibilità di un adapter o una compilazione riuscita non sono prova visiva.
