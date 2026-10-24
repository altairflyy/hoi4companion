# Cataloghi tematici per l'IA esterna

La pagina Importa guida permette di scegliere una sezione e scaricarla come JSON.
Il catalogo completo resta un'alternativa: allegarlo insieme alle sezioni
duplicherebbe gli identificatori forniti al modello.

| `catalog_scope` | Tipi di entità |
| --- | --- |
| `focus` | `focus` |
| `research` | `technology` |
| `production` | `equipment` |
| `construction` | `building`, `state` |
| `projects` | `project` |
| `politics` | `law`, `idea`, `advisor`, `decision` |
| `events` | `event`, `event_option` |
| `other` | Eventuali tipi futuri non riconosciuti |

Gli export dichiarano `catalog_schema_version:1`, `reference_language:"english"`
e un array `catalog`. Le entità e i campi originali vengono preservati, inclusi
nomi localizzati, alias, prerequisiti, successori e collegamenti `event_id`.
I riferimenti fra sezioni restano ID, senza copiare l'entità referenziata.
La partizione completa assegna ogni voce a una sola sezione; non modifica il
catalogo interno, la risoluzione degli ID o le guide già importate.

Il filtro dell'albero nazionale limita i focus e, dai relativi `countries`,
filtra anche eventi e risposte. La proiezione `country_condition` conserva
solo confronti espliciti del tag destinatario, AND/OR, NOT semplice e costanti;
condizioni dinamiche, scripted trigger, tag storici e cambi di scope restano
sconosciuti. L'export elimina soltanto risultati certamente falsi per tutti
i paesi associati all'albero. In particolare, il tag di un paese annidato nel
trigger non è confuso con il destinatario. News e `major` conservano anche
la possibilità di ricezione tramite `show_major`. Definizioni ripetute vengono
combinate per unione per evitare esclusioni basate sulla sola prima variante.
Le opzioni conservate fanno sempre riferimento a un evento conservato.

Il filtro è statico e parziale: non prevede le finestre di una campagna né
garantisce una lista esaustiva di soli eventi realmente raggiungibili. Non usa
prefissi di namespace o nomi di file come prova della nazione destinataria.
`country_filter` dichiara questa limitazione, il numero di eventi esclusi e
quelli rimasti sconosciuti. Cataloghi precedenti senza metadati non vengono
filtrati silenziosamente: la UI mostra un avviso evidente e chiede di riavviare
per aggiornare il catalogo. Finché mancano quei metadati, gli export di eventi
per nazione e del catalogo completo per nazione sono sospesi. Gli export focus
e altre sezioni restano disponibili, così come quelli globali senza filtro.
Il catalogo interno e i binding delle guide rimangono completi; il filtro
agisce sulle esportazioni tematiche e complete quando è selezionato un albero.
La sezione politica applica lo stesso criterio prudente a leggi, idee,
consiglieri e decisioni. Le condizioni nazionali sono compilate dai blocchi
`allowed`, `visible`, `available`, includendo il gruppo di idee, la definizione
del personaggio e dell'incarico, e la categoria delle decisioni. I file di
categorie non sono esportati come se contenessero decisioni selezionabili.
`original_tag` è mantenuto distinto da `tag`: per questa sezione il filtro
assume esplicitamente l'origine della nazione selezionata, dichiarandola con
`original_tag_basis:"selected_nation"`. In assenza di tale assunzione il test
del tag originale rimane sconosciuto. Non vengono inferite condizioni da nomi
di file o prefissi degli ID, né da `target_trigger` che riguarda un altro paese.
Le costanti di disponibilità (`always=no` compreso) restano sconosciute nella
proiezione politica per non eliminare leggi e spiriti assegnabili via script.
Il filtro non certifica disponibilità, nomina o possesso attuali. I metadati
riportano voci escluse e sconosciute; l'export completo contiene anche
`section_country_filters` per distinguere i filtri eventi e politica.
Gli altri temi restano globali. Nessun filtro è applicato al catalogo interno,
alla risoluzione delle guide o all'acquisizione della memoria.

Nella sezione eventi,
il filtro predefinito conserva eventi con almeno due ID di risposta distinti
e le relative risposte, con metadati `event_filter`. Disattivarlo restituisce
la sezione completa per il filtro nazionale corrente, inclusi gli eventi a
risposta unica. Senza albero selezionato vengono esportati tutti i destinatari.
Le risposte definite possono essere nascoste da condizioni della partita.

Il prompt scaricabile indica quali sezioni consultare per ciascuna fase e come
riunire gli step in un solo file guida. Il partizionamento permette allegati
più piccoli; non controlla quali allegati già caricati entrino nel contesto del
servizio IA esterno. Per limitarli davvero, fornire solo le sezioni necessarie
alla fase corrente.

Verifica del 24 ottobre 2026: tutte le 42.388 voci del catalogo locale sono
assegnate una sola volta, senza perdite. Tecnologie: 465 voci; equipaggiamenti:
360; progetti: 49. Selezionando `german_focus`, la sezione focus passa da 7.510
a 438 voci. Le quantità dipendono da installazione, DLC e mod.
Test di modello e controller verificano la partizione, tipi futuri, collegamenti,
filtro focus, filtro eventi e conservazione dell'export completo. Verificata via
HTTP la disponibilità della pagina, degli script e del prompt aggiornati;
nessuna prova visiva interattiva dichiarata.

Verifica del filtro nazionale: 538 test Python e 250 JavaScript superati,
inclusa la valutazione JS di condizioni compilate da Python. Scansione reale
dei file installati in sola lettura: 6.967 eventi; per GER esclusi 127,
conservati 6.840, di cui 6.156 con destinatario sconosciuto. Applicando anche
il filtro risposte multiple restano 3.090 eventi e 7.462 risposte. Non è una
lista esatta di eventi che appariranno: per ridurre gli sconosciuti servirebbe
anche analizzare le catene di chiamata da focus, decisioni e scripted effects.
L'export di controllo è stato costruito offline con il catalogo reale e la
nuova proiezione; il servizio già aperto richiede un riavvio per caricare i
metadati. Nessuna acquisizione della memoria o prova di apparizione in gioco
è stata eseguita per questo filtro.

I contatori del menu e del riepilogo mostrano gli stessi dati del file
scaricato: gli eventi sono contati separatamente dalle risposte, dopo entrambi
i filtri. La voce generale delle entità caricate è invece il catalogo intero.
L'avviso sul vecchio catalogo non viene confuso con un albero dalla nazione
sconosciuta. Le fixture coprono anche il ritorno all'export globale.

Attivazione successiva verificata sul servizio 8766, riavviato dopo esplicita
autorizzazione dell'utente: tutti gli eventi hanno i nuovi metadati nazionali.
Con risposte multiple attive, il servizio e il modello di export producono
3.193 eventi / 7.832 risposte senza nazione e 3.090 / 7.462 con `german_focus`.
Configurazione e avanzamento invariati; confronto in sola lettura del database
con backup precedente al riavvio: tutti i run conservati, nessun campo cambiato.
Suite dopo la correzione dei contatori: 538 Python e 252 JavaScript superati.

Estensione politica verificata sul servizio riavviato: 539 test Python e 255
JavaScript superati, più 14 test mirati dopo la correzione del riconoscimento
dei percorsi relativi delle categorie. Le proiezioni Python delle condizioni
politiche sono valutate anche dal filtro JavaScript sulle stesse fixture.

| Tipo | Catalogo completo | Germania |
| --- | ---: | ---: |
| Leggi | 33 | 31 |
| Idee e spiriti | 6.475 | 2.617 |
| Consiglieri e incarichi | 3.759 | 252 |
| Decisioni | 4.148 | 960 |
| Totale | 14.415 | 3.860 |

Escluse 10.555 voci incompatibili; delle rimanenti, 3.796 hanno ancora
applicabilità incerta e sono conservate. Rimossi cinque falsi ID derivati dai
controlli delle categorie di decisioni, non da decisioni effettive. Le quantità
dipendono dall'installazione. Verificati via HTTP il catalogo con metadati, il
prompt e il runtime della guida; controllati configurazione, conferme, ordini
avviati, identità della campagna e conservazione dei run dopo il riavvio.
Nessun file del gioco o sistema di acquisizione è stato modificato.
