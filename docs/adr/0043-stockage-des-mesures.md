# 0043 — Où vivent les mesures de performance

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Performances », « Budgets de performance », « Seuil de régression » ;
  ADR 0010 (snapshots), 0011 (SQLite), 0033 (`.timonier/.gitignore`) ; tâches PLAN.md 8.1, 8.2,
  8.4, 8.8, 8.9

## Contexte

`timonier test` produit, à chaque run, des mesures de niveau 1 (durée, CPU, mémoire, I/O), la
durée de chaque test (JUnit XML), et les spans et métriques OTLP du niveau 3. Les vues veulent
la timeline d'un test, la carte annotée et l'évolution par commit ; la détection de régressions
veut 5 à 15 exécutions et la médiane des 5 derniers snapshots de la branche principale ; les
budgets veulent la médiane de plusieurs runs. `graph.sqlite` est un cache reconstructible :
une migration peut le vider et réindexer. Un run passé, lui, ne se rejoue pas.

## Décision

**Une base à part**, `.timonier/perf/perf.sqlite`, dans un dossier qui se garde lui-même hors
de git (son propre `.gitignore`, `*`) : un projet déjà initialisé n'a pas à changer son
`.timonier/.gitignore`. Mêmes choix que l'ADR 0011 (rusqlite `bundled`, pragmas, un fil
écrivain unique, des lecteurs en lecture seule), mais ses propres migrations
(`crates/timonier-store/migrations/perf/` ; rusqlite avec `fallible_uint` pour les colonnes
`u64`) : vider le graphe ne touche pas aux mesures. Le démon en est le seul écrivain ;
`timonier test` ouvre le run (`run/start`) et lui envoie ce qu'il a mesuré (`run/finish`,
ADR 0047).

**Contenu**, des faits datés, jamais réécrits :
- `run` : ID, début et fin, commande, code de sortie, test de fumée ou non (`smoke`, migration
  `perf/0003`), empreinte d'un arbre modifié (`worktree`, `perf/0002`, Q77), snapshot (numéro de
  séquence et commit),
  branche, empreinte de la machine, mesures de niveau 1, source du compte CPU (instructions ou
  temps CPU, ADR 0046) ;
- `test` : run, nom, classe, fichier et ligne quand le JUnit les donne, état, durée ;
- `span` : run, test (le span racine dont il descend), IDs de trace et de span, parent, nom,
  début et fin en nanosecondes, statut, les attributs `code.*`, le nœud du graphe auquel il est
  rattaché (ID stable, ou rien), et les autres attributs en JSON, bornés à 4 Kio ;
- `metric` : run, nom, attributs, sorte (compteur, jauge, histogramme), valeur ou résumé ;
- `aggregate` : par run et par nœud rattaché, nombre d'appels, temps propre et temps d'entrée
  (ADR 0051), calculés quand la rétention efface les spans du run (relecture n° 1, I4 ; migration
  `perf/0002`) : ce que la référence des régressions lit au-delà de 50 runs. Les durées des
  tests restent dans `test` ; l'évolution lit les résumés des 50 derniers runs (`perf/runs`,
  relecture n° 2, m-1).

**Empreinte de la machine** : modèle et nombre de cœurs du processeur, mémoire totale, système
et noyau, source du compte CPU, réduits en un hash. Un changement d'empreinte réinitialise la
référence des régressions (spec).

**Rétention** : les spans et les métriques des 50 derniers runs ; au-delà, seuls `run`, `test`
et `aggregate` restent, petits, pour la référence des régressions. Un run garde au
plus 200 000 spans ; les suivants sont comptés, pas gardés, et le comptage remonte comme un
trou d'instrumentation (8.5).

## Alternatives écartées

- **Dans `graph.sqlite`** : une migration qui reconstruit le graphe effacerait l'historique, et
  les rafales d'écriture d'un run attendraient derrière l'indexation.
- **Un fichier par run** (OTLP brut, JSON) : les médianes sur plusieurs snapshots deviendraient
  des lectures de fichiers entiers.
- **DuckDB** : un moteur analytique de plus, lourd à compiler, pour des volumes que SQLite tient.
- **En mémoire seulement** : ni référence ni évolution par commit après un redémarrage.

## Conséquences

- Les mesures ne sont pas des données de l'utilisateur au sens de la spec : les perdre fait
  perdre l'historique, pas une intention ; les budgets, eux, sont dans `intent.json`.
- Un span se rattache à l'ID stable d'un nœud : un renommage le détache des runs anciens comme
  il change l'ID (ADR 0008).
- Les seuils (50 runs, 200 000 spans, 4 Kio) se revoient si un vrai dépôt les atteint.
