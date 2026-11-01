# Journal des modifications

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et le projet suit le
[versionnage sémantique](https://semver.org/lang/fr/).

## [0.2.0] — nouvelle interface

### Interface repensée dans l'esprit des interfaces Apple
- **Écran d'accueil : la bibliothèque.** Chaque matière est une étagère de cahiers affichés comme des
  couvertures colorées ; on choisit un cahier pour l'ouvrir, ou l'on en crée un (le premier lancement
  n'ajoute plus de contenu d'exemple).
- Création et modification d'une matière ou d'un cahier dans une feuille : nom, couleur parmi douze
  couleurs système, pictogramme. Un nouveau cahier s'ouvre avec sa première page.
- **Barre d'outils en icônes** dessinées en vectoriel (stylo, surligneur, gommes, lasso, texte,
  pipette, instruments, formes automatiques, annuler, rétablir, recentrer), avec infobulles ; les
  réglages de l'outil actif (couleurs, épaisseur en pastilles, actions sur la sélection, instruments)
  s'affichent sur une seconde ligne discrète.
- **Sous-matières bien visibles** : boutons « Page » et « Sous-matière » en bas du sommaire du
  cahier, avec l'indication de l'endroit où l'élément sera ajouté ; elles s'imbriquent sans limite.
- Thème clair et sombre aux couleurs système (bleu d'action, fonds groupés, séparateurs fins), menu
  Affichage (thème, sommaire, recentrer), recherche dans la bibliothèque et dans un cahier.
- Les pictogrammes de matière sont des icônes vectorielles ; les anciens émojis sont conservés ou
  traduits.

### Technique
- `calame-core` : `LibraryTree::first_page_in`, `notebook_of` et `page_count`.

## [0.1.0] — première version publique

### Bibliothèque
- Matières, cahiers, sous-dossiers imbriqués sans limite et pages ; nom, couleur et icône des matières.
- Créer, renommer, dupliquer, réordonner, déplacer par glisser-déposer, corbeille avec restauration.
- Recherche par nom ; enregistrement automatique ; stockage SQLite local documenté.

### Dessin
- Canevas vectoriel à zoom et défilement illimités, page infinie en hauteur.
- Stylo sensible à la pression (lissage Catmull-Rom, coins vifs conservés), surligneur.
- Gomme de trait entier et gomme de zone ; lasso (déplacer, redimensionner, pivoter, copier, couper,
  coller, supprimer, recolorer) ; zones de texte ; pipette.
- Couleurs : palette, favoris, sélecteur HSV, saisie hexadécimale. Annuler/rétablir illimités.
- Règle graduée, équerres 45° et 30°/60°, rapporteur, compas : le trait se cale sur leur bord.
- Reconnaissance de formes locale (droite, cercle, ellipse, rectangle, carré, triangle, flèche,
  polygone, courbe lissée) par maintien du stylet ou bouton ; un Ctrl+Z rend le trait d'origine.

### Pages, PDF et export
- Fonds de page : uni, petits et grands carreaux, lignes, pointillés, isométrique, image.
- Import d'un PDF pour l'annoter (rendu en Rust pur) ; import d'images en fond.
- Export d'une page, d'un cahier, d'un dossier ou d'une matière en PDF vectoriel, PNG ou SVG.
- Sauvegarde et restauration complète de la bibliothèque (fichier zip).

### Performances
- Cache de rendu des traits et masquage de ce qui est hors écran : sur une page de 5 000 traits, le
  calcul de ce qu'il faut dessiner prend moins d'une milliseconde par image, et la gomme moins d'une
  milliseconde par échantillon (mesures et limites dans [docs/architecture.md](docs/architecture.md)).
  Le temps de dessin sur le processeur graphique n'est pas mesuré.
- Les SVG exportés ne gardent plus les points invisibles d'un trait.

### Limites connues
- Pas d'accès natif au stylet : le bouton du stylet, l'extrémité gomme, l'inclinaison et le rejet de
  la paume ne sont pas pris en charge (voir [docs/stylet.md](docs/stylet.md)).
- Reconnaissance d'écriture manuscrite non commencée.
- Installeurs non signés ; paquets macOS sous forme d'archive seulement.
