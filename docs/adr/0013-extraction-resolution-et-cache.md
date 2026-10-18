# 0013 — Extraction tree-sitter, résolution approchée et cache

- Statut : Accepté (2026-10-16, validation de la phase par l'humain) ; complété par 0023 (un
  fichier d'exclusion qui change fait refaire le parcours ; dans un dépôt git, les règles de
  git comme git les applique)
- Date : 2026-10-16
- Phase : 3
- Liens : spec « Indexation et modèle de graphe », « Décisions techniques » (granularité des
  `Calls`, indexation incrémentale) ; ADR 0008, 0009, 0012 ; Q14 ; tâches PLAN.md 3.2 à 3.4

## Contexte

L'indexation tree-sitter doit produire, pour chaque fichier, ses symboles et ses arêtes, avec
des identités stables, sans analyseur par langage. Tree-sitter ne résout pas les noms : il
faut une résolution approchée, que le LSP corrige ensuite (ADR 0009). La seconde indexation
d'un dépôt inchangé ne doit reparser aucun fichier (critère de 3.4).

Versions vérifiées le 2026-10-16 : blake3 1.8.7 (CC0-1.0 OR Apache-2.0), rayon 1.12.0 (MIT
OR Apache-2.0), ignore 0.4.33 et globset 0.4.20 (Unlicense OR MIT).

## Décision

**Fichiers indexés** : ceux dont l'extension appartient à un pack. Le parcours respecte
`.gitignore` et saute les dossiers cachés (crate `ignore`).
- Un fichier de plus de 2 Mio est du code généré ou minifié : il n'est pas indexé, et il quitte
  le graphe s'il le devient.
- Un chemin qui ne fait pas d'identifiant de fichier (un `\` dans le nom) est écarté et signalé.
- Une erreur locale ne fait jamais échouer la passe : un manifeste illisible est écarté et
  signalé, les autres paquets restent.

**Paquets** (écosystèmes `cargo` et `npm`)

- Un paquet est :
  - un membre d'un espace de travail (`[workspace] members` de Cargo, `workspaces` de npm,
    motifs compris) ;
  - la racine de l'espace de travail, si elle déclare elle-même un paquet ;
  - ou un manifeste isolé, qui ne se trouve dans le dossier d'aucun autre paquet.
- Un manifeste imbriqué dans un paquet sans en être membre est ignoré : ses fichiers
  appartiennent au paquet englobant.
- Un fichier appartient au paquet dont le dossier le contient au plus près. Un fichier hors de
  tout paquet se range sous le Repo (Q14).
- Un paquet qui ne possède aucun fichier indexé n'apparaît pas.
- Identifiants : `pkg:cargo/<nom>`, `pkg:npm/<nom>`.

**Extraction**, une fonction pure du contenu et du pack :

- Les définitions viennent de `definitions.scm`.
- Le parent d'une définition :
  - le symbole désigné par la propriété `timonier.parent` de son conteneur, s'il existe dans
    le fichier ;
  - sinon la définition qui l'englobe au plus près ;
  - sinon le fichier.
- Deux définitions du même nom qualifié dans un fichier n'en font qu'une : la première qui a
  un corps, sinon la première. C'est le cas d'une surcharge TypeScript suivie de son
  implémentation, d'une fusion de déclarations, ou de deux fonctions sous des `#[cfg]`
  différents.
- Arêtes : chaque import, appel ou référence est relevé comme un chemin (`a::b::c`), avec son
  nombre d'occurrences.

**Noms qualifiés** : `<langue>:<chemin de module><jonction><segments>`, avec les séparateurs du
pack.

- Rust (`::` partout) :
  - le chemin de module part du nom de la crate (le nom de `[lib]`, sinon le nom du paquet
    avec `_` à la place de `-`) ;
  - `src/lib.rs` et `src/main.rs` (sans lib) sont la racine ;
  - les autres fichiers de `src/` sont des modules de la lib : `src/a/b.rs` donne `a::b`, et
    `src/a/mod.rs` donne `a` ;
  - les autres cibles reçoivent un préfixe qu'aucun module ne peut porter, par exemple
    `timonier_store::@tests::graph_history` pour `tests/graph_history.rs`. Il en va de même
    pour `@bin`, `@examples`, `@benches` et `@build`, et pour `src/main.rs` quand le paquet a
    une lib. `build.rs` et un module `src/build.rs` ne se confondent donc pas.
- TypeScript : `<nom du paquet>/<chemin dans le paquet, sans extension>`, puis `#`, puis les
  symboles imbriqués séparés par `.`. Exemple : `typescript:@timonier/webview/src/App#App`.
- Hors paquet, le chemin du fichier tient lieu de chemin de module.
- Le nom ne dépend ni de la mise en forme, ni des commentaires, ni de l'ordre des définitions
  dans le fichier.

**Empreinte et signature**

- Les jetons sont les feuilles de l'arbre, sans les commentaires et sans les jetons ignorés
  par le pack.
- Le hash du corps est un BLAKE3 de ces jetons, pris dans `@body`, ou dans toute la
  définition sans `@body`.
- La signature est faite des jetons qui précèdent le corps, réécrits avec un espacement
  canonique (`pub fn from_hex(hex: &str) -> Result<Self>`).
- Reformater un fichier ne change donc ni l'identité, ni la signature, ni le hash. Une limite
  reste : une mise en forme qui ajoute ou retire des jetons, comme des parenthèses ou des
  accolades, change le hash.

**Résolution approchée** (provenance `TreeSitter`) : un chemin se résout, dans l'ordre, par :

1. les noms importés par le fichier ;
2. les mots-clés relatifs du langage (`crate`, `self`, `super`, chemins `./` et `../`) ;
3. le nom d'un paquet de l'espace de travail ;
4. le nom d'une dépendance déclarée, qui donne le paquet externe ;
5. pour un nom seul, une définition unique de ce nom dans le paquet, et en Rust dans la même
   cible : la lib ne voit ni un test d'intégration, ni les modules d'un binaire.

Un appel sur un receveur (`x.push()`, `this.n()`) ne se résout que par un import ou un chemin
(`ns.k()` quand `ns` est importé), jamais par son seul nom : le type du receveur est inconnu de
tree-sitter. Par son nom, `.iter()` mènerait au seul `iter` du paquet, et ces fausses arêtes
façonneraient les composants. Le LSP les donne.

Une réexportation est suivie, sur quatre niveaux au plus. Quand un chemin continue au-delà
du module trouvé, et que ce module importe le nom suivant, la résolution suit cet import :
- `timonier_core::Node` mène à `node.rs` par le `pub use node::Node` de `lib.rs` ;
- `export { x } from "./x"` mène au fichier qui définit `x`.

Sans cela, tout le code d'une crate semblerait dépendre de son `lib.rs`.

Les arêtes qui en sortent :

| Arête | Granularité | Confiance |
| ----- | ----------- | --------- |
| `Imports` | fichier → fichier, ou fichier → paquet (dépendance externe ou paquet du dépôt sans fichier trouvé) | 0,9 |
| `Calls`, `References` | fichier → fichier, jamais vers soi-même | 0,8 par import ou chemin complet ; 0,5 par nom unique dans le paquet |
| `Implements`, `Inherits` | symbole → symbole, quand le type est défini dans le fichier | idem |
| `DependsOn` (`Manifest`) | paquet → paquet, dépendances normales seulement (ni dev, ni test) | 1,0 |

- Le poids est le nombre d'occurrences.
- Un nom ambigu, défini dans plusieurs fichiers, ne produit pas d'arête.
- Les appels vers un paquet externe ne sont pas gardés : l'import suffit.

**Mise à jour par dépendance** : la résolution d'un fichier dépend des autres. Le modèle du
projet est tenu en mémoire.
- Quand les noms qu'un fichier met à disposition changent, seuls les fichiers qui citent ces
  noms sont résolus à nouveau, sans reparse. Ces noms sont ses définitions et ce que lient ses
  imports, à cause des réexportations.
- Il en va de même quand un fichier apparaît ou disparaît.
- Les cibles de ses réexportations comptent aussi : `pub use a::X` qui devient `pub use b::X`
  résout à nouveau ceux qui citent `X`.
- Un manifeste modifié, ou une lib Cargo créée ou supprimée (pas simplement enregistrée),
  relit les paquets. S'ils sont les mêmes, rien d'autre ne se passe. S'ils ont changé, tous les
  fichiers sont placés et nommés à nouveau, sans reparser ceux dont le contenu n'a pas changé,
  et les composants sont recalculés (ADR 0016). Un manifeste qui ne se lit pas, en cours
  d'édition, garde son paquet tel qu'il était ; les autres suivent leurs manifestes.
- Un chemin nouveau dans un dossier que le parcours a écarté (`target/`, `node_modules/`) est
  ignoré sans nouveau parcours : une compilation ne déclenche pas de travail.

**Cache persistant** : l'extraction d'un fichier, qui ne dépend que de son contenu, est gardée
dans la base SQLite (table `extraction`, migration 0002).

- Clé : le chemin, la langue, la version du pack et celle de sa grammaire, la version de
  l'extracteur et le hash BLAKE3 du contenu.
- La seconde indexation lit et hache chaque fichier, puis reprend l'extraction en cache. Un
  compteur de reparses le vérifie.

**Indexation initiale** : lecture, hachage et extraction en parallèle avec rayon, un parseur
par fil. La résolution et l'écriture se font ensuite en un seul lot (`Store::apply_all`).

**Surveillance** : le watcher ne surveille, un par un, que les dossiers où le parcours entre,
plus le dossier git. `target/` ou `node_modules/`, qui comptent des milliers de dossiers, ne
coûtent aucune surveillance.
- Tous les dossiers parcourus sont surveillés avant l'indexation initiale.
- Un dossier nouveau est ajouté quand le parcours le trouve, puis relu : ce qui y a changé
  avant sa surveillance n'est pas perdu.
- Une surveillance meurt avec son dossier. Un dossier disparu est oublié, et un dossier qui
  revient (changement de branche) est surveillé à nouveau.

**Reprise** : si une écriture dans la base échoue, la mémoire de l'indexeur a pris de l'avance
sur elle. Tout est alors indexé à nouveau, depuis le disque et le cache, pour les remettre
d'accord.

**Reparse incrémental** : les arbres des 64 derniers fichiers modifiés restent en mémoire. La
modification est déduite du plus long préfixe et du plus long suffixe communs, appliquée à
l'arbre (`Tree::edit`), puis le fichier est reparsé à partir de l'ancien arbre.

## Alternatives écartées

- **Résoudre les imports vers des symboles** (`use a::Node` → `rust:a::Node`) : les
  réexportations (`pub use node::Node`) laisseraient la plupart de ces arêtes pendantes. Le
  fichier est la bonne granularité pour tree-sitter ; le LSP donne les symboles.
- **stack-graphs** : de la résolution de noms pilotée par des règles par langue, mais pas de
  règles pour Rust, et le projet n'est plus développé.
- **Résoudre à nouveau tout le dépôt à chaque sauvegarde** : simple, mais sur 5 000 fichiers la
  comparaison avec la base dépasse à elle seule les 100 ms.
- **Garder tous les arbres en mémoire** : plusieurs centaines de Mo sur un gros dépôt, pour un
  gain qui ne vaut que sur les fichiers en cours d'édition.
- **Un cache dans des fichiers à part** : la base est déjà un cache reconstructible, avec un
  seul écrivain.

## Conséquences

- Les arêtes tree-sitter sont volontairement grossières : le LSP les remplace dès qu'il a
  vérifié un fichier.
- Les noms qualifiés Rust sont approchés : un fichier de `src/` utilisé seulement par
  `main.rs` est nommé comme un module de la lib, et `#[path]` est ignoré. SCIP (v2) donnera les
  noms exacts.
- Les extensions ou chemins d'import que l'écosystème ne connaît pas (alias de `tsconfig`,
  macros Rust) ne produisent pas d'arête.
