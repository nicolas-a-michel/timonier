# 0063 — Le pack Go : chemins d'import, `go.mod`, gotestsum

- Statut : Accepté (2026-10-22, Phase 10 validée)
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88 ; ADR 0012, 0013 (packs, noms qualifiés), 0034 (lanceurs), 0050 (SDK par
  `init`), 0055 (inventaire), 0061 (`[walk] skip`), 0062 (pack Python, champ `when`) ; tâche
  PLAN.md 10.3

## Contexte

En Go, un paquet est un dossier, et le code le nomme par son chemin d'import : le chemin du
module (`go.mod`) suivi du dossier (`example.com/shop/cart`). Les fichiers d'un même paquet se
voient sans import. Une méthode se déclare à côté de son type, pas dans un bloc qui le contient.
`go test` n'écrit pas de JUnit, et aucun outil d'OpenTelemetry n'est fourni avec Go.

## Décision

- **Pack `go`** : grammaire `tree-sitter-go` 0.25.0 (MIT, vérifiée le 2026-10-22, chargée par
  tree-sitter 0.27), `.go`. Définitions : fonctions, méthodes, types, interfaces (`trait`) et
  leurs méthodes, alias, champs, constantes et variables du paquet. `[walk] skip = ["vendor"]`.
- **Méthodes** : le récepteur (`func (c *Cart) Total()`) est un conteneur `@scope` posé sur le
  nœud même de la méthode. L'extraction accepte désormais un nœud à la fois conteneur et
  définition (le conteneur d'abord). `Cart.Total` est rangée sous `Cart` quand le type est dans
  le même fichier.
- **Noms** : `go:<chemin d'import>/<nom du fichier sans extension>#<définition>`
  (`go:example.com/shop/cart/cart#Cart.Total`). Le nom du fichier sépare les fonctions `init`
  de deux fichiers d'un même paquet. Déplacer une fonction d'un fichier à l'autre change son
  identifiant, et la détection des déplacements par le corps la suit.
- **Écosystème `gomod`** : un paquet par `go.mod` (son `module`), les membres d'un `go.work` ;
  les `require` directs à l'exécution, les `// indirect` à part.
- **Résolution** : un import lie le dernier segment de son chemin, ou son alias (`_` et `.` ne
  lient rien). Le chemin se résout vers le plus long module du dépôt qui le préfixe, puis vers
  le dossier : le fichier qui définit le nom cité (`cart.Total`), sinon le premier fichier hors
  tests ; sinon vers la dépendance au plus long préfixe ; la bibliothèque standard reste non
  résolue. Un nom seul se résout dans son propre dossier. Le plongement d'un type
  (`struct { cart.Store }`, une interface dans une interface) compte comme héritage.
- **Tests** : gotestsum 1.13.0 (Apache-2.0, vérifié le 2026-10-22),
  `gotestsum --junitfile .timonier/runs/junit.xml -- ./...` ; `timonier test --install` le
  pose par `go install gotest.tools/gotestsum@v1.13.0`.
- **SDK** (`init`) : `go.opentelemetry.io/otel`, `…/otel/trace`, `…/otel/sdk` et
  `…/otlptrace/otlptracehttp` v1.47.0 (Apache-2.0, publiés le 2026-10-02, vérifiés le 2026-10-22 sur
  proxy.golang.org), en un bloc `require` entre `// >>> timonier >>>` et `// <<< timonier <<<`
  à la fin de `go.mod`. Un module déjà requis n'est pas ajouté. Quand `go.mod` requiert déjà
  tout, il reste tel quel : `go mod tidy` trie le bloc et y ajoute ses lignes indirectes, et
  `doctor` ne doit pas y voir une réparation. Paquet `timonier/` (`otel.go`, test de fumée
  `timonier_smoke_test.go`), que Go ne compile que si on l'importe.
- **Inventaire** : `go.mod` donne chaque module requis avec sa version ; Go ne garde pas les
  licences. OSV l'interroge sous l'écosystème « Go », purl `golang`.
- **Trous d'instrumentation** : publique, une fonction dont le nom commence par une majuscule ;
  `_test.go` est du code de test.

## Alternatives écartées

- **Nommer par le paquet seul** (`go:example.com/shop/cart#New`) : deux `init` d'un même paquet
  auraient le même identifiant.
- **`go test -json` lu par Timonier** : un second lecteur de résultats à côté du JUnit, que
  tous les autres langages donnent.
- **Annoter chaque ligne ajoutée (`// timonier`)** : `go mod tidy` déplace les lignes entre ses
  blocs ; un bloc balisé est plus simple à reprendre.

## Conséquences

- Après `uninit`, `go mod tidy` remet ce que le bloc retiré portait d'indirect.
- Un paquet dont le nom diffère du dernier segment (`…/v2`, `gopkg.in/yaml.v3`) n'est pas
  relié par ce segment : l'appel reste vers rien, sauf alias explicite.
- Un appel chaîné (`New().Total()`) ne garde que son chemin extérieur, comme dans les autres
  packs.
