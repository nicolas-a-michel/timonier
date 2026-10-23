# 0077 — Chaque langage au même endroit : des règles dans les packs, un module par écosystème

- Statut : Proposé
- Date : 2026-10-23
- Phase : 11
- Liens : ADR 0003 (crates), 0012 (format des packs), 0013, 0049 (lacunes), 0055 (inventaire),
  0033 et 0034 (`init`), 0061 à 0071 (les langages) ; relecture de la Phase 10, I1 ; tâche
  PLAN.md 11.7 (T-133)

## Contexte

La spec veut qu'ajouter un langage revienne à écrire un pack, sans toucher au démon (ADR 0012).
Après les Phases 9 et 10, environ 5 500 lignes propres à un langage vivent hors des packs, dans
quatre crates, sans interface commune :

- le cœur : le code de test et le code public des lacunes d'instrumentation (`gaps.rs`, un
  `if` par préfixe de langage), et une énumération des écosystèmes de l'inventaire avec leurs
  noms OSV et purl ;
- `timonier-index` : la lecture des manifestes, les chemins de module et la résolution des
  imports, par des `match` sur une énumération fermée de dix écosystèmes ;
- le démon : la lecture des verrous (`Cargo.lock`, `package-lock.json`, `uv.lock`…), les purl,
  le rattachement des spans Rust, les dossiers des indexeurs SCIP ;
- le CLI : la détection du langage et du gestionnaire de paquets, le choix du manifeste et ses
  éditions d'`init` et d'`uninit`.

S'y ajoutent des doublons : deux énumérations d'écosystèmes, la liste des manifestes en quatre
endroits, la lecture de `include(...)` de Gradle trois fois, les noms PEP 508 deux fois.

## Décision

- **Ce qui n'est qu'une donnée va dans `pack.toml`** :
  - `[code.public]` dit quelle fonction est publique (mots ou débuts de signature, préfixes de
    nom, majuscule, membres) ; `[code.tests]` ce qui est du code de test (marques
    d'identifiant, motifs de nom de fichier et de dossier, `*` pour tout texte) ;
    `[code] approximation` la réserve que le rapport affiche ; `[code] function_ids` qu'un span
    nomme sa fonction par son identifiant (Rust). Le cœur reçoit ces règles
    (`gaps::CodeRules`, données par `Packs::code_rules`) et n'applique de lui-même que les
    dossiers `test` et `tests`. Les règles de test de tous les langages s'appliquent à chaque
    fonction, comme avant ;
  - `[scip] bin_dirs` : où l'installeur met l'indexeur, sous le dossier personnel ;
    `[scip.environment]` remplace `virtual_env` : l'environnement du projet (dossiers, fichier
    qui le signale, dossier des programmes, variable), activé comme son script le ferait ;
  - `[tests.runners.config] purpose` : ce que `doctor` dit manquant.
- **Ce qui est du code va dans `timonier_index::tooling`**, un module par écosystème des
  packs (`cargo`, `npm`, `pypi`, `gomod`, `maven`, `cmake`, `swiftpm`, `nuget`, `composer`,
  `sbt`), derrière le trait `Tooling` ; `tooling::get` le trouve par le nom de `pack.toml`.
  Chaque module tient la lecture de ses verrous (`dependencies` ; `cargo metadata` lancé par le
  démon, qui donne la fonction `Run`), la détection du projet (`detect`, dans l'ordre de
  `tooling::detect`), ce dont son lanceur a besoin (`runs`), le manifeste qui prend le SDK et
  ses éditions d'`init` et d'`uninit` (`sdk_place`, `with_sdk`, `sdk_beside`,
  `without_manifest`, `without_sdk`), les étapes de CI (`ci_steps`), et ses écosystèmes de
  paquets avec leur nom OSV et leur type purl (`packages`). `tooling::text` et
  `tooling::ordered` sont les blocs balisés et le JSON ordonné que ces éditions et celles des
  agents partagent.
- **L'écosystème de l'inventaire devient un nom avec un rang** (`inventory::Ecosystem`) : le JSON
  reste le nom, l'ordre reste celui de l'ancienne énumération ; le registre de `tooling` les
  donne.
- **La résolution interne de l'index** (`ecosystem.rs`, `project.rs`) reste où elle est : elle
  est déjà dans `timonier-index`, et la déplacer ne changerait pas ce qu'ajouter un langage
  touche.
- **Ajouter un langage** touche alors `timonier-index` seul : son pack, sa grammaire
  (`grammar.rs`, `Cargo.toml`), la liste `BUILTIN`, et un module de `tooling` s'il apporte un
  écosystème.
- **Vérifié par un test** (`tests/one_place.rs`) : aucune chaîne littérale du code du cœur, du
  démon ou du CLI, hors tests et lignes de commentaire, ne nomme un langage, un écosystème, un
  gestionnaire de paquets ou un de leurs manifestes (`c` et `go`, trop courants, exceptés).
  Exception assumée : les variables `NEXTEST_*` de la télémétrie de Timonier lui-même
  (`telemetry.rs`, ADR 0044), qui instrumente ses propres tests.
- **Sans changement visible** : la suite entière passe sans changer ses attentes. Seuls écarts
  textuels : la raison « OSV knows none of these ecosystems » nomme les écosystèmes en
  minuscules (`vcpkg, cmake`), et la recherche des rapports JUnit saute aussi `__pycache__` et
  `site-packages`, comme le parcours de l'index.

## Alternatives écartées

- **Une crate par langage** : quatorze crates pour quelques centaines de lignes chacune, et le
  registre devrait les connaître toutes ; un module garde l'ajout au même endroit.
- **Tout en données dans `pack.toml`** (un langage de règles pour la résolution et les
  éditions) : la résolution Rust (`super`, `pub use`, globs) ou l'édition d'un `Package.swift`
  ne se décrivent pas sans réinventer un langage de programmation.
- **Les modules dans le CLI et le démon, rangés par langage** : ajouter un langage toucherait
  encore trois crates.

## Conséquences

`timonier-index` grossit de la lecture des verrous et des éditions d'`init` (du texte, et la
lecture des fichiers du projet) ; le démon et le CLI y passent par le trait. Le CLI continue de
ne dépendre que du démon, qui réexporte `tooling` (ADR 0003). Le démon lit les manifestes des
packs une fois (`packs.rs`).
