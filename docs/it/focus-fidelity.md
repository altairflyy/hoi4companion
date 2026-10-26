# Geometria e composizione dei focus

Il Focus Explorer legge la geometria dalla GUI installata, anziché imporre a
tutti gli alberi le distanze e le dimensioni del renderer precedente. Non cambia
gli ID, le coordinate relative risolte dal parser, i prerequisiti, i criteri di
avanzamento o la lettura live.

## Dati e precedenza

`focus_visuals.py` utilizza il parser Jomini esistente per le definizioni GUI:

- `focus_spacing`: passo orizzontale e verticale;
- `national_focus_item`: contenitore, posizione/centratura del simbolo,
  targhetta e rettangolo del nome;
- `national_focus_link`, `link_begin`, `link_end` e `link_spacing`: ancoraggi
  e dimensioni delle tessere dei collegamenti;
- `national_focus_exclusive_item` e offset di esclusione: simboli, estremità
  e tratti fra focus alternativi.

La raccolta dei file mantiene la precedenza di installazione/mod già usata dal
programma, inclusi i `replace_path`. Un file GUI della mod può avere un nome
diverso da quello originale. Le definizioni successive sostituiscono quelle
omonime; un override numerico invalido non viene spacciato per geometria valida.
Le GUI estranee sono scartate prima del parsing completo. Le dimensioni delle
texture sono lette dagli header solo per le risorse utilizzate, senza convertire
preventivamente tutti gli asset del gioco.

L'API `/api/tree` espone il campo aggiuntivo `visuals`, con versione 1,
provenienza delle definizioni, metriche e risorse. Non rimuove campi precedenti.
`web/focus-layout.js` è un componente puro: produce coordinate, ancoraggi e
tessere, senza DOM, accesso alla memoria o valutazione dei trigger.

## Cambiamenti visivi

Nell'installazione locale verificata, `nationalfocusview.gui` dichiara un passo
di **96×130** e un contenitore **165×128**. Il renderer precedente utilizzava
rispettivamente **100×190** e **174×154**. Il simbolo centrato del template ha
ancora locale `(87,5; 20)`; la targhetta parte da `(5; 40)` e il nome da
`(15; 58)`, con rettangolo 147×20. Sono dati di questa installazione, non
costanti da imporre alle altre configurazioni.

Il frontend compone simbolo e targhetta separatamente, conservando le dimensioni
dell'asset. Il font nominale dichiarato dalla GUI determina la dimensione iniziale
del testo; il browser conserva i propri font disponibili. Se i glifi sono più
larghi, il testo viene ridotto nel rettangolo dichiarato, fino a 10 pixel;
per nomi ancora troppo lunghi compare un'ellissi. Il nome completo rimane
nel tooltip e nell'ispettore. Il badge della guida è posizionato sopra l'emblema.

I collegamenti usano gli ancoraggi GUI, tessere dritte e angolari originali.
I prerequisiti alternativi mantengono tutti i collegamenti e la variante
tratteggiata; non viene scelto un ramo. Il percorso selezionato mantiene
l'evidenziazione della guida. Le coppie di esclusione reciproche vengono
disegnate una sola volta.

Gli atlas installati dei collegamenti contengono verde continuo/tratteggiato
e blu continuo/tratteggiato. Il frame GUI iniziale non dimostra completamento:
il verde viene usato soltanto quando entrambi i focus risultano completati;
il collegamento a un focus attivo può usare il blu quando il precedente risulta
completato. Negli altri casi l'immagine viene desaturata. Non viene dichiarata
la disponibilità dinamica di un focus sulla sola base del colore.

Se mancano geometria o risorse, rimangono disponibili il layout precedente o
il collegamento SVG. Le coppie di esclusione diagonali e sovrapposte conservano
un collegamento esplicito senza inventare una disposizione orizzontale.

## Verifiche e limiti

Test mirati coprono precedenza delle mod, override invalidi, errori di parsing,
atlas e dimensioni, coordinate negative, ancoraggi centrati, collegamenti in
entrambe le direzioni, esclusioni, risorse mancanti e sostituzione dei nodi.
Un test della vista verifica che gli stessi ID siano ancora consegnati alle
evidenziazioni della guida e che i percorsi non verificati non usino il verde.

La verifica nel browser utilizza l'albero tedesco installato (438 focus), con
la guida Kaiserin e localizzazione configurate. Questo controllo visivo e i
riscontri sui file GUI non sono una comparazione pixel per pixel con una
schermata contemporanea del gioco. Non è stato richiesto di portare HOI4 in
primo piano o di cambiare la partita.

Restano limiti espliciti: animazioni/shader, font del gioco, finestre GUI
scriptate, composizioni specifiche dei focus congiunti e selezione delle icone
condizionali non sono un'emulazione completa dell'interfaccia di HOI4. Le
varianti condizionali rimangono anteprime dichiarate. Il percorso ortogonale
fra gli ancoraggi è ricostruito: non è garantita l'identità di ogni incrocio.
Non sono stati modificati renderer tecnologico, HUD o sistema di acquisizione.

Riferimento di confronto: renderer GUI e collegamenti di
[HOI4 Utilities](https://github.com/MillenniumDawn/MD-VSCode-Utility-Tool/tree/main/src/util/hoi4gui).
Non è stata introdotta una dipendenza da VS Code o un secondo parser.
