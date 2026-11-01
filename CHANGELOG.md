# Journal des modifications

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et le projet suit le
[versionnage sémantique](https://semver.org/lang/fr/).

## [1.0.0] — « l'encre d'abord »

Nouvelle interface, d'après les maquettes de `docs/refonte-ux`. Aucune fonction n'a été retirée : seule
leur présentation change.

### Apparence
- Palette « l'encre d'abord » : encre bleue `#2340C4`, safran pour la mise en avant, gris bleutés ;
  thème sombre repensé. Huit couleurs de couverture (Encre, Nuit, Brique, Safran, Sauge, Prune, Lagon,
  Kraft), texte choisi d'après le contraste (au moins 4,5:1) ; les anciennes couleurs restent affichées
  telles quelles.
- Polices **Instrument Sans** (interface) et **Bricolage Grotesque** (titres), sous licence OFL,
  intégrées au programme (`assets/fonts`).
- Icônes reprises des tracés des maquettes (lecteur de tracés SVG intégré), trait de 1,8 px à bouts
  ronds ; cibles cliquables d'au moins 40 px (44 px dans le dock).

### Bibliothèque
- Barre latérale : recherche, Accueil, Récents, Favoris, Corbeille, matières avec leur nombre de
  cahiers, Réglages et thème.
- Accueil : date et salutation, **Reprendre là où vous étiez** (miniature réelle de la dernière page
  modifiée), **Note rapide** (Ctrl N), zone de dépôt de PDF (glisser-déposer sur la fenêtre), pages
  récentes, couvertures des cahiers filtrables par matière (nombre de pages, date de modification).
- Cahiers favoris ; corbeille affichée dans la bibliothèque.
- Feuille **Nouveau cahier** avec aperçu en direct : titre, matière, couverture et **papier**, qui
  devient le fond des nouvelles pages du cahier.

### Éditeur
- Barre du haut : retour, sommaire, chemin Matière / Cahier / Page, « ✓ Enregistré », annuler,
  rétablir, recherche, mode concentration, menu « … » (anciens menus Fichier et Affichage), Exporter
  (page ou cahier).
- Sommaire avec miniatures des pages, dossiers repliables, vue **Aperçu** en grille, boutons « + Page »
  et « + Dossier » ; glisser-déposer et menus contextuels conservés.
- Page centrée sur un bureau à points, avec ombre portée ; pastille de zoom « − 100 % + ».
- **Dock d'outils flottant** (stylo, surligneur, gomme, lasso, texte, pipette, instruments), **trousse**
  de stylos préréglés enregistrée, interrupteur « Formes auto », **bulle de réglages** au second clic
  (palette, HSV, hexadécimal, épaisseur, gomme, instruments).
- Sélection au lasso : cadre en pointillés, barre flottante (couleur, dupliquer, couper, **Redresser
  les formes**, supprimer).
- **Mode concentration** (F11) : plein écran sombre, mini-dock ; les encres foncées sont éclaircies à
  l'affichage sans modifier la page.
- **Palette Ctrl K** : recherche sur les noms (sans tenir compte des accents), filtres, résultats
  groupés avec miniatures, commandes des menus, navigation au clavier.
- Raccourcis d'outils : P, H, E, L, T, I, R ; Ctrl E exporte la page en PDF.

### Technique
- `calame-storage` : `Store::page_timestamps` (date de modification des pages) ; nouvelles couleurs
  par défaut des matières.
- Réglages enregistrés : trousse (`pens`), cahiers favoris, papier de chaque cahier, cahier des notes
  rapides.
- Dépendance `chrono` pour l'heure locale.

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
