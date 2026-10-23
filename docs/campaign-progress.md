# Avanzamento persistente e identità della campagna

Il Guide Runtime resta deterministico e non accede al disco o al lettore. Il
server associa l'avanzamento a **guida, campagna nativa e nazione** e lo salva
automaticamente in `.local/guide-progress.sqlite3`, accanto alle impostazioni.
Le guide v1 e il prompt da fornire all'IA non richiedono modifiche.

## Dati conservati

- Conferme manuali, compresi i valori espliciti `false`.
- Avvii dichiarati dall'utente e conteggi iniziali di costruzione/produzione.
- Checklist attiva, checklist precedenti e associazioni alle campagne/nazioni.
- Sessione tecnica, revisione della checklist e ultimo tempo di gioco
  registrato insieme a un aggiornamento dell'avanzamento.

I completamenti, le code, le leggi e le altre osservazioni live **non vengono
trasformati in uno storico di azioni**: continuano a essere riletti e valutati.
La definizione della guida resta separata dal suo avanzamento.

SQLite effettua transazioni con `synchronous=FULL`. Una scrittura fallita non
viene presentata come riuscita e ripristina anche lo stato precedente in memoria.
Versione e contenuto dell'archivio vengono validati al caricamento. Un archivio
corrotto o futuro viene conservato; l'interfaccia segnala il problema e blocca le
modifiche dell'avanzamento, senza sovrascriverlo con una checklist vuota.

Le scritture avvengono quando cambiano progressi o associazioni, non a ogni
controllo della memoria o ora di gioco. Due processi Companion che aprono lo
stesso archivio non possono sovrascrivere silenziosamente snapshot differenti:
l'istanza rimasta indietro deve essere riavviata. Per il normale utilizzo mantenere
un solo server per checkout; più schede dello stesso server sono supportate.

## Identità osservata

La build Windows già supportata conserva in `CCurrentGameState`:

- `game_unique_seed`, offset `+0x720`;
- `game_unique_id`, UUID stringa a `+0x728`.

Il campo pubblico è `sample.campaign_identity` con stato, ID e provenienza.
L'ID ha formato `hoi4:<uuid>:<seed-esadecimale>`; l'adattatore espone il fatto
osservato `campaign.id`. Il tag nazionale viene controllato separatamente.
PID, indirizzi, data e nazione da soli non costituiscono un'identità di campagna.

La lettura usa gli stessi permessi, witness e piano di controllo esistenti.
UUID, header, seed e token vengono controllati anche a tick invariato. Una
lettura incoerente non autorizza un ripristino. Se l'identità non è qualificata,
l'avanzamento già associato a una campagna è sospeso e non è modificabile;
gli altri domini live mantengono i propri controlli.

L'estensione del lettore è limitata a questi campi, necessari perché il payload
precedente conteneva soltanto l'identità tecnica del processo/mondo/nazione.
Nessuna modifica a memoria, file di HOI4, salvataggi, mod o comandi del gioco.

## Comportamento nell'applicazione

- **Riavvio di Companion:** ripristino automatico della checklist della stessa
  campagna e nazione, inclusi i conteggi iniziali, senza nuove conferme.
- **Riavvio di HOI4:** una campagna identificata viene riassociata alla nuova
  sessione tecnica; non si riutilizzano indirizzi o handle del processo precedente.
- **Altra campagna, anche stessa nazione/data/processo:** checklist separata.
  Tornando a una campagna conosciuta si ritrova il suo avanzamento.
- **Cambio nazione:** avanzamento separato anche all'interno della stessa campagna.
- **Tempo di gioco retrocesso:** se precedente all'ultima osservazione in memoria
  o al checkpoint di avanzamento salvato, viene creata una checklist nuova;
  quella futura resta nello storico. Non se ne trasferiscono conferme o baseline.
- **Nuova checklist:** conserva la precedente, con possibilità di annullamento
  e selezione nello storico della campagna. Una checklist di un'altra campagna,
  nazione o di un tempo futuro non può essere attivata sulla partita corrente.

Il walkthrough mostra **Campagna … · Avanzamento salvato su questo PC** e
**Checklist salvate per questa campagna**. `Ricarica storico` aggiorna l'elenco.
I reset automatici precedentemente eseguiti da ogni browser sono stati rimossi:
la riconciliazione è unica, nel server. Le schede riconoscono anche il riavvio del
server e le richieste rimaste indietro vengono respinte con un errore esplicito.

Le API precedenti restano disponibili. `/api/guide/progress` aggiunge metadati
di campagna, `run_revision`, `server_id`, `history` e `storage`. Le scritture della
nuova interfaccia inviano `expected_run_id`, `expected_revision` e `server_id`;
il client non può impostare campagna, conteggi iniziali o revisione dell'archivio.
SSE trasporta gli stessi metadati, senza un nuovo ciclo di acquisizione.

## Evidenze e verifiche del 23 ottobre 2026

REA `open_binary` ha confermato il PE x86-64 installato e l'hash supportato
`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`.
La ricerca approfondita non ha fornito un risultato utilizzabile; non viene
dichiarata una decompilazione riuscita. L'associazione dei campi è stata verificata
offline sul PE con Capstone e poi con letture runtime in sola lettura:

| Riscontro | Evidenza |
| --- | --- |
| Token UUID | Registrazione stringa `game_unique_id`, token inizializzato `0x3B80` a RVA `0x3423158` |
| Scrittura nel formato del gioco | Serializer RVA `0x1F2F19` per seed e `0x1F2F2D` per UUID |
| Lettura del formato del gioco | Parser RVA `0x1E8C6E`, stesso offset `+0x728` |
| Partita reale | UUID e seed validi nella campagna POL, identici in due acquisizioni consecutive |

Il record attivo precedente è stato esportato e migrato, mantenendo il suo
`run_id`. Backup locale: `.local/progress-migration-backup-20261023T073535Z.json`.
Due riavvii reali del server hanno mantenuto ID campagna, run e contenuti
dell'avanzamento. L'API espone archivio persistente, nessun errore e runtime
`progress_status: accepted`; server ID differenti provano il riavvio.
In questa checklist conferme/avvii/baseline erano vuoti: il recupero di contenuti
non vuoti è verificato da test deterministici e HTTP, non attribuito alla partita.
Il browser reale mostra identità e stato di salvataggio senza modificare conferme.

Artefatti locali: `.local/campaign-native-evidence.json`,
`.local/campaign-restart-verification.json`, `.local/campaign-browser-proof.png`.
Le fixture coprono persistenza non vuota, isolamento fra campagne/nazioni,
cambio sessione, ritorno alla campagna, rewind, archivio corrotto/futuro,
scritture fallite, conflitto fra processi/schede, campagna sconosciuta e parità
del contratto Python/JavaScript. Gli archivi dei test sono temporanei.
Suite complete: **344 test Python e 221 JavaScript superati**, nessuno saltato.
Confrontati anche gli output completi di normalizzazione e runtime sui sei casi
di campagna condivisi: identici. `PRAGMA quick_check` sull'archivio locale: `ok`.

## Limiti residui

- Il passaggio reale fra due campagne caricate e il riavvio del **gioco** non sono
  stati forzati: richiedono che l'utente cambi partita senza perdere progressi.
  Questi percorsi sono coperti da fixture; la prova reale eseguita riguarda
  l'identità osservata e i riavvii di Companion.
- Copie/rami di salvataggi che conservano UUID e seed sono la stessa campagna
  nativa. Un rewind rilevato separa le checklist; rami con la stessa identità e
  tempi uguali o successivi non hanno un identificatore distinto qualificato.
  Per separarli esplicitamente usare **Nuova checklist**.
- Il tempo viene salvato insieme a modifiche dell'avanzamento, non a ogni tick.
  Non si dichiara una cronologia completa dei caricamenti o delle azioni.
- Se il conteggio iniziale non era mai stato osservato, la persistenza non lo
  ricostruisce. Le vecchie checklist prive di identità nativa rimangono archiviate
  ma non vengono automaticamente attribuite a una nuova campagna.
- La copertura nativa resta vincolata alla build supportata. Modificare una guida
  mantenendo ID e chiavi conserva le associazioni dell'avanzamento: per istruzioni
  con significato cambiato è opportuno aprire una nuova checklist.
