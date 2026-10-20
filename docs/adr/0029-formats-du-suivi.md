# 0029 — Formats du suivi : plan, todo et sessions

- Statut : Accepté (2026-10-20, validation de la Phase 6 par l'humain)
- Date : 2026-10-20
- Phase : 6
- Liens : spec « Pilotage », « Stockage » (`.timonier/`) ; Q10, Q11 ; H7 ; ADR 0010 (snapshots),
  0027 (serveur MCP) ; skill `mcp-tools` ; tâches PLAN.md 6.1, 6.3, 6.4

## Contexte

La spec place le plan, la todo et le journal des sessions dans `.timonier/` (`plan.json`,
`todo.json`, `sessions/`), versionnés dans le dépôt, et veut que l'agent ne les modifie jamais à
la main : il passe par des outils qui valident chaque écriture. Elle ne fixe ni leur schéma, ni
les états d'une tâche, ni ce qu'une session retient du code.

## Décision

**Fichiers** : JSON, chacun avec `"version": 1` en tête. Leurs schémas JSON sont tirés des types
Rust de `timonier-core` (`schemars`) et écrits dans `docs/schemas/` ; un test échoue s'ils ne sont
plus à jour, comme pour les types TypeScript.

**Seul écrivain : le démon.** Il écrit un fichier entier à la fois (fichier temporaire puis
renommage), trié et indenté pour des diffs git lisibles. Il relit un fichier changé hors de lui
(une modification à la main, un `git checkout`) ; s'il est invalide, il garde le dernier état
valide, refuse les écritures sur ce fichier et le dit dans `daemon/status` et dans l'erreur des
outils.

**`plan.json`** : des jalons (`M-1`, `M-2`…), chacun avec des tâches (`T-1`, `T-2`…). Les
identifiants sont donnés par le démon, jamais réutilisés.
- Une tâche : titre, critère, état, composants du tableau blanc (`C-…`, vide jusqu'à la Phase 7,
  H7), sessions qui l'ont touchée, notes datées et attribuées à une session.
- États : `todo`, `in_progress`, `blocked`, `done`, `dropped`. Transitions permises :
  `todo → in_progress | dropped` ; `in_progress → blocked | done | dropped` ;
  `blocked → in_progress | dropped` ; `done → in_progress` (rouverte, avec une note). Toute
  autre est refusée. L'avancement d'un jalon est calculé (tâches `done` sur tâches non
  `dropped`), jamais écrit.

**`todo.json`** : les éléments (`D-1`…) de la session en cours : texte, état `open` ou `done`,
tâche du plan éventuelle. À la fin de la session, ses éléments vont dans son journal et la todo
se vide.

**`sessions/<id>.json`** : une session par fichier, identifiant `S-<date>-<n>`
(`S-2026-10-20-1`).
- Objectif, agent (le nom du client MCP), tâches liées, début et fin, état `open` ou `ended`,
  résumé de fin.
- Le démon fige la tête du graphe en snapshot `working_tree` au début et à la fin (ADR 0010) ;
  la session retient leurs numéros. Le diff structurel et les composants touchés sont calculés
  entre les deux, comme le diff de revue (ADR 0024), et écrits à la fin.
- **Une session ouverte à la fois** dans le MVP solo : `session_start` est refusé tant qu'une
  autre est ouverte, avec son identifiant dans le message. Les voies par agent arrivent avec le
  multi-agents (v2).
- Toute écriture du plan ou de la todo exige une session ouverte et lui est attribuée.

**Non suivi (6.4)** : ce qui change sans session ouverte se lit entre deux snapshots, la fin
d'une session et le début de la suivante (ou le premier snapshot, avant la première session).
À chaque `session_start`, le démon calcule les fichiers que ce diff touche (nœuds et arêtes,
ADR 0010) et, s'il y en a, ajoute un écart à `sessions/untracked.json` : les deux snapshots, la
session qui suit, les fichiers. Rien à surveiller en continu, et rien de perdu si le démon
s'arrête entre deux sessions. La dette de revue (6.5) s'en sert. Une modification qui ne change
aucun nœud ni aucune arête (un commentaire) n'y paraît pas.

**Le plan de Timonier (Q11)** : à la bascule (6.13), les phases deviennent des jalons, les phases
faites des jalons terminés, et leurs tâches des tâches ; `PLAN.md` garde le reste.

## Alternatives écartées

- **Le plan dans SQLite** : la base est un cache reconstructible ; ce qui appartient à
  l'utilisateur vit dans `.timonier/*.json` (skill `rust-daemon`).
- **Un état libre par tâche** : le panneau et l'avancement calculé demandent un ensemble fermé.
- **Un nouveau type de snapshot `session`** : la spec fixe les types (`commit`, `review_mark`,
  `working_tree`) ; un `working_tree` figé suffit.

## Conséquences

- Une modification à la main d'un fichier valide est acceptée et relue ; seul l'agent est tenu
  de passer par les outils, par ses instructions et ses hooks (6.8).
- Deux snapshots de plus par session : quelques millisecondes, sans copie (ADR 0010).
