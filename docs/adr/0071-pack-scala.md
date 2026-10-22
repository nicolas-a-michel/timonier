# 0071 — Le pack Scala : sbt, imports à sélecteurs, sbt 2 et son cache de tests

- Statut : Proposé
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88, Q90 ; ADR 0012, 0013, 0034, 0050, 0064 (pack Java), 0066 (pack Kotlin) ;
  tâche PLAN.md 10.11

## Contexte

Scala vit sur la JVM avec les artefacts Maven, mais se construit le plus souvent avec sbt :
`build.sbt` est du Scala, `%%` ajoute au nom d'un artefact le suffixe de la version de Scala
(`upickle_3`). Ses imports listent des sélecteurs (`import a.b.{C, D => E}`, `import a.b.*`).
sbt 2 ne relance que les tests touchés (`test` est `testQuick`, et `testOnly` rejoue son cache),
passe par un serveur qui garde l'environnement de son démarrage, et fait tourner les tests dans
sa propre JVM, dans un chargeur de classes qu'il ferme dès qu'ils finissent.

## Décision

- **Pack `scala`** : grammaire `tree-sitter-scala` 0.26.3 (MIT, vérifiée le 2026-10-22),
  `.scala` et `.sc`. Définitions : classes, objets, traits (`trait`), énumérations et leurs cas,
  alias de type, membres, paramètres de classe, fonctions et valeurs de fichier.
- **Noms** : `scala:<paquet>.<Fichier>#<définition>`, depuis `src/<ensemble>/scala`, comme Java.
- **Écosystème `sbt`** : `build.sbt` lu comme du texte : `organization` et `name`
  (`com.shop:shop`), les `"groupe" %% "artefact" % "version"` avec le suffixe tiré de
  `scalaVersion` ; `% Test` hors exécution. L'inventaire les compte comme des artefacts Maven
  (OSV « Maven »).
- **Imports** : le préfixe d'une liste d'import prend tous les enfants `path` qui précèdent le
  nom (Scala écrit `a`, `b` puis les sélecteurs ; Rust n'en a qu'un, inchangé). Un import lie
  son dernier segment ou son renommage ; `*` et `_` ne lient rien. La résolution est celle de
  Java et Kotlin, qui se voient tous les trois ; une dépendance se trouve aussi par son artefact
  sans suffixe (`upickle.default` → `com.lihaoyi:upickle_3`).
- **Tests** : sbt écrit un JUnit par suite dans `test-reports/` sous le `target` de chaque
  projet (`target/out/jvm/scala-3.10.0/<projet>/test-reports/` avec sbt 2) : `junit =
  "**/test-reports/"`, cherché sur sept niveaux, `target` et `build` compris. Une règle de
  lanceur peut exiger un texte dans son fichier `when` (`when_text`) : avec
  `sbt.version=2` dans `project/build.properties`, `sbt --server -batch testFull`, et le test de
  fumée passe par `testFull` aussi ; sinon `sbt -batch test`.
- **SDK** (`init`) : les artefacts Java d'OpenTelemetry 1.67.0 (ADR 0064) dans un bloc à la fin
  de `build.sbt`, entre les marqueurs `//` de Gradle, l'API pour le code, le reste `% Test`.
  L'objet `timonier.TimonierOtel` attend la fin du vidage (dix secondes au plus) : sinon
  l'exportateur part après la fermeture des classes du test (`IllegalAccessError`, constaté).
  Test de fumée pour munit.
- **Trous d'instrumentation** : publique, une méthode sans `private` ni `protected`.

## Alternatives écartées

- **Lancer `sbt` comme client** (par défaut avec sbt 2) : le serveur réutilisé n'a pas les
  variables de la passe, et aucun span ne part.
- **Forcer `Test / fork := true` dans le bloc** : change la façon dont le projet lance ses
  tests ; attendre le vidage suffit.

## Conséquences

- Dans un build de plusieurs projets, le bloc de `build.sbt` vaut pour le projet racine ; celui
  qui lance les tests le reprend.
- L'indexation de Scala est la plus lente des packs (17,9 ms pour mille lignes sur Cats), la
  résolution des imports à joker en prenant la plus grande part.
