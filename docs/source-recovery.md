# Recupero verificato dei sorgenti

Il Companion recupera in memoria 31 sorgenti dell’installazione verificata il
24 ottobre 2026: 29 chiusure finali mancanti, una chiusura finale in eccesso e
un’assegnazione `force =` senza valore. Nessun file di HOI4 viene modificato.

## Confini e verifica

`native/parser/src/source-recoveries.json` contiene hash SHA-256 del testo
decodificato, provenienza, intervalli esatti e impronta dell’intero AST atteso.
Il modulo Rust `recovery` applica solo queste operazioni ai contenuti identici
a quelli revisionati, controllando intervalli, testo originale, tipi di
operazione e AST finale. Non corregge automaticamente documenti sconosciuti,
non sceglie chiusure in base alla sola indentazione e non assegna valori
presunti. Un aggiornamento del contenuto richiede una nuova revisione.

Gli altri file restano accantonati come prima. Stringhe e commenti non vengono
scambiati per parentesi. La proprietà senza valore del repository delle
particelle viene omessa, senza inventare una forza; l’evidenza segnala
`omitted_properties: true`. Le chiusure e gli spazi sostitutivi preservano le
posizioni originali; le coordinate sulle chiusure sintetiche sono riportate
alla riga finale del sorgente originale.

## Un solo recupero per cataloghi e analisi

Il C ABI aggiunge il modo AST 4 con recupero verificato; il modo 0, `parse`,
`parse_bytes` predefinito e l’esportazione JSON restano rigorosi. `parse_file`
usa il modo 4 e mantiene l’evidenza in `Node.source_recovery`. Il backend
richiede `reviewed_recovery: 1`: una vecchia libreria deve essere ricompilata,
non può svuotare silenziosamente i cataloghi.

La preparazione CWTools chiama lo stesso recupero Rust prima della
normalizzazione lessicale. Il rapporto `source_recoveries` include solo i
file recuperati accettati anche da CWTools. Cataloghi e analisi confrontano
la stessa evidenza; non esiste una seconda implementazione dei rimedi in
Python o F#. Il rapporto mantiene stato `partial`: il recupero di una
definizione statica non certifica il comportamento del motore del gioco né
osserva il completamento di un’azione nella partita.

L’importazione dei focus condivisi segue anche i discendenti delle radici
esplicitamente referenziate: vengono così recuperati i sette focus del ramo
congiunto cinese. Gli antenati richiesti non trascinano rami fratelli non
selezionati. Le condizioni e i filtri DLC rimangono applicati: un ramo legato
a un DLC non attivo non viene reso disponibile per effetto del recupero.

## Registro e manutenzione

`.local/logs/source-errors.json` conserva i record e i messaggi originali,
aggiornando la disposizione in `recovered_in_memory` e registrando tutte le
operazioni. Percorsi e hash delle versioni precedenti non vengono persi.
La pagina Analisi separa recuperi, omissioni ed esclusioni ancora presenti.

Per aggiungere una revisione: verificare il sorgente reale e i confini dei
blocchi, proporre il minimo adattamento, controllare l’intero AST e le
definizioni attese, eseguire entrambi i parser e i test, quindi aggiungere
hash e operazioni al manifesto e ricompilare. L’accettazione da parte dei
parser da sola non prova che il significato ipotizzato coincida con quello
del motore HOI4.

## Verifiche eseguite

- 431 test Python, 238 JavaScript e 21 Rust superati; Clippy senza avvisi.
- Installazione reale: 31 recuperi accettati dai due parser, nessun errore di
  parsing residuo sulla rappresentazione adattata; 5.632 documenti e 154.318
  definizioni. Questo non azzera le segnalazioni semantiche CWTools.
- Verificati cataloghi politici SOV/PER/SWI, ricerche iniziali norvegesi e
  tutti i sette focus congiunti del ramo recuperato. Filtri DLC preservati.
- SHA-256 dei 31 file originali invariati. Una versione modificata non
  revisionata viene rifiutata nei test; la modalità rigorosa resta rigorosa.
- Wheel costruito da sdist e provato in un ambiente isolato con entrambi i
  parser, senza dipendenze dai toolchain nel PATH.
- Server locale: live e SSE `observed` / `usable`, stessa campagna e guida,
  impostazioni, mod/DLC e payload dell’avanzamento invariati al riavvio.
  Pagine principali HTTP 200; nessuna verifica visiva del browser eseguita.
