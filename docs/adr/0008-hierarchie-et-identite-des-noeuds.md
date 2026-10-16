# 0008 — Hiérarchie et identité des nœuds

- Statut : Accepté (2026-10-16, validation de la phase par l'humain)
- Date : 2026-10-14
- Phase : 2
- Liens : spec « Indexation et modèle de graphe » ; PLAN.md Q4 (réponse de l'humain) ; skill
  `graph-model`

## Contexte

La spec fixe les types de nœuds (Repo, Component, Package, File, Symbol), une hiérarchie
`parent` en arbre strict, et une identité stable : le symbole SCIP s'il existe, sinon
`lang:qualified_name`. L'humain a tranché la place des composants (Q4, option a). Il reste à
fixer :

- quels parents sont permis ;
- l'identité des nœuds qui ne sont pas des symboles ;
- la forme textuelle exacte des identifiants ;
- l'unité des positions.

## Décision

**Parents permis** (Q4 a : `Repo → Component* → Package → Component* → File → Symbol`)

| Nœud | Parent permis |
| ---- | ------------- |
| Repo | aucun (racine) ; un seul Repo à la fois |
| Component | Repo, Component ou Package |
| Package | Repo ou Component ; jamais de Package parmi ses ancêtres |
| File | Repo, Package ou Component |
| Symbol (module, type, trait, function, method, field, constant) | File ou Symbol |

Tout nœud autre que Repo a un parent, et la hiérarchie ne contient aucun cycle.

**Identifiants** : une chaîne, dont le préfixe dit la nature.

| Nœud | Forme | Exemple |
| ---- | ----- | ------- |
| Repo | `repo:<nom>` | `repo:timonier` |
| Package | `pkg:<écosystème>/<nom>` (comme un purl, utile pour le SBOM de la Phase 9) | `pkg:cargo/timonier-core`, `pkg:npm/@timonier/webview` |
| Component | `component:<clé>` (la clé est stabilisée par le clustering en Phase 3) | — |
| File | `file:<chemin relatif POSIX>` (sans `/` initial, sans `\`, sans segment vide, `.` ou `..`) | — |
| Symbol, symbole SCIP connu | `scip:<symbole SCIP tel quel>` | — |
| Symbol, sinon | `<langue>:<nom qualifié>` (forme de la spec) | `rust:timonier_core::Node` |

Précisions sur ces identifiants :

- Le préfixe `scip:` évite toute confusion avec la forme de repli. Le symbole lui-même est
  conservé à l'identique.
- `repo`, `pkg`, `component`, `file` et `scip` sont réservés et ne peuvent pas servir de
  langue.
- Une langue s'écrit `[a-z][a-z0-9_+-]*` (`rust`, `typescript`). Ce sont les packs qui la
  définissent.
- Les identifiants ne contiennent aucun caractère de contrôle.

**Cohérence identifiant ↔ type**, contrôlée à chaque écriture :

- le préfixe de l'identifiant doit désigner le type du nœud ;
- un symbole `<langue>:…` doit porter cette langue s'il en déclare une.

**Clés de portée** (ADR 0009) : `structure`, ou l'identifiant d'un fichier (`file:<chemin>`).
Rien d'autre n'est accepté.

**Extension de Q4, acceptée par l'humain (Q14, 2026-10-16)** : un File peut aussi se placer directement sous le
Repo ou sous un Component, pour les fichiers qui n'appartiennent à aucun package (scripts à la
racine, par exemple). La chaîne de Q4 ne prévoyait de File que sous un Package ou sous un
Component à l'intérieur d'un package.

**Positions** : ligne et colonne comptées à partir de 0, la colonne en octets UTF-8, comme
tree-sitter. Le front-end convertit vers les unités de son éditeur (UTF-16 dans VSCode).

**Empreinte des symboles** : une signature normalisée et le hash du corps normalisé (32
octets). Les packs les calculent en Phase 3 ; le modèle se contente de les porter.

## Alternatives écartées

- **Symbole SCIP sans préfixe** : les deux formes ne se distingueraient que par la présence
  d'espaces dans le symbole SCIP. C'est fragile.
- **Fichiers identifiés par le hash de leur contenu** : l'identité changerait à chaque
  modification. Le chemin est stable, et les déplacements sont détectés au diff (ADR 0010).
- **Colonnes en UTF-16** : il faudrait convertir chaque position renvoyée par tree-sitter,
  alors que le front-end n'en convertit que quelques-unes, au moment d'ouvrir le code.

## Conséquences

- Les noms qualifiés de repli peuvent entrer en collision (surcharges). Chaque pack doit
  produire des noms uniques par fichier, avec un discriminant si nécessaire (Phase 3).
- Déplacer un fichier change l'identifiant du fichier et souvent celui de ses symboles. C'est
  voulu : le diff reconnaît le déplacement par similarité au lieu de l'afficher comme une
  suppression suivie d'une création.
