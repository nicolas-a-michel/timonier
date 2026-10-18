# 0023 — Suivre les fichiers d'exclusion

- Statut : Accepté (2026-10-18, validation de la phase par l'humain)
- Date : 2026-10-18
- Phase : 4
- Liens : spec « Architecture technique » (un worktree par agent), « Indexation et modèle de
  graphe » (indexation incrémentale) ; complète l'ADR 0013 (« Fichiers indexés ») et
  l'ADR 0015 (« Détection ») ; ADR 0019 (racine du démon) ; relectures n° 15 (I15-1),
  n° 16 (m16-3, m16-4), n° 17 (m17-1, m17-2, m17-7) et n° 18 (m18-1, m18-2) ; tâche
  PLAN.md 4.12

## Contexte

L'ADR 0013 indexe ce que le parcours accepte, `.gitignore` respecté (crate `ignore`). Le
parcours n'était refait qu'au démarrage : un fichier déjà indexé que `.gitignore` se mettait à
sauter restait dans la tête et dans les snapshots de commit (relecture n° 15, I15-1). Deux cas
courants échappaient encore au suivi (relecture n° 16) :
- la racine est un sous-dossier du dépôt, comme un dossier de l'espace de travail dans un
  monorepo (ADR 0019) : `.git/info/exclude` et les `.gitignore` des dossiers au-dessus d'elle ne
  sont pas sous la racine ;
- la racine est un worktree, comme celui d'un agent (spec) : `info/exclude` est celui du dépôt
  principal, et la crate `ignore` ne le trouve que si on lui demande les règles de git
  (`require_git(true)`, ignore 0.4.33).

## Décision

- **Les règles de git, comme git les applique** : dans un dépôt git (un `.git`, dossier ou
  fichier, dans la racine ou au-dessus), le parcours demande les règles de git
  (`require_git(true)`). Aucun `.gitignore` au-dessus du haut du dépôt ne compte, et
  `info/exclude` vient du dossier git commun d'un worktree (créé en chemins absolus, le défaut
  de git : voir « Conséquences »). Hors dépôt git, `.gitignore` compte toujours, comme avant
  (`require_git(false)`).
- **Un fichier d'exclusion qui change fait refaire le parcours** : ce que le parcours n'accepte
  plus sort de l'index, comme un fichier supprimé (dépendants, résolution, portée, cache) ; ce
  qu'il accepte désormais entre, comme un fichier créé. Sont des fichiers d'exclusion :
  - `.gitignore` et `.ignore`, à la racine ou dans un de ses dossiers : l'indexeur les reconnaît
    à leur nom ;
  - `info/exclude` du dossier git commun, et les `.gitignore` et `.ignore` des dossiers entre le
    haut du dépôt et la racine : c'est la watch qui les nomme, car ils sont hors de la racine.
- **Ce que la watch surveille en plus de la racine** (`git rev-parse` avec
  `--absolute-git-dir`, `--git-common-dir` et `--show-toplevel`) : le dossier git et son
  `logs/` (ADR 0015), `info/` du dossier git commun, et chaque dossier entre la racine et le
  haut du dépôt, sans récursion. De ces dossiers, seuls les fichiers d'exclusion comptent.
- **Avant le gel d'un commit**, le parcours est refait aussi (ADR 0022) : un fichier
  d'exclusion changé sans que la watch l'ait encore dit est pris en compte.

## Alternatives écartées

- **Garder `require_git(false)` et lire nous-mêmes `info/exclude` du dossier commun** : on
  referait à côté de la crate ce qu'elle sait faire, et les `.gitignore` au-dessus du haut du
  dépôt compteraient encore, ce que git ne fait pas.
- **`require_git(true)` partout** : hors dépôt git, `.gitignore` ne compterait plus, ce qui
  changerait l'ADR 0013 sans raison.
- **Surveiller récursivement le haut du dépôt** : dans un monorepo, ce serait surveiller tout
  le dépôt pour quelques fichiers.

## Conséquences

- La tête et les snapshots de commit suivent sans redémarrage les fichiers d'exclusion de la
  racine et de ses dossiers, ceux des dossiers entre le haut du dépôt et la racine, et
  `info/exclude`, dans un worktree comme dans un sous-dossier.
- Limites de la crate `ignore` 0.4.33 (relectures n° 17, m17-1, et n° 18, m18-1) :
  - elle lit la ligne `gitdir:` d'un worktree par rapport au dossier courant du processus.
    Dans un worktree créé en chemins relatifs (git 2.48 et plus,
    `worktree.useRelativePaths`), `info/exclude` n'est donc appliqué, à la tête comme aux
    snapshots de commit, que si le démon a pour dossier courant le haut du worktree : la même
    racine donne une autre tête selon d'où le démon est lancé. La CLI et l'extension ne
    fixent pas ce dossier ;
  - elle ne cherche `info/exclude` que par le fichier `commondir`, qu'un sous-module n'a pas :
    dans un sous-module pris comme racine, `info/exclude` n'est jamais appliqué.

  Les worktrees que Timonier créera pour les agents devront l'être en chemins absolus, ou le
  démon prendre pour dossier courant le haut du worktree.
- Appliqués par le parcours mais pas suivis (relectures n° 17, m17-2, et n° 18, m18-2) : ils
  comptent au parcours suivant, celui d'un fichier d'exclusion suivi, d'un commit ou d'un
  redémarrage, mais leur propre changement n'en déclenche aucun. Ce sont :
  - dans un dépôt git, le fichier d'exclusion global de git (`core.excludesFile`) et un
    `.ignore` au-dessus du haut du dépôt ;
  - hors dépôt git, un `.gitignore` ou un `.ignore` au-dessus de la racine (pas de commit
    alors : seul un fichier d'exclusion de la racine, ou un redémarrage, refait le parcours).

  Un dossier `info/` absent au démarrage n'est surveillé qu'au redémarrage.
- Refaire le parcours coûte un parcours complet : 18 ms pour 10 000 fichiers, 80 ms pour 40 000
  (relecture n° 15). Retirer ou ajouter des fichiers coûte ce que coûte un fichier supprimé ou
  créé : environ 2 s à 10 000 fichiers, un coût quadratique antérieur à cette décision, noté
  pour la Phase 8 (relecture n° 16, m16-8).
- Le démon lit l'emplacement du dossier git au démarrage : un dépôt créé ensuite autour de la
  racine (`git init`) n'est suivi qu'au redémarrage.
- Hypothèse à vérifier sous macOS (H8), non mesurée (relecture n° 17, m17-7) : notify 8.2
  surveille par FSEvents, récursif, et filtre dans le processus le mode non récursif. Surveiller
  le haut d'un monorepo y ferait donc passer par ce filtre les événements de tout le dépôt.
