# Stato live della nazione

Verifica del 21 ottobre 2026. Vista: <http://127.0.0.1:8766/country>.
Il payload è `sample.country_overview` nel flusso SSE già usato da focus e
tecnologie. Non legge salvataggi. Il gioco è stato avviato da Steam con
Computer Use, caricando la partita rumena esistente e lasciandola in pausa.

## Dati disponibili e confini

| Richiesta | Copertura | Verifica sulla partita |
| --- | --- | --- |
| Divisioni totali e per tipo | Rilevate: totale, tipo del battaglione principale e dettaglio per modello. Incluse forze di spedizione controllate con modello estero. | 69, come nel teatro del gioco. Tipi: 50 fanteria, 3 cavalleria, 3 alpini, 2 ciclisti, 1 motorizzata, 10 artiglieria semovente media. Il nome personalizzato “Marines” non viene usato per indovinare il tipo. |
| Manpower | Disponibili e totale schierato di tutte le armi. | Disponibili 0; totale schierato 337.816. Tooltip: esercito 266,85k, marina 66,99k, aviazione 3,96k (valori GUI arrotondati). |
| Industria nei territori | Fabbriche civili/militari, cantieri, infrastrutture e distinzione posseduto/controllato. | 16 territori associati; controllo diretto su Muntenia: 1 civile, 7 militari, infrastrutture 5. Non tutti i territori sono stati controllati individualmente nella GUI. |
| Industria effettiva | Totali nativi, separati dagli edifici fisicamente costruiti. | 131 civili, 78 militari, 2 cantieri, come nel tooltip Fabbriche. Inclusi contributi da soggetti, commercio e occupazione. |
| Produzioni | Tipo militare/navale, equipaggiamento, fabbriche richieste/assegnate/attive/danneggiate e uscita teorica per giorno. | 8 linee: richieste 4/1/1/1/3/5/150/10; assegnate 4/1/1/1/3/5/63/1. Confrontate con Produzione. |
| Equipaggiamento | Magazzino lordo per tipo, incluse varianti estere, con dettaglio varianti nel payload. | 64 varianti, 20 tipi; contraerea 337, anticarro 23, autoblindo 97, carri leggeri 120 sommando i due telai, come voci senza fabbisogno del pannello Logistica. |
| Materie prime | Prodotte, importate, esportate, disponibilità prima della domanda produttiva, domanda e saldo corrente. | Tutti i 7 saldi coincidono con Produzione/Logistica: petrolio +60, alluminio −12, gomma −1, tungsteno −24, acciaio −154, cromo −8, carbone −36. |

La colonna **Magazzino lordo** non è il saldo netto della schermata Logistica:
quel saldo sottrae anche i fabbisogni. Fabbisogno, consumo e saldo netto
giornaliero degli equipaggiamenti non sono ancora qualificati; restano `null`
e la sezione è **Parziale**. Anche il manpower in addestramento resta `null`,
pur risultando 0 nel tooltip di questa partita: non si generalizza quel singolo
valore. Carburante, guarnigioni, code di costruzione e produzione effettivamente
consegnata nel tempo non sono esposti da questa estensione.

Il tipo di una divisione segue il battaglione principale del modello: non è
un inventario dei battaglioni né una valutazione della forza combattente.
Le unità controllate includono spedizionari; il contatore manpower nazionale
ha un perimetro diverso. La somma dei materiali nel magazzino non include
quelli già assegnati alle divisioni.

La UI localizza i nomi dal catalogo installato e conserva gli identificatori.
Le uscite produttive positive sono troncate a due decimali come nel gioco;
le velocità navali molto basse usano quattro decimali. Non sono ETA.

## Evidenze e layout della sola build supportata

REA `open_binary` ha identificato il target PE x86-64 installato (`hoi4.exe`), SHA-256
`7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5`.
La sessione è stata chiusa dopo l'indagine. L'apertura ha restituito identità
inline, senza un Evidence ID conservato: non le viene attribuita una
decompilazione o un'osservazione dell'esecuzione. Le istruzioni sotto sono
riscontri offline Capstone sullo stesso PE; il confronto numerico è runtime.

Registro locale riutilizzabile (indirizzi RVA, non indirizzi del processo):

| Evidenza locale | Layout/riscontro | Tipo e limite |
| --- | --- | --- |
| CL-01 | Identità REA e profilo già presente `ExecutableProfile` | Identità statica; hash diverso rifiutato. |
| CL-02 | Lista divisioni country+290; CArmy proprietario+1D8, modello+3B8; modello primario+1A8→CSubUnitDefinition, chiave+18. | RTTI e appartenenza runtime. Modelli stranieri ammessi per gli spedizionari, identità unità non duplicate. |
| CL-03 | Disponibili = i32(country+334) + somma i32(state+848) della lista country+460; schierati = i32(country+348). | Registrazioni script `manpower`/`manpower_k`→RVA14F7390/14F7890→CFC300; `deployed_total_manpower_k`→14F91C0→CFE190. Non è il manpower massimo. |
| CL-04 | country+F68→CProductionStatus, proprietario+290, linee+58. Linea+20→status, variante+88→CEquipmentVariant, tipo variante+3F0→CEquipmentType, chiave tipo+18. | Serializzatore linea1939760 e riferimento tipo1939966; RTTI runtime. |
| CL-05 | Richieste i32(linea+E8), attive+18, danneggiate+1C, non assegnate+40. Richieste−non assegnate deve coincidere con attive+danneggiate. | Serializzatore14B59B0; cache token già inizializzata:35CD=`damaged_factories`,35CE=`active_factories`. Confronto GUI di tutte le 8 linee. |
| CL-06 | Costo i64+28 e uscita IC i64+30; unità/giorno = uscita/costo. Totali industria i64 status+2B8/+318/+378, scala100000 e parte intera. | Serializzatore14B59B0, formatter14B53A0, riferimenti E64A93/E64B01; confronto GUI. Il campo+38 è `produced`, non il costo. |
| CL-07 | Stati world+2C8; ID+58, owner+C8, controller+CC. Definizioni edifici root RVA332EE28, lista+40; chiave+270, indice+2E0. Mappa stato+140, edifici+158; livello costruito i16 edificio+40. | Serializzatore9E0E90, token283E=`owner`,283F=`controller`; strutture/runtime e confronto Muntenia. Mappa−1 significa assenza; un indice invalido non significa zero. |
| CL-08 | Pool incorporato status+200→CEquipmentVariantPool; header+20, righe16: variante puntatore e quantità i64/100000. | Serializzatore produzione E7209C/E720A3 token2F5A=`equipments`; serializer pool1012DB0. Il campo status+190 è CLicensedProductionStatus e non è stato interpretato come scorta. |
| CL-09 | country+11F8→CCountryResources, tag proprietario+20. Registro risorse RVA332F088, definizioni+28, token+8, indice+D8. Gruppi: prodotte+28, importate+188, disponibilità+2E8, domanda+398, esportate+5B0; header gruppo+8, righe16 {i64/100000, token}. | Serializer CBE2D0 riceve un `this` secondario spostato di18; getter CAED60 somma +2E8 e+398 (il loop bitwise esegue una sola iterazione). Tutti i saldi confrontati con la GUI. |
| CL-10 | Nomi token già inizializzati: flag RVA35E1AB0, massimo35E1AB4, tabella35E1AE0, stringhe32. | Lettura passiva della cache; nessuna inizializzazione o chiamata nativa. Nomi attesi e chiavi risorse convalidati. |

Gli indizi nei riferimenti locali `saslifat-gif/hoi4-mcp` e
`klimPaskov/HOI4-Live-Tools`, già descritti in live-state.md, sono stati trattati
come ipotesi. Non sono stati eseguiti moduli dei riferimenti né riutilizzate le
parti che iniettano o modificano il gioco. Alcuni indizi sono stati smentiti:
scala i64 dei totali industriali, base del pool, ordine owner/controller.

## Coerenza, aggiornamento e test

Ogni sezione usa limiti di dimensione, RTTI, controlli di proprietà, unicità e
rilettura dei byte. Si ricontrollano insieme i testimoni delle sezioni riuscite
prima di pubblicare; una sezione incompleta non pubblica le righe parziali.
Il campione resta non atomico, come il lettore precedente: non si sospende il
motore. In caso di perdita del processo il frontend nasconde tutti i valori.

I controlli leggeri includono ora contatori, quantità, livelli, appartenenza e
collegamenti delle nuove sezioni. Definizioni, nomi e RTTI vengono rivalidati
dall'audit completo ogni 5 secondi. La cadenza di controllo resta 250 ms e
l'SSE pubblica solo cambiamenti semantici, senza uno stream per ogni sezione.
La lettura non crea eventi nativi del motore. Senza client gli handle si chiudono.

Misura sul gioco in pausa: prima acquisizione circa152 ms, mediana di15
controlli stabili circa8,6 ms, una sola acquisizione completa. Sono tempi del
reader, non latenza clic→browser né impatto FPS. Le modifiche reali ai nuovi
valori durante avanzamento della partita non sono state forzate per la prova.

Test sintetici coprono valori negativi/impossibili, proprietà errata, modelli
esteri, mappe edifici assenti/invalide, aggregazione varianti, risorse riordinate
per token, cache non valida, isolamento errori e cambi rilevati a tick invariato.
Il browser reale mostra i dati localizzati senza errori di console; disattivare
la lettura elimina i valori. Verifiche e immagini nella cartella `outputs` della
chat, incluse `country-live-benchmark-selective.json` e `country-live-verification.json`.

I permessi restano `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ` (0x410).
Nessuna scrittura di memoria, iniezione, hook, mod, comando console o modifica
dei file di gioco. Le prove GUI hanno soltanto aperto pannelli e tooltip.
