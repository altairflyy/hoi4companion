# Produzione live e Guide Schema v1

Il collegamento usa il lettore `CountryReader.production` esistente, senza nuove
letture native né scritture al gioco. Per ogni linea rimangono separati tipo nativo
(`equipment`), nome del modello, categoria military/naval, fabbriche richieste,
assegnate, attive e danneggiate. Le assegnate includono le danneggiate; il solo
conteggio richiesto non soddisfa l'assegnazione. L'uscita giornaliera resta teorica.

## Catalogo e importazione

`operation_catalog` legge `common/units/equipment` con gli overlay attivi e le
localizzazioni inglesi/locali. Gli archetipi espongono gli ID concreti della
famiglia; i nomi nazionali sono usati soltanto nel paese dichiarato dalla guida.
Nessuna regola dipende da Polonia o da una guida specifica.

Input in `action.params`:

- `category: production`, `equipment_id` nativo oppure `equipment_name` univoco;
- `variant_name` facoltativo: restringe il modello, senza accettare nomi simili;
- `assigned_factories`: obiettivo assoluto; `assignment_test` eq predefinito o gte;
- `quantity_mode: additional`: incremento rispetto alla baseline del run;
- `production_category`: military/naval facoltativo.

Il compilatore produce `production_selector` con `equipment_ids`, `variant_names`
e `category`, più condizioni v1 sui fatti calcolati riservati
`guide.production.<step>.assigned`, `.added` e `.started`. Un modello custom può
essere riferito da altre istruzioni quando la guida dichiara una sola coppia
tipo/modello compatibile. Ambiguità e ID espliciti invalidi non vengono risolti
per somiglianza. Le condizioni manuali esplicite sono conservate. Una rivalidazione
rimuove i collegamenti generati diventati obsoleti dopo la modifica dei parametri.

## Qualificazione e runtime

Il nuovo fatto `production.lines` è una copia qualificata della lista completa.
Richiede sezione observed, paese corrispondente, ID linea unici e contatori
coerenti: `active + damaged == assigned <= requested`. Sezione partial, stale,
campi mancanti o contatori incoerenti producono unknown, mai una lista vuota.
Una lista completa vuota o senza corrispondenze produce invece un totale noto 0.
Questa distinzione è condivisa da Python e JavaScript.

Il runtime puro seleziona tipo/modello/categoria e somma tutte le linee compatibili.
Gli ID di linea sono indici di enumerazione, mai identità persistenti: riordinare
le linee non cambia la corrispondenza. I totali sono calculated; nessun totale
derivato è presentato come una nuova osservazione nativa.

Per un obiettivo assoluto, una richiesta o assegnazione positiva indica attività;
il completamento dipende soltanto dalle assegnate. Per un incremento, RunStore
registra la baseline quando dipendenze e condizioni consentono l'istruzione e non
ci sono assegnazioni pendenti (`requested == assigned`). Una richiesta superiore
alla baseline segnala avvio, ma il completamento richiede che le assegnate siano
almeno baseline + incremento. Baseline di selettori diversi non sono intercambiabili.
Le richieste successive sullo stesso selettore non riutilizzano lo stesso incremento.

La baseline usa il contratto opzionale `OperationBaseline` del progresso v1;
`ConstructionBaseline` rimane un alias compatibile. È conservata solo in memoria
come gli altri progressi. La guida non contiene dati della partita. Una nuova
checklist/sessione isola i progressi precedenti. Non si ricostruisce una baseline
da richieste già pendenti. L'identità della campagna conserva i limiti del runtime.

## Interfaccia e limiti

La vista Nazione aggiunge attive e danneggiate; il walkthrough mostra per ogni
istruzione le linee corrispondenti, i contatori e il target, senza pulsanti manuali
quando il criterio è automatico. L'importatore spiega separatamente ID/modello
ambiguo, quantità assente, MIO non osservato e criterio manuale esplicito.

MIO, moduli/ruoli del designer e generici nomi SPG/Flame Tank non diventano
osservabili dalla sola assegnazione. Modelli custom devono essere identificati
con tipo nativo e nome effettivo. Istruzioni con più modelli richiedono step distinti.
Il runtime ricalcola la conformità corrente: spostare fabbriche può rendere nuovamente
incompleto un vecchio obiettivo assoluto. Non viene inventato uno storico di azioni.
L'aggiornamento usa il flusso SSE esistente e non aggiunge polling del gioco.

## Verifiche del 22 ottobre 2026

- 276 test Python e 194 JavaScript superati, nessun test saltato; 22 fixture
  produzione e 13 costruzioni con output completo identico tra i due runtime.
- HTTP reale con lettore simulato: stessi contatori in API stato/runtime,
  assegnazione parziale, completamento e rimozione, nuovo modulo JS servito.
- Browser reale su fixture isolata: 3 richieste/1 assegnata resta in corso;
  3 assegnate (2 attive + 1 danneggiata) completa l'obiettivo senza pulsanti
  manuali, con eventi mentre il resto della partita simulata è invariato.
- Campione reale della Polonia: lette 3 linee (fanteria 2 fabbriche, supporto 1,
  telaio piccolo interbellico 1); gli stessi conteggi passano nei nuovi criteri.
  Vista Nazione verificata con modelli, tipi localizzati e quattro contatori.
  Nessuna assegnazione reale è stata modificata per provare una transizione.
- Guida revisionata validata e ricompilabile: 15 operazioni produzione automatiche,
  1 con solo avvio e 6 non collegate (MIO, SPG/Flame Tank e modelli custom non
  identificabili). Il totale delle operazioni guida diventa 20 automatiche,
  2 con solo avvio e 112 con conferma. Originale preservato, nessuna installazione
  di prova nella raccolta dell'utente.

Evidenze in `.local/production-verification.json`, rapporti importazione, log delle
suite e screenshot `production-live-browser.png` / `production-browser-completed.png`.
Il server fixture è stato arrestato dopo le prove.
