# 0033 — `timonier init` : détection, blocs délimités, hooks et `actual.json`

- Statut : Accepté (2026-10-20, validation de la Phase 6 par l'humain)
- Date : 2026-10-20
- Phase : 6
- Liens : spec « Initialisation et configuration de l'IA » ; Q48 à Q51 ; ADR 0003 (graphe des
  crates), 0027 (serveur MCP), 0029 (formats du suivi) ; tâches PLAN.md 6.8 à 6.10

## Contexte

La spec veut un `timonier init` qui détecte le projet et les agents, écrit un bloc géré dans
leurs instructions, déclare le serveur MCP dans leur format, pose des hooks quand ils existent,
un hook git qui régénère `actual.json`, et un plan avec un jalon « Initialisation » et une
première session. `--dry-run` montre le diff, l'opération est idempotente et `uninit` la défait.
Tes réponses fixent les agents (Q48 : Claude Code, Copilot, Cursor, Codex), le hook d'arrêt
(Q49 : bloquer une fois), le moment d'`actual.json` (Q50 : pre-commit) et les questions (Q51 : au
terminal, avec des options). Les formats des agents ont été lus dans leurs documentations
officielles le 2026-10-20 (liste plus bas).

## Décision

**Où** : dans `timonier-cli` (module `init`), qui orchestre les entrées-sorties (ADR 0003). Le
calcul des modifications est pur : à partir du contenu actuel des fichiers et de la détection,
il rend une liste de fichiers avec leur contenu avant et après. `--dry-run` en affiche le diff
unifié (`similar` 2.7.0, Apache-2.0, déjà dans le dépôt par `insta`) ; sinon elles sont écrites.
Une deuxième exécution ne trouve rien à changer.

**Détection** : langages et gestionnaires de paquets par les manifestes (`Cargo.toml`,
`package.json` et son fichier de verrou), runner de tests (`cargo test`, le script `test` de
`package.json` ; le détail est en 6.11), état git (dépôt, `core.hooksPath`). Agents, sur la
machine ou dans le projet :
- Claude Code : `claude` dans le `PATH`, `~/.claude/`, l'extension `anthropic.claude-code`, ou
  `CLAUDE.md` / `.claude/` dans le projet ;
- Copilot : VSCode présent (`code` dans le `PATH`, `~/.vscode/`), ou `.vscode/` dans le projet ;
- Cursor : `~/.cursor/` ou `.cursor/` dans le projet ;
- Codex : `codex` dans le `PATH`, `$CODEX_HOME` ou `~/.codex/`, ou `.codex/` dans le projet.

Ce que la détection ne tranche pas est demandé au terminal (Q51) ; `--agents`, `--yes` et
`--dry-run` le disent sans question.

**Ce qui est écrit**, toujours délimité, pour que `uninit` retire exactement sa part :
- **Instructions** : un bloc entre `<!-- timonier:begin -->` et `<!-- timonier:end -->` dans
  `AGENTS.md`, lu nativement par les quatre agents ; si `CLAUDE.md` existe (Claude Code ne lit
  alors plus `AGENTS.md`), un bloc qui l'importe (`@AGENTS.md`). Le bloc dit d'ouvrir une
  session, de découper en todo, de passer par les outils, de ne jamais écrire `.timonier/`.
- **MCP**, une entrée `timonier` (`timonier mcp`) : `.mcp.json` (`mcpServers`, Claude Code),
  `.vscode/mcp.json` (`servers`, `type: "stdio"`, racine `${workspaceFolder}`, Copilot),
  `.cursor/mcp.json` (`mcpServers`, idem, Cursor), `.codex/config.toml` (bloc
  `[mcp_servers.timonier]` entre marqueurs de commentaire, Codex). Un JSON existant est relu
  et réécrit avec l'entrée ajoutée ; un JSON qui ne se lit pas (commentaires) est laissé, et
  `init` dit quoi y ajouter.
- **Hook d'arrêt** (Q49) : `timonier hook stop` dans `.claude/settings.json` (`Stop`),
  `.github/hooks/timonier.json` (Copilot, `Stop`) et `.codex/hooks.json` (`Stop`). Le hook lit
  l'entrée de l'agent ; si l'arrêt a déjà été bloqué une fois (`stop_hook_active`), il laisse
  faire ; sinon il demande au démon (`agent/stop`) une raison de bloquer : session ouverte dont
  la todo est vide ou a des éléments non cochés, ou du code changé depuis la fin de la
  dernière session sans session ouverte. Il répond dans le format de l'agent (`decision` en
  tête pour Claude Code et Codex, dans `hookSpecificOutput` pour Copilot). Cursor ne sait pas
  bloquer un arrêt : pas de hook pour lui.
- **Git** (Q50) : un bloc entre marqueurs dans le hook `pre-commit` du dossier que git donne
  (`git rev-parse --git-path hooks`), créé s'il n'existe pas ; il lance
  `timonier hook pre-commit`, qui fait écrire `actual.json` par le démon (`actual/write`) puis
  l'ajoute à l'index, et n'échoue jamais le commit. Un gestionnaire de hooks (`core.hooksPath`
  posé : husky, lefthook…) n'est pas modifié : `init` dit quoi y ajouter.
- **`.timonier/`** : le `.gitignore` de la base SQLite, comme le démon le pose déjà.
- **Ce qui a été fait** : `.timonier/init.json` liste les fichiers créés et modifiés, pour
  `uninit` (6.9) et `doctor` (6.10).

**`actual.json`** : version 1 ; les boîtes (paquets et composants : id, sorte, nom, rôle, boîte
parente) et les dépendances entre elles (agrégées comme sur la carte : de, vers, sortes, poids),
triées, sans date ni commit : deux écritures du même graphe donnent le même fichier.

**Le suivi** : le démon reste le seul écrivain de `plan.json`, `todo.json` et `sessions/`
(ADR 0029). `init` le trouve ou le lance, puis appelle les outils comme un agent nommé
`timonier-init` : une session « Initialise Timonier », la tâche « Configure Timonier » dans un
jalon « Initialisation », faite, puis la fin de session (en anglais, comme l'interface de base ;
le mot « Initialisation » est le même dans les deux langues). Seulement s'il n'y a encore ni
plan ni session. `--dry-run` liste ces appels sans diff : leur contenu dépend de l'heure et des
snapshots.

**`uninit`** (Q54) retire la configuration : les blocs, les entrées, les hooks, les conventions,
`tests.json`, `init.json`. Le suivi du projet (`plan.json`, `todo.json`, `sessions/`,
`intent.json`) reste dès qu'il y a eu du travail après `init` ; il ne part que s'il ne tient que
ce qu'`init` y a écrit (sa session, son jalon), ou avec `--purge`, confirmé au terminal (ou
`--yes`). Le suivi d'avant `init` (un plan, une session déjà là) se juge à la première exécution
(`tracked_before` dans `init.json`), pas à l'existence du dossier, que le démon crée dès son
démarrage. `actual.json`, écrit par le hook git d'`init`, part avec la configuration.

**Le tableau blanc vide** de la spec n'est pas créé avant que son format existe : ta réponse
Q57 ; la Phase 7 l'ajoutera à `init`.

**Documentations lues** (2026-10-20) : code.claude.com/docs/en/{memory,mcp,hooks} ;
code.visualstudio.com/docs/copilot/customization/{custom-instructions,mcp-servers,hooks} et
docs/agents/reference/hooks-reference ; cursor.com/docs/context/{rules,mcp},
cursor.com/docs/agent/hooks ; learn.chatgpt.com/docs/{agent-configuration/agents-md,extend/mcp,
config-file/config-basic,hooks}.

## Alternatives écartées

- **Un fichier d'instructions par agent** (`.cursor/rules/*.mdc`,
  `.github/copilot-instructions.md`) : les quatre lisent `AGENTS.md` ; un seul bloc à tenir.
- **Écrire `plan.json` et la session directement** : deux écrivains, et pas de snapshots pour
  la session.
- **Un hook post-commit** : exact, mais laisse un fichier modifié à chaque commit (Q50).
- **Gemini CLI** : hors MVP (Q48).

## Conséquences

- Les hooks et le serveur MCP supposent `timonier` dans le `PATH` : `doctor` le vérifie (6.10).
- Les agents demandent souvent d'approuver un serveur MCP ou un hook de projet (Claude Code,
  Codex) : `init` le dit à la fin.
- Protocole version 7 : `agent/stop` et `actual/write`.
