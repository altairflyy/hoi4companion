# Eventi, politica e promemoria

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
