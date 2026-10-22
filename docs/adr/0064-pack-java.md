# 0064 — Le pack Java : paquets pointés, Maven et Gradle, rapports JUnit par dossier

- Statut : Proposé
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88 ; ADR 0012, 0013 (packs, noms qualifiés), 0034 (lanceurs), 0050 (SDK par
  `init`), 0055 (inventaire), 0062 (champ `when`), 0063 (pack Go) ; tâche PLAN.md 10.4

## Contexte

Java nomme son code par paquet pointé (`com.shop.model`), rangé sous une racine de sources
(`src/main/java`, `src/test/java`) ; un fichier tient une classe publique, et les classes d'un
même paquet se voient sans import, quelle que soit la racine. Deux outils de construction se
partagent les projets, Maven (`pom.xml`) et Gradle (`build.gradle(.kts)`, `settings.gradle(.kts)`,
catalogue de versions `gradle/libs.versions.toml`), avec les mêmes coordonnées
`group:artifact`. Surefire et Gradle écrivent un rapport JUnit par classe, dans un dossier par
module. Kotlin (10.6) partage ces outils.

## Décision

- **Pack `java`** : grammaire `tree-sitter-java` 0.23.5 (MIT, vérifiée le 2026-10-22, chargée
  par tree-sitter 0.27), `.java`. Définitions : classes, records, enums, annotations (`type`),
  interfaces (`trait`), méthodes et constructeurs, champs, constantes d'énumération. Les
  surcharges partagent un chemin, la première avec un corps est gardée. `[walk] skip` :
  `target`, `build`, `out`.
- **Noms** : `java:<paquet>.<Classe>#<définition>` (`java:com.shop.Cart#Cart.total`), le chemin
  pris depuis la racine de sources `src/<ensemble>/java` (ou `src/`). Comme pour Python, deux
  modules qui déclarent la même classe partagent son identifiant.
- **Écosystème `maven`** pour Maven et Gradle. `pom.xml` lu par `roxmltree` 0.21.1 (MIT OR
  Apache-2.0, vérifié le 2026-10-22, sans autre dépendance que memchr) : `groupId:artifactId`,
  le groupe hérité du `<parent>`, les `<modules>` membres, les dépendances directes (`test` hors
  exécution). Gradle lu ligne à ligne : `group`, le nom du dossier, les configurations connues
  (`implementation`, `api`, `testImplementation`…) écrites en clair ou par le catalogue
  (`libs.spring.boot.web`) ; `settings.gradle(.kts)` ne fait pas de paquet, ses `include` sont
  des membres.
- **Résolution** : un import lie son dernier segment (la classe, ou le membre d'un import
  statique) et se résout vers le plus long préfixe qui nomme une classe du dépôt ; sinon vers
  la dépendance dont le groupe le préfixe, puis celle qui partage les deux premiers segments du
  groupe (`org.springframework`) ; le JDK reste non résolu. `import a.b.*` ne lie aucun nom :
  un nom seul y est cherché. Un nom seul se résout aussi dans le même paquet, quelle que soit
  la racine de sources.
- **Tests** : dans l'ordre, l'enveloppe Gradle (`when = "gradlew"`,
  `./gradlew test --rerun --continue`), l'enveloppe Maven (`mvnw`), Maven (`pom.xml`), Gradle.
  Le champ `junit` peut nommer un dossier de rapports (finissant par `/`), et `**/` le cherche
  dans chaque module, à sept niveaux au plus (quatre d'abord, sept depuis sbt 2, ADR 0071) :
  `timonier test` lit chaque XML que la passe a
  écrit. Sous Windows, une commande qui commence par `./` passe à `cmd` en `.\`.
- **SDK** (`init`) : `io.opentelemetry:opentelemetry-api` à la compilation ;
  `opentelemetry-sdk`, `opentelemetry-exporter-otlp` et
  `opentelemetry-sdk-extension-autoconfigure` pour les tests, tous en 1.67.0 (Apache-2.0,
  vérifiés sur Maven Central le 2026-10-22). Dans `pom.xml`, un bloc entre
  `<!-- >>> timonier >>> -->` et `<!-- <<< timonier <<< -->` en tête des `<dependencies>` du
  projet (jamais de `<dependencyManagement>`), ou un `<dependencies>` à lui avant
  `</project>`. Dans un `build.gradle(.kts)` racine, un bloc `dependencies { }` à lui à la fin,
  entre `// >>> timonier >>>` et `// <<< timonier <<<`, que Gradle ajoute aux autres. Un build
  Gradle sans fichier racine (celui de `gradle init`, tout dans `app/`) : le build de son seul
  sous-projet reçoit le SDK (ADR 0066) ; avec plusieurs, `init` le dit et le SDK s'ajoute à la
  main. Classe `timonier.TimonierOtel` et test de fumée JUnit 5 dans
  `src/test/java/timonier/` ; l'autoconfiguration du SDK lit les variables de la passe.
- **Inventaire** : les dépendances du `pom.xml` racine et de ses modules, versions écrites ou
  par `${propriété}` ; celles des builds Gradle, versions écrites ou du catalogue. Une version
  gérée par un parent ou une BOM reste inconnue ; pas de licences. OSV sous « Maven », purl
  `pkg:maven/<groupe>/<artefact>` (et `pkg:golang/<chemin>` pour Go, corrigé au passage).
- **Trous d'instrumentation** : publique, une méthode `public` ; `src/test/` est du code de test.

## Alternatives écartées

- **Lire la déclaration `package` pour nommer** : le chemin depuis la racine de sources lui est
  égal dans les projets Maven et Gradle, et il est connu avant l'extraction.
- **Résoudre l'arbre Maven complet (parents, BOM)** : il faut le réseau ou le dépôt local ;
  `mvn dependency:tree` reste possible plus tard, pour l'inventaire seulement.
- **Un pack par outil de construction** : les deux nomment les mêmes artefacts, et Kotlin les
  partage.

## Conséquences

- Deux classes d'un même paquet dans deux modules se voient pour Timonier, alors que le
  compilateur ne les relie que si l'un des modules dépend de l'autre.
- Le test de fumée suppose JUnit 5 ; un projet en JUnit 4 l'adapte.
