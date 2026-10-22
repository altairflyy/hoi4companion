# Walkthrough Polonia e HUD

Fonte: [Secret Poland Guide: Wojtek Conquers The World, MachiavellianStrategist](https://www.youtube.com/watch?v=FBPdhlwqMwM), pubblicato il 18 aprile 2026, durata 42:54. Le priorità vengono dalla descrizione recuperata con yt-dlp; i checkpoint dai sottotitoli automatici inglesi. Non è stata eseguita una verifica visiva di ogni fotogramma. Il documento privato per gli abbonati menzionato dall’autore non è stato utilizzato.

## Percorso

I dati conservano tutte le 34 righe della priorità dei focus (inclusi attesa,
duplicato condizionale e scelta libera) e tutte le 48 righe della ricerca
(inclusi due progetti speciali e scelta libera). Focus e tecnologie sono
evidenziati nelle rispettive schede, con un comando per centrare il prossimo
elemento e un contrassegno sulla categoria tecnologica corretta.
`/walkthrough` presenta invece soltanto istruzioni operative e checkpoint:
costruzioni, produzione, politica, personaggi, forze armate, template e progetti.
L’ordine della fonte non rappresenta date di completamento garantite.
Le ricerche si assegnano ai diversi slot in parallelo.

Organizzazione attuale, metadati e verifiche in
[contextual-guide.md](contextual-guide.md).

Gli omonimi sono associati al ramo Romanov: Demand Lithuania → `POL_demand_LIT_pavel`, Demand Slovakia → `POL_demand_slovakia_pavel`, Pan-Slavic Revanchism → `POL_pan_slavism`. La versione falangista e altre omonime non sono intercambiabili. Kb PPanc → `infantry_at`, verificato nella localizzazione nazionale polacca. Truck → `motorised_infantry`, distinto da `tech_trucks`, già nella base polacca 1936.

UP viene espanso attraverso le catene locali dalla base polacca 1936, comprese le tecnologie intermedie di miglioramento. L’autore non specifica ogni livello nella descrizione: questi target sono marcati come interpretazioni. Esempio fanteria: `infantry_weapons2` → `improved_infantry_weapons` → `improved_infantry_weapons_2` → `advanced_infantry_weapons`. Nessun nome di livello viene presentato come una trascrizione certa del video.

## Avanzamento

Il modello puro `web/guide-model.js` riceve il catalogo curato, l’ultimo evento live valido e le conferme manuali. Non scrive completamenti nel gioco e non avanza sulla base di timer o clic sui focus.

- Applica completamenti solo a tag POL e albero `polish_focus`, con `usable=true`.
- Distingue un focus attivo da un focus completato/saltato. Le ricerche già attive non vengono proposte una seconda volta.
- Propone le priorità per gli slot liberi e segnala ricerche/focus in corso fuori percorso.
- Non passa automaticamente oltre l’attesa per l’annessione lituana. Il completamento della richiesta non dimostra l’annessione.
- Non presume sconti di ricerca. Le due righe che li richiedono attendono conferma.
- Tratta i progetti speciali come una checklist manuale indipendente dagli slot normali.
- Dopo Nuova Accademia Militare, una conferma esplicita di disponibilità dà priorità all’unità slava e salta i tre riempitivi nella guida. Dopo l’unità slava, Modernizzazione dell’artiglieria viene riproposto se manca; il doppione ha lo stesso ID.
- Se la fonte diventa obsoleta, ritorna all’anteprima senza applicare completamenti vecchi.

`GET/POST /api/guide/checks` condivide esclusivamente le checkbox dichiarate dalla guida. Non consente di confermare manualmente una tecnologia o un focus ordinario. Il server conserva queste conferme solo in memoria. La revisione delle conferme fa parte degli eventi SSE, così HUD e pagina si sincronizzano anche a gioco in pausa o chiuso. Le richieste di modifica richiedono JSON e rispettano il controllo della stessa origine già applicato alla configurazione.

Il frontend azzera le conferme quando osserva un cambio di nazione o la perdita di completamenti dopo un caricamento precedente. Il reader non fornisce un identificatore universale della campagna: per partite differenti con progressi indistinguibili va usata Nuova checklist. La sessione del browser non viene interpretata come prova dell’identità della partita.

## HUD

`avvia-hud.vbs` avvia `scripts/hud.ps1` senza console visibile. Lo script apre `/hud` in Edge in modalità app con un profilo dedicato al progetto. Una stringa casuale per ogni lancio identifica la finestra esatta, anche quando Edge ha altre finestre aperte o la finestra app inizialmente è nascosta. Il pannello viene impostato Topmost senza attivarlo; compare quando HOI4 è in primo piano e si nasconde passando ad altre applicazioni. Non apre handle della memoria del gioco: usa il servizio locale esistente.

Il riquadro HUD mostra l'istruzione operativa da verificare e i collegamenti
agli alberi del companion, dove sono evidenziati i nodi. Non è una cornice
agganciata alla GUI di HOI4: quella funzione richiede il rilevamento del menu,
del nodo e dello zoom, non osservati dal reader attuale. La sovrapposizione va
qualificata sul gioco in finestra/senza bordi; non è garantita in schermo intero
esclusivo. L’anteprima desktop è disponibile con `-Preview`.

## Prove storiche della prima vista a liste (superata)

Il catalogo locale riconosce tutti i target ordinari della guida: nessuna corrispondenza mancante all’API. La vista reale del browser mostra 34 righe focus, 48 righe ricerca, i prossimi passi dal 1936 e nessun errore JavaScript osservato. I collegamenti aprono e selezionano `POL_clamp_down_on_danzig` e `electronic_mechanical_engineering` nei relativi esploratori.

La prova browser/SSE sintetica è separata dal lettore reale, con titolo TEST SINTETICO e server temporaneo distinto: completamento e attività aggiornano i suggerimenti senza ricaricare, l’attesa lituana blocca la sequenza, confermare lo sconto non completa la tecnologia, l’unità slava prende priorità e l’artiglieria torna dopo, una disconnessione e un’altra nazione non conservano completamenti applicati. Questi risultati non costituiscono una prova su una nuova partita realmente aperta: l’utente ha richiesto di preparare il percorso dal 1936 a gioco chiuso.

Le prove sono nella cartella `outputs` della chat: `walkthrough-browser-synthetic-verification.json`, `hoi4-walkthrough-hud-preview.jpg` e la verifica finale. Gli originali descrizione e sottotitoli restano in `work/youtube-FBPdhlwqMwM` della chat; nessun asset del gioco è distribuito.

## Spegnimento richiesto separatamente

`scripts/shutdown-countdown.ps1` non è avviato dal gioco, dal server o dal lanciatore HUD. Viene eseguito solo a fine lavoro su richiesta esplicita. Mostra un conto alla rovescia annullabile; chiudere la finestra annulla. Alla scadenza richiede `shutdown /s /t 0`, senza `/f`, così Windows può bloccare la chiusura per applicazioni con dati non salvati. `-TestOnly` disabilita il comando anche alla scadenza ed è disponibile per provare il pulsante Annulla senza rischio di spegnimento.

Verifica finale: 107 test Python e 17 JavaScript superati. Il test del conto alla rovescia ha prodotto status cancelled e test_expired con test_only=true, senza comando di spegnimento. La finestra HUD di anteprima ha riportato topmost=true; il gioco era chiuso.
