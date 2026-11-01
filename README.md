# Calame

Application de bureau **open source** de prise de notes manuscrites, écrite entièrement en **Rust**,
pensée pour les stylets (Wacom, Surface Pen…) et les tablettes graphiques. Elle fonctionne aussi à la
souris et au tactile. Cibles : Windows (priorité), Linux, macOS.

> Le calame est le roseau taillé qui servait de plume dans l'Antiquité.

**État : développement précoce (phase 6 sur 8).** Voir la feuille de route plus bas.

## Fonctionnalités

Disponible :

- Canevas vectoriel avec zoom (Ctrl + molette) et déplacement (molette, bouton du milieu)
- Stylo sensible à la pression, lissage Catmull-Rom, surligneur, épaisseurs prédéfinies et curseur
- Gommes : trait entier ou zone (les traits sont coupés là où passe la gomme)
- Lasso : déplacer, redimensionner, pivoter, copier, couper, coller, supprimer, recolorer
- Zones de texte tapées au clavier (déplaçables, modifiables, recolorables)
- Couleurs : palette, favoris, sélecteur HSV, saisie hexadécimale, pipette
- Reconnaissance de formes locale (crate `calame-shapes`, sans service externe) : droite, cercle, ellipse,
  rectangle, carré, triangle, flèche, polygone, courbe lissée. On la déclenche en maintenant le stylet
  immobile en fin de trait, ou avec le bouton « Formes automatiques ». Un seul Ctrl+Z rend le trait
  d'origine
- Instruments : règle graduée, équerre 45° et 30°/60°, rapporteur, compas. On les déplace, tourne
  et redimensionne ; un trait qui commence près d'un bord s'y cale (droite, ou arc de cercle)
- Annuler / Rétablir illimités (Ctrl+Z, Ctrl+Maj+Z / Ctrl+Y)
- Thèmes clair et sombre, interface en français
- Bibliothèque matières > cahiers > sous-dossiers (imbriqués sans limite) > pages, avec nom, couleur
  et icône pour les matières
- Créer, renommer, dupliquer, réordonner, déplacer par glisser-déposer, corbeille avec restauration
- Recherche par nom dans l'arborescence
- Stockage SQLite local ([format documenté](docs/format.md)), enregistrement automatique

Prévu : rejet de la paume, gomme automatique du stylet, export PDF/PNG/SVG, import PDF,
fonds de page, sauvegarde/restauration en zip.

## Compilation

Prérequis : Rust stable (https://rustup.rs).

```bash
cargo run -p calame-app --release
cargo test --workspace
```

Sous Windows, il faut soit les *Build Tools* de Visual Studio (cible `msvc`), soit un MinGW-w64 dans le
`PATH` avec la toolchain `stable-x86_64-pc-windows-gnu`.

## Structure

| Crate | Rôle |
|---|---|
| `calame-core` | Modèle de données (traits, pages, historique, arborescence), sans dépendance à l'interface |
| `calame-shapes` | Reconnaissance de formes : ajustements géométriques, indépendante de l'interface |
| `calame-storage` | Stockage SQLite : hiérarchie, corbeille, contenu des pages *(zip de sauvegarde à venir)* |
| `calame-export` | Export PDF/PNG/SVG, import PDF *(à venir)* |
| `calame-input` | Entrée unifiée souris/tactile/stylet ; backends natifs *(à venir)* |
| `calame-app` | Interface `egui`/`eframe`, canevas et outils |

## Stylet et pression

La pression arrive aujourd'hui via les événements tactiles de `winit`/`egui`. Le bouton du stylet,
l'extrémité gomme et le rejet de paume demandent un accès natif (`WM_POINTER` sous Windows,
XInput2/`tablet-v2` sous Linux, `NSEvent` sous macOS), prévu dans `calame-input`.

## Licence

Double licence [MIT](LICENSE-MIT) ou [Apache-2.0](LICENSE-APACHE), au choix.
