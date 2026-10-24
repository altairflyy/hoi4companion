# Progetti speciali: lettura live in sola lettura

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
