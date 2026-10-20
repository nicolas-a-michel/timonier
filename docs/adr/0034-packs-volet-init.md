# 0034 — Packs de langage, volet init : runners de tests, JUnit XML et conventions

- Statut : Accepté (2026-10-20, validation de la Phase 6 par l'humain)
- Date : 2026-10-20
- Phase : 6
- Liens : spec « Initialisation et configuration de l'IA » (3. Instrumentation ; « Packs de
  langage ») ; ADR 0012 (format des packs), 0033 (`timonier init`) ; skills `language-pack`,
  `otel-instrumentation` ; tâche PLAN.md 6.11

## Contexte

La spec veut que `init` configure le runner de tests pour produire du JUnit XML, et copie les
conventions d'instrumentation de chaque langage dans `.timonier/conventions/<langage>/` ; tout ce
qui dépend du langage vit dans son pack. L'ADR 0012 a réservé la section `[tests]` du manifeste.
L'ajout du SDK OpenTelemetry et du module d'initialisation est en Phase 8 (H6). Vérifié le
2026-10-20 :
- `cargo test` n'a pas de sortie JUnit stable ; cargo-nextest l'écrit par un profil
  `[profile.<nom>.junit]` de `.config/nextest.toml` (nexte.st/book/junit) ;
- vitest 5.0.4 l'écrit avec `--reporter=junit --outputFile.junit=<fichier>` (essayé) ;
- `node --test` (Node 24.18) avec `--test-reporter=junit --test-reporter-destination=<fichier>`
  (essayé) ;
- jest a besoin du paquet `jest-junit`.

## Décision

**`[tests]`** devient une liste de runners, `[[tests.runners]]`, essayés dans l'ordre ; le
premier qui convient est celui du projet :
- `name` ; `uses` : un mot que le script `test` du manifeste contient (`vitest`, `node --test`,
  `jest`) ; sans `uses`, le runner convient toujours ;
- `command` : comment lancer les tests en écrivant du JUnit XML, et `junit` : où il arrive,
  relatif à la racine (`.timonier/runs/` pour vitest et `node --test`) ;
- `config` : un bloc qu'`init` écrit entre marqueurs dans un fichier du projet (le profil
  `timonier` de `.config/nextest.toml`) ;
- `program` : un programme qui doit être dans le `PATH`, et `note` : ce qu'il faut faire sinon,
  ou ce qu'`init` ne fait pas (ajouter `jest-junit`).

Rust : cargo-nextest, profil `timonier` (on ne réécrit pas les tests). TypeScript : vitest, puis
`node --test`, puis jest.

**Ce qu'`init` écrit** : `.timonier/tests.json` (version 1 : par langage, le runner, la commande
et le JUnit attendu), lu par `timonier test` en Phase 8 ; le bloc `config` du runner ; les
conventions du pack.

**Conventions** : les fichiers `conventions/*.md` du pack, copiés tels quels dans
`.timonier/conventions/<langage>/`. Ils traduisent les règles du skill `otel-instrumentation`
(une span par frontière de composant, les attributs `code.*`, pas de span dans une boucle
chaude, l'instrumentation désactivable) et nomment les paquets du SDK. Le bloc d'`AGENTS.md`
renvoie à ce dossier. Une nouvelle version du pack les réécrit ; `doctor` les vérifie.

**`.timonier/.gitignore`** garde aussi `runs/` hors de git : les rapports de tests ne se
versionnent pas.

## Alternatives écartées

- **Modifier `vitest.config.*` ou `package.json`** : du code du développeur, en TypeScript,
  réécrit par un outil ; la commande suffit, et `timonier test` la lance.
- **`cargo test` avec `-Z unstable-options --format junit` et `RUSTC_BOOTSTRAP`** : une
  interface instable, contournée.
- **Ajouter `jest-junit` aux dépendances** : une dépendance de plus, sans le demander.

## Conséquences

- Un pack décrit ses runners sans changer le démon ni le CLI (spec « Packs de langage »).
- Le format des packs reste 1 : `[tests]` était réservé, et n'entre pas dans la clé du cache
  d'extraction.
