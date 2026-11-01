# Contribuer à Calame

Merci de votre intérêt ! Calame est un projet open source sous double licence MIT ou Apache-2.0 :
en proposant du code, vous acceptez qu'il soit publié sous ces deux licences.

## Mise en route

Il faut Rust stable (https://rustup.rs). Sous Windows, soit les *Build Tools* de Visual Studio, soit un
MinGW-w64 dans le `PATH` avec la toolchain `stable-x86_64-pc-windows-gnu`.

```bash
cargo run -p calame-app --release   # lancer l'application
cargo test --workspace              # tous les tests
```

La première compilation est longue (la bibliothèque graphique et le moteur PDF sont volumineux). Si le
compilateur manque de mémoire, limitez les tâches parallèles : `CARGO_BUILD_JOBS=2`.

## Avant de proposer une modification

La CI exige ces trois commandes, sans avertissement :

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Règles du projet

- **Le modèle de données et la reconnaissance de formes restent indépendants de l'interface.**
  `calame-core`, `calame-shapes`, `calame-storage` et `calame-export` ne dépendent pas d'`egui`.
- **Pas de `unwrap()` ni de `expect()` hors des tests** (avertissement Clippy). Les erreurs se
  propagent avec `thiserror` dans les bibliothèques et `anyhow` dans l'application.
- **Tout comportement se teste.** Les formes reconnues ont un jeu de tests (tracés bruités, tournés,
  mal fermés) : ajoutez-y le cas qui pose problème avant de toucher aux seuils.
- **Les textes de l'interface passent par `i18n::t`**, jamais en dur, pour permettre d'autres langues.
- Les commentaires et l'interface sont en français ; les noms de code en anglais.
- Une modification du format de stockage change `SCHEMA_VERSION`, ajoute une migration testée et met
  à jour [docs/format.md](docs/format.md).

## Mesures de performance

Les mesures sur de grandes pages sont des tests ignorés par défaut :

```bash
cargo test --release --workspace -- --ignored --nocapture
```

Elles affichent des lignes `PERF …`. Ne dégradez pas ces chiffres sans raison.

## Signaler un problème

Indiquez le système, le modèle de stylet ou de tablette, et ce que vous attendiez. Pour un problème
de pression ou de bouton du stylet, voir [docs/stylet.md](docs/stylet.md).
