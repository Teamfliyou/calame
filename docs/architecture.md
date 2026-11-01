# Architecture de Calame

Calame est un workspace Cargo de six crates. Le modèle de données, la reconnaissance de formes, le
stockage et l'export ne dépendent pas de l'interface : ils se testent sans fenêtre.

```
calame-app ──► calame-core ◄── calame-shapes
    │              ▲  ▲
    ├──► calame-storage   └── calame-export
    ├──► calame-export
    ├──► calame-shapes
    └──► calame-input   (types d'entrée ; accès natif au stylet à écrire)
```

| Crate | Rôle | Dépend de |
|---|---|---|
| `calame-core` | Éléments d'une page, historique, arborescence, fonds, instruments, géométrie | — |
| `calame-shapes` | Reconnaissance de formes | core |
| `calame-storage` | SQLite : bibliothèque, pages, images, réglages, sauvegarde zip | core |
| `calame-export` | SVG, PNG, PDF ; import de PDF et d'images | core |
| `calame-input` | Entrée unifiée souris, tactile, stylet | — |
| `calame-app` | Interface `egui`/`eframe`, canevas, outils | toutes |

## Modèle de données (`calame-core`)

Une **page** est une liste ordonnée d'**éléments** : un trait (pinceau, couleur, épaisseur, points avec
pression) ou une zone de texte. Le dernier élément est dessiné au-dessus.

L'**historique** est un seul type de commande, « retirer ces éléments puis en ajouter ceux-là », avec
leurs index. Ajouter, supprimer, déplacer, recolorer, coller, découper à la gomme ou corriger une forme
en sont des cas particuliers, et tous s'annulent de la même façon. Un compteur de révision lui permet de
dire à l'application quand la page a changé (enregistrement automatique).

La **bibliothèque** est un arbre matières > cahiers > sous-dossiers > pages. Les règles de placement
(une page ne va pas directement dans une matière) et le calcul du glisser-déposer vivent dans `core`.

Les **fonds de page** (carreaux, lignes, pointillés, isométrique) y sont décrits comme de la géométrie :
la même fonction fournit les lignes à l'écran et à l'export.

Les **instruments** (règle, équerres, rapporteur, compas) exposent leurs bords, sur lesquels un trait
se projette ; ce sont des outils posés sur la page, pas des éléments de la note.

## Stockage (`calame-storage`)

Un fichier SQLite en mode WAL, un schéma versionné avec migrations testées. Les pages sont sérialisées
avec `postcard` ; leur format est versionné lui aussi, et l'ancien est encore lu. Voir
[format.md](format.md).

La sauvegarde complète est un zip qui contient une copie cohérente de la base (`VACUUM INTO`). La
restauration extrait la base à côté de l'actuelle, la vérifie (schéma, `quick_check`), puis seulement
l'échange ; l'ancienne est conservée.

## Interface (`calame-app`)

- **Canevas** (`canvas.rs`) : machine à états des gestes (tracé, gommes, lasso, transformation,
  instruments) alimentée par une entrée unifiée. Le tactile porte la pression ; la souris est
  complétée par 0,5. Le trait en cours est dessiné séparément des traits terminés.
- **Rendu** : chaque trait est une bande de triangles d'épaisseur variable (`egui::Mesh`) avec des
  extrémités rondes. `render_cache.rs` garde, pour chaque trait, son lissage et son rectangle
  englobant ; seuls les traits qui touchent la zone visible sont dessinés.
- **Tâches de fond** (`files.rs`) : l'import d'un PDF et l'export tournent dans un fil séparé ; le
  résultat revient par un canal et l'interface continue de répondre.
- **Textes** : tous passent par `i18n::t`, en attendant un passage à `fluent`.

## Reconnaissance de formes (`calame-shapes`)

Le tracé est rééchantillonné (96 points). Selon qu'il est fermé ou non :

- **ouvert** : droite (écart maximal à la corde), flèche (simplification de Douglas-Peucker, cinq
  sommets de forme précise), sinon courbe lissée ;
- **fermé** : cercle ou ellipse (axes principaux, puis cercle de Kåsa si presque rond), rectangle
  (rectangle d'aire minimale), triangle ou polygone (sommets de Douglas-Peucker).

Chaque forme candidate reçoit un score : l'écart moyen entre le tracé et son contour, rapporté à la
diagonale du tracé ; une couverture minimale écarte les formes qui n'en suivent qu'une partie. La
meilleure sous son seuil l'emporte. Le jeu de tests couvre chaque forme tournée, mal fermée, bruitée,
plus 400 tracés aléatoires.

## Export (`calame-export`)

Une page est décrite en **SVG** (fond, traits en contours remplis, textes), puis :

- rastérisée en **PNG** par `resvg` ;
- convertie en **PDF** vectoriel par `svg2pdf`, une page PDF par SVG ;
- importée : `hayro` rend chaque page d'un PDF en image, qui devient le fond d'une page.

## Performances

Mesures sur la machine de développement (Windows 11, mode `release`), page de
**5 000 traits et 1 000 000 de points**. Elles se relancent avec
`cargo test --release --workspace -- --ignored --nocapture`.

| Opération | Mesure |
|---|---|
| Calcul initial du lissage et des rectangles de tous les traits | 157 ms (une fois par page ouverte) |
| Même parcours une fois en cache | 0,7 ms |
| Choix des traits visibles pour un écran typique | 0,6 ms (289 traits sur 5 000) |
| Gomme de zone, par échantillon du stylet | 0,9 ms |
| Gomme de trait entier, par échantillon | 0,4 ms |
| Enregistrement de la page (17 Mo) | 42 ms |
| Chargement de la page | 30 ms |
| Reconnaissance d'un cercle de 121 points | 0,7 ms |
| Reconnaissance d'une courbe de 3 000 points | 0,2 ms |

Export d'une page dense de 1 500 traits (120 000 points) :

| | Avant l'allègement des points invisibles | Après |
|---|---|---|
| Taille du SVG | 13,1 Mo | 5,3 Mo |
| Taille du PDF | 3,4 Mo | 1,2 Mo |
| Taille du PNG | 604 Ko | 595 Ko |

Les durées (SVG, PNG et PDF entre 0,5 et 1,5 s) varient d'une mesure à l'autre sur cette machine ; elles
ne sont pas assez stables pour conclure à une accélération.

**Ce qui n'est pas mesuré** : le temps de dessin par le processeur graphique et la latence réelle
entre le stylet et l'écran. Ils dépendent du pilote graphique, de l'écran et de la synchronisation
verticale, et ne se mesurent pas sans affichage. `CALAME_VSYNC=0` retire l'attente de la
synchronisation verticale. Aucune prédiction de trajectoire n'est faite.

## Choix à connaître

- **Pas d'index spatial** (`rstar`, prévu au départ) : un parcours linéaire des rectangles englobants
  en cache coûte moins d'une milliseconde pour 5 000 traits, ce qui suffit. À reconsidérer au-delà de
  plusieurs dizaines de milliers de traits sur une seule page.
- **Toolchain Windows** : le développement s'est fait avec `stable-x86_64-pc-windows-gnu` et MinGW-w64,
  faute de Build Tools Visual Studio. La CI et les paquets de publication utilisent MSVC.
- **Versions de dépendances** : `resvg` et `hayro` sont dans des versions compatibles avec la version Rust
  minimale du projet (1.80), un peu plus anciennes que les dernières publiées.
