# Creazione dell’agenzia rilevata live

Verifica locale del 21 ottobre 2026, istanza <http://127.0.0.1:8771/walkthrough>.
La guida usa i fatti booleani osservati `country.intelligence.in_creation` e
`country.intelligence.is_created`. `started_when` verifica almeno uno dei due;
`completion` richiede soltanto `is_created=true`. L’avvio non sblocca dipendenze
che richiedono il completamento. Nessuna logica del runtime dipende dalla Polonia.

Nella partita POL in pausa è stato letto `in_creation=true`, `is_created=false`:
l’API restituisce `active`, selezione vera e completamento falso. Le conferme
manuali precedenti restano accettate dalle API, ma non sostituiscono questi
criteri live. Dati mancanti, malformati, obsoleti o di un’altra nazione non
dimostrano avvio o completamento. La scheda non offre più la conferma manuale.

## Evidenze e limiti

REA `open_binary` ha identificato il PE x86-64 installato, SHA-256
`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`.
La risposta inline non fornisce un Evidence ID; la sessione è stata chiusa.
I riscontri statici seguenti sono disassemblati offline con Capstone sullo
stesso PE, non risultati di decompilazione Ghidra. Gli indirizzi sono RVA.

| Riscontro | Layout della build supportata |
|---|---|
| Getter `6CF7F0` | Puntatore `country+FC0` all’agenzia |
| Serializzatore `FDF3A0` | Byte `+C0` token `3D33=is_created`; byte `+C1` token `3D34=in_creation` |
| Comando creazione `1A27C00` → `FDE7C0` | Imposta `+C1=1`; `+78` identifica la nazione proprietaria |
| Comando annullamento `1A279F0` → `FDAA50` | Azzera `+C1`; non prova un completamento |

Il reader verifica profilo eseguibile, RTTI `CIntelligenceAgency`, proprietà
reciproca e byte booleani esatti. I campi entrano nei testimoni e nei controlli
leggeri esistenti, anche a tick invariato. Un errore isola la sezione come non
disponibile. Cadenza, SSE e controlli d’identità del lettore restano invariati.
Non sono modificati memoria o file del gioco, né eseguite funzioni native.

Il campo `+C8` è serializzato come `upgrade_progress`: non viene presentato
come percentuale della creazione. Migliorie, agenti e operazioni non sono
decodificati. Non è stata forzata alcuna transizione nel gioco: completamento,
annullamento e cambio dei flag sono verificati con fixture sintetiche; la
prova reale riguarda l’agenzia attualmente in creazione.

Suite complete: **200 test Python e 145 JavaScript superati**, nessuno saltato.
Le fixture condivise coprono stato sconosciuto/falso/avviato/completato,
stale, nazione errata e tentativi di override manuale. La parità dell’intero
output Python/JavaScript è verificata su 56 fixture runtime e 36 dello stato.
Evidenze locali: `.local/agency-live-verification.json`, log
`agency-python-tests.log` / `agency-js-tests.log` e immagine
`.local/guide-context-proof/agency-auto-live.png`.
