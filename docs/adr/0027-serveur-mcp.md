# 0027 — Serveur MCP : rmcp sur stdio, mince proxy vers le démon

- Statut : Accepté (2026-10-20, validation de la Phase 6 par l'humain)
- Mise à jour de version (2026-10-23, sans changer la décision) : `rmcp` 3.6.0, pour le `resultType`
  que demande la révision 2026-07-28 du protocole ; vérifiée dans PLAN.md, « Dépendances vérifiées »
- Date : 2026-10-20
- Phase : 6
- Liens : spec « Architecture technique », « Interface MCP », « Pilotage » ; Q10 ; H10 ;
  ADR 0003 (graphe des crates), 0019 (protocole JSON-RPC du démon), 0028 (en-tête de boîte aux
  lettres), 0029 (formats du suivi) ; skill `mcp-tools` ; tâches PLAN.md 6.1 à 6.3

## Contexte

La spec veut que l'agent lise et écrive le même modèle que l'IDE, par MCP, et que le serveur MCP
soit un mince proxy vers le démon, sans logique métier ni accès au stockage. Elle ne fixe ni le
SDK, ni le transport, ni la façon dont le proxy connaît la liste des outils.

## Décision

- **SDK** : `rmcp` 3.5.1 (crates.io, SDK officiel `modelcontextprotocol/rust-sdk`, licence
  Apache-2.0, Rust 1.88 au moins ; vérifié le 2026-10-20). La 3.6.0, sortie le jour même, attend
  deux semaines. Fonctions : `server`, `transport-io` ; rien de HTTP. `schemars` 1.2.3 (MIT,
  vérifié le 2026-10-20), que `rmcp` utilise déjà, décrit les entrées.
- **Transport** : stdio. L'agent lance `timonier mcp` (ou `tim mcp`) dans le dossier du projet ;
  le processus parle MCP sur son entrée et sa sortie, et journalise sur la sortie d'erreur.
- **Proxy** : `timonier-mcp` ne connaît que deux méthodes du démon, ajoutées au protocole
  (version 4) :
  - `tools/list` : les outils, leur description et leur schéma d'entrée ;
  - `tools/call { name, arguments }` : le résultat structuré, son résumé d'une ligne et
    l'en-tête de boîte aux lettres (ADR 0028).

  Il traduit `tools/list` et `tools/call` de MCP en ces deux requêtes, et retour. Le catalogue,
  la validation et les écritures sont dans le démon, seule autorité : un client JSON-RPC
  (l'extension, un test) appelle les mêmes outils.
- **Démarrage** : `timonier mcp` (le CLI) trouve le démon du projet (verrou et socket,
  ADR 0019) ; s'il n'y en a pas, il le lance comme `timonier daemon start`, sans rien écrire sur
  la sortie standard, qui porte MCP. Il passe ensuite le socket au proxy. Le proxy s'annonce au
  démon avec le nom du client MCP (`clientInfo`), que le démon attribue aux sessions ouvertes
  par cette connexion. Le nom vient de la poignée de main `initialize`, ou des métadonnées de
  chaque requête dans le cycle de vie MCP 2026-07-28 (SEP-2575), qui permet de la sauter :
  `RequestContext::client_info()` de rmcp lit les deux (relecture n° 1 de la Phase 6, B1). Sans
  nom (il y est facultatif), l'agent est `mcp-client`.
- **Graphe des crates** : `timonier-mcp` dépend de `timonier-core` seul (ADR 0003). Il parle
  JSON-RPC avec le cadrage de `timonier-core` sur une `UnixStream` de tokio ; plusieurs
  requêtes partagent la connexion, chaque réponse va à la sienne.
- **Schémas** : chaque entrée d'outil est un type Rust de `timonier-core` (serde et `schemars`),
  sans champ inconnu (`deny_unknown_fields`, `additionalProperties: false`), avec des chaînes et
  des listes bornées, ses sous-schémas en ligne (pas de `$ref`). Le schéma JSON en est tiré :
  une seule source. Un test fige la liste des outils et leurs schémas
  (`crates/timonier-core/tests/tools.json`, refait par `TIMONIER_UPDATE_TOOLS=1`), et échoue sur
  tout changement non voulu du contrat.
- **Outils de la Phase 6** : ceux du suivi de la spec (`session_start`, `session_end`,
  `plan_add_task`, `plan_update_task`, `todo_add`, `todo_complete`) et, par Q10, `get_plan`,
  `get_todo`, `get_session`. Les autres domaines arrivent avec leur phase.
- **Erreurs** : un résultat d'outil en erreur (`isError`), avec un code stable et un message qui
  dit quoi faire (« aucune session ouverte : appeler session_start »), et l'en-tête. Jamais de
  trace de pile.

## Alternatives écartées

- **Le catalogue dans le proxy** : il faudrait y valider les entrées, donc y mettre de la
  logique, et l'extension ne pourrait pas appeler les mêmes outils.
- **HTTP « streamable »** : un port à ouvrir et à protéger pour un usage local ; les agents
  visés (Claude Code, Codex, Cursor) lancent tous des serveurs stdio.
- **Le client bloquant de `timonier-daemon`** : `timonier-mcp` dépendrait du démon, contre
  l'ADR 0003.

## Conséquences

- Le protocole du démon passe en version 4 ; l'extension s'y adapte.
- Un nouvel outil est un type d'entrée dans `timonier-core`, un traitement dans le démon et une
  ligne de plus à l'instantané : le proxy ne change pas.
