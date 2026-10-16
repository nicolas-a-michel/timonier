# 0012 — Format des packs de langage

- Statut : Accepté (2026-10-16, validation de la phase par l'humain)
- Date : 2026-10-16
- Phase : 3
- Liens : spec « Initialisation » (« Packs de langage »), « Indexation et modèle de graphe » ;
  skill `language-pack` ; ADR 0008, 0013 ; tâche PLAN.md 3.1

## Contexte

La spec veut que tout ce qui dépend du langage vive dans un pack versionné et extensible :
ajouter un langage revient à écrire un pack, pas à modifier le démon. Le MVP livre Rust et
TypeScript, pour que Timonier s'indexe lui-même. Il faut fixer le format, l'emplacement, et la
frontière entre ce qui est donnée et ce qui reste du code.

Versions vérifiées le 2026-10-16 : tree-sitter 0.27.1, tree-sitter-rust 0.24.2,
tree-sitter-typescript 0.23.2 (toutes MIT) ; toml 1.1.8 (MIT OR Apache-2.0).

## Décision

**Emplacement** : `crates/timonier-index/packs/<langue>/`, incorporé au binaire à la
compilation. Un pack contient :

- `pack.toml` : le manifeste ;
- `definitions.scm` : les définitions, qui deviennent des symboles ;
- `references.scm` : les imports, appels, références, implémentations et héritages ;
- `roles.scm` : les signaux de rôle lus dans le code.

**Manifeste** (`format = 1`), refusé s'il contient une clé inconnue :

- `language` : l'identifiant de langue de l'ADR 0008 (`rust`, `typescript`) ;
- `version` : version du pack. Elle entre dans la clé du cache d'extraction (ADR 0013) :
  toute modification d'une requête ou d'une règle l'incrémente ;
- `ecosystem` : l'écosystème de paquets, `cargo` ou `npm` ;
- `[[grammars]]` : pour chaque grammaire, son nom dans la table des grammaires, la crate, sa
  version, sa licence, les extensions de fichier qu'elle lit ;
- `[naming]` : séparateurs du nom qualifié (entre segments de module, entre le module et le
  premier symbole, entre symboles imbriqués) ;
- `[paths]` : comment lire un chemin écrit dans le code :
  - `separators` : les séparateurs (`::`, `.`) ;
  - `prefix_field` : le champ d'une liste d'import qui préfixe les chemins qu'elle contient ;
  - `trailing_self` : le dernier segment qui désigne le module lui-même (`use a::{self}`) ;
  - `bind` : les noms qu'un import lie. `last_segment` en Rust, `captures` en TypeScript ;
- `[fingerprint]` : jetons ignorés dans l'empreinte (par exemple `;` en TypeScript) et jetons
  ignorés devant un délimiteur fermant (la virgule finale) ;
- `[signature]` : jetons sans espace avant ou après, pour écrire la signature sous une forme
  canonique ;
- `[dependencies]` : catégories d'usage des dépendances externes (réseau, sérialisation, base
  de données, async…), par nom de paquet ;
- `[[roles]]` : signaux de rôle par bibliothèque utilisée ou par chemin importé, avec une
  confiance ;
- sections réservées, lues telles quelles sans être interprétées : `[lsp]`, `[tests]` et
  `[instrumentation]` (Phases 6 et 8), `[security]` (v2).

**Requêtes** : des requêtes tree-sitter dont les noms de capture forment un contrat. Une
capture inconnue est refusée au chargement.

| Fichier | Capture | Sens |
| ------- | ------- | ---- |
| `definitions.scm` | `@definition.<type>` | La définition entière. Types : module, type, trait, function, method, field, constant |
| | `@name` | Son nom, qui devient le dernier segment du nom qualifié |
| | `@body` | Son corps, facultatif : la signature est ce qui précède, l'empreinte porte sur lui |
| | `@scope`, `@scope.<x>` | Un conteneur qui n'est pas un symbole (un `impl` Rust) mais qui nomme ce qu'il contient |
| `references.scm` | `@import`, `@import.path` | Une instruction d'import, et chaque chemin qu'elle importe |
| | `@import.alias`, `@import.name` | L'alias d'un chemin importé (`as`), un nom lié par un import |
| | `@reference.call`, `@reference.method` | Un chemin appelé ; un appel sur un receveur (`x.f()`), dont le type est inconnu |
| | `@reference.type`, `@reference.implements`, `@reference.inherits` | Le type cité, le trait implémenté, la classe étendue |
| | `@reference.source` | Le type du fichier qui implémente ou hérite |
| `roles.scm` | `@role.<rôle>` | Un signal de rôle trouvé dans le code |

Une capture qui commence par `_` est libre : elle sert aux prédicats (`#eq?`, `#match?`). Un
motif sans capture est refusé : c'est presque toujours un prédicat écrit hors des parenthèses
de son motif.

Quand plusieurs motifs capturent la même définition, le premier motif du fichier l'emporte
(une méthode avant une fonction).

Une requête est compilée pour chaque grammaire du pack. Les motifs propres à une grammaire vont
dans `<requête>.<grammaire>.scm`, ajouté à la requête pour cette grammaire seulement : les
éléments JSX sont dans `references.tsx.scm`, car la grammaire `typescript` refuserait ces
nœuds.

Un conteneur `@scope` nomme son segment par la propriété `timonier.segment`, un gabarit où
`{scope.x}` est remplacé par le texte de la capture. La propriété `timonier.parent` dit sous
quel symbole ranger ce qu'il contient. Exemple Rust : `impl Display for Foo` donne le segment
`<Foo as Display>`, et ses méthodes se rangent sous `Foo`.

**Ce qui reste du code**, dans `timonier-index` et jamais dans le démon :

- la table des grammaires : une grammaire est du C compilé, à raison d'une ligne par grammaire ;
- les écosystèmes `cargo` et `npm`. Un écosystème découvre les paquets dans les manifestes,
  donne le chemin de module d'un fichier et résout un chemin importé vers un fichier ou un
  paquet.

Une langue qui rejoint un écosystème existant (JavaScript chez `npm`) n'a besoin que d'un
pack. Un nouvel écosystème (Python, Go) demande du code dans `timonier-index`.

**Packs de l'utilisateur** : plus tard, dans `.timonier/packs/`, avec des grammaires WASM. Le
format le permet déjà, car un pack ne nomme sa grammaire que par son nom.

## Alternatives écartées

- **Un trait Rust implémenté par langue** : c'est l'analyseur par langage que la spec refuse.
- **Des grammaires chargées à l'exécution** : une bibliothèque partagée demande un compilateur
  C chez l'utilisateur ; la feature `wasm` de tree-sitter embarque wasmtime, ce qui est lourd
  pour le MVP.
- **Les `tags.scm` livrés avec les grammaires** : ils n'ont ni imbrication, ni corps, et leurs
  types ne sont pas ceux de la spec. Nos requêtes s'en inspirent.
- **Un manifeste en JSON** : il n'a pas de commentaires. TOML est l'usage pour ce genre de
  fichier (Cargo, Helix).

## Conséquences

- Un test charge chaque pack livré, compile ses requêtes avec sa grammaire et vérifie le
  contrat des captures.
- Les requêtes sont testées par des instantanés `insta` des nœuds et arêtes extraits.
- Modifier une requête sans incrémenter `version` laisserait le cache servir l'ancienne
  extraction. Le skill `language-pack` le rappelle.
- La clé du cache porte aussi la version de la grammaire, déclarée dans le manifeste. Un test
  vérifie qu'elle est celle de l'espace de travail : monter `tree-sitter-rust` oblige à mettre
  le pack à jour.
