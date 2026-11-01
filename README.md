# Calame

Application de bureau **open source** de prise de notes manuscrites, écrite entièrement en **Rust**,
pensée pour les stylets (Wacom, Surface Pen…) et les tablettes graphiques. Elle fonctionne aussi à la
souris et au tactile. Cibles : Windows (priorité), Linux, macOS.

> Le calame est le roseau taillé qui servait de plume dans l'Antiquité.

**État : développement précoce (phase 2 sur 8).** Voir la feuille de route plus bas.

## Fonctionnalités

Disponible :

- Canevas vectoriel avec zoom (Ctrl + molette) et déplacement (molette, bouton du milieu)
- Stylo sensible à la pression, lissage Catmull-Rom, surligneur
- Annuler / Rétablir illimités (Ctrl+Z, Ctrl+Maj+Z / Ctrl+Y)
- Thèmes clair et sombre, interface en français

Prévu : matières > cahiers > sous-dossiers > pages avec stockage SQLite, gommes, lasso, règle,
équerre, rapporteur, compas, reconnaissance de formes locale, export PDF/PNG/SVG, import PDF,
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
| `calame-core` | Modèle de données (traits, pages, historique), sans dépendance à l'interface |
| `calame-shapes` | Reconnaissance de formes *(à venir)* |
| `calame-storage` | Stockage SQLite, sauvegarde automatique, zip *(à venir)* |
| `calame-export` | Export PDF/PNG/SVG, import PDF *(à venir)* |
| `calame-input` | Entrée unifiée souris/tactile/stylet ; backends natifs *(à venir)* |
| `calame-app` | Interface `egui`/`eframe`, canevas et outils |

## Stylet et pression

La pression arrive aujourd'hui via les événements tactiles de `winit`/`egui`. Le bouton du stylet,
l'extrémité gomme et le rejet de paume demandent un accès natif (`WM_POINTER` sous Windows,
XInput2/`tablet-v2` sous Linux, `NSEvent` sous macOS), prévu dans `calame-input`.

## Licence

Double licence [MIT](LICENSE-MIT) ou [Apache-2.0](LICENSE-APACHE), au choix.
