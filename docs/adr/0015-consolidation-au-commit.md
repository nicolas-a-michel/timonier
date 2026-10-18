# 0015 — Vitesse 3 : consolidation au commit

- Statut : Accepté (2026-10-16, validation de la phase par l'humain) ; complété par 0022 (avant
  le gel, l'index est remis au niveau du disque, parcours compris ; le retour de l'arbre de
  travail après le gel garde la vérification LSP de ce qui revient tel quel, et suit le reste)
  et par 0023 (sont surveillés aussi `info/` du dossier git commun et les dossiers entre le
  haut du dépôt et la racine) ; écart connu, un manifeste ou un `src/lib.rs` différents du
  commit, à corriger en Phase 5 (Q23)
- Date : 2026-10-16
- Phase : 3
- Liens : spec « Décisions techniques » (trois vitesses), « Stockage » ; H4 ; ADR 0010
  (« Conséquences »), 0013 ; tâche PLAN.md 3.7

## Contexte

La spec demande, au commit : SCIP si disponible (en v2 d'après H4), un re-clustering complet
initialisé par la partition précédente, et un snapshot figé. L'ADR 0010 a laissé un point
ouvert : figer la tête au commit prendrait aussi les modifications non commitées. Il faut
indexer l'arbre commité, ou marquer le snapshot comme « sale ». Il faut aussi détecter le
commit.

Outil vérifié le 2026-10-16 : git 2.53 sur la machine de développement. git est présent sur
les runners GitHub ; la spec le suppose déjà (hooks, forges).

## Décision

**Le snapshot `commit` décrit exactement l'arbre commité.**
- D'abord, les fichiers qui ont changé depuis le dernier snapshot `commit`
  (`git diff --name-only --relative`) sont relus sur le disque. Le commit peut arriver au
  watcher avant les sauvegardes qui l'ont précédé : la tête est ainsi l'arbre de travail.
- Les fichiers qui diffèrent de ce commit viennent de
  `git status --porcelain=v1 -z --untracked-files=all --no-renames`. On ne garde que ceux que
  le parcours indexerait (ADR 0013). Un fichier supprimé de l'arbre de travail compte s'il
  n'est ni dans un dossier caché, ni exclu par `.gitignore` (`git check-ignore --no-index`).
- Leur version commitée vient de `git show <sha>:<chemin>`, et leur présence dans le commit de
  `git ls-tree`. Un fichier absent du commit (non suivi, ou ajouté sans être commité) est
  absent du snapshot.
- La consolidation applique ces versions au graphe, recalcule les composants (tâche 3.8),
  fige le snapshot `commit` avec le sha, puis réapplique les versions de l'arbre de travail
  dans la nouvelle tête.
- Le cache d'extraction ne garde que les versions de l'arbre de travail.
- Sans fichier modifié, le cas courant d'un commit de tout le travail, la tête est figée telle
  quelle.
- L'arbre de travail revient toujours, même si le gel échoue.
- Si git échoue, aucun snapshot n'est pris cette fois : un contenu inconnu ne passe jamais
  pour un fichier absent. Le prochain changement sous le dossier git réessaie.

**Détection** : par le watcher, sans hook git.
- Un changement dans le dossier git (`git rev-parse --absolute-git-dir`, dans le dépôt ou
  ailleurs pour un worktree) ou dans son `logs/` déclenche `git rev-parse --verify HEAD`.
  Ces deux dossiers sont surveillés à part.
- Si le sha diffère de celui du dernier snapshot `commit`, la consolidation a lieu. Elle
  couvre aussi un amend, un checkout ou un reset.
- Au démarrage, si HEAD n'a pas encore de snapshot `commit`, il en reçoit un. Le premier diff
  a ainsi une base.
- Sans dépôt git, ou sans commit, il n'y a pas de consolidation.

**Ordre** : la consolidation passe dans la même file que les sauvegardes, sur le fil du
pipeline. Une sauvegarde qui arrive pendant ce temps attend la fin de la consolidation.

## Alternatives écartées

- **Marquer le snapshot comme « sale »** : plus simple, mais l'évolution par commit
  (performances, voyage dans le temps) comparerait des états qui ne sont aucun commit.
- **Un hook `post-commit`** : il dépend de `timonier init` (Phase 6) et ne voit ni les
  checkouts, ni les resets, ni les commits faits par un autre outil sans hook. Le hook prévu
  par la spec pour `actual.json` pourra s'appuyer sur la même consolidation.
- **libgit2 ou gix** : une dépendance lourde pour quelques commandes. Le CLI de git est la
  référence et il est déjà installé.

## Conséquences

- Un commit qui laisse des fichiers modifiés coûte deux mises à jour de ces fichiers. Le
  diff depuis la dernière revue n'en montre que l'effet net.
- Les commits rapprochés, comme un rebase, sont consolidés un par un, au rythme de
  l'anti-rebond : des commits intermédiaires peuvent être sautés. Ce n'est pas grave, la tête
  reste exacte.
