# Verifica della vista tecnologica

20 ottobre 2026. Integrazione successiva a [technology-fidelity.md](technology-fidelity.md), che conserva sorgenti, hash e confronto con i nove screenshot del gioco forniti dall'utente.

## Risultato

La pagina `/technologies` usa rettangoli e coordinate GUI senza il precedente ingrandimento delle distanze di tre volte. Distingue icone compatte 72×72, equipaggiamenti 183×84 e scafi navali 332×69. Mostra sfondi, cornici, progetti speciali, intestazioni e anni dichiarati dall'installazione locale. Le cornici animate usano un fotogramma statico. Le cornici estensibili dei gruppi di supporto usano il bordo a nove porzioni dichiarato dal GFX, senza ripetere il riquadro intero a griglia; un test di regressione copre questa distinzione rispetto alle strisce ripetute.

Il canvas occupa l'area disponibile; il pannello dei dettagli e i filtri dei rami sono richiudibili. La vista iniziale conserva tutta la scheda del gioco. `Adatta scheda` applica una scala uniforme, mentre zoom e trascinamento permettono di leggere i particolari. I nomi nazionali degli equipaggiamenti precedono le etichette generiche: verificati Mannlicher M1893, ZB vz.24, Orita M1941 e AFB 3-ton per ROM.

## Verifica nel browser reale

Le nove schede sono state aperte tramite l'interfaccia e controllate sul DOM, con caricamento confermato dall'intestazione. Il viewport osservato era circa 1033×663 pixel per il grafo; lo zoom automatico varia secondo l'estensione della scheda.

| Scheda | Nodi nella scheda | Compatti | Equipaggiamenti |
|---|---:|---:|---:|
| Fanteria | 34 | 20 | 14 |
| Compagnie di supporto | 42 | 36 | 6 |
| Corazzati | 28 | 14 | 14 |
| Artiglieria | 27 | 16 | 11 |
| Navale | 38 | 18 | 20 |
| Supporto navale | 39 | 39 | 0 |
| Aerea | 41 | 23 | 18 |
| Ingegneria | 27 | 27 | 0 |
| Industriale | 43 | 43 | 0 |

Totale 319 appartenenze alla scheda, su 326 nel catalogo. I sette discendenti di radici fanteria escluse dai DLC non compaiono nella scheda principale. Verificato il filtro esplicito `Altri dati del catalogo · 7`: mostra sette elementi e una nota che li distingue dalla scheda del gioco.

Verificata ricerca `Mannlicher`: un nodo evidenziato, selezione `infantry_weapons` e pannello Dettagli con il nome corretto. Il tooltip da tastiera mostra nome, stato Ricercata, anno e descrizione. Nessun errore JavaScript nei log osservati. Nelle nove schede non sono stati osservati segnaposto o immagini fallite al controllo; gli screenshot acquisiti immediatamente dopo un cambio possono precedere il completamento delle immagini caricate in differita. Il passaggio finale ha acquisito nuovamente tutte e nove le schede: tutte le immagini dei nodi risultavano caricate, zero immagini fallite. Il conteggio include anche le cornici separate dei progetti speciali, quindi può superare il numero di tecnologie.

Le acquisizioni e il riepilogo DOM sono nella cartella di lavoro della chat `outputs/technology-fidelity-*.png` e `outputs/technology-fidelity-browser.json`, `outputs/technology-fidelity-browser-final.json` e `outputs/technology-fidelity-runtime.json`. Non vengono distribuiti nel progetto gli asset del gioco.

## Test e stato del gioco

- `PYTHONPATH=src py -3 -m unittest discover -s tests`: **103 test superati**.
- `node --test tests/test_*.cjs`: **9 test superati**.
- Endpoint catalogo, scheda e stato: HTTP 200 dopo il riavvio del solo companion.
- HOI4 rimasto aperto con PID 12988. Durante i controlli il lettore ha mostrato ROM e, alla fine, POL; durante una transizione ha temporaneamente rifiutato il paese non identificato con certezza e non ha colorato dati incerti. Questa verifica non ripete la prova del cambio di focus, documentata separatamente. Il catalogo delle icone resta quello nazionale configurato, ROM.

## Contenitori centrati

La centratura `orientation=center_up` / `origo=center_up` e la dimensione minima `min={width=100%%}` sono applicate ai contenitori: in particolare allineano le intestazioni dell'artiglieria con i rispettivi rami e collocano gli anni sul lato destro del viewport. Anche le strisce di sfondo rispettano la dimensione minima del contenitore. Un test ricostruisce il centro dell'intestazione Artiglieria (x=1301 con larghezza 400) rispetto alla GUI e allo screenshot.

## Intestazione PESANTI

La GUI installata contiene `position = { x = 275k y = 50 }` sull'intestazione `armour_subtitle_heavy` (countrytechtreeview.gui:1085). Il parser numerico conservativo la scartava, facendo sparire PESANTI. La ricostruzione corregge soltanto questo nome e questo valore a x=275, coerente con la posizione della didascalia nello screenshot fornito; altri valori malformati continuano a essere rifiutati. È una correzione locale corroborata dall'immagine, non una regola generale dimostrata del parser dell'engine. Un test copre il limite preciso della correzione.

## Limiti

La disposizione è ricostruita e confrontata con punti di riferimento degli screenshot, non certificata pixel per pixel per ogni nodo. Shader, font proprietario e animazioni del motore non sono riprodotti. I colori Ricercata/In ricerca derivano da dati osservati; il grigio degli altri nodi non prova che siano irricercabili. Condizioni dinamiche, bonus di ricerca e disponibilità corrente restano da verificare nel gioco. Le risorse originali sono lette soltanto dall'installazione locale; nessun file di HOI4 è stato modificato.
