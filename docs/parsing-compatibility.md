# Compatibilità CWTools/Jomini — 24 ottobre 2026

**Aggiornamento:** i 31 casi residui descritti sotto ora hanno un
[recupero verificato in memoria](source-recovery.md), con hash e AST attesi.
Il riepilogo seguente documenta la fase precedente al recupero strutturale.

L’analisi reale è passata da **46 a 31 errori di parsing**, con **zero disaccordi
residui fra i due parser**. File e memoria di HOI4 non sono stati modificati.

| Causa dei 46 casi iniziali | Esito |
| --- | --- |
| 13 script accettati da Jomini ma rifiutati da CWTools | Compatibilità lessicale corretta |
| 2 documenti di testo scambiati per script | Classificati come documentazione |
| 29 file con blocchi aperti non chiusi | Rifiutati da entrambi; errore conservato |
| 1 file con una parentesi chiusa in eccesso | Rifiutato da entrambi; errore conservato |
| 1 assegnazione senza valore | Rifiutata da entrambi; errore conservato |

CWTools interpretava `RGB` e ID come `RGB_democratic_RU_Gen` come introduzioni di
un colore. Non accettava inoltre alcuni caratteri Unicode, virgolette interne
a token non quotati e commenti dopo `=`. Il tokenizer Jomini ora prepara in
memoria una rappresentazione equivalente: quota i token opachi e sostituisce
con spazi solo quei commenti. Mantiene le righe e traduce le colonne delle
diagnosi nelle coordinate originali. I colori `RGB { ... }` e `hsv 360 { ... }`
restano colori. Non si aggiungono valori o parentesi.

Il confronto sull’installazione reale ha verificato **valori e struttura Jomini
identici in tutti i 52 file adattati**. Questi includono i 13 script che prima
fallivano in CWTools e altri file con gli stessi costrutti. Le licenze dei font
sotto `gfx/fonts/` e `interface/credits.txt` restano indicizzate come risorse
documentali: cinque documenti complessivi, due dei quali producevano errori.

## Casi residui

Nei file di alcuni vagoni il blocco `state` o `entity` non è chiuso;
`gfx/particles/particle_repository.txt` contiene `force =` senza valore;
`common/national_focus/TSR_lingguang_incident_joint_branch.txt` ha una chiusura
senza apertura. Questi e gli altri casi sono rifiutati da entrambi i parser.
Ciò non dimostra che ogni sezione venga usata dalla partita o che il parser
interno del gioco la gestisca allo stesso modo. Non è stata ricostruita una
struttura presunta per far apparire riuscita la verifica.

`parser_error_details` espone classificazione (`rejected_by_both` oppure
`parser_disagreement`), errore Jomini, posizione e SHA-256 del file originale.
`input_adjustments` documenta gli adattamenti; `non_script_files` elenca i
documenti. La pagina Analisi distingue questi casi. Un file non interpretabile
può ancora causare falsi positivi sui riferimenti alle sue definizioni.

## Registro dei sorgenti accantonati

I file classificati `rejected_by_both` vengono saltati dall’analisi senza
interrompere gli altri documenti e senza comparire come avvisi ordinari.
Le divergenze tra parser rimangono segnalate. Questo non certifica un difetto
del gioco: registra solo che entrambi i parser non interpretano il sorgente.

L’applicazione salva automaticamente `.local/logs/source-errors.json`, accanto
alle impostazioni locali. Il JSON versionato conserva percorso effettivo
(anche per le mod), posizione, messaggi dei due parser, SHA-256, revisioni dei
componenti, prima/ultima osservazione e versioni storiche senza duplicati.
Non contiene copie degli asset. Il file è locale e ignorato da Git.

Ogni nuova analisi riprova tutti i file: non esiste una blacklist permanente.
Le versioni precedenti dei percorsi nuovamente analizzati diventano inattive;
i record di altre installazioni restano conservati. `active` descrive l’ultima
analisi di quel percorso, non uno stato monitorato live. Un registro corrotto
non viene sovrascritto; un errore di scrittura appare negli avvisi. Scrittura
atomica e controlli della generazione impediscono di pubblicare un’analisi
superata. Le risposte in cache non riscrivono il registro.

Il contratto mantiene `coverage.parser_errors` e `parser_error_details` per
compatibilità; aggiunge `coverage.ignored_parser_errors`,
`ignored_source_errors` e `source_error_log`. La vista distingue il numero
accantonato dagli errori da indagare. La copertura resta parziale: non si
inventano definizioni dai file esclusi e non cambia la valutazione live.

## Verifiche

- Registro: 426 test Python e 237 JavaScript superati, compresi deduplicazione,
  versioni storiche, scrittura fallita, generazione superata e riepilogo UI.
  Rust non modificato in questo intervento; nella verifica precedente erano
  passati 17 test Rust e Clippy senza avvisi.
- Analisi completa: 5.601 documenti e 153.536 definizioni tipizzate, circa 45 s
  nell’ultima misura; risultati in cache.
- SHA-256 dei 46 file iniziali identici prima e dopo l’analisi.
- Wheel ricostruito da sdist e verificato in ambiente isolato senza Rust/.NET
  nel PATH. La validazione continua a segnalare un booleano errato dopo la
  normalizzazione: non è stato disattivato il controllo semantico.
- Server sulla porta 8772: configurazione e payload dell’avanzamento invariati
  al riavvio, stessa campagna/guida della Polonia, live e SSE
  `observed`/`usable: true`. Nessuna verifica visiva del browser in questa sessione.
