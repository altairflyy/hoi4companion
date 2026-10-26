# Eventi, politica e promemoria

## Collegamento risposte guida e HUD, 26 ottobre 2026

Le risposte dichiarate nella guida attiva sono preparate prima dell'apertura
dell'evento. Il lettore GUI può quindi evidenziare subito il pulsante tramite
la coppia `event_id` / `event_option_id` e l'istanza corrente della finestra,
anche prima che l'aggiornamento SSE riporti quell'evento nel riepilogo live.
La geometria deve essere riletta e qualificata: il piano non conserva rettangoli
da riutilizzare alla comparsa successiva. Scomparsa del popup, cambio nazione,
campagna o guida non certificano una risposta e non conservano una cornice.

Il percorso visivo riusa ora la stessa associazione evento/risposta del percorso
nativo e richiede anche l'opzione nell'elenco live osservato. Non accetta un ID
risposta di un altro evento. Un piano preparato per un'altra guida viene
ricompilato, anche se le guide riutilizzano le stesse chiavi degli step. Il
percorso nativo controlla separatamente ID evento, ID opzione, identità composta
e istanza della finestra; non seleziona pulsanti né deduce completamenti.
Non servono nuovi campi nel prompt o modifiche alle guide già associate.

`tests/test_event_overlay_flow.py` verifica il percorso definizione -> runtime
-> associazione -> geometria nativa -> protocollo HUD con fixture, compresi
popup aperto/chiuso, condizioni non soddisfatte o sconosciute e contesti diversi.
L'audit del catalogo locale conferma le due risposte dell'ultima guida Kaiserin
senza riferimenti irrisolti. Queste verifiche non costituiscono una nuova prova
visiva nel gioco: HOI4 era chiuso durante l'intervento.

## HUD nativo degli eventi, 24 ottobre 2026

### Materiale da fornire all'IA esterna

In **Importa guida**, oltre al prompt aggiornato, è disponibile **Scarica catalogo
eventi a risposta multipla** (`hoi4-companion-eventi-risposte.json`). Contiene gli ID e i
nomi inglesi/localizzati degli eventi e delle risposte; ogni `event_option`
conserva `event_id`, il collegamento al proprio evento. Include solo gli eventi
con almeno due ID di risposta distinti definiti negli script; duplicati dello
stesso ID non contano come scelte diverse. Le opzioni condizionali possono
ridurre a una sola le risposte mostrate in una particolare partita. Non applica il filtro
dell'albero dei focus: la guida può includere eventi di altri paesi. Il catalogo
ID completo conserva tutte le entità, inclusi gli eventi a risposta unica; non
occorre allegare entrambi i file per le stesse associazioni.

Per aggiornare soltanto gli eventi, fornire all'IA prompt, video/fonti, catalogo
eventi e guida attuale chiedendo di conservare i passaggi già corretti. Il prompt
richiede evento, testo completo della risposta e coppia di ID dal medesimo evento,
con evidenze e condizioni della fonte. Scelte non leggibili o non identificabili
devono restare dichiarate irrisolte, senza inventare una strategia.

Il pacchetto API facoltativo conserva l'evento padre e tutte le sue risposte
quando il testo acquisito ne cita una. Quel contesto è selezionato in base al
testo, non all'analisi dei fotogrammi: per scelte presenti solo nel video serve il
catalogo completo o quello dedicato. Verificato il catalogo locale con 6.967
eventi e 11.481 risposte, senza collegamenti orfani. L'esportazione dedicata
filtrata contiene 3.193 eventi e 7.832 risposte: 3.343.875 byte rispetto ai
5.464.103 byte del precedente file completo, con lo stesso formato JSON.
Il filtro interessa soltanto l'esportazione dedicata, non il catalogo interno,
la risoluzione degli ID o il rilevamento live. Test dell'esportazione,
contratto del prompt e chiusura dei riferimenti nel pacchetto inclusi nelle
suite: 533 Python e 244 JavaScript superati nella verifica iniziale; dopo il
filtro sono superati 245 test JavaScript e i test Python mirati dell'importer.
HTTP verificati sulla porta 8766;
la verifica visiva del browser non è stata eseguita perché il browser locale è
disabilitato nelle impostazioni Work.

L'overlay prepara in anticipo le associazioni evento/risposta dichiarate nella
guida con `guide_events.compile_event_plan`. Riusa il catalogo inglese e le
localizzazioni attive. Una citazione parziale esplicita del pulsante è ammessa
solo se contiene almeno quattro parole contigue e identifica una sola scelta;
un ID esplicito errato non viene sostituito tramite testo. La guida originale
non viene modificata. Associazioni mancanti o ambigue restano irrisolte.

Il piano contiene anche i passaggi futuri, ma le indicazioni mostrate rispettano
condizioni, percorso e dipendenze valutati dal runtime. Non è una previsione degli
eventi che si verificheranno. Due risposte diverse ammesse per lo stesso evento
vengono sospese, senza sceglierne una in base all'ordine. I promemoria con una
risposta esplicita possono fornire un'indicazione senza acquisire un criterio di
completamento.

La lettura HUD è indipendente dal vecchio riconoscimento OCR: `event_ui_native.py`
si collega al registro GUI condiviso di `focus_ui_native.py`. Per ogni finestra
verifica CEventWindow, root, proprietario, ID evento e griglia corrente. Le scelte
provengono soltanto dalle righe effettivamente presenti; le opzioni condizionali
della definizione non diventano pulsanti visibili. CButtonDrag, CEventOption,
parent e sprite sono verificati; il rettangolo usa lo stesso calcolo del motore
di focus e tecnologie. Gli indirizzi eventualmente ricevuti dal lettore live sono
solo candidati e vengono ricontrollati. L'inventario incrementale cerca nuovi
oggetti quando cambia il registro, senza ripetere una scansione completa a ogni
frame. Zero finestre note significa sconosciuto, non assenza globale certificata.
Il riuso di un indirizzo già visto senza cambiamenti del registro può richiedere
un nuovo candidato dal lettore live.

Un popup qualificato ha priorità sui consigli dell'albero sottostante, anche se
non riguarda la guida. L'HUD evidenzia soltanto una risposta esatta della guida;
finestre sovrapposte/ambigue, risposte non mostrate, dati incoerenti o di un'altra
nazione non producono una scelta evidenziata. Nessuna risposta viene premuta.
Nessun riconoscimento, chiusura o scomparsa della finestra modifica checklist,
avanzamento o stato di completamento.

Prova reale in background: nazione GER, evento `lar_news.10`, istanza
`1d1970e39b0`, unica opzione mostrata `lar_news.10.d` (indice nativo 5). Pulsante
osservato `[1078,915,1430,963]` in coordinate client LTRB, con root, proprietà,
griglia e parent verificati. Una proiezione diagnostica non mostrata né installata
ha confermato l'associazione esatta; la guida tedesca non raccomanda questo evento
e quindi il piano reale non lo evidenzia. Nessun evento provocato o risposta
selezionata. La cattura disponibile mostrava un'altra applicazione: l'allineamento
visivo dell'overlay non è dichiarato verificato.

Nella guida locale della Kaiserin, `o-044` si risolve a
`wtt_germany.1000:wtt_germany.1000.a` tramite la citazione «We shall send a liaison».
`o-027` («Gestione eventi SA») rimane irrisolto: contiene soltanto un rinvio al
video, senza evento o risposta identificabili. Non serve rigenerare le associazioni
già risolte; quella mancante richiede un riferimento esplicito nella guida.

Verifiche: 531 test Python, 243 JavaScript e 10 Rust superati. Coperti piano e
citazioni ambigue, applicabilità del runtime, conflitti di risposta, opzioni
presenti solo nella definizione, proprietari estranei, chiusura e letture
incoerenti. La geometria copre CEventWindow sulla build supportata; altri tipi di
popup e tutti i layout introdotti dalle mod non sono qualificati universalmente.

Estensione del 22 ottobre 2026, sola build SHA-256
`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`.
Il processo viene aperto solo con le autorizzazioni di lettura esistenti.
Nessun input, iniezione, salvataggio o file del gioco modificato.

## Contratti

`country_overview.politics` qualifica separatamente `ideas.items` e
`advisors.items`. Si veda [politics-live.md](politics-live.md).
L'adattatore Python/JS espone `politics.ideas` e `politics.advisors` come
membership calcolate da elenchi completi, tipizzati, della nazione corrente.
Elenco vuoto osservato significa assenza nota; sezione mancante, incoerente,
parziale per quella membership o stato stale significa sconosciuto.

`events.visible` contiene le finestre eventi marcate visibili dal motore,
con `instance_id`, `event_id`, `option_ids`. Non è una prova di visibilità
fisica sul monitor quando HOI4 è minimizzato/coperto. L'adattatore espone
`events.visible` e `events.visible_options` (`event_id:option_id`). Una scelta
non letta non viene sostituita con tutte le opzioni della definizione.

La guida associa evento e risposta tramite catalogo inglese/localizzato.
L'evento aperto viene evidenziato nel walkthrough insieme alla scelta prevista.
Non viene selezionata alcuna risposta automaticamente. Chiusura e scomparsa
non provano la scelta né completano un passaggio. Il completamento necessita
di un criterio indipendente oppure della conferma dell'utente.

Le decisioni espongono solo `listed_ids` e `execution_counters`: appartenenza
al manager e contatori nativi relativi all'esecuzione/casualità. Non sono ancora
qualificati come disponibilità, avvio, cronologia completa o completamento.
Nessuna regola della guida usa questi contatori come falsa prova di esecuzione.

## Layout e provenienza

REA ha identificato il PE installato; il tentativo di ricerca profonda Ghidra
è scaduto. Non viene dichiarata decompilazione riuscita. Le strutture sono state
riscontrate tramite byte PE/Capstone e RTTI runtime dello stesso eseguibile.
Riferimenti RVA e registri locali nella cartella `work` della chat:

- `events-reset-disasm.txt`, RVA `123B0B0`: finestra→evento `+68`, creazione
  griglia `+1280`; `123B740..123B7A3` applica i trigger prima di aggiungere righe.
- `events-option-item-disasm.txt`, RVA `1238CA0`: riga `+10`→container,
  container `+10D8`→pulsanti, pulsante `+8`→opzione selezionabile.
- `events-show-disasm.txt`, RVA `1238FE0`: uso dell'indice opzione `+1F0`.
- `events-grid-probe.json`: griglia `+1A0`→CStandardGridBoxItem; confronto
  con CEventOption di definizione `event+368`, ID evento `+20`, ID scelta `+178`.
- GUI root RVA `34531E0`, registri `+108`/`+120`; RTTI CGameGui,
  CEventWindow e legame container+30→root e flag di visualizzazione `+75`.
- Decision manager country+FA0→CDecisionStatus, owner+8; lista+40,
  contatori+148 con record24; `decisions-counter-disasm.txt` RVA737EF7.

La scoperta dei candidati GUI è limitata e memorizza indirizzi solo per il
processo corrente. I candidati non sono dati pubblicati: tipo, root, legame,
flag, evento e righe selezionate vengono ricontrollati con i testimoni standard.
Se la finestra non è individuata o la cache non è più valida la sezione è
non disponibile, senza inventare assenza o usare puntatori di un'altra sessione.
La scoperta iniziale sulla partita osservata ha richiesto circa511ms; la
lettura successiva circa0,23ms. Non sono benchmark generali né latenza click→UI.

## Verifica e limiti

Lettura reale coerente POL: evento `news.25` (Rimilitarizzazione della Renania),
sola scelta mostrata `news.25.a`. Le cinque opzioni di definizione, comprese
varianti condizionali duplicate, non diventano cinque scelte mostrate.
Artefatto riproducibile: `.local/events-implemented-live.json`.
L'utente ha confermato la presenza di un evento a schermo; titolo e testo sono
letti dal catalogo, senza confronto visuale indipendente con il pannello HOI4.
Le variazioni flag/griglia sono verificate con fixture; non è stata provocata
una scelta o un cambio di legge nella partita. Nessun tracciamento di risposte
passate o eventi già chiusi, né di eventi popup diversi da CEventWindow.

`tracking:"reminder"` è un'estensione additiva v1 per istruzioni sul metodo:
nessuna conferma, completamento o barriera implicita. Condizioni e provenienza
rimangono. È vietato usarlo con criteri completion/started_when oppure come
dipendenza da completare. L'anteprima permette la conversione esplicita di
un'istruzione; dati mancanti non vengono riclassificati automaticamente.

## Esito integrazione

Server locale aggiornato sulla porta8772. Validata via HTTP una copia della
guida esistente:211step, nessun errore bloccante;34operazioni con criterio
automatico,5con solo avvio,67con criterio manuale,28promemoria. Copia:
`.local/polonia-romanov-wojtek-politica-promemoria.hoi4-guide.json`; sorgente
Downloads invariato. Le specifiche di questa guida restano dati, non condizioni
speciali nel motore. Nessuna guida di prova installata né progresso resettato.

Suite309Python e207JavaScript superate, zero skip;60output completi delle
fixture runtime identici fra Python/JS;6membership politiche/eventi/decisioni
del campione reale identiche nei due adattatori. Pacchetto IA sul caso reale
899.210byte, sotto il limite2MB; validazione sempre sul catalogo completo.
Regressione corretta: l'esistenza dell'agenzia non completa una richiesta di
cinque upgrade. Verificati anche riconoscimento consigliere contro semplice
reclutamento, sostituzione dei soli criteri generati dopo edit, conservazione
dei criteri manuali dichiarati e isolamento di dati stale/estranei.

Verifica grafica non eseguita: il browser locale è disabilitato nelle impostazioni
Work. Test DOM simulati e API non vengono dichiarati prova visuale. Nessuna
nuova verifica reale di nomina non vuota, cambio legge, scelta evento o avvio
decisione. Il rilevamento della finestra aperta è stato invece osservato sulla
partita POL e verificato dopo il riavvio del server.
