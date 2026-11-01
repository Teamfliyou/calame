# Format de stockage de Calame

La bibliothèque est un fichier **SQLite** unique (`calame.db`), en mode WAL.
Sous Windows : `%APPDATA%\calame\Calame\data\calame.db`. La variable d'environnement `CALAME_DB` permet
de choisir un autre fichier.

La version du schéma est dans `PRAGMA user_version` (actuellement **3**). Une base créée par une
version plus récente est refusée plutôt qu'abîmée.

## Table `nodes` : la hiérarchie

| Colonne | Type | Rôle |
|---|---|---|
| `id` | TEXT, clé | UUID de l'élément |
| `parent_id` | TEXT, nullable | Parent ; `NULL` pour une matière. Suppression en cascade |
| `kind` | TEXT | `subject`, `notebook`, `folder` ou `page` |
| `name` | TEXT | Nom affiché |
| `color` | INTEGER | Couleur RGBA empaquetée `0xRRGGBBAA` |
| `icon` | TEXT | Icône (un caractère ou emoji) |
| `position` | INTEGER | Rang parmi les frères |
| `created_at` | INTEGER | Secondes Unix |
| `trashed_at` | INTEGER, nullable | Secondes Unix ; non nul = dans la corbeille |
| `background` | TEXT | Fond de la page : `blank`, `grid-small`, `grid-large`, `lines`, `dots`, `isometric` ou `asset:<uuid>` (schéma 3) |

Règles de placement : une matière est à la racine ; un cahier est dans une matière ; un
sous-dossier ou une page est dans un cahier ou un sous-dossier. Les sous-dossiers s'imbriquent sans
limite. Mettre un élément à la corbeille marque tout son sous-arbre ; le restaurer le démarque.

## Table `page_data` : le contenu des pages

| Colonne | Type | Rôle |
|---|---|---|
| `page_id` | TEXT, clé | Référence `nodes.id` (suppression en cascade) |
| `format` | INTEGER | Version du codec : **2** (éléments), **1** (traits seuls, encore lu) |
| `data` | BLOB | Page sérialisée avec [`postcard`](https://docs.rs/postcard) (serde) |
| `updated_at` | INTEGER | Secondes Unix |

Une page sans ligne dans `page_data` est une page vide. Le modèle sérialisé est `calame_core::Page` : une liste d'éléments, soit un trait (pinceau, couleur,
épaisseur, points avec pression et inclinaison), soit une zone de texte (position, texte, corps,
couleur, angle, dimensions mesurées). Les pages au format 1 sont converties à la lecture et
réécrites au format 2 au prochain enregistrement.

## Table `settings` : les réglages

Paires clé/valeur en texte (`favorites` : couleurs `#rrggbb` séparées par des virgules ;
`dark` : `1` ou `0` ; `shapes` : `1` si la reconnaissance automatique de formes est active). Ajoutée par le schéma 2.

## Table `assets` : les images de fond

Ajoutée par le schéma 3. Chaque ligne est une image (PNG ou JPEG) servant de fond à une ou plusieurs
pages : photo importée, ou page de PDF rendue en PNG.

| Colonne | Type | Rôle |
|---|---|---|
| `id` | TEXT, clé | UUID, référencé par `nodes.background` (`asset:<uuid>`) |
| `mime` | TEXT | `image/png` ou `image/jpeg` |
| `width`, `height` | REAL | Taille en unités de page (1 unité = 1/96 pouce) |
| `data` | BLOB | Octets de l'image |
| `created_at` | INTEGER | Secondes Unix |

Une image qu'aucune page n'utilise plus est supprimée automatiquement. Dupliquer une page partage
l'image au lieu de la copier.

## Unités et zone d'une page

La page n'a pas de bord : on écrit où l'on veut, sans limite en hauteur. Les coordonnées sont en
unités de page (1 unité = 1/96 pouce, soit 0,2646 mm). À l'export, la zone est la page A4
(793,7 × 1122,5 unités), ou l'image de fond, élargie à tout le contenu qui déborde (marge de 24 unités).
