# 0073 — L'intégration de SCIP : au commit, par les packs, sur nos identifiants

- Statut : Proposé
- Date : 2026-10-23
- Phase : 11
- Liens : spec « Indexation et modèle de graphe », « Décisions techniques » ; Q94, Q95 ; ADR 0008
  (identifiants), 0009 (fusion), 0014 (affinage LSP), 0015 (commit), 0018 (compaction), 0034
  (packs, volet init) ; tâches PLAN.md 11.1 à 11.4

## Contexte

La mesure de 11.1 montre que les liens de tree-sitter sont justes à 90-100 % (75 % en C#), mais
qu'il n'en trouve que 46 à 87 %. SCIP donne les références exactes, avec trois contraintes
vérifiées le 2026-10-22 et le 2026-10-23 :
- aucun indexeur n'est incrémental, et chacun réécrit tout l'index ;
- plusieurs exigent un projet prêt, avec ses dépendances installées. Sans Flask installé,
  scip-python ne voyait aucun lien des tests vers `src/`. scip-java fait le build Maven ou
  Gradle, scip-dotnet fait `dotnet restore`, et scip-clang veut `compile_commands.json` ;
- la crate `scip` 0.10.0 décode l'index en entier en mémoire.

La spec place SCIP au commit, et Q95 garde nos identifiants.

## Décision

**Quand.** Après chaque instantané de commit (ADR 0015), et au démarrage si le dernier commit
n'a pas d'index. Le démon lance les indexeurs en tâche de fond, un à la fois, à basse priorité
(`nice 10` sous Linux et macOS) :
- **annulé** par un nouveau commit, puis relancé sur celui-ci ;
- **délai maximal** de 10 minutes par indexeur, réglable ;
- **jamais bloquant** : la carte reste servie pendant ce temps, avec tree-sitter et le LSP.

**Les packs déclarent leur indexeur**, dans une section `[scip]` de `pack.toml` :
- `program` et `args`, avec `{output}` pour le chemin de l'index (sans lui, l'indexeur écrit
  `index.scip` là où il tourne), `{project}`, `{needs}` et `{variant}` ;
- `needs` : les fichiers qui doivent exister, comme `compile_commands.json`, ou un
  environnement virtuel où le projet est installé. S'ils manquent, `doctor` dit quoi faire et
  l'indexeur n'est pas lancé ;
- `in_place` : faux pour un indexeur qui nettoie ou réécrit les sorties de build du
  développeur (scip-java lance `clean`). Il tourne alors dans un worktree git du commit, sous
  `.timonier/scip/` ; les autres tournent dans l'arbre de travail. Un fichier que l'arbre de
  travail a changé depuis le commit ne reçoit rien d'un index fait dans le worktree : ses faits
  seraient ceux du commit, sous le hash du contenu présent (relecture de la Phase 11, I1).

- `writes` : les fichiers que l'indexeur peut créer dans le projet, retirés après lui s'ils n'y
  étaient pas : `scip-typescript --infer-tsconfig` écrit un `tsconfig.json` (vérifié le
  2026-10-23) ;
- `variants` : une valeur choisie par les fichiers présents, pour `{variant}` : scip-java doit
  savoir l'outil de build quand un projet a à la fois Maven et Gradle (spring-petclinic) ;
- `[scip.environment]` : l'environnement du projet, activé pour l'indexeur comme son script
  d'activation le ferait (dossiers candidats, fichier qui le signale, dossier des programmes,
  variable) ; scip-python ne résout les imports du projet que là où il est installé
  (`virtual_env` jusqu'à l'ADR 0077) ;
- `bin_dirs` : où l'installeur met l'indexeur, sous le dossier personnel (ADR 0077), créé
  avant l'installation ;
- `install` : la commande d'installation, une par indexeur, montrée puis lancée seulement sur
  approbation, comme les lanceurs de tests (Q94, ADR 0034) ; `{home}` y est le dossier
  personnel. scip-java est installé une fois par `coursier bootstrap`, qui en
  écrit un lanceur, au lieu d'être téléchargé par `coursier launch` à chaque commit (relecture
  de la Phase 11, I6).

Deux langages du même indexeur (mêmes `program`, `args` et `in_place` : C et C++ ;
TypeScript et JavaScript diffèrent par `--infer-tsconfig`) le font tourner une fois ; chacun lit ses fichiers dans le même index. Un
indexeur sans `{output}` écrit `index.scip` là où il tourne : un fichier de ce nom qui était au
projet est mis de côté pendant ce temps, puis remis.

Un programme ne compte comme présent que s'il répond à `--version` : le raccourci de rustup
pour un composant non installé est dans le `PATH` et échoue. Le démon le cherche aussi dans les
dossiers où les installeurs le mettent (`~/.cargo/bin`, `~/go/bin`, `~/.dotnet/tools`,
`~/.local/bin`, `~/.local/share/coursier/bin`), que le `PATH` hérité de l'éditeur peut ne pas
avoir.

Un langage sans indexeur n'a pas de section `[scip]` : c'est le cas de Scala, de Swift et pour
l'instant de PHP. Sa carte reste approchée, et 11.5 le montre.

**L'index** s'écrit dans `.timonier/scip/<langage>.scip` ; le dossier a son propre
`.gitignore` (`*`), comme `.timonier/perf/`, avec les statuts, les journaux et le worktree. Il est supprimé une
fois lu : ses faits passent dans le graphe. Un index de plus de 256 Mo n'est pas lu, et `doctor`
le signale. C'est un seuil à revoir sur mesure.

**Sur nos identifiants (Q95).** Chaque occurrence est rattachée au graphe par sa plage, comme
l'appelé du LSP (ADR 0014) :
- la **source** d'une référence est le plus petit symbole de Timonier qui contient
  l'occurrence, et à défaut son fichier ;
- la **cible** est le plus petit symbole qui contient l'occurrence de définition du symbole
  SCIP, et à défaut son fichier. Un symbole défini hors du dépôt est écarté ;
- les plages sont lues dans `typed_range` quand il existe, sinon dans `range` (crate 0.10.0).
  Elles sont converties vers les colonnes en octets UTF-8 de l'ADR 0008, selon le
  `position_encoding` du document.

**Les arêtes produites**, en provenance `Scip` :
- une référence à un symbole de genre fonction ou méthode devient un `Calls` symbole → symbole ;
- une autre référence devient un `References` ;
- une relation `is_implementation` devient un `Implements` si la cible est une interface ou un
  trait, et un `Inherits` sinon.

Chaque fichier indexé reçoit `verifying([Calls, References, Implements, Inherits])` ;
`Implements` et `Inherits` seulement si l'index contient au moins une relation, puisque
rust-analyzer n'en émet aucune et que tree-sitter doit alors garder les siennes. Tout cela, avec
`computed_on` au hash du contenu que l'indexeur a lu, relevé avant son lancement. La fusion de
l'ADR 0009 fait le reste : dans les fichiers vérifiés, les arêtes de tree-sitter de ces genres
tombent, et un fichier modifié depuis garde les siennes, puisque les observations calculées sur
un autre contenu sont ignorées. La compaction de l'ADR 0018 compte aussi les arêtes `Scip`.

**Les imports confirmés** (ajouté le 2026-10-23, mesure de 11.8) : aucun des six indexeurs ne
marque ses occurrences d'import (`SymbolRole::Import`, 0 occurrence sur les six index de 11.1),
et `Imports` n'est donc pas un genre vérifié. Un `Imports` de tree-sitter de A vers B est
confirmé quand l'index relie A à B (A cite un symbole défini dans B) : il devient un `Imports`
`Scip` de même poids, que la fusion préfère ; un import que l'index ne relie pas reste celui de
tree-sitter (`timonier_index::scip::confirm_imports`, `Project::file_imports`). Sans cela,
après SCIP, 93 à 100 % des lignes encore pointillées entre boîtes étaient des imports.

**Un index incomplet est refusé.** Si SCIP confirme moins de 70 % des liens entre fichiers que
voit tree-sitter, dont 90 à 100 % sont justes d'après 11.1, l'index est jugé incomplet : un
projet pas prêt ou un build qui a échoué. Il n'est pas appliqué, la carte reste celle de
tree-sitter et du LSP, et `doctor` en donne la raison. Sur Flask sans dépendances, SCIP n'en
confirmait que 51 %, contre 90 % une fois Flask installé. Revu en 11.8 : les six projets
préparés confirment 90 % ou plus (C# 99 % après 11.11) ; un projet pas prêt reste sous 70 %
(Flask, 51 %). Le seuil est gardé.

**Annuler ou arrêter un indexeur** (un commit plus récent, les 10 minutes passées) arrête aussi
ce qu'il a lancé : sous Unix, il mène son propre groupe de processus, tué en entier ; sous
Windows, `taskkill /T` (relecture de la Phase 11, I5).

**L'état** de chaque langage s'écrit dans `.timonier/scip/status.json` : commit, indexeur,
état (`applied`, `missing`, `not_ready`, `incomplete`, `failed`, `timed_out`, `too_large`,
`cancelled`), durée, part confirmée. Le journal de l'indexeur va dans
`.timonier/scip/<langage>.log`. `timonier scip` le dit langage par langage ; `--install`
installe les indexeurs manquants par les commandes de leurs packs, que le clic de la commande
« Installer les indexeurs SCIP » de l'extension approuve (Q94) ; `--now` demande l'indexation
du dernier commit (méthode `scip/run`). `doctor` en fait des **notes**, jamais des constats : la
carte tient sans SCIP. `TIMONIER_SCIP=off` coupe SCIP, `TIMONIER_SCIP_TIMEOUT` change le délai.

**La dépendance** : la crate `scip` =0.10.0 (Apache-2.0) et `protobuf` =3.7.2 (MIT, la
rust-protobuf de stepancheg, et non le `protobuf` 4.x de Google), vérifiées le 2026-10-22. Elles
entrent dans `timonier-index`, qui lit l'index. Le lancement des indexeurs appartient au démon.

## Alternatives écartées

- **SCIP à chaque sauvegarde** : aucun indexeur n'est incrémental. Il faudrait des secondes,
  voire un build, à chaque fois.
- **Les identifiants SCIP** (« le symbole SCIP quand il existe », spec) : l'historique les
  verrait supprimés puis recréés au premier passage. C'est écarté par Q95.
- **Tous les indexeurs dans l'arbre de travail** : `clean` effacerait les sorties de build du
  développeur pendant qu'il travaille.
- **Tous dans un worktree propre** : sans `node_modules` ni environnement virtuel, scip-python
  et scip-typescript donneraient l'index incomplet mesuré sur Flask.
- **Appliquer tout index** : un projet pas prêt rendrait la carte moins juste qu'avec
  tree-sitter seul.

## Conséquences

- La carte gagne les liens que tree-sitter manque, après chaque commit, mais seulement dans
  les langages qui ont un indexeur et dans un projet prêt. Entre deux commits, les fichiers
  modifiés gardent les liens de tree-sitter.
- Le démon lance des programmes lourds : jusqu'à 1,1 Go et 35 s mesurés en 11.1. Il faut le
  dire dans `doctor` et dans la documentation, et laisser couper SCIP par un réglage.
- Le seuil de 70 %, la limite de 256 Mo et le délai de 10 minutes sont des valeurs de départ,
  à mesurer en 11.4 et 11.8.
