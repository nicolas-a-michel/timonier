# 0067 — Le pack Swift : cibles, extensions, SwiftPM, rapports xUnit

- Statut : Accepté (2026-10-22, Phase 10 validée)
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88, Q90 ; ADR 0012, 0013, 0034, 0050, 0063 (conteneur), 0064 (dossier de
  rapports) ; tâche PLAN.md 10.7

## Contexte

En Swift, le module est la cible SwiftPM (`Sources/<Cible>/`) : ses fichiers voient tous les
noms de la cible sans import, et un `import Shop` amène tous ceux d'une autre. Une extension
ajoute des membres à un type défini ailleurs. `Package.swift` est du Swift, pas un format de
données ; `Package.resolved` épingle les versions. `swift test` écrit un rapport pour XCTest et
un autre pour Swift Testing.

## Décision

- **Pack `swift`** : grammaire `tree-sitter-swift` 0.7.4 (MIT, vérifiée le 2026-10-22, ABI 15
  chargée par tree-sitter 0.27), `.swift`. Définitions : classes, acteurs, structures,
  énumérations (`type`), protocoles (`trait`) et leurs exigences, alias, cas d'énumération (un
  par nom), méthodes, initialiseurs, propriétés, fonctions et valeurs de fichier. Une extension
  est un conteneur : ses membres ont le chemin de leur type. `[walk] skip` : `DerivedData`,
  `Pods`, `Carthage` (`.build` est caché).
- **Noms** : `swift:<Cible>/<dossiers>/<fichier>#<définition>`
  (`swift:Shop/Cart+Total#Cart.total`) : deux fichiers qui étendent un même type gardent des
  identifiants distincts.
- **Écosystème `swiftpm`** : `Package.swift` lu comme du texte : `name:`, les `.package(url:
  …)` nommés par leur emplacement sans schéma ni `.git` (`github.com/apple/swift-log`), comme
  Go, avec pour noms de code les modules que les cibles en prennent (`.product(name:
  "Logging", package: "swift-log")`, séparés par des virgules) ; un paquet des seules cibles de
  test n'est pas à l'exécution. Les dépendances locales (`path:`) sont laissées.
- **Résolution** : `import Shop` mène au premier fichier de la cible `Shop` (sous `Sources/` ou
  `Tests/`), sinon à la dépendance qui fournit ce module ; Foundation reste non résolu. Un nom
  seul se résout dans sa cible et dans celles que le fichier importe.
- **Tests** : `swift test --parallel --xunit-output .timonier/runs/xunit/tests.xml` (sans
  `--parallel`, XCTest n'écrit pas de rapport sous Linux ; Swift Testing écrit
  `tests-swift-testing.xml` à côté) ; Timonier lit le dossier, qu'il crée avant la passe (vérifié
  avec Swift 6.4.1).
- **SDK** (`init`) : `OpenTelemetryApi` et `OpenTelemetrySdk` d'opentelemetry-swift-core 2.6.0,
  `OpenTelemetryProtocolExporterHTTP` d'opentelemetry-swift 2.6.0 (Apache-2.0, vérifiés sur
  GitHub le 2026-10-22). `init` ajoute les deux paquets aux `dependencies` de `Package.swift` (un
  tableau à lui avant `targets` s'il n'y en a pas) et une cible de test `TimonierTests` ; chaque
  ligne ajoutée finit par `// timonier`, et `uninit` les retire. `TimonierOtel.start()` construit
  l'URL `…/v1/traces` depuis `OTEL_EXPORTER_OTLP_ENDPOINT` (l'exportateur ne le lit pas) ;
  l'exportateur lit les en-têtes, `EnvVarResource` les attributs. Le test de fumée est un
  XCTest.
- **Inventaire** : les paquets de `Package.swift`, versions et paquets transitifs de
  `Package.resolved` ; OSV sous « SwiftURL », purl `pkg:swift/<emplacement>`.
- **Trous d'instrumentation** : publique, une fonction `public` ou `open`.

## Alternatives écartées

- **Exécuter `swift package dump-package`** : exact, mais il faut la chaîne d'outils et une
  compilation du manifeste à chaque indexation.
- **Nommer par la cible seule** (`swift:Shop#Cart.total`) : les extensions de deux fichiers
  donneraient le même identifiant à des membres différents.

## Conséquences

- Les instructions de `main.swift` ne sont pas reconnues comme un point d'entrée (seul `@main`
  l'est) : le nom du fichier n'est pas un signal des requêtes.
- La première passe compile opentelemetry-swift et ses dépendances (swift-nio, swift-protobuf).
