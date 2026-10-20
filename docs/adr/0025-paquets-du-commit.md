# 0025 — Snapshot de commit : les paquets du commit

- Statut : Accepté (2026-10-20)
- Date : 2026-10-18
- Phase : 5
- Liens : Q23 ; complète l'ADR 0015 (« Ce qui est gelé ») et l'ADR 0022 (retour de l'arbre de
  travail) ; ADR 0013 (paquets, nommage de `src/main.rs`), 0016 (composants) ; tâche
  PLAN.md 5.4

## Contexte

Le gel d'un commit prend la version commitée des fichiers source qui en diffèrent (ADR 0015).
Il gardait pourtant les paquets de l'arbre de travail. Un manifeste modifié sans commit (crate
renommée, dépendance ajoutée), ou un `src/lib.rs` présent d'un seul côté, faisait donc un
snapshot de commit faux. Tu as reporté la correction à la Phase 5 (Q23). Rien ne lit encore
ces snapshots : le diff de revue compare la dernière revue et la tête. Ils serviront à
comparer des commits (historique, diff de PR en v2).

## Décision

- **Les manifestes du commit** : avant le gel, ceux que git dit modifiés, ajoutés ou supprimés
  sont lus dans le commit, comme les fichiers source, et filtrés comme eux : seuls comptent
  ceux que le parcours lit, ou, supprimés du disque, hors d'un dossier caché et que git
  n'ignore pas.
- **Ses paquets** : la découverte des paquets (ADR 0013) est refaite sur une superposition. Les
  versions commitées des manifestes et des `src/lib.rs` (ou leur absence) y remplacent le
  disque. Le reste vient du disque, identique au commit. Si le résultat diffère de ce que donne
  le disque seul, le commit a d'autres paquets.
- **Le gel sous ces paquets** : tous les fichiers sont replacés et renommés, depuis les faits
  gardés, sans relire le disque. Leurs identifiants peuvent changer : leur vérification LSP
  pendrait à des identifiants que le commit n'a pas, et masquerait les arêtes de tree-sitter.
  Elle est donc retirée le temps du gel. Les composants sont recalculés, comme à tout commit.
- **Le retour** : en plus des fichiers sales (ADR 0022), les paquets de l'arbre de travail
  reviennent. Tous les fichiers sont replacés, les composants suivent, et la vérification LSP
  de chaque fichier revient telle qu'elle était, si son contenu n'a pas changé. Rien n'est
  redemandé au serveur.
- Sans paquet différent, rien ne change : le gel ne replace que les fichiers sales.

## Alternatives écartées

- **Indexer à part l'arbre du commit** (`git archive` dans un dossier temporaire) : ce serait
  une indexation complète à chaque commit de ce genre, et les composants partiraient de rien.
- **Ne replacer que les fichiers des paquets changés** : un paquet renommé change la résolution
  des noms des fichiers qui le citent, dans d'autres paquets.

## Conséquences

- Les snapshots de commit décrivent le commit dans les trois cas de Q23. Des tests les
  comparent à une indexation neuve d'un clone du commit, et la tête à une indexation neuve de
  l'arbre de travail.
- Un tel commit coûte deux placements de tous les fichiers et deux recalculs des composants.
  Mesuré le 2026-10-19 (`commit_bench`, build release, espace de travail synthétique de
  2 450 fichiers) : 697 ms, contre 216 ms pour un commit aux mêmes paquets. C'est rare : il
  faut un manifeste ou une bibliothèque Cargo différents du commit au moment du commit.
- Un manifeste que le parcours ne lit jamais (dans un dossier caché, ou ignoré par git même
  s'il est suivi) est écarté avant : il ne ferait qu'un paquet sans fichier, invisible, au
  prix d'un placement de tout et de la vérification LSP de tout le commit (relecture n° 2 de
  la Phase 5, m-7).
- Au retour, les composants sont recalculés à partir de ceux du commit : ils peuvent différer
  un peu de ceux d'avant le gel, comme après tout commit.
