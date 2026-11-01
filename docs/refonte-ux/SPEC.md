# Calame 0.3 — Refonte de l'interface « l'encre d'abord »

Document de passation pour Claude Code. Les maquettes de référence sont dans `docs/refonte-ux/maquettes/`
(fichiers `.dc.html` : HTML + styles en ligne). **Elles ne s'exécutent pas seules** : lis-les comme une
source de valeurs exactes (couleurs, tailles, rayons, espacements, textes) et reproduis-les en `egui`.
Ne copie jamais de HTML dans l'application.

| Maquette | Écran de Calame | Fichiers concernés |
|---|---|---|
| `Main.dc.html` | Bibliothèque (accueil) | `home_ui.rs`, `theme.rs`, `icons.rs` |
| `Editeur.dc.html` | Cahier ouvert : sommaire + canevas + outils | `main.rs`, `library_ui.rs`, `toolbar.rs`, `canvas.rs` |
| `Concentration.dc.html` | Mode concentration, thème sombre | `main.rs`, `toolbar.rs`, `theme.rs` |
| `NouveauCahier.dc.html` | Feuille de création ou de modification | `home_ui.rs` (`sheet_window`) |
| `Recherche.dc.html` | Palette Ctrl K | nouveau `palette_ui.rs` |

## Règles du chantier

- Ne retire **aucune fonction existante** : outils, instruments, fonds, import/export, corbeille,
  glisser-déposer, menus contextuels, raccourcis. Seule leur présentation change.
- Tous les textes passent par `i18n::t`. L'interface reste en français.
- Les icônes restent vectorielles et dessinées dans `icons.rs` (trait de 1,8 px, bouts ronds). Les tracés
  SVG des maquettes (viewBox 24) servent de modèle.
- Cibles cliquables d'au moins 40 px (44 px dans le dock).
- Avance par étapes (ci-dessous). Après chaque étape : `cargo build`, `cargo test --workspace`,
  `cargo clippy`, et un résumé des changements. Fais un commit par étape.
- Si une fonction de la maquette demande des données qui n'existent pas (date de modification, pages
  récentes…), vérifie d'abord dans `calame-storage` / `calame-core`, puis propose la modification avant
  de l'écrire.

## 1. Thème (`theme.rs`)

Remplace la palette « couleurs système Apple » par celle-ci.

### Clair

| Rôle | Valeur | Remplace |
|---|---|---|
| Accent (encre) | `#2340C4` (survol `#1A3099`) | `ACCENT` |
| Accent doux (fond de l'élément actif) | `#E6EAFA` | — |
| Safran (surligneur, mise en avant) | `#F4B740` ; fond doux `#FFF4DC`, texte dessus `#7A4D00` | — |
| Fond de l'application | `#F4F5F7` | `grouped_background` |
| Barre latérale | `#FAFBFC` | — |
| Surface (cartes, barres, dock) | `#FFFFFF` | `surface` |
| Bureau autour de la page | `#E9EBEF` + points `#D9DCE3` tous les 22 px | — |
| Papier | `#FFFEFB` ; quadrillage `#EDF0F6` | — |
| Bordure | `#E2E5EB` ; bordure de champ `#D5D9E2` | `separator` |
| Survol | `#EEF0F4` | `fill_hover` |
| Texte | `#15171C` ; texte d'icône `#2B2F38` | `label` |
| Texte secondaire | `#5B616E` | `label_secondary` |
| Succès (« Enregistré ») | `#2F7A5C` | — |
| Bouton principal sombre | fond `#15171C`, texte blanc | — |

### Sombre (mode concentration et thème sombre)

| Rôle | Valeur |
|---|---|
| Fond / bureau | `#0F1014`, points `#1C1E25` |
| Papier | `#17191F`, quadrillage blanc à 4 % |
| Surface (dock, barres) | `#1C1E25` à 92 % d'opacité, bordure blanche à 8 % |
| Texte | `#E7E9EF` ; secondaire `#9CA2B0` |
| Accent | `#9DB0FF` (points `#7F96FF`) |
| Rouge d'encre | `#FF9A86` |

### Couleurs de couverture (remplacent `COVER_COLORS`)

Encre `#2340C4`, Nuit `#15171C`, Brique `#C24E3A`, Safran `#E9A93A`, Sauge `#2F7A5C`, Prune `#7A4FC4`,
Lagon `#167676`, Kraft `#D8CBB0`. Texte blanc sur toutes, sauf Safran et Kraft : texte `#15171C`.
`on_color()` doit choisir la couleur du texte d'après la luminance (contraste minimal de 4,5:1). Garde la
correspondance avec les anciennes couleurs enregistrées (la plus proche, ou conservation telle quelle).

### Formes et typographie

- Rayons : bouton 10–12 px, carte 22 px, feuille ou fenêtre 26 px, dock 20 px, élément du dock 13 px,
  puce 999 px. Couverture : `6, 14, 14, 6` (dos à gauche).
- Ombres douces : carte `0 12 32 -18` noir à 18 % ; dock `0 20 40 -16` à 35 % ; fenêtre `0 40 100 -30`
  à 60 %. Utilise `egui::epaint::Shadow`.
- Polices : **Instrument Sans** (interface, 400/500/600) et **Bricolage Grotesque** (titres, 700),
  toutes deux sous licence OFL. Ajoute les `.ttf` dans `assets/fonts/`, charge-les avec `include_bytes!`
  et `FontDefinitions`, et ajoute leurs licences. Tailles : titre d'écran 40, titre de section 22, titre de
  carte 28, texte 14–15, légende 12–13. Les titres de section en capitales font 12 px, interlettrage
  0,06 em. (Caveat ne sert qu'à simuler l'écriture dans les maquettes : ne l'intègre pas.)

## 2. Bibliothèque (`home_ui.rs`) — `Main.dc.html`

Mise en page : barre latérale fixe de 248–280 px + contenu (marges 36 px en haut, ~56 px sur les côtés).

**Barre latérale** : logo (carré de 32 px à l'accent avec plume blanche et vague safran, d'après
`assets/icon.png`) + « Calame » ; bouton de recherche qui ouvre la palette (indication `Ctrl K`) ;
navigation Accueil / Récents / Favoris / Corbeille (l'élément actif : fond `#E6EAFA`, texte accent,
gras) ; section MATIÈRES avec pastille de couleur, nom, nombre de cahiers et bouton « + » ; en bas,
Réglages et Thème sombre.

**Contenu, de haut en bas :**
1. En-tête : date du jour en secondaire + « Bonsoir. » / « Bonjour. » selon l'heure (titre 40). À droite :
   « Importer un PDF » (secondaire) et « Nouveau cahier » (principal, accent).
2. Rangée « Reprendre » (grille, 2/3 + 1/3) :
   - grande carte de la **dernière page modifiée** : miniature réelle de la page à gauche (rendu existant
     de `render_cache` / export raster), à droite pastille « Reprendre là où vous étiez », nom du cahier,
     « Matière · Dossier · page N sur M », « Modifié il y a … », bouton sombre « Continuer à écrire » ;
   - carte sombre **Note rapide** (Ctrl N) : crée une page dans un cahier « Notes rapides » (créé au
     besoin) et l'ouvre ;
   - zone en pointillés « Glissez un PDF ici » qui accepte le dépôt de fichiers (`egui` `dropped_files`).
3. « Pages récentes » : 5 à 6 miniatures de page au format 3:4 avec titre et pastille du cahier.
4. « Vos cahiers » : puces de filtre par matière (« Tous » actif en sombre), puis grille de couvertures au
   format 5:7 avec dos ombré de 12 px, matière en petites capitales, titre en Bricolage 19 px, légende
   « N pages · date ». Survol : la couverture monte de 4 px et tourne de -0,6°. Dernière case : « Nouveau
   cahier » en pointillés.

Remplace la présentation en étagères par matière. Clic droit sur une couverture : même menu qu'avant.

## 3. Cahier ouvert — `Editeur.dc.html`

**Barre du haut (56 px, blanche)** : « ‹ Bibliothèque », bouton du sommaire, chemin
`Matière / Cahier / **Titre de la page**`, état « ✓ Enregistré » (ou « Enregistrement… »), puis Annuler,
Rétablir, Recherche, Mode concentration, bouton sombre « Exporter ». Les menus Fichier / Affichage
actuels passent dans un bouton « … » ou dans la palette.

**Sommaire (248 px, `#FAFBFC`)** : sélecteur Pages / Aperçu ; arborescence des dossiers (chevron + icône
de dossier + nombre de pages) ; les pages sont des lignes avec **miniature 46×60** + titre + « Page N » ;
page active : fond `#E6EAFA`, contour accent de la miniature. En bas : boutons « + Page » et
« + Dossier ». Garde le glisser-déposer et le menu contextuel actuels.

**Canevas** : bureau à points, page centrée (largeur max. 840 px) avec ombre portée.

**Dock d'outils flottant** (remplace les deux lignes de `toolbar.rs`) : centré en bas, à 20 px du bord,
fond blanc à 96 %, rayon 20 px, ombre.
- Groupe Outils : Stylo, Surligneur, Gomme, Lasso, Texte, Image, Règle et instruments. Outil actif : fond
  `#15171C`, icône blanche. Infobulles avec raccourci (P, H, E, L, T, R : nouveaux raccourcis à ajouter).
- Groupe **Trousse** : stylos préréglés (couleur + épaisseur) affichés en pastilles ; actif = anneau de sa
  couleur. Par défaut : encre bleue fine, rouge moyen, graphite, surligneur jaune. « + » ajoute le réglage
  courant. À enregistrer dans les réglages (`calame-storage`).
- Interrupteur « Formes auto » : actif = fond `#FFF4DC`, texte `#7A4D00`.
- Les réglages fins (palette complète, HSV, hexadécimal, épaisseur, taille de gomme, instruments) s'ouvrent
  dans une **bulle au-dessus du dock** au second clic sur l'outil actif ou au clic sur une pastille de la
  trousse.

**Sélection au lasso** : barre sombre flottante au-dessus de la sélection (couleur, dupliquer, « Redresser
les formes », supprimer), au lieu de la ligne de contexte. Cadre en pointillés accent, poignées carrées
blanches de 9 px.

**Zoom** : pastille en bas à droite « − 100 % + ».

## 4. Mode concentration — `Concentration.dc.html`

F11 ou le bouton de la barre : masque la barre du haut et le sommaire, passe en sombre, garde un mini-dock
(trois stylos de la trousse, gomme, lasso, annuler). En haut au centre, un repère discret
« Cahier · page N » ; en haut à droite « Quitter la concentration · Échap ». Le dessin à l'encre reste
lisible : en sombre, adapte l'affichage des couleurs sombres sans modifier les données de la page.

## 5. Feuille « Nouveau cahier » — `NouveauCahier.dc.html`

Fenêtre centrée de 860 px max sur un voile sombre (`#15171C` à 55 %). À gauche, **aperçu en direct** : la
couverture et, derrière, une page du papier choisi. À droite : Titre (champ de 46 px), Matière (liste
déroulante), Couverture (8 pastilles de 30 px), **Papier** (vignettes : Uni, Petits carreaux, Lignes,
Pointillés, Grands carreaux, et les autres fonds existants), puis « Annuler » et « Créer et commencer à
écrire ». Le papier choisi devient le fond par défaut des nouvelles pages du cahier (à stocker sur le
cahier, voir avec `calame-core::background`).

## 6. Palette Ctrl K — `Recherche.dc.html`

Fenêtre de 660 px placée à 96 px du haut, sur un voile. Champ de recherche de 62 px. Filtres
Tout / Pages / Cahiers / Commandes. Résultats groupés (PAGES avec miniature, DOSSIERS, COMMANDES avec
raccourci), partie trouvée surlignée en safran, élément actif en `#E6EAFA`. Navigation ↑ ↓, Entrée pour
ouvrir, Échap pour fermer. Recherche sur les **noms** (la reconnaissance d'écriture n'existe pas). Les
commandes reprennent les actions des menus actuels.

## Étapes conseillées

1. Thème, polices et icônes (aucun changement de disposition).
2. Barre du haut, sommaire et dock d'outils de l'éditeur.
3. Bulle de réglages, trousse de stylos (avec stockage), barre de sélection du lasso.
4. Nouvelle bibliothèque (sauf « Pages récentes » et « Reprendre » si les données manquent).
5. Feuille « Nouveau cahier ».
6. Palette Ctrl K et raccourcis d'outils.
7. Mode concentration et thème sombre.
8. Pages récentes, Reprendre, Note rapide, dépôt de PDF ; mise à jour de `guide-utilisateur.md` et du
   `CHANGELOG.md` (version 0.3.0).
