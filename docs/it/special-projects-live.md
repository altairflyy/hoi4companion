# Progetti speciali: lettura live in sola lettura

## Saldi e ammissione HUD, 26 ottobre 2026

Il catalogo ora legge i `breakthrough_cost` letterali del gioco e delle mod
caricate. L'API arricchisce la guida in memoria con `resource_costs`, per esempio
`projects.points.specialization_land: 1` per il carro lanciafiamme. Non riscrive
la guida importata e non richiede di rigenerare il prompt. Costi scriptati o
condizionali non valutabili restano sconosciuti, senza prezzi predefiniti.

La lettura nativa qualifica `CBreakthroughProgress` a status +0x50. Il relativo
albero MSVC ha sentinel +0x58 e conteggio +0x60; ogni nodo contiene il token
della specializzazione a +0x1C e il saldo in unità di progresso a +0x20.
`CHasBreakthroughPointsTrigger` (`0x1404a1930`) usa il getter `0x140e76a10`,
che cerca quel token e divide il saldo per **10000** con troncamento intero.
Il lettore verifica sentinel, parent, ordinamento, duplicati e conteggio. Un
errore rende sconosciuti i punti senza cancellare le membership dei progetti.
I byte del saldo entrano nel watch plan esistente: nessun secondo polling.

La risposta live separa punti spendibili e progresso frazionario in
`projects.breakthroughs.items`; l'adattatore Python/JavaScript pubblica i fatti
`projects.points.<id_specializzazione>`. 9999 unità non abilitano un costo di
un punto. L'HUD richiede risorse note e sufficienti prima di proporre anche
il punto di accesso. Restano necessari ruolo, impianto e geometria qualificati.

Verifica reale in GER, partita del 27 gennaio 1937 lasciata in pausa: punti
fisici/navali/aerei/terrestri tutti zero; progresso terrestre 48,76%, potere
politico 589,85146. Il pannello mostrava zero punti e costo terrestre 1 del
Flammpanzer. Nessuna ricerca, nomina o costruzione è stata avviata. Prova API
isolata con catalogo e guida attuali riuscita. HOI4 è stato poi chiuso.

La lettura dei saldi è verificata su quella partita; la comparsa positiva
della cornice con un punto disponibile rimane provata soltanto con fixture.
Il contesto nativo dei controlli azione risultava parziale: questa estensione
non certifica l'allineamento reale del selettore dei progetti.

L'estensione è vincolata allo stesso SHA-256 supportato dal reader esistente:
`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`.
Non esegue funzioni native, non scrive memoria e non usa salvataggi. Tutte le
letture attraversano NativeSample e vengono rilette prima della pubblicazione.
Le sonde dei cambiamenti includono riferimenti, membership e stato della macchina.
Un errore invalida la sezione, non produce liste vuote per simulare assenze note.

Il catalogo legge `common/special_projects/projects`, con nome inglese e alias
nazionali/localizzati. Il compilatore associa `project_id`/`project_name` o una
entità `project` ai predicati distinti `projects.active` e `projects.completed`.
L'impianto costruito non prova l'avvio di un progetto. Uno stato live incompleto,
stale, di un'altra nazione o senza sezione qualificata non prova completamenti.
Un progetto riconosciuto viene normalizzato come azione `special_project`, così
non diventa un blocco manuale implicito per le successive ricerche ordinarie:
può proseguire in parallelo agli slot tecnologici, salvo dipendenze esplicite.

## Provenienza del layout

PE originale aperto con REA e hash verificato; le interrogazioni approfondite
Ghidra sono scadute dopo 300 secondi. La qualificazione effettiva deriva da
disassemblaggio locale del PE, RTTI e confronti passivi sulla partita, non da
un risultato Ghidra non ottenuto. Script e dump locali in task-operational-audit.

- CCountry +0xFA8 → CProgramStatus, identità paese a +0x10.
- Status +0x18 → CProjectPool, identità paese a +8; +0x20 elenca gli impianti.
- Pool +0x10: vettore per valore di CProject, stride 0x1C0, conteggi limitati.
- Project +0x20 → dati del template, il cui oggetto RTTI è 0x18 byte prima;
  ID nativo a template +0x238. Non leggere +0x10 del progetto come TAG: contiene
  un flag e padding, non l'identità della nazione.
- Project +0x60 è il riferimento all'impianto; va confrontato con identità e
  template dell'impianto della stessa nazione (+0x18 e +0x50).
- Project +0x28 → CProjectStateMachine. `0x1404a1de0` (trigger di ricerca attiva)
  richiede riferimento impianto non nullo e ricerca non completata.
- `0x1404a1e20` delega il completamento a `0x140fe38c0`, poi `0x141a37260`:
  flag di transizione +0x10 uguale a zero e stato corrente +8 uguale a machine
  +0xE8. Si verifica inoltre tipo CSimpleProjectState e label RESEARCH_COMPLETED.
  Un riferimento storico all'impianto dopo il completamento non prova che il
  medesimo impianto stia ancora ricercando quel progetto.

Il 24 ottobre 2026 `sp_land_military_engineering_vehicles` è stato osservato
attivo e non completato in GER con `land_facility`, coerentemente con la conferma
dell'utente. Il completamento e i casi negativi sono provati con fixture strutturali;
non è stato provocato o osservato il completamento reale del progetto. Non sono
qualificati percentuale, stima del termine, dettagli dello scienziato e disponibilità
dinamica dei progetti. Nessun controllo dell'identità del lettore preesistente è
stato indebolito.
