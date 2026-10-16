# 0011 — SQLite : moteur embarqué, migrations, écrivain unique

- Statut : Accepté (2026-10-16, validation de la phase par l'humain)
- Date : 2026-10-14
- Phase : 2
- Liens : spec « Décisions techniques » (« un seul écrivain SQLite en WAL ») ; skill
  `rust-daemon` ; ADR 0010

## Contexte

Le graphe vit dans SQLite. La spec impose un seul écrivain en mode WAL. Il faut encore choisir :

- la manière de lier SQLite ;
- l'outil de migration ;
- les pragmas ;
- le modèle de concurrence.

Versions vérifiées le 2026-10-14 : rusqlite 0.40.2 (MIT), rusqlite_migration 2.6.0
(Apache-2.0).

## Décision

**Lien avec SQLite** : rusqlite avec la feature `bundled`. La même version récente de SQLite
est compilée partout, y compris sur macOS. Elle fournit les index partiels, les tables
`STRICT` et les CTE récursives.

**Migrations** : rusqlite_migration.

- L'état est suivi par `PRAGMA user_version`, sans table supplémentaire.
- Fichiers `crates/timonier-store/migrations/NNNN_nom.sql`, inclus à la compilation.
- Une migration n'avance que vers l'avant et n'est jamais modifiée une fois commitée.
- Un test applique toutes les migrations et les valide.

**Pragmas** : `journal_mode = WAL`, `synchronous = NORMAL` (sûr en WAL ; la base est un cache
reconstructible), `foreign_keys = ON`, `busy_timeout = 5 s`.

**Concurrence**

- Un fil dédié possède la seule connexion en écriture. Chaque écriture est une tâche envoyée
  par un canal borné et exécutée dans une transaction.
- Une panique dans une tâche est rattrapée : la transaction est annulée et le fil continue.
- L'API d'écriture est synchrone. Le démon l'appellera depuis `spawn_blocking` en Phase 4.
- Les lecteurs ouvrent leurs propres connexions en lecture seule. Le WAL leur donne une vue
  cohérente pendant les écritures.

## Alternatives écartées

- **SQLite du système** : sa version change d'une machine à l'autre, et macOS fournit une
  version ancienne.
- **refinery** : plus lourd, avec des macros et sa propre table de suivi, pour aucun besoin
  supplémentaire ici.
- **sqlx** : asynchrone, il vérifie les requêtes à la compilation via une base, et il est
  surdimensionné pour un seul écrivain local.
- **Un `Mutex<Connection>` partagé** : un appelant asynchrone qui écrit bloquerait ses
  voisins. Le fil dédié isole les écritures.

## Conséquences

- La compilation de SQLite ajoute du temps au premier build, qui est mis en cache ensuite.
- Le stockage n'expose pas de SQL : on passe par ses fonctions typées.
