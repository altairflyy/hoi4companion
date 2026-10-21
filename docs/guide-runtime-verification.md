# Verifica Guide Runtime — 21 ottobre 2026

Base di sviluppo: `main`, commit `d178ad84d1c77d10492b157beec06836955a9612`.
Le modifiche di questo incremento sono locali. Nessun file o memoria di HOI4
è stato scritto; nessuna mod installata e nessun input impartito al gioco.

## Test eseguiti

- Suite completa Python: **176 test superati**, nessun errore o test saltato.
- Suite completa JavaScript: **107 test superati**, nessun errore o test saltato.
- Confronto esatto dell'intero output Python/JavaScript su **34 fixture condivise**:
  tutti uguali, inclusi motivi, provenienza, dipendenze, scope, capacità e priorità.
- Catene di 2000 dipendenze, cicli, chiavi numeriche/Unicode delle tracce,
  versioni JSON `1`/`1.0`, conferme negative, isolamento dei run e rollback.
- Test HTTP reali con cataloghi sintetici: API nuove, compatibilità checklist,
  storico senza attivazione involontaria, ID partita non attivo rifiutato,
  controlli di origine e validazione delle conferme.
- Controller guida/HUD in un DOM in memoria: condizioni di fase/completamento,
  stati non verificabili, nuova checklist e ripristino, checkbox negative.
- Eventi frontend: cambi di run raggiungono le viste anche con gameplay e
  checklist identici. Suite SSE e native comprese nelle suite complete.

Comandi eseguiti dalla radice:

```powershell
$env:PYTHONPATH='src'
py -3 -m unittest discover -s tests -p 'test_*.py'
node --test tests/*.cjs
```

Il test sprite segnala un'avvertenza di deprecazione Pillow su `getdata`;
non è un fallimento. Non sono state aggiunte dipendenze.

## Partita reale e integrazione

Partita polacca, processo `hoi4.exe` PID `23556`. Il primo campione (focus
`POL_the_four_year_plan` e ricerche `electronic_mechanical_engineering`,
`basic_machine_tools`, `construction1`) è stato confermato direttamente
dall'utente. In seguito l'utente ha precisato che stava continuando a giocare.
Le scelte successive sono state osservate dal lettore, senza interrompere o
controllare il gioco; non vengono attribuite alla prima conferma UI.

Un'istanza separata con il codice finale sulla porta **8769** ha verificato:

- `/api/state`: `source=process_memory`, `status=observed`, `usable=true`,
  paese `POL`. Il campione conteneva nessun focus selezionato e ricerche
  `basic_medium_tank_chassis`, `artillery1`, `infantry_weapons2`.
- `/api/guide`: **34 focus e 48 ricerche**, nessun avviso di corrispondenza
  nel catalogo installato. La guida e i suoi metadati restano dati editoriali.
- `/api/guide/runtime`: modalità `live`, progressione accettata, 82 step,
  ricerche attive `r10`, `r11`, `r15`, zero slot ricerca liberi.
  Capacità focus 1, nessuna proposta automatica perché il checkpoint della
  guida non era confermato. Nessun completamento dedotto dal checkpoint assente.
- `/api/state/events`: campione corrente valido con stesso ID run della
  checklist. Nessun salvataggio o snapshot usato come sostituto della memoria.
- Parità completa tra runtime Python e modello JavaScript sullo stesso
  campione reale e sulla stessa progressione, senza confrontare letture di
  istanti diversi come se la partita fosse ferma.
- HTTP 200 per guida, HUD, nuovi script e `/api/framework/state`.
- Controller reale di `/walkthrough` e `/hud` eseguito in un DOM in memoria,
  collegato alle API del nuovo server e alimentato da un evento SSE reale:
  modalità `Percorso collegato · POL`, 34/48 righe, ricerche attive visibili.
  Entrambe le viste indicavano `Attendi l’annessione della Lituania` e `Radio`
  come prossimi passaggi dichiarati. Sono passaggi della guida, non una
  strategia generata né un'affermazione che vi fossero slot disponibili.

Rapporti locali, ignorati da Git: `.local/guide-runtime-live-evidence.json`,
`.local/guide-runtime-controller-evidence.json`, `.local/guide-runtime-parity.json`.
Contengono campioni e risultati della verifica, non vengono usati dal runtime
come fallback o stato attuale.

## Diagnosi `stale`

Sono stati distinti due errori:

1. Lo storico `Paese giocato non identificato con certezza` nasce dal selettore
   primario non positivo. Il ramo è isolato e coperto da test; il suo verificarsi
   storico durante il ciclo del gioco non è stato riprodotto. Nessun fallback
   al selettore alternativo o alla nazione configurata.
2. Il nuovo `Stato focus incoerente` è stato riprodotto sulla partita reale:
   stesso puntatore completato presente due volte, identità e proprietà valide.
   L'interpretazione dell'elenco ora pubblica una sola appartenenza per puntatore
   identico validato. Rimangono invariati offset, accessi alla memoria, witness,
   controllo dei cambiamenti, identità del paese e rifiuto di riferimenti esterni
   o focus contemporaneamente attivi/completati. Recupero verificato nella
   stessa partita. Dettagli e distinzione osservazioni/inferenze in
   [stale-diagnosis.md](stale-diagnosis.md).

## Limiti

- Nessuna verifica grafica nel browser o dell'overlay nativo: computer use e
  browser locali erano disabilitati in Work. Il test DOM non è prova visuale.
- Nessuna misura della latenza clic→HUD, prova di altre build o copertura
  universale delle transizioni/caricamenti. Campioni strappati vengono ancora
  rifiutati mentre il gioco cambia; questo controllo non è stato indebolito.
- Lo storico delle checklist è in memoria, senza persistenza su disco.
  La sessione tecnica non certifica tutti i cambi di campagna nello stesso
  processo; per una nuova campagna usare Nuova checklist.
- Le istanze preesistenti sulle porte `8766` e `8768` non sono state riavviate:
  conservano il modulo Python precedente. Per provare l'incremento usare
  `http://127.0.0.1:8769/walkthrough`; un ricaricamento della sola pagina sulla
  vecchia istanza non carica le nuove API nel processo Python.
