# 0002 — Format de PLAN.md, lisible par l'humain et par les hooks

- Statut : Proposé
- Date : 2026-10-14
- Phase : 0
- Liens : ADR 0001 ; règles de travail 1 et 2 du prompt de démarrage

## Contexte

`PLAN.md` est lu par trois lecteurs :

- l'humain, qui le valide ;
- le hook `SessionStart`, qui affiche la phase en cours et ses tâches restantes ;
- le subagent `phase-reviewer`, qui vérifie les critères d'acceptation.

Il faut donc un format agréable à lire et sans ambiguïté pour un parseur.

## Décision

**Phases**

- Une section par phase, titrée `## Phase N — Titre`. Le parseur accepte aussi un tiret
  demi-cadratin ou un trait d'union.
- La première ligne `Statut : …` de la section donne l'état : `à faire`, `en cours` ou
  `terminée`.
- Une seule phase est `en cours` à la fois. Sans phase `en cours`, le hook prend la première
  phase non terminée.

**Tâches**

- Seules les cases à cocher **de premier niveau** (`- [ ] N.k Titre — critère : …`) sont des
  tâches. Les détails vont en lignes indentées, sans case.
- Chaque tâche porte son critère d'acceptation vérifiable dans la même ligne, après
  « critère : ».

**Fin de section** : chaque phase se termine par deux lignes, `Décisions :` (liens vers les
ADR) et `Écarts :` (ce qui a différé du plan, et pourquoi).

**Autres sections** : celles qui ne sont pas des phases (« Questions ouvertes »,
« Hypothèses », « Journal ») sont ignorées par le parseur.

## Alternatives écartées

- **Plan en YAML ou en JSON** : moins lisible pour l'humain. Le plan structuré du produit
  arrive de toute façon en Phase 6 (`.timonier/plan.json`, écrit uniquement via MCP).
- **GitHub Issues** : demande un dépôt distant et une décision de licence, qui n'existent pas
  encore. Prévu en Phase 9 avec le processus de développement.

## Conséquences

- Toute réorganisation de `PLAN.md` doit garder ces trois motifs : titre de phase, `Statut :`
  et cases de premier niveau. Sinon le hook ne trouve plus la phase.
- En Phase 6, il faudra décider de la relation entre `PLAN.md` et `.timonier/plan.json`
  (question ouverte Q11 dans `PLAN.md`).
