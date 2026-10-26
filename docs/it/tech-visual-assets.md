# Risorse native della vista Tecnologie

Verifica statica del 20 ottobre 2026, sulla copia installata di Hearts of Iron IV. Nessun file di gioco modificato. Le nove schermate fornite come riferimento visivo e le misure seguenti sono lette nei file GUI/GFX e negli header delle texture DDS, non dedotte dalle dimensioni ridotte degli screenshot.

## Risultati osservati

`technology_visuals.load_technology_visuals(gui_files, gfx_files, sprite_library=None)` ricostruisce 23 template e decorazioni per 12 cartelle GUI, comprese varianti alternative selezionate dal gioco in base ai DLC. Questi conteggi non significano che tutte le 12 cartelle siano visibili contemporaneamente. Il filtro delle cartelle rimane affidato al catalogo e ai DLC.

| Famiglia visibile | Dimensioni texture, unità native |
| --- | --- |
| Piccola, senza nome | 72 × 72 |
| Equipaggiamento nominato, fanteria/carri/artiglieria/aerei | 183 × 84 |
| Scafi navali nominati | 332 × 69 |
| Piccola con cornice progetto speciale | 92 × 92 |
| Aerei con cornice progetto speciale | 203 × 104 |
| Scafi con cornice progetto speciale | 352 × 89 |

Le dimensioni dichiarate dal contenitore GUI non sono sempre le dimensioni disegnate. Per esempio `techtree_mtgnavalfolder_item`, a `countrytechtreeview.gui:5334`, dichiara 204 × 72; la sua texture di stato, identificata in `countrytechtreeview.gfx:787`, misura **332 × 69**. Il centro dell'icona a x=167 e il campo nome largo 270 corroborano la larghezza della texture. Industria ed elettronica dichiarano anch'esse 204 × 72 ma usano sfondi **72 × 72**, senza campo nome. Usare uniformemente il contenitore come rettangolo visibile distorce le distanze dei rami.

I quattro stati grafici sono risorse distinte: `unavailable`, `available`, `researched`, `currently_researching`. La presenza di una texture non prova che una tecnologia sia ricercabile nella partita. Il caricatore restituisce le immagini; il modello live decide quali stati siano realmente noti. Le animazioni sono intenzionalmente rappresentate dal solo **frame 1**: il numero di frame viene conservato nei metadati e le dimensioni vengono divise per il numero di frame dichiarato. Non viene simulata l'animazione del gioco.

Le cornici dei progetti speciali superano di 10 unità per lato la forma normale. Si possono disegnare come decorazione esterna senza traslare i centri del ramo. Le forme storiche `armour_folder_small` e `secret_weapons_folder` prive di una famiglia specifica completa conservano il fallback esplicito del GFX: non sono le cartelle attive degli screenshot della Romania.

## Sfondi e posizioni

Le coordinate delle decorazioni sono relative all'origine del contenuto della cartella e non includono l'offset del viewport scorrevole a y=47. I layer restano nell'ordine dichiarato, dietro collegamenti e nodi.

- Fanteria: `GFX_infantry_techtree_bg`, 959 × 715, origine 0,0.
- Supporto: `GFX_support_techtree_bg`, 977 × 628; riquadri `GFX_tiled_research_bg` alle origini 150,720 e 160,1075.
- Carri: bande `GFX_techtree_stripes`, contenitore 1900 × 1090 per la variante NSB.
- Artiglieria: bande 1515 × 1390 e `GFX_artillery_techtree_bg`, 974 × 649.
- Scafi e supporto navale: `GFX_naval_techtree_bg`, 1230 × 593.
- Aerei BBA: bande 2240 × 1300 e `GFX_air_techtree_bg`, 948 × 589.
- Industria: `GFX_industry_techtree_bg`, 1016 × 761.
- Ingegneria: `GFX_engineering_techtree_bg`, 1071 × 766; `GFX_wonderweapons_bg`, 586 × 890, origine 915,0.

I background decorativi sono definiti in `countrytechnologyview.gfx:26–32,62,154–169`; bande e riquadri sono in `countrytechtreeview.gfx:24–46`. Gli elementi `corneredTileSpriteType` conservano dimensioni, bordo e flag di ripetizione. La loro resa nel browser deve rispettare il tiling; un semplice stretch della texture produce bande o bordi errati.

## Identità delle fonti

SHA-256:

```
countrytechtreeview.gui  fec641fb738ab59369cac40ae902fdba793bed7cbaa2c807756207a2e1f01045
countrytechtreeview.gfx  a65a41f2a3d94b62c38ddd7311d2606cd4a8817872f374893d851f6ec517dc8c
countrytechnologyview.gfx  24b1c515ddfed282c757089354b94a7aeac7c7012154b334b5c37fa8b78658bf
```

Sono osservazioni su risorse distribuite, non risultati di una cattura dell'esecuzione. Non è stato necessario aprire un disassemblatore per leggere strutture GUI e texture. Test locali del modulo: 3 superati; verificano dimensioni DDS e atlas, origine annidata delle decorazioni, campi nome/icone, risorse assenti e percorsi non validi. Il confronto nel browser della vista integrata è separato da questi test.
