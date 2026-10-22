# Guide Importer MVP v1

Pagina `/importer`, collegata dal walkthrough. Importa contenuti editoriali:
non legge la partita, non genera strategie, non avvia ordini e non estende il
lettore nativo. Il risultato eseguibile resta **Guide Schema v1**, valutato
dal runtime universale esistente.

## Uso principale: prompt → tua IA → file importabile

1. Premi **Copia prompt completo** o **Scarica prompt .txt**. Il prompt è
   disponibile anche nel [file del progetto](../src/hoi4companion/guide-import-prompt.md).
   Non serve acquisire fonti nel programma e non servono credenziali API.
2. Dai il prompt alla tua IA insieme al link o al materiale della guida.
   Facoltativamente allega il **catalogo ID** esportato dalla pagina:
   selezionare l’albero riduce il contesto e aiuta le corrispondenze.
3. L’IA deve restituire un file autocontenuto `nome.hoi4-guide.json`, con
   `format:"hoi4-guide-import"`, `import_schema_version:1`, `sources` (testi,
   segmenti ed evidenze) e `guide` (Guide Schema v1). Il prompt specifica tutti
   i campi, le categorie operative e come segnalare dati mancanti. Non chiede
   hash all’IA: il programma li calcola e rimappa gli ID delle fonti.
4. Carica il file restituito o incollalo integralmente nell’editor. La risposta
   viene normalizzata senza interrogare IA, scaricare di nuovo le fonti o
   richiedere un’acquisizione preliminare nel programma.
5. Premi **Risolvi ID e valida**. Correggi titoli e corrispondenze nelle schede;
   usa la ricerca del catalogo o l’editor JSON per gli altri ID. Tutti i parametri,
   condizioni, dipendenze e priorità sono modificabili nel JSON. Un ciclo o una
   bozza non valida rimane correggibile, senza essere installata.
6. Dopo la revisione, esporta il `.json` v1 o premi **Aggiungi alle mie guide**.
   Il collegamento apre il walkthrough con `?guide=<id>`; gli alberi ricevono
   la stessa selezione tramite il normale controller delle guide.

**Salva bozza modificabile** conserva fonti e definizione in un unico file
`import_schema_version:1`. Il file finale contiene soltanto la definizione v1
e i metadati delle fonti; conserva il pacchetto/bozza per le revisioni future.
Un vecchio file contenente solo la definizione v1 richiede anche le fonti per
verificarne le citazioni. La nuova risposta autocontenuta dell’IA le include.
Le bozze rimangono nel browser finché non le scarichi.

Esempio autocontenuto: [importer-opening.draft.json](../examples/importer-opening.draft.json).
È un testo **inventato per provare il formato**, non una strategia di gioco.

## Estrazione API facoltativa

Acquisizione nel programma e provider API sono conservati nella sezione
**Opzioni aggiuntive**, chiusa inizialmente. È ancora possibile acquisire URL o
testo e scaricare un pacchetto con fonti e catalogo già assemblati. Questo non
è un requisito per usare il prompt o importare il file della propria IA.

Il server usa OpenAI Responses, JSON mode e `store:false`. Il pulsante è
disabilitato senza entrambe le variabili locali:

- `OPENAI_API_KEY`: credenziale API locale, mai restituita al frontend.
- `HOI4_IMPORT_MODEL`: ID di un modello del tuo account compatibile con Responses
  e JSON mode. Nessun modello/costo viene scelto automaticamente.

Impostale nell’ambiente del processo che avvia il server e riavvia quel processo.
Non inserirle nei JSON delle guide o in file tracciati. Le credenziali ChatGPT
non sono utilizzate come credenziali API. Le chiamate avvengono solo con il
pulsante **Estrai con API IA**: inviano fonti e catalogo e possono consumare
credito. Nessuna installazione automatica della risposta. Errori HTTP, timeout,
rifiuti e risposte incomplete vengono segnalati senza esporre la chiave.

Il contratto del provider è basato sulla [documentazione ufficiale degli output
strutturati](https://developers.openai.com/api/docs/guides/structured-outputs?api-mode=responses).
Il JSON mode non garantisce da solo conformità semantica: la risposta passa
comunque dal validatore v1 e dalle verifiche dell’importatore.

## Contratti e limiti

### Collegamento delle istruzioni operative al monitoraggio live

`manual` è un tipo di azione dello schema, non una dichiarazione che il suo
avanzamento debba essere confermato a mano. Il compilatore `operation_import`
risolve edificio e regione dai dati installati (nomi inglesi, locali e varianti
nazionali), quindi aggiunge solo i predicati mancanti supportati dal runtime:

- Coda qualificata: `construction.queued` indica l'avvio anche in pausa.
- Livello/totale esplicito: confronto con i contatori costruiti già esposti per
  infrastrutture, fabbriche civili/militari e cantieri nella regione richiesta.
- Agenzia: flag qualificati `in_creation` e `is_created` per avvio e completamento.

Il lettore nativo non cambia. Condizioni esplicite già presenti sono conservate.
Quantità aggiuntive senza `baseline_level` vengono collegate al conteggio iniziale
registrato automaticamente nel run, prima dell'ordine, e al relativo incremento.
La guida resta separata da quel conteggio. Un `baseline_level` dichiarato dalla
fonte o un obiettivo assoluto restano supportati. Non si deduce una baseline
da un conteggio corrente letto dopo l'inizio dei lavori. Uscire dalla coda non
prova il completamento. Dettagli e limiti in [guide-runtime.md](guide-runtime.md).
Impianti/progetti senza un contatore finale esposto possono avere l'avvio
automatico e mantenere una conferma per la fine. Regioni o quantità non
identificate rimangono esplicitamente non collegabili, senza interpretare prosa.

Il report `operations` classifica ogni istruzione come `automatic`, `start_only`
o `manual`, con motivazione. La schermata mostra il riepilogo e raccoglie le
note sulle fonti in un pannello espandibile; non ripete un avviso per ogni
azione chiamata `manual`. L'importatore prepara la definizione: la valutazione
dei dati della partita avviene nel walkthrough. La validazione tecnica non
certifica la fedeltà editoriale del video né la disponibilità corrente del live.

Verifica del collegamento (22 ottobre 2026): **259 test Python e 156 JavaScript
superati**, nessun test saltato. Sul catalogo installato, anteprima HTTP e browser
del file revisionato: zero errori bloccanti, 4 operazioni con completamento
automatico, 2 con solo avvio automatico, 128 con conferme ancora necessarie.
Verificati i nomi nazionali delle regioni, l'idempotenza della compilazione,
l'assenza di errori console e il mantenimento del blocco installazione prima
della revisione. Il file originale resta invariato e la guida non è stata
installata durante queste prove. Nessuna nuova verifica su una partita reale:
le transizioni e il rifiuto di stato stale/nazione diversa sono provati con
fixture, non costituiscono una nuova validazione dell'acquisizione live.
Evidenze: `.local/operation-import-verification.json`, log delle suite e
`.local/operation-import-preview.png`.

Aggiornamento successivo: le quantità aggiuntive ora usano la baseline del run
descritta sopra. La stessa guida passa a **5 operazioni con completamento
automatico, 1 con solo avvio automatico e 128 con conferme necessarie**; l'ordine
di due fabbriche a Katowice è collegato al nuovo criterio relativo. Suite finali:
**266 Python e 172 JavaScript**, nessun test saltato; output completi del runtime
identici tra Python e JavaScript su 13 fixture condivise. Verificati nel browser
reale avvio, annullamento parziale e completamento su un server fixture isolato.
Il lettore della partita reale continua a restituire uno stato utilizzabile;
nessuna costruzione reale è stata aggiunta/annullata per questa prova.
Evidenze: `.local/construction-baseline-verification.json` e screenshot con prefisso
`construction-baseline-`. Il server fixture è stato arrestato dopo la verifica.

Il collegamento successivo della produzione aggiunge 248 tipi di equipaggiamento
al catalogo installato e converte 15 delle 22 istruzioni di produzione in criteri
automatici, con un'altra istruzione riconoscibile nel solo avvio. Le condizioni
distinguono fabbriche richieste, assegnate, attive e danneggiate; supportano tipo,
modello e incrementi dal conteggio iniziale del run. Dettagli e risultati in
[production-guide-monitoring.md](production-guide-monitoring.md).

### Identità inglese e diciture installate

L'ID nativo resta l'identità stabile salvata nelle guide. Il catalogo
`/api/import/catalog` espone `name` come riferimento inglese e `localized_name`
come testo della lingua configurata, applicando le mod attive del playset.
La risoluzione cerca prima l'ID esatto, poi il nome inglese, infine gli alias
locali. I nomi locali restano quindi accettati all'importazione. L'anteprima e gli alberi
mostrano le diciture installate; cambiare lingua non cambia gli ID delle guide.

La localizzazione viene letta una volta per aggiornamento del catalogo, separando
i dizionari di riferimento e presentazione. Le traduzioni che sovrascrivono
anche i file `*_l_english.yml` non cancellano i nomi inglesi del gioco base.
Per gli ID aggiunti dalle mod si usa il primo riferimento inglese disponibile;
in assenza di inglese si mantiene il nome disponibile, senza inventarlo.

I focus includono `countries` dai selettori degli alberi già valutati dal
framework. Paese e `scope` dichiarati limitano le corrispondenze; metadati
sconosciuti e omonimie residue non vengono risolti con supposizioni sui prefissi
degli ID. Abbreviazioni, refusi e punti di partenza UP non dichiarati possono
quindi richiedere ancora una scelta esplicita.

Verifica locale del 22 ottobre 2026: **235 test Python e 154 JavaScript
superati**, senza test saltati. Sul catalogo reale di 7.007 entità e con le
mod attive, l'API restituisce `The Four Year Plan` come riferimento e
`Il piano quadriennale` come dicitura installata; anche l'albero e la guida
Polonia inclusa conservano quest'ultima. Il file revisionato ricevuto passa
da 40 a 13 riferimenti non risolti, con 30 dei 32 step focus riconosciuti.
Restano un refuso Fulfill/Fulfil, due focus omonimi Demand Slovakia, dieci
UP senza un'ancora associata e Kb PPanc. Originale non modificato, nessuna
guida installata e nessuna nuova verifica live o grafica dichiarata.

### Refusi e omonimie nei focus

ID e nomi esatti conservano la precedenza. In assenza di corrispondenze, viene
accettato un solo errore di battitura in una sola parola lunga almeno quattro
caratteri, su nomi di almeno dieci caratteri normalizzati. Sono ammessi
inserimento, cancellazione, sostituzione o trasposizione di due lettere vicine.
Numeri, livelli romani brevi, ID scritti con underscore, nomi brevi e più
modifiche non vengono corretti. Le corrispondenze multiple restano ambigue;
ogni correzione automatica compare negli avvisi dell'anteprima.

Per focus omonimi il componente deterministico `focus_resolution.py` utilizza
il percorso dichiarato nell'intera guida, senza IA. Parte dagli ID riconosciuti
nei passaggi obbligatori, propaga i prerequisiti indiretti certamente necessari
ed elimina i candidati incompatibili con le esclusioni reciproche. Ripete la
propagazione quando una nuova corrispondenza diventa univoca. Il risultato non
dipende dalla posizione delle tracce o dal DOM.

Per un gruppo OR si conservano le alternative possibili e si deducono solo gli
antenati comuni a tutte; per gruppi AND servono tutti i requisiti. Un focus
omesso non è considerato escluso. Passaggi successivi obbligatori possono
stabilire il ramo, ma step, fasi o dipendenze condizionali non lo impongono.
Riferimenti mancanti e cicli rimangono sconosciuti. Percorsi obbligatori
contraddittori bloccano l'importazione, anche se gli ID erano già espliciti.

L'anteprima mostra i candidati esclusi e gli ID del percorso che motivano la
scelta; la spiegazione resta disponibile dopo la conversione agli ID nativi.
Il report `focus_route` contiene i focus strutturalmente necessari, lo stato
`no_conflict_detected`/`incomplete`/`conflict` e i conflitti rilevati. Sono
vincoli editoriali della guida: non sono focus osservati come completati.
Non viene interpretato il testo libero di eventi/personaggi né eseguito alcun
trigger dinamico del gioco. Se rimangono più candidati, serve una scelta
esplicita. La propagazione è conservativa, non un risolutore generale di tutte
le combinazioni possibili dei trigger HOI4; non genera un percorso strategico.

Verifica dell'estensione all'intero percorso: **253 test Python e 155
JavaScript superati**. Via API locali la guida Polonia mantiene 32 step focus
e 45 ricerche associati. Il ramo richiesto implica `POL_romanov`, esclude
`POL_demand_slovakia` e conserva `POL_demand_slovakia_pavel`. La stessa
spiegazione resta disponibile dopo una seconda validazione degli ID compilati.
Una variante con il focus incompatibile imposto esplicitamente viene rifiutata
come conflitto di percorso. Originale e impostazioni invariati, nessuna guida
installata e nessuna nuova verifica live dichiarata.

Nei file installati, i due Demand Slovakia non hanno un'esclusione reciproca
diretta. `POL_demand_slovakia_pavel` richiede `POL_demand_LIT_pavel`, condiviso
dal percorso Pavel e da quello Romanov. `POL_demand_slovakia` richiede invece
`POL_royal_dictatorship`. Il suffisso Pavel non implica esclusività di Pavel.

Verifica del file revisionato tramite API reali: **32 step focus e 45 ricerche
associati, zero errori di importazione**. Fulfill viene riconosciuto come refuso
di Fulfil; Demand Slovakia viene associato a `POL_demand_slovakia_pavel` grazie
a `POL_demand_LIT_pavel` già presente nella sequenza. La seconda validazione
conserva gli stessi obiettivi. **246 test Python e 155 JavaScript superati**.
La bozza con gli ID risolti è salvata separatamente dall'originale, senza
installazione automatica né nuove verifiche sulla partita live. I limiti
editoriali e le operazioni manuali della guida restano quelli documentati.

### UP significa avanzare nella stessa famiglia

Le famiglie vengono risolte nel primo livello utile anche quando manca UP.
Con una nazione dichiarata, l'importatore legge in sola lettura la storia
iniziale della nazione (`history/countries`), rispettando gli override delle
mod, le condizioni statiche DLC e i blocchi datati. La data predefinita è
quella del primo bookmark installato; `guide.start_date` può selezionare un
altro scenario. Questo contesto viene mantenuto separato dallo stato live:
non prova che l'utente abbia completato una ricerca nella partita corrente.
Gli ID vengono fissati all'importazione e non spostati continuamente in avanti.

I livelli iniziali conosciuti vengono saltati; se nessun livello è sbloccato
si seleziona il primo del ramo. Le voci successive avanzano dallo step già
programmato, conservando anche gli upgrade intermedi. Un dato iniziale incerto,
un bivio o una famiglia ambigua restano irrisolti. Non viene eseguito il codice
del gioco: effetti scriptati arbitrari e scope non supportati non sono
qualificati come stato iniziale certo. Il contesto è riletto con il refresh.

Il catalogo comprende inoltre i nomi nazionali degli equipaggiamenti/tecnologie
e accetta abbreviazioni formate da parole iniziali complete, solo se univoche:
`Kb PPanc` corrisponde al nome polacco `Kb Ppanc wz. 35 Ur` di `infantry_at`.
`Heavy MG`, `Heavy Machine Gun` e `Heavy Machinegun` condividono la stessa
normalizzazione; l'italiano continua a essere un alias di presentazione.

Verifica successiva: **241 test Python e 154 JavaScript superati**. Importazione
HTTP del file Polonia revisionato: tutte le 45 ricerche associate, compresi
Support Weapons I-IV, i quattro avanzamenti di fanteria, Genieri II, contraerea,
Heavy MG e Kb PPanc. Una seconda validazione conserva gli stessi obiettivi.
Restano soltanto i due riferimenti focus Fulfill/Fulfil e Demand Slovakia già
segnalati. Salvata una nuova bozza locale; originale invariato e nessuna
installazione automatica. Nessuna nuova verifica su una partita live.

`Artillery UP`, `Tools UP` e le altre voci `Nome UP` indicano il prossimo step
del ramo, non un ID letterale o una voce incompleta. L’importatore percorre i
collegamenti tecnologici del catalogo. Nei bivi utilizza la continuità della
corsia GUI già letta dal gioco; se questa non distingue un unico successore
chiede una scelta, senza deviare verso un ramo diverso. Anche i miglioramenti
senza sblocco di un nuovo equipaggiamento contano come step.

Gli upgrade si riferiscono all’ultima voce della medesima famiglia nella
traccia, anche se altre ricerche sono intercalate. Si può dichiarare l’ancora
con `action.params.upgrade_from` (chiave di uno step precedente della stessa
traccia oppure ID della tecnologia di partenza). Se la prima voce è già UP,
`guide.research_baseline` può fornire il livello iniziale noto per famiglia.
Il livello di partenza assente richiede chiarimento; non è un’incertezza sul
significato di UP. I suffissi fra parentesi, come `(discount)`, non cambiano
il successore; le relative condizioni editoriali restano separate.

La conversione produce un ID concreto v1 e conserva `research_upgrade` con
query originale e tecnologia di partenza. Il runtime resta invariato: riconosce
gli obiettivi già ricercati, senza riassegnare un nuovo obiettivo ad ogni stato
live. Per ricalcolare un piano modificato si riparte dalla voce UP originale.
Fine ramo, bivi non risolti, precedenti irrisolti e cicli non vengono aggirati.

Test completi dopo questo incremento: **228 Python e 153 JavaScript superati**.
Coperti UP ripetuti/intercalati, base esplicita o assente, ramo principale e
laterali, livelli intermedi, fine ramo, target mancanti, ripetizioni cicliche,
idempotenza e riconoscimento dei livelli già ricercati.
Coperta anche la collisione fra nomi che condividono una parola ma appartengono
a rami distinti, come Construction e Aircraft Construction. Sul file ricevuto
sono risolti 8 UP con base presente nella sequenza; le prime voci UP senza base
e gli altri ID non risolti restano da qualificare. Originale non modificato.

- `import_sources.py`: acquisizione passiva, limiti espliciti, titolo, testo,
  metadati, checksum, segmenti temporali e avvisi. HTML statico, senza eseguire
  script. Rifiuta endpoint locali/privati, credenziali negli URL e redirect
  verso destinazioni non pubbliche; l’IP verificato è quello usato per la
  connessione, con verifica TLS sul nome originale.
- YouTube usa `HOI4_YTDLP`, `yt-dlp` nel PATH oppure il download Windows in
  `~/Downloads/yt-dlp.exe`. Configurazioni e plugin sono disabilitati. Nessun
  login, cookie, playlist, download audio/video, OCR o analisi dei fotogrammi.
  Se mancano i sottotitoli viene acquisita la descrizione e dichiarata la
  copertura parziale. Nessun testo viene troncato silenziosamente: oltre
  180.000 caratteri per fonte è necessario suddividerla.
- `guide_importer.py`: funzioni pure di risoluzione/validazione, adattatore IA
  opzionale e pubblicazione locale. ID o nomi normalizzati corrispondono solo
  se univoci; i risultati fuzzy sono suggerimenti. Gli ID ambigui o assenti
  bloccano l’installazione. Per i focus si rispetta lo scope dell’albero.
- Il catalogo copre focus e tecnologie compatibili caricati dal gioco. Il
  catalogo dimostrativo non qualifica gli ID del gioco reale. ID di progetti
  speciali, personaggi, stati, template e altre azioni operative non sono
  risolti automaticamente: i relativi parametri restano descrittivi/manuali.
  Un progetto nativo non risolto resta bloccato; può essere riscritto come
  istruzione manuale esplicita, conservandone la citazione.
- Ogni step richiede un riscontro presente nel testo della fonte;
  i timestamp devono corrispondere a un segmento acquisito. Checksum e citazioni
  attestano coerenza con il testo fornito, non autenticità della pagina né
  correttezza dell’interpretazione IA. Le informazioni solo visive possono
  mancare. La revisione umana e il confronto con la fonte restano necessari.
  Le osservazioni visive dichiarate da un’IA esterna sono ammesse con avviso
  esplicito; per YouTube richiedono un timestamp/segmento coerente. Non vengono
  presentate come frasi letterali del narratore o come osservazioni native.
  Le nuove importazioni usano passaggi operativi con evidenze; eventuali
  `milestones` legacy vanno convertite in questi passaggi. Le guide già incluse
  continuano a utilizzare la migrazione legacy esistente.
- Condizioni, cicli, riferimenti a fonti/fasi e dipendenze sono validati dal
  contratto v1 esistente. Nessun completamento viene dedotto da evidenze o
  dati assenti. I nuovi parametri non inventano nuove capacità live.
- Le guide installate sono in `.local/guides/<id>.json`, separate dalle guide
  incluse. Pubblicazione del documento completo e creazione esclusiva;
  stesso ID rifiutato, nessuna sovrascrittura o modifica della guida predefinita.
  Le revisioni possono usare un nuovo ID. Il catalogo le rileva senza riavvio;
  le checklist rimangono separate per guida/sessione come prima.

## API locali

| Metodo e percorso | Payload / risultato |
|---|---|
| `GET /api/import/status` | Stato provider e disponibilità yt-dlp, senza segreti |
| `GET /api/import/prompt` | Prompt autonomo, disponibile senza fonti o API IA |
| `GET /api/import/catalog` | Entità con nome, ID, alias e scope; indicatore demo |
| `POST /api/import/source` | `{url?,text?}` → fonte acquisita |
| `POST /api/import/document` | `{document}` → fonti canoniche e riferimenti rimappati del file IA |
| `POST /api/import/packet` | `{sources,tree_id?}` → pacchetto per IA esterna |
| `POST /api/import/extract` | `{sources,tree_id?}` → bozza/rapporto da provider |
| `POST /api/import/preview` | `{guide,sources}` → guida normalizzata, errori, avvisi, corrispondenze |
| `POST /api/import/install` | `{guide,sources,reviewed:true}` → ID, percorso e link walkthrough |

Le nuove POST accettano JSON fino a 2 MB e applicano i controlli d’origine
locali. Nessun percorso file scelto dalla richiesta. Un rapporto `valid:false`
è una risposta di revisione, non un’approvazione. L’installazione rivalida
l’intero documento e il catalogo: un’anteprima precedente non è un bypass.

## Verifiche del 22 ottobre 2026

- Suite complete: **215 Python e 151 JavaScript**, nessun test saltato.
- HTTP reale su fixture: acquisizione → pacchetto → risoluzione/anteprima →
  installazione temporanea → elenco guide/API guida; isolamento dalla guida
  inclusa, blocco overwrite, errore cross-origin e richieste malformate.
- Test delle corrispondenze, ambiguità, scope, cicli, citazioni inventate,
  timestamp, integrità fonti, metadata malformati e condizioni v1.
- Provider simulato: payload Responses, segreto non esposto, risposte incomplete,
  vuote ed errore 401. **Nessuna chiamata IA reale**, chiave e modello assenti.
- Controller JavaScript con DOM simulato: acquisizione, schede, revisione,
  esportazione/installazione e invalidazione di una risposta tardiva dopo edit.
  Non costituisce una verifica grafica nel browser.
- Acquisizione reale YouTube sul video `FBPdhlwqMwM`: descrizione 4.202 caratteri;
  sottotitoli non scaricabili, copertura segnalata come parziale. HTML reale
  su example.com: testo acquisito. Nessuna guida viene installata da queste prove.
- Catalogo del gioco reale: 7.007 entità, 43 alberi. L’esempio modifica il nome
  `Basic Machine Tools` nell’ID reale `basic_machine_tools`, con istruzione
  operativa mantenuta manuale. Nessuna nuova verifica live della partita.
- Browser locale indisponibile nella sessione: nessuna prova grafica dichiarata.

Log ed evidenze nella `.local` del progetto, prefisso `importer-`.
File e memoria di HOI4, acquisizione nativa e flusso live non modificati.

### Correzione del flusso principale: prompt autonomo

Il prompt di 14.584 caratteri è servito e scaricabile prima di acquisire fonti;
è incluso anche nel pacchetto installabile del progetto. La pagina mette prima
prompt e caricamento del file IA, conservando API e acquisizione nelle opzioni
aggiuntive. Il file autocontenuto senza hash è stato normalizzato e validato
via HTTP sul catalogo reale; non è stata installata una guida di prova.

Suite complete dopo la correzione: **219 test Python e 153 JavaScript superati**.
Verificati rimappatura degli ID fonte, rigetto di hash errati, importazione senza
acquisizione preliminare, copia/download del prompt nel controller simulato e
osservazioni visive dichiarate con avvisi e timestamp. La prova grafica nel
browser e una risposta generata da un’altra IA non sono state eseguite.
Evidenze: `.local/importer-prompt-verification.json` e log delle suite.

### Politica, eventi e metodi operativi (22 ottobre)

Il catalogo ora comprende leggi, idee, ruoli dei personaggi, eventi con opzioni
e decisioni. La risoluzione usa riferimento inglese e alias della localizzazione
installata. I tratti sono alias dei ruoli, non nomi di persone inventati.
ID espliciti sconosciuti non ricadono su un nome diverso. Nomi ambigui rimangono
da chiarire; uno scienziato non specificato non viene scelto automaticamente.

Il prompt scaricabile contiene questi parametri e `tracking:"reminder"` per i
consigli sul metodo. L'anteprima permette anche la conversione esplicita in
promemoria; quella scelta rimuove i vecchi criteri dalla copia modificabile,
senza toccare il file originale. I promemoria non sono azioni completate e non
richiedono checkbox. I criteri espliciti di una vera azione vengono preservati.

Il pacchetto IA opzionale include candidati politici menzionati nelle fonti e
opzioni dei relativi eventi, evitando di allegare tutti gli eventi mondiali.
Questa selezione riduce solo il contesto inviato: la validazione finale usa
sempre l'intero catalogo locale. Il flusso con prompt e file esterno resta
indipendente da Codex e dalle API IA.

Leggi/idee selezionate e incarichi attivi hanno membership qualificate live;
eventi mostrati e scelte consigliate hanno rilevamento distinto dall'esito.
Decisioni: identità associata, appartenenza al manager osservata; disponibilità,
avvio e completamento non ancora qualificati. Dettagli e limiti in
[events-live.md](events-live.md). Una legge rimossa o un incarico cambiato non
provano una scelta passata: lo stato attuale non è una cronologia persistente.
Il runtime non ricostruisce retroattivamente le azioni compiute prima della
lettura, e non deduce una risposta dalla chiusura dell'evento.
