# Schede e disposizione della ricerca tecnologica

Verifica del 20 ottobre 2026. Questa modifica usa solo la lettura delle risorse installate del gioco; non cambia file di HOI4, mod, processo o partita.

## Evidenza osservata nei file installati

`interface/countrytechtreeview.gui`

- SHA-256: `fec641fb738ab59369cac40ae902fdba793bed7cbaa2c807756207a2e1f01045`.
- Riga 3768: contenitore `folder_tabs`; righe 3773–3870 circa: pulsanti delle schede, posizione orizzontale e sprite, compresi i pulsanti alternativi selezionati dai DLC. L'ordine ricostruito è Fanteria, Supporto, Corazzati, Artiglieria, Navi, Supporto navale, Aviazione, Ingegneria, Industria.
- Riga 247: `support_weapons_tree`, origine `(140,210)`, celle `(70,70)`, formato `LEFT`.
- Riga 1137: `gwtank_chassis_tree`, origine `(750,40)`, celle `(70,70)`, formato `UPPER_LEFT`.
- Riga 3284: `basic_machine_tools_tree`, origine `(0,172)`, celle `(70,70)`, formato `UP`.
- Riga 73 e altri blocchi `instantTextBoxType`: intestazioni localizzate e riferimenti cronologici.

`common/technology_tags/00_technology.txt`

- SHA-256: `fccf0824abcf0d741f740704d92310c430b84d062a5b43e65119dcdb2896eeea`.
- Riga 142: `technology_folders`; righe 157, 170 e 188: alternative corazzati, aviazione e supporto navale con condizioni DLC.
- Righe 199, 203, 207, 217: le dottrine sono marcate separatamente e restano fuori da questa vista delle tecnologie standard.

I nomi dei gruppi GUI terminano in `_tree` e corrispondono a tecnologie radice. I file `common/technologies/*.txt` forniscono appartenenza alla cartella, coordinate `folder.position` e archi `path.leads_to_tech`. Il parser conserva chiavi ripetute e variabili numeriche, senza eseguire gli script.

Il dettaglio riproducibile della verifica locale, con identificatori, rami e tecnologie non mappate, è in [technology-layout-evidence.json](technology-layout-evidence.json). Non sono Evidence ID prodotti da una decompilazione: qui l'evidenza è il contenuto delle risorse installate, identificato da percorso, hash e righe.

## Modifica del companion

Le categorie prima disponibili nel menu diventano schede visibili, nell'ordine e con le icone dichiarati dalla GUI. Il filtro DLC già esistente resta applicato prima della costruzione delle schede. Una seconda fila permette di esplorare singolarmente i gruppi effettivamente dichiarati nella GUI: è una facilitazione del companion, non l'affermazione che ciascun gruppo abbia un pulsante separato nel gioco.

I gruppi sono associati alle tecnologie attraversando i percorsi dalla radice, fermandosi all'ingresso in un'altra radice GUI. Associazioni ambigue, origini percentuali e tecnologie senza coordinate numeriche non vengono inventate. Le tecnologie non mappate rimangono accessibili in una sezione esplicita.

Le coordinate dichiarate sono usate per ricostruire orientamento e disposizione: il formato `LEFT` porta l'asse temporale `y` sull'asse orizzontale; `UP` e `UPPER_LEFT` conservano gli assi delle coordinate dichiarate. Il canvas scala le distanze per ospitare schede leggibili, conserva immagini e tooltip, ricerca, zoom, collegamenti e indicatori della lettura live. I collegamenti delle griglie verticali vengono disegnati dall'alto verso il basso.

Con DLC locali e Romania configurati sono presenti **9 schede, 324 tecnologie distinte e 326 appartenenze alle schede** (due tecnologie condivise). **317/326 appartenenze** hanno una posizione ricostruita dalla GUI. Le nove appartenenze non mappate sono sette nella fanteria e due nelle corazze navali: nessuna viene rimossa dal catalogo.

## Verifica e limiti

- Test Python mirati: parsing delle schede, icone/ordine, coordinate dei due orientamenti, gruppi ciclici, discendenti ambigui, risorse malformate, condizioni DLC e tecnologie fuori categoria.
- Test JavaScript: ricostruzione delle coordinate, mantenimento dei nodi senza mappa e selezione dei rami dichiarati; controllo sintattico del frontend.
- I file installati sono stati analizzati staticamente. L'origine relativa delle griglie e gli identificatori sono osservazioni; la trasformazione grafica nel companion è una ricostruzione. Non è verificata la centratura automatica interna del motore, né è dichiarata una copia pixel per pixel del renderer HOI4.
- Le dimensioni delle schede, i filtri dei singoli rami e lo stile del companion sono adattamenti. Condizioni dinamiche, percentuali di ricerca e disponibilità effettiva restano soggette ai limiti del lettore live documentati separatamente.
- La prova nel browser e l'integrazione dell'endpoint sono registrate dal rapporto della sessione principale; i test di questo modulo non vengono presentati come prova visiva del gioco.
