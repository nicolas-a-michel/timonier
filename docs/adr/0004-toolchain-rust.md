# 0004 — Toolchain Rust : canal stable, MSRV = plus ancienne version testée

- Statut : Proposé
- Date : 2026-10-14
- Phase : 1

## Contexte

Il faut fixer l'édition et la toolchain, et dire avec quelles versions de Rust le projet
compile. Rust stable est en 1.99.0 (vérifié le 2026-10-14), et la machine de développement a
la 1.96. Timonier est une application, pas une bibliothèque : une MSRV basse n'apporte rien
aux utilisateurs.

## Décision

- `rust-toolchain.toml` : canal `stable`, composants `rustfmt` et `clippy`, profil
  `minimal`. La CI installe cette toolchain avec `rustup toolchain install`.
- Édition 2024, stable depuis Rust 1.85.
- `rust-version = "1.96"` : la plus ancienne version réellement testée, sur la machine de
  développement. On la relève dès qu'une dépendance l'exige, ou quand la machine locale passe
  à une version plus récente.
- `rustfmt.toml` porte `edition = "2024"`, pour que le hook de formatage, qui appelle
  `rustfmt` directement, formate comme `cargo fmt`.

## Alternatives écartées

- **Épingler une version exacte (`1.99.0`)** : reproductible, mais oblige à des montées de
  version manuelles. Le canal stable plus le lockfile suffisent pour une application.
- **Une MSRV basse (1.85)** : il faudrait une tâche de CI dédiée, pour un bénéfice nul ici.

## Conséquences

- La CI teste la dernière stable, et la machine locale teste la MSRV.
- Une montée de stable peut faire apparaître de nouveaux avertissements clippy, bloquants avec
  `-D warnings`. On les corrige au fil de l'eau.
