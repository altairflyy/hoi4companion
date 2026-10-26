# Politica e incarichi in sola lettura

`sample.country_overview.politics` espone due insiemi qualificati separatamente:

- `ideas: {status, items:[{id,group,category,is_law}]}`: idee effettivamente
  selezionate, comprese le leggi, con gruppi nativi. Non è il catalogo delle
  opzioni disponibili.
- `advisors: {status, items:[{id,character_id,slot}]}`: ruoli effettivamente
  attivi. `id` è la chiave nativa del ruolo; `character_id` identifica il
  personaggio e `slot` conserva il codice numerico nativo senza inventarne il
  significato. Il catalogo statico risolve nomi e categorie.

Ogni sottoinsieme completo può essere vuoto. Se una decodifica fallisce, quel
sottoinsieme è `unavailable` e non contiene righe parziali; l'altro può restare
osservato. `status:partial` non autorizza a considerare noto il sottoinsieme
mancante. Costi, disponibilità delle nomine, comandanti di unità e tratti
dinamicamente modificati non sono letti da questa estensione.

## Evidenze della build

Build supportata: SHA-256
`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`.
REA `open_binary` ha confermato il PE x86-64 installato. La successiva ricerca
di stringhe con il provider profondo è scaduta: non si attribuisce a REA una
decompilazione riuscita. Le evidenze seguenti provengono da lettura offline del
medesimo PE e disassemblaggio Capstone, più verifica strutturale runtime.
I riferimenti locali `politics_reader.py` e `advisor_reader.py` sono stati
usati come ipotesi e non eseguiti; nessun componente che impartisce ordini è
stato incorporato o avviato.

| Evidenza | Riscontro offline (RVA) | Campo |
| --- | --- | --- |
| PL-01 | Getter `6CF890`, byte `488b81900f0000c3`; controllo appartenenza `1161B14/1161B1F` | country+F90 → CPoliticalStatus; lista selezionata +50, conteggio +5C |
| PL-02 | Getter stringa secondario `647E90`; getter categoria `FCF630`; parser `FD5F66` | CIdea chiave +18; gruppo +AA0 → CIdeaGroupType chiave +38, flag legge +58, categoria +60 → CIdeaCategory chiave +10 |
| PL-03 | Getter `6CF2C7`; inserimento ruolo attivo `10E9CCA` | country+FF0 → CCountryCharacters; proprietario +8, soli ruoli attivi +58. +40 elenca ruoli disponibili e non viene usato come prova di nomina |
| PL-04 | Getter ruolo `6C00E0`; serializer `12A5411`; getter proprietario `A6AF60`; serializer `FA658C` | CAdvisor personaggio +10, codice slot +28, chiave ruolo +30; CCharacter chiave +48, nazione +68 |

Dodici sequenze di istruzioni dei riferimenti sono state confrontate byte per
byte con il PE e coincidono. Il profilo dell'applicazione rifiuta qualsiasi
hash differente. Tutti gli oggetti, gruppi e categorie sono qualificati con
RTTI; puntatori, nomi, array e identità vengono riletti prima della
pubblicazione. Si rifiutano duplicati, due leggi selezionate nello stesso
gruppo, flag impossibili, personaggi stranieri e manager di un'altra nazione.

## Verifica effettivamente eseguita

Lettura esterna del 22 ottobre 2026 sulla partita polacca già aperta, senza
input né modifica della partita: 8 idee selezionate, comprendenti
`civilian_economy`, `uncontrolled_exports` e `volunteer_only`, rispettivamente
nei gruppi `economy`, `trade_laws` e `mobilization_laws`. L'array degli
incarichi attivi era vuoto e il proprietario del manager corrispondeva alla
nazione osservata. La rilettura di tutti i testimoni è riuscita. Questi sono
valori letti dalla memoria, non un confronto visuale indipendente con il
pannello Politica.

Prova riproducibile e risultati locali:
`work/politics_probe.py` e `work/politics_probe.json` nella cartella della chat.
Non sono necessarie al funzionamento distribuito.

Le fixture coprono anche nomine non vuote, errori di proprietà/tipo, insiemi
vuoti, duplicati, flag legge invalidi, conflitto fra leggi, campioni cambiati e
sonde a tick invariato. Non è stata provocata una nomina o un cambio di legge
nel gioco per questa verifica. Le sonde osservano membership, collegamenti e
identità attraverso il piano esistente; non vengono creati hook o polling
separati. Le stesse limitazioni di campionamento non atomico del lettore
generale continuano ad applicarsi.
# Saldo politico e risorse HUD (26 ottobre 2026)

`politics.power` qualifica separatamente il potere politico disponibile. Il
getter del trigger `CHasPoliticalPowerTrigger` (`0x1403eda70`) legge lo stato
politico del paese, campo +0xE0, fixed point con divisore 100000. Il lettore
usa lo stesso manager `CPoliticalStatus` già qualificato tramite paese +0xF90,
include il saldo nel watch plan e conserva precisione e valori negativi.
Il contratto Python/JavaScript espone `country.politics.political_power` solo
per la nazione corrente e un campione osservato valido.

La verifica in pausa ha letto 589,85146, coerente con 589 nell'interfaccia.
La disponibilità economica delle azioni viene confrontata con il costo
esplicito della guida; un costo mancante resta sconosciuto e non abilita l'HUD.
I modificatori dinamici dei prezzi non sono stati qualificati da questo lavoro.
