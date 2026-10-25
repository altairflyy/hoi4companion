# Compilazione semplificata delle guide

Il prompt principale produce `format:"hoi4-guide-authoring"`,
`authoring_schema_version:1`, con `sources` autocontenute e `guide.steps`.
Ogni step dichiara direttamente `type`, identità e obiettivo. Il modulo puro
`guide_authoring.py` convalida e converte questo documento in una normale
importazione v1. Resolver, binder, runtime, persistenza e lettore live restano
gli stessi. Il precedente `hoi4-guide-import` continua a essere accettato.

Esempio minimo completo (testo inventato per mostrare il formato):

```json
{
  "format": "hoi4-guide-authoring",
  "authoring_schema_version": 1,
  "sources": [{"id":"s","type":"manual","title":"Esempio",
    "text":"Build two military factories in Example State.","segments":[]}],
  "guide": {
    "id":"example-authoring", "title":"Esempio", "applies_to":{},
    "steps":[{
      "key":"build-first", "title":"Due fabbriche aggiuntive", "type":"build",
      "building":"Military Factory", "state":"Example State",
      "quantity":{"mode":"add","value":2},
      "instructions":["Accoda due fabbriche militari nella regione indicata."],
      "evidence":[{"source_id":"s","basis":"explicit",
        "quote":"Build two military factories in Example State."}]
    }]
  }
}
```

Nomi esatti e riferimenti `{id:"..."}` o `{name:"..."}` vengono affidati al
resolver già presente. Non si sceglie il primo candidato ambiguo. Per i focus
è ammesso `scope`; gli altri riferimenti non lo accettano. L'elenco delle
azioni, dei campi e dei requisiti viene inserito nel prompt da `SPECS`, la stessa
tabella usata per convalidare i campi del compilatore.

## Azioni

- `focus`, `research`, `special_project`: target; eventuale upgrade_from per research.
- `build`: edificio, regione e quantità add/total; baseline solo se documentata.
  `state` può contenere il nome di una città: l'importatore usa localizzazioni,
  punti vittoria e appartenenza delle province del gioco e delle mod attive per
  trovare la regione (per esempio Berlin → provincia 6521 → Brandenburg).
  Le traduzioni sono riconosciute quando presenti nelle localizzazioni installate.
  Le fabbriche usano il conteggio regionale; forti e altri edifici provinciali
  richiedono la città univoca o `province` esplicita. Nomi ambigui e ID in conflitto
  restano da correggere. La coda provinciale prova soltanto l'avvio; non certifica
  il livello finale se il lettore non lo espone. Le relazioni geografiche sono
  locali e non vengono aggiunte agli allegati per l'IA.
  La copertura dei nomi riguarda città con punti vittoria nominati nei file
  installati; non è un atlante di ogni località. Le relazioni sono statiche:
  non provano proprietà/controllo attuali o trasferimenti provinciali eseguiti
  da script durante la campagna. Non implicano che un ordine sia consentito.
- `production`: una linea oppure lines; quantità requested/assigned e incrementi
  soltanto per una singola linea. Le linee multiple indicano totali.
- `law`, `idea`, `appoint`, `event`, `decision`: identità specifiche; due nomine
  o scelte sono step distinti. Event richiede evento e risposta.
- `create_agency`, `agency_upgrade`, `intelligence_operation`, `mio`:
  distinguono esplicitamente attività prima spesso confuse nei parametri.
- `template`, `design`, `army`, `diplomacy`: descrizione strutturata del metodo;
  non introducono un monitoraggio automatico che il lettore non possiede.
- `instruction`: azione concreta con categoria e istruzioni; details conserva
  dati descrittivi ulteriori o parziali. Non è automaticamente un promemoria.
- `reminder`: consiglio sul metodo; non completabile e non usabile come dipendenza.
- `free`: scelta libera prevista dalla fonte nella traccia focus o research.
- `advanced_v1`: step v1 esplicito, per politiche avanzate compatibili. Nessun
  tipo sconosciuto o incompleto viene convertito implicitamente in questo tipo.

## Errori e compatibilità

Un campo necessario assente produce un errore con chiave e nome del campo,
per esempio `build-first: manca state`. Il file originale rimane nell'editor
e può essere corretto e salvato come bozza. Un dettaglio non accertabile deve
essere dichiarato; un'azione concreta parziale può essere un'instruction con
i dati certi e il limite esplicito, mantenendo la verifica necessaria.

Le chiavi fornite vengono preservate. Omettendole si generano step-0001, ecc.:
sono sconsigliate per revisioni dove cambiano ordine e dipendenze. Non vengono
riscritti gli ID di guide installate o trasferite conferme fra guide/campagne.
Condizioni, evidenze, fasi e dipendenze dichiarate passano al validatore v1.
La creazione automatica dei criteri supportati resta nei binder. Non devono
essere inseriti selector, binding politici o fatti guide.* nei details.

## Risoluzione contestuale e indici locali

L'importatore usa un indice condiviso per tipo/ID, nome inglese, localizzazione,
alias, evento/risposte e geografia. I nomi uguali conservano tutti i candidati.
Filtri di nazione, albero, evento padre, gruppo di legge e ruolo vengono applicati
prima della scelta del nome. I focus mantengono inoltre la propagazione dei
prerequisiti ed esclusioni del percorso dichiarato. Gli ID espliciti sconosciuti
o incompatibili non vengono sostituiti con un'altra identità.

Le condizioni nazionali riconosciute sono conservative: `tag` e combinazioni
booleane; `original_tag`, ambiti e trigger non supportati restano sconosciuti.
Restano compatibili i metadati nazionali già usati dagli advisor; una condizione
nazionale certa dello script prevale sulle indicazioni del namespace.
La selezione non valuta disponibilità attuale, date, risorse o progressi live:
un obiettivo futuro rimane un'identità valida. I cataloghi caricati hanno già
il filtro statico di DLC/mod; il resolver non tenta di simulare altri trigger.

L'indice appartiene alla generazione di catalogo del servizio. Alla ricarica
dei dati/impostazioni (installazione, mod, lingua, nazione o DLC riletti) viene
ricostruito. Non c'è una scansione automatica continua dei file esterni.
Ogni guida mantiene risultati e percorso propri: si riusano gli indici, non le
decisioni di un'altra guida o partita. I domini contestuali hanno una cache
limitata e sincronizzata; nomi sconosciuti non incrementano gli indici.
L'anteprima espone le esclusioni in dettagli chiusi «Perché questa associazione?».
Cataloghi esportati e risultati delle anteprime non possono alterare gli indici.

L'anteprima e i file installati sono sempre Guide Schema v1: non ci sono due
runtime o due sistemi di salvataggio. Nessuna guida installata viene migrata
automaticamente. L'API IA opzionale usa lo stesso prompt e conserva le fonti
fornite, senza fidarsi di testi sorgente riscritti dal modello; accetta ancora
le precedenti risposte del provider in formato v1.

Il file sorgente `guide-import-prompt.md` è un modello: per il testo completo
con la tabella generata usare Copia prompt completo, Scarica prompt oppure
`Importer.prompt()`. La compilazione prova coerenza strutturale e supporto del
contratto, non autenticità delle citazioni, completezza rispetto al video o
validazione del lettore live.
