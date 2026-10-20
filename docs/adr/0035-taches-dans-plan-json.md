# 0035 — Les tâches dans `plan.json` : la bascule

- Statut : Accepté (2026-10-20, validation de la Phase 6 par l'humain)
- Date : 2026-10-20
- Phase : 6
- Liens : Q11 ; ADR 0002 (format de `PLAN.md`), qu'elle remplace pour les tâches ; ADR 0029
  (formats du suivi) ; tâche PLAN.md 6.13

## Contexte

L'ADR 0002 fait des cases à cocher de premier niveau de `PLAN.md` les tâches du projet, lues par
le hook de début de session et par le relecteur de phase. Ta réponse Q11 fait de
`.timonier/plan.json` la seule source des jalons et des tâches, tenue par les outils MCP et
montrée dans le panneau ; `PLAN.md` garde ce qui est pour toi. Et tu as choisi (2026-10-20) de
retirer les cases en gardant le détail de chaque tâche.

## Décision

- **Reprise** : chaque phase de `PLAN.md` est devenue un jalon de `plan.json` (« Phase N —
  Titre »), chaque tâche une tâche (son titre, son critère), dans son état : faite, en cours
  (6.13), à faire. Par les outils MCP, dans une session de l'agent `timonier-migration`.
- **`PLAN.md`** garde ses sections de phase, leur `Statut :`, `Décisions :` et `Écarts :`, et
  chaque tâche avec son détail (fait, tests, écarts, tests des cinq secondes), mais en puce sans
  case : `- N.k Titre — critère : …`. Une ligne en tête le dit. Une tâche nouvelle s'ajoute aux
  deux : à `plan.json` par `plan_add_task`, à `PLAN.md` pour son détail.
- **L'état d'une tâche** ne s'écrit plus que par `plan_update_task`.
- **Le hook de début de session** lit `plan.json` : le jalon où est le travail, et ses tâches
  restantes avec leur identifiant. Le jalon où est le travail (relectures n° 2 et 3 de la
  Phase 6) : celui d'une tâche en cours ; sinon le dernier qui a une tâche faite, s'il n'est pas
  fini ; sinon le premier non fini qui le suit ; sinon le premier non fini. Un jalon est fini
  quand ses tâches sont toutes faites ou abandonnées. Une tâche restée à faire dans une phase
  close ne le ramène donc pas en arrière. Le pont applique la même règle
  (`webview/src/tracking/Bridge.tsx`) ; elle est écrite deux fois, côté client, chacune testée.
  Quand tous les jalons sont finis, le pont montre le dernier, et le hook dit que tout est fait.
  Sans `plan.json`, le hook lit encore `PLAN.md` comme le dit l'ADR 0002 ; si `plan.json` est là
  sans se lire, il le dit : `PLAN.md` ne tient plus l'état des tâches.
- **Le relecteur de phase** lit l'état des tâches dans `plan.json`, et leurs critères et leur
  détail dans `PLAN.md`.

## Alternatives écartées

- **Garder les cases, gelées** : deux états qui divergent dès la tâche suivante.
- **Retirer les tâches de `PLAN.md`** : leur détail (tests, écarts) est ce que tu relis.

## Conséquences

- L'ADR 0002 reste vraie pour les phases, les statuts et la fin de section ; pour les tâches,
  celle-ci la remplace.
- Les sessions Timonier de l'agent, tâche par tâche, deviennent le journal du développement de
  Timonier (critère de 6.13).
