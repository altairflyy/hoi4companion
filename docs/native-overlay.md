# Evidenziazioni sopra HOI4 (sperimentale)

L'overlay è una finestra Windows esterna, trasparente ai clic, che disegna cornici
sulle indicazioni attuali della guida selezionata. Non seleziona pulsanti, non
genera strategie e non modifica file o memoria del gioco.

## Avvio dal checkout locale

1. Avvia Companion e verifica che lo stato live sia valido per la partita.
2. Per riconoscere anche le tecnologie senza testo, installa l'extra facoltativo:
   `py -3 -m pip install -e ".[overlay]"`.
3. Avvia `avvia-overlay.vbs`, seleziona una **guida installata**, quindi passa a
   HOI4. Il launcher locale usa la porta **8772**. Per un'altra porta:
   `wscript.exe avvia-overlay.vbs 8766`.
4. Dopo un'importazione usa **Ricarica guide installate** e scegli la nuova guida.
   Una bozza ancora nell'anteprima dell'importer non viene installata dall'overlay.
5. Per sospendere togli la spunta **Attiva cornici sopra HOI4**. Chiudi la finestra
   di controllo per terminare anche il processo OCR. Un secondo avvio nello
   stesso checkout non crea una seconda istanza.

Avvio equivalente da PowerShell nella cartella del progetto:

```powershell
$env:PYTHONPATH = "$PWD\src"
py -3 -m hoi4companion.overlay_windows --port 8772 --start
```

Serve Windows con OCR disponibile nelle lingue del profilo, Pillow e Tkinter.
OpenCV/NumPy sono facoltativi: senza di essi restano le corrispondenze testuali.
Il percorso attuale è predisposto per il checkout del repository, che contiene
`scripts/overlay-ocr.ps1`; il solo wheel non include questo launcher/script.
La composizione sopra il gioco è stata provata sulla finestra DirectX 11 locale;
altre modalità video, schermi, DPI e scale richiedono verifica.

## Criteri e copertura

- Le indicazioni vengono proiettate dal **Guide Runtime**, con i suoi vincoli,
  dipendenze e slot paralleli. Non vengono reinterpretati il parlato o i tempi
  descritti nella guida. I promemoria non producono cornici.
- Le nuove ricerche proposte sono tante quanti gli slot **liberi osservati**,
  se la guida offre abbastanza azioni ammesse. Tecnologie ripetute non consumano
  due posti; ricerche già avviate, dipendenze e dati sconosciuti non sono aggirati
  per riempire il numero. La selezione vale su tutte le schede, non sulla sola
  pagina aperta. I nodi nelle altre schede diventano visibili quando le si apre.
- Per raggiungere una scheda, l'overlay riconosce il suo sprite **inattivo**
  insieme a due altre icone della barra nella disposizione dichiarata dalla GUI.
  La cornice delimita l'interno riconosciuto dell'icona, evitando i bordi coperti
  dall'albero. Mostra nome della scheda e numero di suggerimenti contenuti;
  più ricerche della stessa scheda condividono una cornice di navigazione.
  Quando i nodi sono riconosciuti nella pagina aperta, prendono il posto della
  relativa indicazione sulla scheda. Non si inventano posizioni fuori schermo.
- Guida, nazione del runtime e nazione osservata devono corrispondere. Se manca
  l'identità osservata o lo stato live è inutilizzabile, non viene mostrato nulla.
- Focus, ricerche con nome leggibile, leggi, consiglieri e produzioni: il nome
  localizzato o inglese deve essere riconosciuto **esattamente e univocamente**
  nella schermata pertinente. Si normalizzano accenti e punteggiatura, senza
  corrispondenze approssimate. Una cornice OCR delimita la dicitura, non recupera
  il rettangolo nativo completo del pulsante.
- Tecnologie senza dicitura: si usano sprite e layout già estratti dal gioco
  tramite le API locali. Occorrono un'intestazione della scheda, un'icona bersaglio
  univoca e **due altri sprite distinti** nella stessa disposizione. Le icone
  riutilizzate per ID diversi vengono rifiutate. Si provano scale 1, 0,8, 1,25 e
  1,5; altre scale o modifiche del layout possono non essere riconosciute.
- Eventi: sono richiesti l'evento **visibile osservato dal lettore**, l'ID esplicito
  dell'opzione e il riscontro visivo di titolo e opzione. L'overlay non provoca
  eventi e non sceglie risposte.
- Produzione: i nomi delle linee già qualificate dal runtime vengono riutilizzati;
  il numero nella cornice indica il **totale** richiesto, anche per un incremento
  relativo con baseline. Non viene individuato il singolo pulsante di assegnazione
  di una fabbrica. Più diciture indistinguibili fanno sospendere la cornice.
- Politica/personaggi: servono associazioni esplicite ai cataloghi; una descrizione
  generica o un promemoria non bastano. Per lo stesso gruppo/ruolo viene proposta
  soltanto la prima istruzione attualmente ammessa.

Il colore giallo indica un'azione suggerita, l'azzurro una selezione attiva quando
il runtime la espone. L'osservazione dello schermo non completa mai uno step:
completamento e avanzamento restano responsabilità del runtime esistente.

## Aggiornamento e risorse

Lo stato della guida arriva dal flusso SSE già esistente. L'overlay non interroga
nuovamente la memoria. Un controllo visivo viene avviato al massimo ogni 500 ms,
solo con HOI4 in primo piano e indicazioni disponibili, senza lavori concorrenti.
OCR e sprite vengono riutilizzati quando i pixel del pannello non cambiano;
la barra superiore/FPS non invalida da sola la cache.

Le cornici si nascondono uscendo dal gioco, disattivando il controllo, cambiando
guida/geometria o perdendo lo stato live. Le cornici senza riscontro fresco da
oltre due secondi vengono scartate; prima del disegno si ricontrollano i pixel
del bersaglio e, per gli sprite, dei due riscontri di layout. Non è un hook
sincrono del rendering: durante movimenti o transizioni resta una latenza visiva.
Testi troppo piccoli, tooltip, occlusioni o ambiguità possono impedire la cornice.

Le immagini OCR viaggiano in memoria verso un processo Windows locale. L'overlay
si esclude dalla cattura per non riconoscere i propri testi. Nessuna immagine
viene inviata a servizi esterni. La diagnostica `--proof percorso.png` salva
esplicitamente un fotogramma reale con overlay, l'ingresso OCR e misure locali.
I log in `.local/overlay-*.json` non sono prove di completamento della partita.

## Verifiche del 23 ottobre 2026

Sulla partita polacca reale, in pausa, finestra 2560×1440 DirectX 11:

- Cornice gialla effettivamente visibile su **Bloccare Danzica**, riconosciuto
  dalla dicitura italiana. A zoom più basso il testo non era riconosciuto:
  l'overlay ha correttamente omesso l'indicazione.
- Cornice effettivamente visibile sul nodo a icona **Attrezzature di base**, con
  due riscontri indipendenti nel layout Industria. Misure locali del confronto
  sprite circa **0,14–0,17 s**, ciclo completo osservato circa **1 s** dopo
  l'ottimizzazione; non sono garanzie per altri PC o schede.
- Cornice effettivamente visibile su **Ingegneria elettro-meccanica** nella
  scheda Ingegneria, con due riscontri di layout. Otto campioni consecutivi
  dello stato dell'overlay mostravano `visible:true`, soltanto `r01`, nessun errore.
- Cornice di focus rimossa aprendo Ricerca; `visible:false` e nessuna associazione
  residua passando alla finestra di controllo.
- Secondo avvio: una sola finestra di controllo rimasta aperta.

Le immagini reali sono `.local/overlay-focus-proof.png`,
`.local/overlay-research-proof.png` e `.local/overlay-engineering-proof.png`
(non incluse nel repository).

Test mirati coprono ammissione del runtime, identità della nazione, dipendenze
non verificabili, nomi multilinea, ambiguità, eventi osservati, produzione e
baseline, recupero dopo heartbeat inutilizzabile, cambio guida, sprite duplicati,
landmark mancanti e rifiuto di URL esterni. Gli sprite dei test sono sintetici.
Suite eseguite: **327 test Python e 213 JavaScript superati**, nessuno saltato
sull'ambiente locale con le dipendenze opzionali disponibili.

**Restano da verificare nella partita:** opzioni evento, menu di leggi/personaggi
e produzioni con istruzioni attive della guida; altri alberi tecnologici, scale e
modalità video. Non sono dichiarati validati sulla sola base delle fixture.

## Navigazione delle ricerche e capacità (23 ottobre 2026)

Suite aggiornate: **351 Python e 226 JavaScript** superati, nessuno saltato;
65 output completi delle fixture runtime identici fra Python e JavaScript.
Coperti zero, uno e tre slot liberi, capacità sconosciuta, ricerche già attive,
ID ripetuti, ordine del runtime fra schede diverse, raggruppamento dei pulsanti,
immagini attive/inattive, occlusione e barre duplicate.
Le icone dei nodi rispettano anche le origini non centrate dichiarate dalla GUI
(per esempio Artiglieria). La scoperta dei candidati usa lo stesso errore RGB
assoluto dell'ammissione, eliminando un filtro normalizzato che penalizzava
icone scure; soglia finale e due riscontri indipendenti restano richiesti.

Verificata nella partita reale POL la cornice sulla scheda **Artiglieria** mentre
era aperta **Ingegneria**, con un solo slot libero osservato e le ricerche
Attrezzature di base e Ingegneria elettro-meccanica già avviate. Prova composita
reale: `.local/overlay-navigation-proof.png`; diagnostica:
`.local/overlay-navigation-tab-verification.json`. Il caso di tre slot liberi
è verificato con fixture, senza annullare ricerche o cambiare partita per provarlo.
Aprendo Artiglieria è stata verificata la scomparsa dell'indicazione sulla scheda
e la cornice sul nodo **Artiglieria Interguerra** (`r03`), con due riscontri GUI;
prova composita `.local/overlay-navigation-node-proof.png` e diagnostica
`.local/overlay-navigation-node-verification.json`. Nessuna ricerca selezionata.
Companion riavviato conservando identità della campagna e run attivo.
