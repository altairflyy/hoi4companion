# Varianti tecnologiche e disponibilità delle schede

Le definizioni degli script non equivalgono alle tecnologie disponibili nella
configurazione attuale. Ad esempio `landing_craft` e `mtg_landing_craft` hanno
lo stesso nome inglese, ma appartengono rispettivamente a `naval_folder` e
`mtgnavalsupportfolder`, con condizioni opposte su Man the Guns.

Il caricatore conserva tutte le definizioni già lette, insieme a
`declared_folders` e `catalog_eligible` (true/false/null). Quest'ultimo è un
risultato calcolato dalle condizioni statiche delle schede, non un'osservazione
live della ricercabilità. La presenza di una scheda ammessa basta per true;
tutte le schede dichiarate devono essere provatamente escluse per false.
Schede non note, condizioni non risolte e tecnologie prive di scheda restano
null: non vengono scartate arbitrariamente. I sotto-nodi restano disponibili.

Il catalogo per importer/esportazione omette solo i candidati false e i relativi
collegamenti di upgrade. Lo stesso filtro vale per nomi, alias e ID espliciti:
non converte automaticamente un ID esplicito di una variante esclusa in quello
di un'altra. Le definizioni grezze restano disponibili alle altre viste e ai
controlli dei riferimenti. Nessuna regola dipende da un prefisso come `mtg_`, da
una nazione o da una guida specifica.

Verifiche del 24 ottobre 2026: DLC enabled/integrated/disabled/unknown, appartenenze
multiple, tecnologie senza scheda e successori esclusi; 444 test Python e 238
JavaScript superati. Il file esterno Kaiserin fornito dall'utente risolve r-44
in `mtg_landing_craft` con Man the Guns integrato, e supera l'intera validazione
tecnica senza errori. Non è una certificazione di accuratezza della strategia
né della copertura della fonte. Il file originale non viene riscritto e la
guida non viene installata automaticamente.
