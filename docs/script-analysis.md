# Analisi statica degli script con regole CWTools

Aprire **Analisi script** dalla navigazione oppure `/analysis`. Il controllo è
facoltativo e non aggiunge conferme al walkthrough, non modifica le guide e non
determina disponibilità o completamenti live. Nessun file o memoria del gioco
viene modificato; non sono eseguiti trigger, effetti o comandi.

## I quattro controlli

1. **Riferimenti:** indice delle definizioni effettive di focus/shared/joint,
   tecnologie, personaggi, idee/token dei consiglieri, decisioni, eventi, regioni,
   progetti ed edifici. Include tag e alias dei tag nazionali. Controlla i
   prerequisiti/esclusioni dei focus e i riferimenti tipizzati delle regole
   supportate. Un alias nazionale è un'identità valida, non prova del paese a cui
   si risolverà nella partita.
2. **Contesti:** verifica gli scope dichiarati dei trigger/effetti e le
   transizioni `owner`, `controller`, `capital_scope`, tag e regioni. Segue
   `ROOT`, `THIS` e `PREV`; `FROM` senza evidenza rimane sconosciuto. I contenitori
   con destinazione non supportata vengono analizzati con contesto sconosciuto.
3. **Parametri:** valida tipi scalari/blocchi, booleani, numeri/range, enum e
   riferimenti coperti. Le forme alternative vengono provate separatamente:
   diagnosi di una forma scartata non devono contaminare quella selezionata.
   La pagina consente anche di consultare nomi, contesti e schemi delle varianti.
4. **Localizzazioni:** controlla chiavi implicite delle entità, chiavi note,
   riferimenti `$KEY$` raggiungibili e cicli. Segnala un fallback quando una
   chiave manca nel dizionario visualizzato ma esiste nel riferimento inglese.
   Il caricatore attuale fonde già inglese e traduzione: il controllo non prova
   quindi che ogni testo sia stato tradotto in italiano. Una stringa esplicita
   non risolta può essere testo inline; senza evidenza delle virgolette resta
   non verificabile, evitando falsi errori su nomi come quelli delle agenzie.

Le diagnosi hanno `schema_version: 1`, `source: static_analysis` e
`runtime_evaluated: false`. I contatori separano riferimenti, contesti, parametri
e localizzazioni. Non esiste un esito «tutto valido»: lo stato è `partial`.

## Regole e dipendenze

Sono inclusi sottoinsiemi dei file originali del progetto
[cwtools-hoi4-config](https://github.com/cwtools/cwtools-hoi4-config), revisione
`297dd69c42aabe10078a8b7ab7f2d07ee631d4ef`, con licenza MIT e attribuzione originali.
`rules/cwtools/provenance.json` elenca SHA-256 di ogni file: il caricamento rifiuta
regole mancanti o alterate. Non si scaricano aggiornamenti a runtime.

L'adattatore `cwtools-subset-v2` usa le forme dichiarate nei `.cwt` e gli scope
del `script_documentation.json` dello stesso repository/revisione. Gli atomi
CWT, compresi `<type>`, vengono racchiusi lessicalmente tra virgolette in memoria
prima di passare a **Jomini**: non è stato introdotto un secondo parser di
contenitori. I commenti/opzioni CWT non vengono eseguiti o reinterpretati come
regole complete. Il catalogo include 615 nomi di trigger, 573 effetti e 17 link.

È un adattatore delle regole, **non l'integrazione dell'intero motore CWTools**.
Non richiede .NET o VS Code. Il codice del controllo è un modulo del framework
Python; il parser resta Rust/Jomini. La compatibilità completa del ruleset con
la build HOI4 installata non è certificata.

## Limiti espliciti

- Cardinalità/obbligatorietà, sottotipi complessi, `single_alias`, value set,
  enum dinamici e tipi non indicizzati possono restare non supportati.
- Un campo non coperto dal sottoinsieme CWT è registrato nella copertura, non
  come errore certo del gioco o della mod. Le variabili/espressioni dinamiche
  non diventano valori inventati.
- Le chiamate a scripted trigger/effect vengono riconosciute nell'indice; non
  sono espanse automaticamente nel contesto del chiamante. Le definizioni sono
  analizzate separatamente con contesto sconosciuto.
- Un file non leggibile rende incompleti solo i namespace che potrebbe definire.
  In quel namespace l'assenza di un ID non è prova che l'ID non esista.
- I testi non sono un emulatore delle localizzazioni dinamiche e delle stringhe
  bindable del gioco. Si usano i dizionari già caricati dal Companion.
- Si analizzano le directory elencate in `script_analysis.DIRECTORIES`, non ogni
  tipo di file Paradox. L'ordine delle mod e `replace_path` usano il caricatore
  esistente, senza alterare la selezione del launcher.

Le prime 2.000 diagnosi sono restituite; `truncated` indica l'omissione delle
successive e i totali comprendono tutte le segnalazioni. I casi non verificabili
sono raggruppati nei limiti di copertura, non moltiplicati per ogni occorrenza
nell'elenco principale. Gli avvisi del ruleset non equivalgono a problemi che
l'utente debba correggere nei file del gioco.

## API e costi

- `GET /api/script-analysis`: analisi della configurazione corrente, in cache.
- `GET /api/script-analysis?refresh=1`: ripete la lettura statica dal disco.
- `GET /api/script-rules?kind=trigger&name=has_completed_focus`: definizioni
  consultabili; `kind` può essere `trigger` o `effect`.

«Ricarica» invalida la cache insieme ai cataloghi. Il controllo viene eseguito
solo su richiesta e non nel ciclo live; la sua elaborazione non mantiene il lock
condiviso con le API live. Una ricarica simultanea impedisce di pubblicare un
report relativo ai cataloghi precedenti. Non viene cambiato il sistema di
acquisizione, il polling o la persistenza delle campagne.

Le prove comprendono fixture deterministiche delle quattro categorie, overload,
scope sconosciuti, tag alias, namespace incompleti, testi ciclici, integrità delle
regole, cache/API e rendering DOM testato con Node. Il browser Work disabilitato
impedisce una nuova verifica visiva: le verifiche HTTP/DOM non sono screenshot
o prova dell'overlay in gioco.

## Qualificazione locale del 23 ottobre 2026 (adattatore v1)

- **405 test Python e 235 JavaScript superati**. Il parser Rust non è stato
  modificato in questa fase.
- Analizzati **2.013 file e 38.209 definizioni** dell'installazione e del playset
  locali; 237.721 istruzioni riconosciute. Il report riporta separatamente
  190.250 controlli non verificabili e 48.545 occorrenze con contesto sconosciuto:
  questi conteggi non sono percentuali di copertura né errori da correggere.
- 3.428 avvisi statici, di cui 130 sui riferimenti, 238 sui contesti, 334 sui
  parametri e 2.726 sulle localizzazioni. Sono suggerimenti del sottoinsieme di
  regole, non problemi del gioco confermati singolarmente. La pagina mostra
  l'eventuale troncamento dell'elenco. I namespace focus/shared/joint e idee
  risultano incompleti per file originali non leggibili: i loro ID assenti non
  vengono dichiarati inesistenti.
- **55 risposte API identiche** tra istanza precedente e nuova: configurazione,
  cataloghi, 43 alberi nazionali e 9 schede tecnologiche. Nessuna regressione
  osservata nei contratti confrontati.
- Sdist e wheel Windows CPython 3.13 costruite; installazione separata con regole,
  manifest e licenza presenti e verificati. Il controllo funziona senza Rust nel
  PATH, senza override e senza dipendenze .NET aggiuntive.
- Dopo il riavvio su **8772**: API live e SSE `observed`/utilizzabili, pagine e
  risorse HTTP 200, runtime identico, stessi run/campagna/checklist/avvii/storia e
  intero payload SQLite identico al backup precedente al passaggio.

L'analisi completa ha richiesto 66,76 s in una prova concorrente con altri lavori;
avvio e preanalisi della nuova istanza 122,10 s complessivi. La prima analisi può
quindi richiedere circa un minuto. Una richiesta del report già in cache ha
impiegato 65 ms nella verifica HTTP. Sono singole misure locali, non benchmark
controllati; l'analisi rimane fuori dal ciclo live e non si esegue all'avvio
ordinario del programma. Solo il launcher locale usato per il passaggio ha
precalcolato il report prima di rendere attiva l'istanza.

Evidenze locali escluse da Git: `.local/cwtools-real-analysis-final.json`,
`.local/cwtools-api-parity.json`, `.local/cwtools-production-verification.json`,
`.local/cwtools-package-smoke.json` e log delle suite `.local/cwtools-*-tests.log`
e `.local/cwtools-python-final.log`. Backup:
`.local/backups/guide-progress-before-cwtools.sqlite3`.

## Correzione dei falsi positivi (adattatore v2)

I contatori sono avvisi statici dell'intera installazione, non errori confermati
né operazioni della guida. I controlli riconosciuti, non verificabili e con
contesto sconosciuto sono misure sovrapposte: non vanno sommati per calcolare
una percentuale di copertura. La pagina ora lo esplicita.

La verifica degli esempi reali ha riprodotto e corretto:

- `66.owner` e `DEN.hosts_danish_brigades`, prima confusi con tag letterali
  inesistenti. Le espressioni composte negli scope restano non verificabili;
  il controllo sui tag letterali assenti rimane attivo.
- La scelta dell'overload quando tutte le forme hanno un problema. Un blocco
  `country_event={id=... days=90}` veniva presentato come «atteso uno scalare»
  perché veniva scelta la variante scalare. Ora viene scelta la forma a blocco
  e riportato il vero conflitto con l'intervallo CWT `days=1..31`. È una
  discordanza delle regole da verificare, non prova che l'evento del gioco sia
  errato. Gli intervalli CWT non sono stati rimossi o allargati arbitrariamente.
- I sottotipi di evento dichiarati (`event.country_event`, ecc.) vengono
  verificati nel rispettivo indice. Un ID evento effettivamente assente rimane
  una segnalazione, anche quando la forma a blocco è valida.
- I focus con `text=...` usano quella chiave per nome e descrizione. Gli esempi
  reali comprendono `USA_CAS` → `CAS_effort` e `uk_industrial_focus` →
  `industrial_effort`; l'assenza del nome sotto l'ID originale non è un errore.

Superati **410 test Python e 235 JavaScript**, inclusi cinque nuovi casi di
regressione. Nessuna modifica a cataloghi pubblici, lettore live o file HOI4.

La nuova analisi reale conserva 237.721 istruzioni riconosciute e 48.545 contesti
sconosciuti; i controlli non verificabili passano da 190.250 a **183.141**.
Gli avvisi sui riferimenti scendono da 130 a **89**, quelli sulle localizzazioni
da 2.726 a **2.722**; restano **238** avvisi sui contesti e **334** sui parametri,
ora con distinzione tra tipo e intervallo CWT. Totale 3.383 avvisi: non sono
stati confermati singolarmente come difetti del gioco. I conteggi v1 sopra
restano come evidenza storica, non come risultati correnti.

L'istanza 8772 usa l'adattatore v2. Verificati stato live/SSE osservati e
utilizzabili, runtime identico, stessi identificativi, checklist, avvii, storia
e payload SQLite interamente identico al backup del passaggio. La wheel v2 è
stata costruita, installata separatamente e provata sul caso `text=...`.
Evidenze locali: `.local/cwtools-v2-analysis.json`,
`.local/cwtools-v2-production-verification.json`,
`.local/cwtools-v2-python.log`, `.local/cwtools-v2-javascript.log`.
