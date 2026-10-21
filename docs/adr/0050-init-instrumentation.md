# 0050 — Le volet instrumentation de `timonier init`, et le test de fumée de `doctor`

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Initialisation et configuration de l'IA » (3. instrumentation ; 7. preuve de
  bout en bout) ; Q72 ; H6 ; ADR 0033 (`init`), 0034 (packs, volet init), 0044, 0047, 0048 ;
  tâche PLAN.md 8.6

## Contexte

La spec veut qu'`init` ajoute le SDK OpenTelemetry du langage et un module d'initialisation
derrière une feature ou une variable, puis que `doctor` lance un test de fumée instrumenté qui
produit une première trace rattachée au graphe. Ta réponse Q72 : `init` touche les manifestes,
entre marqueurs, et écrit deux fichiers neufs, le module et le test de fumée ; `--dry-run` montre
le diff, `uninit` retire tout.

## Décision

**Dans le pack**, section `[instrumentation]` : les paquets du SDK (nom, version, features), la
feature Cargo, les fichiers du module (modèles `instrumentation/<nom>` du pack) et ce qui reste à
faire ; le test de fumée appartient au runner (`[tests.runners.smoke]` : sa commande, son
fichier), parce qu'il est écrit pour lui (vitest).

**Rust** : dans le `Cargo.toml` du paquet racine, deux blocs entre marqueurs, les dépendances
que le paquet n'a pas en `[dependencies.<nom>]` optionnelles à la fin, et `otel = ["dep:…"]`
juste sous le `[features]` du développeur (ou sous un `[features]` à lui). Un workspace sans
paquet racine ne reçoit rien (c'est dit) ; une feature `otel` déjà là est refusée, avec quoi
faire. Fichiers : `src/timonier_otel.rs` (le module : export quand
`OTEL_EXPORTER_OTLP_ENDPOINT` est posée, `OTEL_SDK_DISABLED` lue par lui, le test de nextest
nommé par `NEXTEST_TEST_NAME` et `NEXTEST_BINARY_ID`, sans `async` pour toute édition), et
`tests/timonier_smoke.rs` (un span `#[instrument]`, qui inclut le module par `#[path]` sans
toucher à `lib.rs`), lancé par `cargo test --features otel --test timonier_smoke`. Versions :
celles de l'ADR 0044, et `tokio` 1.53.3.

**TypeScript** : `@opentelemetry/sdk-node` 0.223.0 et `@opentelemetry/api` 1.9.1 (Apache-2.0,
vérifiés le 2026-10-21 dans leurs sources : sdk-node apporte les exporteurs OTLP, lit
`OTEL_SDK_DISABLED`, `OTEL_RESOURCE_ATTRIBUTES` et l'adresse, et `shutdown()` vide la file),
ceux que `package.json` n'a pas : l'API en `dependencies`, puisque le code livré l'importe (Q78,
relecture n° 1), le SDK en dépendance de développement. La ligne 0.300 (SDK 3.0),
sortie la veille, attend. Fichiers : `timonier.otel.ts` (`startTelemetry()`, logs coupés : le
récepteur ne les prend pas) et, pour vitest, `timonier.smoke.test.ts`, lancé par `npx vitest run
timonier.smoke.test.ts`, sauté sans run. Seulement là où le runner a une commande : pas en tête
de workspaces npm.

**Un fichier déjà là** reste au développeur. **`uninit`** retire les deux blocs de `Cargo.toml`,
les paquets que `package.json` n'avait pas avant `init`, et les fichiers créés.

**`doctor`** lance, après ses vérifications, le test de fumée de chaque langage où `init` a mis le
SDK (`smoke` dans `.timonier/tests.json`), sous `timonier test` ; il demande ensuite au démon
(`get_instrumentation_gaps` du run) s'il a reçu des spans et si l'un se rattache au graphe.
Échec, aucun span ou aucun rattachement : un constat « à faire à la main » ; sinon, une ligne
qui le dit. `--no-smoke` le saute (il compile le SDK la première fois).

## Alternatives écartées

- **Fichiers neufs seulement, commandes affichées** et **conseils seulement** : écartés par Q72.
- **Déclarer le module dans `lib.rs`** : du code du développeur réécrit ; le test de fumée
  l'inclut par `#[path]`, et les conventions disent la ligne à ajouter.
- **Le test de fumée par `cargo nextest`** : un outil de plus ; le JUnit n'y sert pas.
- **La ligne 0.300 du SDK JavaScript** : un jour, une rupture (Node 22.15, `sdk-trace-base`
  retiré).

## Conséquences

- La première exécution de `doctor` compile le SDK (une minute, à peu près) ou demande `npm
  install`.
- Timonier, workspace sans paquet racine, s'instrumente à la main (ADR 0044) : `init` ne lui
  ajoute rien.
