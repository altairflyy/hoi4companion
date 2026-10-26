# Fedeltà della geometria tecnologica dopo gli screenshot del gioco

20 ottobre 2026. Questo rapporto corregge e sostituisce le ipotesi geometriche della prima ricostruzione descritta in `technology-layout.md`. La prima versione conservava i nomi delle schede e i gruppi, ma ometteva centratura delle griglie e dimensioni dei veri elementi GUI. Inoltre ingrandiva uniformemente tutte le schede: non riproduceva la disposizione mostrata dagli screenshot dell'utente.

## Risorse e riscontri

Le nove immagini di riferimento fornite sono screenshot del gioco (`Screenshot 2026-10-20 185*.png`), non screenshot del companion. La verifica geometrica utilizza i file installati del gioco (`interface/countrytechtreeview.gui`) e non richiede accesso o modifiche al processo.

`interface/countrytechtreeview.gui`, SHA-256 `fec641fb738ab59369cac40ae902fdba793bed7cbaa2c807756207a2e1f01045`, contiene:

- griglie con origine, dimensione, passo delle celle e formato `LEFT`, `UP`, `UPPER_LEFT`;
- template `techtree_{folder}_item` e `techtree_{folder}_small_item`, con offset diversi, per esempio `(-56,-7)` per l'equipaggiamento di fanteria e `(-55,0)` per i carri;
- testi con larghezza, altezza, allineamento e font;
- cartelle con margini sinistro 13, destro 25 e superiore 13.

I template dichiarano talvolta dimensioni diverse dal rettangolo realmente visibile. La lettura degli header DDS delle cornici, effettuata dal modulo di risorse visive, conferma **72×72** per gli elementi compatti, **183×84** per l'equipaggiamento ordinario e **332×69** per gli equipaggiamenti navali. Per esempio il template industriale dichiara 204×72, ma mostra uno sfondo quadrato di 72×72 e non ha il campo `Name`. Le cornici dei progetti speciali possono sporgere di 10 pixel per lato, senza spostare la geometria di base.

## Regole corrette

Il canvas lavora in unità GUI native, su un viewport di riferimento di 2560 pixel. L'area interna larga 2522 pixel deriva da `2560 - 13 - 25`; serve soprattutto per le griglie la cui larghezza è espressa in percentuale. Lo zoom del companion può poi scalare l'intero risultato senza cambiare la distanza relativa dei rami.

1. `UP`: la griglia centra le celle sull'asse orizzontale: all'origine si aggiunge `width/2 - slot_width/2`. Le coordinate delle tecnologie mantengono gli assi.
2. `LEFT`: gli assi dei dati vengono scambiati; il centro trasversale deriva da `height/2 - slot_height/2`.
3. `UPPER_LEFT`: le coordinate dei dati sono applicate direttamente, senza centratura della griglia.
4. All'ancora così ottenuta si applicano gli offset del template specifico. Il risultato esposto dall'API è il rettangolo visibile `x,y,width,height`, insieme al nome esatto del template e al suo centro.
5. La variante ampia è determinata da equipaggiamenti sbloccati o da `show_equipment_icon=yes`; `force_use_small_tech_layout=yes` mantiene l'elemento compatto. La presenza del campo `Name` nel template distingue le schede realmente dotate di titolo dalle icone quadrate.

`MTG_naval.txt:1077–1079` dichiara `ignore_for_layout=yes` sul collegamento dallo scafo pesante alla corazza pesante. La prima ricostruzione ignorava questo campo e assegnava ambiguamente le corazze a due griglie. Ora tali collegamenti non partecipano alla scelta della griglia e sono esposti separatamente in `layout_ignored_paths`, preservando il dato originale per i dettagli.

## Contenuti aggiuntivi e DLC

La configurazione locale continua a includere 9 schede e 324 tecnologie distinte, per 326 appartenenze alle schede. **319 appartenenze** hanno ora la geometria nativa ricostruita.

I sette elementi residui della fanteria discendono da radici escluse dalle condizioni di ramo: `rangers_tech` richiede No Compromise, No Surrender (`infantry.txt:876–877`); `tech_special_forces` richiede l'assenza di Arms Against Tyranny (`infantry.txt:999`). I discendenti non ripetono il filtro sul proprio blocco. Non è corretto aggiungerli automaticamente come un ramo ordinario visibile: vengono conservati nei dati con `layout_hidden_reason` e nell'elenco `folder.layout.hidden`, per una consultazione aggiuntiva esplicita.

## Controllo sui landmark delle immagini

Le immagini sono larghe 2559 pixel; il riferimento di layout è 2560. I centri sono stati identificati manualmente sulle anteprime delle immagini e convertiti nella risoluzione originale. Non sono misure subpixel automatiche. L'origine del contenuto del gioco è circa `(10,60)`, coerente con posizione del pannello e margini dichiarati.

| Elemento | Screenshot | Errore approssimativo del centro |
| --- | --- | ---: |
| Macchine utensili di base | 185644 | 1,3 px |
| Industria concentrata | 185644 | 1,4 px |
| Costruzione I | 185644 | 1,5 px |
| Equipaggiamento fanteria iniziale | 185555 | 3,3 px |
| Marines I | 185555 | 2,9 px |
| Scafo interguerra | 185611 | 4,6 px |
| Artiglieria interguerra | 185617 | 5,4 px |

Il dettaglio, gli hash degli screenshot e i rettangoli attesi sono conservati in [technology-fidelity-evidence.json](technology-fidelity-evidence.json). Questi confronti corroborano le regole di centratura e gli offset, soprattutto rispetto all'errore macroscopico precedente. Non dimostrano che ogni etichetta e ogni sprite coincidano pixel per pixel nelle nove categorie.

## Test e limiti

Otto test del modulo geometrico coprono le tre orientazioni, i rettangoli di tre categorie ricavati da GUI e screenshot, percentuali e margini, centratura trasversale delle griglie navali, archi esclusi dal layout, radici filtrate, cicli e associazioni ambigue. I test del catalogo esistente restano separati.

Le verifiche visuali nel browser e la composizione delle cornici sono responsabilità dell'integrazione frontend. Questo modulo fornisce coordinate, template, metadati dei titoli e motivi di esclusione; non disegna elementi e non modifica file del gioco. La geometria resta una ricostruzione corroborata, con riferimento di viewport dichiarato: impostazioni UI del gioco differenti possono richiedere un'altra scala o dimensione di riferimento.
