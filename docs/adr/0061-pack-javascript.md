# 0061 — Le pack JavaScript, et ce que tous les packs ne parcourent pas

- Statut : Proposé
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88 ; ADR 0012 (format des packs), 0013 (extraction et noms qualifiés), 0034
  (lanceurs de tests), 0049 (trous d'instrumentation) ; tâche PLAN.md 10.1

## Contexte

JavaScript est le premier des cinq langages de la Phase 10. Sa grammaire tree-sitter est celle
sur laquelle TypeScript est construit, et ses projets partagent npm avec TypeScript. Trois points
n'étaient pas couverts : le dossier des dépendances installées (`node_modules`), parcouru quand
un projet ne l'ignore pas ; les guillemets, que les formateurs changent (`'a'` ↔ `"a"`) et qui
changeaient l'empreinte des corps ; deux packs du même écosystème, qui revendiquaient tous deux
le lanceur de tests d'un `package.json`.

## Décision

- **Pack `javascript`** : grammaire `tree-sitter-javascript` 0.25.0 (MIT, vérifiée le
  2026-10-22, chargée par tree-sitter 0.27), extensions `.js`, `.mjs`, `.cjs`, `.jsx` (JSX
  compris dans la grammaire) ; écosystème npm ; noms `javascript:<paquet>/<chemin sans
  extension>#<nom>`. Imports ES et CommonJS (`const x = require("y")`, `const { a } = require(…)`)
  ; un spécificateur npm se résout d'abord vers TypeScript puis vers JavaScript (`.js`, `.jsx`,
  `.mjs`, `.cjs`, `index.js`), comme TypeScript avec `allowJs`. Rôles, catégories et inventaire
  repris de TypeScript.
- **Tests et instrumentation** : les lanceurs vitest, `node:test` et jest ; le module
  `timonier.otel.mjs` et le test de fumée `timonier.smoke.test.mjs` sous `node --test`, qui ne
  demande ni vitest ni TypeScript. `timonier test` crée le dossier du JUnit avant de lancer les
  tests (`node --test` ne le crée pas), et lit le JUnit de Node, dont les cas ne sont dans aucun
  `<testsuite>`.
- **`[walk]`**, nouvelle table du format des packs, pour des dossiers jamais parcourus, ignorés
  ou non. `skip` : un dossier de ce nom à côté d'un manifeste de l'écosystème du pack (`bin/` à
  côté d'un `.csproj`, `target/` à côté d'un `pom.xml`) ; `skip_anywhere` : partout, pour les
  noms qu'aucun projet ne donne à son code (`node_modules`, `__pycache__`). Une première version
  sautait les dossiers de `skip` partout et pour tous les langages : `packages/` (C#) cachait
  les espaces de travail npm, `bin/` les binaires de Rust (relecture de la phase 10, corrigé le
  2026-10-22). TypeScript et JavaScript mettent `node_modules` dans `skip_anywhere`. La
  surveillance n'indexe un fichier que dans un dossier parcouru : elle suit d'elle-même.
- **Guillemets** : un atome de type `string` (JavaScript, TypeScript) entre dans l'empreinte sans
  ses délimiteurs `'` ou `"`. Le pack TypeScript change de version : son cache d'extraction est
  refait.
- **Un lanceur par langage détecté** : `init` ne prend le lanceur d'un pack que si son langage est
  celui que la détection a trouvé (TypeScript si `tsconfig.json` ou une dépendance `typescript`,
  JavaScript sinon).
- **Trous d'instrumentation** : en JavaScript, toute fonction et méthode compte comme publique,
  sauf `#privée`, comme en TypeScript ; la vue dit cette approximation.

## Alternatives écartées

- **Lire JavaScript avec la grammaire TypeScript** : les fichiers auraient porté le langage
  `typescript`, et leurs conventions, leur SDK et leur test de fumée auraient supposé TypeScript.
- **Exclure `node_modules` en dur dans l'indexeur** : chaque écosystème a son dossier (`.venv`,
  `vendor`, `target`…) ; la table vit avec le pack qui la connaît.

## Conséquences

- Le dépôt de Timonier, qui contient du JavaScript (hooks, scripts de build), voit ces fichiers
  sur sa carte.
- Une fois mis à jour, les corps TypeScript qui contiennent des chaînes changent d'empreinte une
  fois : la revue suivante peut les montrer modifiés.
