# Guide d'utilisation de Calame

Calame prend des notes manuscrites avec un stylet, une souris ou le doigt. Tout est enregistré
automatiquement sur votre ordinateur, sans compte ni connexion.

## Organiser ses notes

Le panneau de gauche est la **bibliothèque**. Elle a quatre niveaux :

- **Matières** (Maths, Histoire…), avec un nom, une couleur et une icône ;
- **Cahiers**, rangés dans une matière ;
- **Sous-dossiers**, dans un cahier ou un autre sous-dossier, sans limite de profondeur ;
- **Pages**, dans un cahier ou un sous-dossier.

Clic droit sur un élément : créer, renommer, dupliquer, monter ou descendre, exporter, mettre à la
corbeille. « Style… » (matières) change la couleur et l'icône. On déplace un élément en le glissant
sur un conteneur (il y entre) ou sur un autre élément (il se place juste avant). La **corbeille**, en
bas du panneau, restaure ou supprime définitivement. Le champ de recherche filtre par nom.

## Écrire et dessiner

| Outil | Rôle |
|---|---|
| **Stylo** | Trait lissé dont l'épaisseur suit la pression du stylet |
| **Surligneur** | Trait large et translucide |
| **Gomme (trait)** | Efface les traits entiers touchés |
| **Gomme (zone)** | Coupe les traits là où elle passe |
| **Lasso** | Entoure des éléments pour les déplacer, redimensionner, pivoter, copier, recolorer |
| **Texte** | Clic sur la page pour taper ; clic sur un texte pour le modifier |
| **Pipette** | Prend la couleur du trait ou du texte cliqué |
| **Instruments** | Règle, équerres, rapporteur, compas (voir plus bas) |

La deuxième ligne de la barre d'outils règle la **couleur** : palette, favoris (★+ pour ajouter,
clic droit pour retirer), sélecteur complet et code hexadécimal. La troisième ligne règle
l'**épaisseur** (préréglages et curseur), la taille de la gomme ou du texte.

**Lasso.** Entourez des éléments : une boîte apparaît. Glissez dedans pour déplacer, tirez un coin
pour redimensionner, tirez la pastille ronde du dessus pour pivoter. « Appliquer la couleur »
recolore la sélection.

### Se déplacer dans la page

La page n'a pas de bord : elle grandit vers le bas sans limite. **Ctrl + molette** zoome, la
**molette** fait défiler, le **bouton du milieu** déplace la vue. « Recentrer » revient au départ.

### Annuler

**Ctrl+Z** annule, **Ctrl+Maj+Z** (ou Ctrl+Y) rétablit, sans limite. L'historique d'une page est
conservé tant que l'application reste ouverte, même si l'on change de page.

## Formes parfaites

Tracez un cercle, un rectangle, un triangle, une flèche ou une droite, puis **maintenez le stylet
immobile** une demi-seconde sans le lever : le trait devient une forme parfaite. Le bouton
**Formes automatiques** fait la même chose pour chaque trait, au relâchement. Reconnues : droite,
cercle, ellipse, rectangle, carré, triangle, flèche, polygone, courbe lissée.

**Ctrl+Z** rend le trait d'origine. Une flèche se trace en un seul geste : la tige, puis une aile,
retour à la pointe, puis l'autre aile.

## Règles et instruments

Choisissez l'outil **Instruments** puis « + Règle », « + Équerre », « + Rapporteur » ou « + Compas ».
Glissez le corps pour déplacer ; la pastille orange pivote, le carré vert agrandit. Le compas a une
pastille bleue (pointe sèche, déplace tout) et une rouge (mine, règle l'écartement).

Revenez au stylo : **un trait qui commence près d'un bord s'y cale** (droite le long d'une règle ou
d'une équerre, arc le long du rapporteur, cercle autour du compas). Un repère orange indique où le
trait s'accrochera. Les instruments ne font pas partie de la note : ils ne sont pas enregistrés ni
exportés.

## Fonds de page

Menu **Fond de page** : uni, petits carreaux (5 mm), grands carreaux (8 mm), lignes, pointillés,
isométrique, ou **Image…** (PNG ou JPEG) posée en haut à gauche de la page.

## PDF : importer, annoter, exporter

- **Importer un PDF** (menu Fichier, ou clic droit sur une matière, un cahier ou un dossier) : chaque
  page devient une page de Calame avec la page du PDF en fond. Écrivez par-dessus.
- **Exporter** (menu Fichier pour la page ouverte, clic droit pour n'importe quel élément) : PDF
  vectoriel multipage, PNG ou SVG. Un cahier ou un dossier donne un seul PDF, ou un fichier par page
  pour PNG et SVG.

L'import et l'export se font en arrière-plan : un indicateur s'affiche sous la barre d'outils.

## Sauvegarde

Calame enregistre une page une seconde après votre dernière modification, au changement de page et à
la fermeture. La bibliothèque est un fichier SQLite unique (voir [format.md](format.md)).
**Fichier ▸ Sauvegarder la bibliothèque** écrit une copie complète dans un fichier zip, et
**Restaurer une sauvegarde** la remet en place (l'ancienne bibliothèque est gardée à côté).

## Raccourcis

| Raccourci | Action |
|---|---|
| Ctrl+Z | Annuler |
| Ctrl+Maj+Z, Ctrl+Y | Rétablir |
| Ctrl+C, Ctrl+X, Ctrl+V | Copier, couper, coller la sélection (entre pages) |
| Ctrl+A | Tout sélectionner (passe au lasso) |
| Suppr, Retour arrière | Supprimer la sélection |
| Échap | Désélectionner |
| Ctrl + molette | Zoom |
| Molette | Défiler |
| Bouton du milieu | Déplacer la vue |

Les raccourcis sont suspendus pendant la saisie d'un texte ou d'un nom.
