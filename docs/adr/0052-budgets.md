# 0052 — Les budgets de performance vérifiés sur les runs

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Budgets de performance », « Performances » (budgets : médiane sur plusieurs
  runs, en rouge sur la carte et dans le journal, `get_budget_violations`), « Interface MCP »
  (`get_budgets`, `get_perf(component)`) ; Q74, Q75 ; ADR 0038 (notes budget), 0041 (ancrage),
  0043, 0051 ; tâche PLAN.md 8.8

## Contexte

Une note budget du tableau blanc fixe des limites (p50, p95, mémoire max, allocations, appels)
sur un composant, une couche, une flèche, ou rien. Il faut dire ce que chaque limite mesure,
sur quels runs, et où le dépassement se voit. Tes réponses : un dépassement se voit toujours
sur la carte (Q74) ; la latence porte sur chaque appel entrant (Q75).

## Décision

**Ce que vise un budget** (`timonier-core::budgets`, pur) : attaché à un composant dessiné, la
boîte réelle qu'il désigne (l'ancrage de l'ADR 0041) ; à une couche, celles de ses composants ;
à une flèche, les appels de la source vers la cible ; à rien, les tests. Un composant qui ne
désigne rien rend le budget « non ancré ».

**Ce qu'un run mesure** (Q75) : chaque span des boîtes visées dont le parent est hors d'elles
est un appel entrant (pour une flèche : un span de la cible dont le parent est dans la source).
p50 et p95 portent sur la durée de ces appels (rang le plus proche), « appels » les compte, «
mémoire max » est le pic du processus pendant eux d'après ses échantillons. Sans attache : la
durée des tests, leur nombre, le pic du run. Rien ne compte encore les allocations : la limite
reste « non mesurée ».

**Sur quels runs** : pour chaque budget, les 5 derniers runs finis avec traces qui ont atteint
ce qu'il vise (un appel entrant mesuré, ou des tests pour un budget sans attache) ; les tests de
fumée de `doctor` n'en sont jamais (marqués `smoke`, migration `perf/0003`). Un run d'un seul
test ou d'un autre runner ne change donc rien aux budgets qu'il n'atteint pas (relecture n° 1,
m5 ; n° 2, I-1) ; la médiane de leurs chiffres est
comparée à chaque limite ; dépassée si elle est au-dessus. États : `ok`, `exceeded`,
`unmeasured` (aucun run n'a rien mesuré), `unanchored`.

**Où ça se voit** :
- `perf/budgets` du démon, et sur la carte, **toujours** (Q74) : la boîte qui montre une boîte
  visée porte le badge « Budget dépassé » (forme et mot de la violation) dans sa barre de titre,
  les chiffres au survol, dans son nom pour les lecteurs d'écran et dans le détail ; la légende
  l'explique. L'extension redemande les budgets quand la carte s'ouvre, quand un run avec traces
  finit, et quand `intent.json` change. Rien ne s'affiche tant que tout tient ;
- MCP : `get_budgets` (tous, vérifiés), `get_budget_violations` (les dépassés, en mots dans le
  résumé), `get_perf { component, run? }` (un composant trouvé par son ID, par le composant
  dessiné qui le désigne, ou par son nom ; ses appels entrants, leur p50 et p95, la mémoire, le
  temps passé dedans, ce qui dedans a pris le plus, les tests qui l'ont atteint, ses budgets) ;
- le journal : à `session_end`, les budgets dépassés à ce moment sont gardés dans la session
  (`budgets`, format de session v1, champ facultatif) et montrés dans sa partie « Tests », en
  rouge avec l'icône ; le bloc géré d'`AGENTS.md` dit de vérifier `get_budget_violations`.

## Alternatives écartées

- **La marque avec le calque seulement** : écartée par Q74 ; une violation n'attend pas qu'on
  allume un calque.
- **p95 sur le temps passé par test dans le composant** : écarté par Q75.
- **Un seul run** : la spec veut la médiane de plusieurs, contre le bruit.
- **Compter les allocations par un compteur maison** : aucune convention ne le demande encore ;
  la limite le dit plutôt que d'inventer un chiffre.

## Conséquences

- Un composant non instrumenté laisse ses budgets « non mesurés », jamais « ok ».
- La médiane se stabilise avec les runs : un seul run lent ne fait pas un dépassement tant que
  d'autres tiennent.
- `perf/budgets` lit les spans des 50 derniers runs pour trouver, par budget, les 5 qui
  l'atteignent : quelques centaines de spans par run sur Timonier, mais jusqu'à 10 millions au
  plafond de l'ADR 0043 (hypothèse, coût non mesuré ; relecture n° 3, m-2). À revoir si une
  mesure le montre lent.
