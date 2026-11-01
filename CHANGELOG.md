# Journal des modifications

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et le projet suit le
[versionnage sémantique](https://semver.org/lang/fr/).

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
