# 0056 — Le processus de développement : `workflow.json`, GitHub par `gh`, la CI

- Statut : Proposé
- Date : 2026-10-21
- Phase : 9
- Liens : spec « Processus de développement » ; Q80 à Q83 ; ADR 0027 à 0029 (suivi), 0033
  (init), 0052 (budgets), 0042 (intention) ; tâche PLAN.md 9.2

## Contexte

La spec décrit trois préréglages (Solo rapide, Solo discipliné, Équipe), des réglages un à un
(tickets, branches, commits, pull requests, revue et fusion, CI), un fichier versionné
`.timonier/workflow.json` dont le bloc géré de l'agent est dérivé, et des outils MCP qui
l'appliquent : `task_start` crée le ticket et la branche, `session_end` ouvre la PR. Q81 fixe
l'accès par la connexion de `gh`, Q82 la fusion par l'auto-merge de GitHub, Q83 le préréglage
par défaut, Solo rapide.

## Décision

**Le fichier** `.timonier/workflow.json` (version 1, schéma dans `docs/schemas/`) : le
préréglage choisi et chaque réglage, que le panneau ou l'outil change un à un.

| Réglage | Solo rapide | Solo discipliné | Équipe |
| --- | --- | --- | --- |
| `tickets` (fournisseur) | `none` | `github` | `github` |
| `branches.per_task`, modèle `tim/{ticket}-{slug}` | non | oui | oui |
| `commits.convention` | `conventional` | `conventional` | `conventional` |
| `commits.reference_ticket` | non | oui | oui |
| `pull_requests.open_at_session_end` / `draft` | non / — | oui / non | oui / non |
| `merge.human_approval` / `merge.auto` / `strategy` | non / non / `squash` | non / oui / `squash` | oui / non / `squash` |
| `ci` | `none` | `github_actions` | `github_actions` |

Un réglage changé à la main fait passer `preset` à `custom`. GitLab, Jira et Linear attendent
leur fournisseur : le format les nomme, les outils les refusent avec ce qui manque.

**GitHub par `gh`** (Q81) : le démon lance `gh` et `git` dans le dépôt, sans jeton à lui ; un
`gh` absent ou non connecté donne une erreur qui dit `gh auth login`. Le programme est réglable
(`Options::gh`), ce qui permet aux tests un faux `gh` qui enregistre ses appels.

**Le ticket d'une tâche** : le champ `ticket` d'une tâche de `plan.json` (`"github#12"`), posé par
`task_start` quand il crée l'issue, ou par `link_ticket` ; l'état suit la tâche (fermé avec
`done` ou `dropped`, par `gh issue close`).

**Les outils** :
- `get_workflow` : le processus, et ce que l'agent doit faire, en mots ;
- `task_start { task_id }` : la tâche passe en cours ; selon le processus, l'issue est créée et
  la branche `tim/<n>-<slug>` est créée depuis la branche principale et prise ;
- `open_pull_request { draft? }` : pousse la branche, ouvre la PR avec la description générée
  (la tâche et son critère, le diff structurel de la session, les tests du dernier run, l'effet
  sur les performances, les budgets dépassés et les arêtes interdites franchies), et active
  l'auto-merge si le processus le dit (Q82) ;
- `link_ticket { task_id, ticket }` ;
- `session_end` ouvre la PR quand `pull_requests.open_at_session_end` et qu'une branche de tâche
  est prise ; sinon rien ne change.

**Le bloc géré** de `AGENTS.md` dit de lire `get_workflow` avant de commencer une tâche et de la
commencer par `task_start` : une ligne fixe. Les consignes dérivées du fichier, elles, viennent
de l'outil au moment où l'agent le lit (`Workflow::instructions`), et `task_start` comme
`session_end` les appliquent : un changement du panneau vaut tout de suite, sans réécrire le bloc
ni attendre un `doctor --fix`.

**La CI** : `timonier init` (et le panneau) écrit `.github/workflows/timonier.yml` quand `ci` vaut
`github_actions` : il installe `timonier`, lance `timonier test`, puis `timonier check`, une
commande nouvelle qui échoue sur un budget dépassé (ADR 0052) ou une arête interdite franchie
(ADR 0042), ce que la spec appelle « rejouer tests, budgets et règles d'architecture ». Avec
l'auto-merge, ce job est le contrôle requis de la protection de branche ; Timonier le propose
(`gh api` sur la protection) et ne le fait qu'à la demande.

**Le panneau** : « Timonier : Processus de développement » ouvre une page de la webview (même
système de conception) : les trois préréglages, puis chaque réglage ; un changement passe par le
démon (`workflow/set`), seul écrivain de `.timonier/`.

## Alternatives écartées

- **L'API GitHub directement** : un jeton à garder, ce que Q81 écarte.
- **Fusionner depuis le démon** : il devrait tourner jusqu'à la fin de la CI (Q82).
- **Les réglages de VSCode** : le processus est celui du dépôt, versionné, pas celui d'un poste.

## Conséquences

- Le scénario réel sur GitHub (Q80) attend ; les tests passent par un faux `gh`.
- `timonier check` sert aussi hors CI, à la main.
- La PR ouverte par `session_end` rend la fin de session plus longue d'un `git push` et d'un
  `gh pr create`.
