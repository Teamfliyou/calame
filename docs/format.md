# Format de stockage de Calame

La bibliothèque est un fichier **SQLite** unique (`calame.db`), en mode WAL.
Sous Windows : `%APPDATA%\calame\Calame\data\calame.db`. La variable d'environnement `CALAME_DB` permet
de choisir un autre fichier.

La version du schéma est dans `PRAGMA user_version` (actuellement **1**). Une base créée par une
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

Règles de placement : une matière est à la racine ; un cahier est dans une matière ; un
sous-dossier ou une page est dans un cahier ou un sous-dossier. Les sous-dossiers s'imbriquent sans
limite. Mettre un élément à la corbeille marque tout son sous-arbre ; le restaurer le démarque.

## Table `page_data` : le contenu des pages

| Colonne | Type | Rôle |
|---|---|---|
| `page_id` | TEXT, clé | Référence `nodes.id` (suppression en cascade) |
| `format` | INTEGER | Version du codec (actuellement **1**) |
| `data` | BLOB | Page sérialisée avec [`postcard`](https://docs.rs/postcard) (serde) |
| `updated_at` | INTEGER | Secondes Unix |

Une page sans ligne dans `page_data` est une page vide. Le modèle sérialisé est `calame_core::Page`
(liste de traits : pinceau, couleur, épaisseur, points avec pression et inclinaison).
