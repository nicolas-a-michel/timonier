# Décisions d'architecture (ADR)

Chaque décision technique non couverte par [la spécification](../spec.md) est consignée ici.
La procédure et le modèle sont dans le skill [`adr`](../../.claude/skills/adr/SKILL.md).

| N°   | Titre                                                                  | Statut  | Phase |
| ---- | ---------------------------------------------------------------------- | ------- | ----- |
| 0001 | [Hooks Claude Code écrits en Node.js, sans dépendance](0001-hooks-claude-code-en-node.md) | Accepté | 0 |
| 0002 | [Format de PLAN.md, lisible par l'humain et par les hooks](0002-format-de-plan-md.md)       | Accepté ; remplacé pour les tâches par 0035 | 0 |
| 0003 | [Découpage des crates, sens des dépendances et alias `tim`](0003-decoupage-des-crates.md) | Accepté | 1 |
| 0004 | [Toolchain Rust : canal stable, MSRV = plus ancienne version testée](0004-toolchain-rust.md) | Accepté | 1 |
| 0005 | [Monorepo Node, build de l'extension et chargement de la webview](0005-monorepo-node-et-build-de-l-extension.md) | Accepté | 1 |
| 0006 | [Langue de l'interface : anglais de base, traduction française](0006-langue-de-l-interface.md) | Accepté | 1 |
| 0007 | [Point d'entrée Timonier dans la barre d'activité](0007-entree-dans-la-barre-d-activite.md) | Accepté | 1 |
| 0008 | [Hiérarchie et identité des nœuds](0008-hierarchie-et-identite-des-noeuds.md) | Accepté | 2 |
| 0009 | [Portées, observations et fusion des sources](0009-portees-et-fusion-des-sources.md) | Accepté | 2 |
| 0010 | [Versionnement : tête de travail ouverte, snapshots figés, diff et déplacements](0010-versionnement-snapshots-et-diff.md) | Accepté | 2 |
| 0011 | [SQLite : moteur embarqué, migrations, écrivain unique](0011-sqlite-migrations-et-ecrivain-unique.md) | Accepté | 2 |
| 0012 | [Format des packs de langage](0012-format-des-packs-de-langage.md) | Accepté | 3 |
| 0013 | [Extraction tree-sitter, résolution approchée et cache](0013-extraction-resolution-et-cache.md) | Accepté | 3 |
| 0014 | [Vitesse 2 : affinage LSP en arrière-plan](0014-affinage-lsp-en-arriere-plan.md) | Accepté | 3 |
| 0015 | [Vitesse 3 : consolidation au commit](0015-consolidation-au-commit.md) | Accepté | 3 |
| 0016 | [Composants : structure, clustering stable, identité](0016-composants-et-clustering.md) | Accepté | 3 |
| 0017 | [Rôles architecturaux et dépendances externes](0017-roles-architecturaux.md) | Accepté | 3 |
| 0018 | [Compaction des arêtes de symboles](0018-compaction-des-aretes-de-symboles.md) | Accepté | 3 |
| 0019 | [Protocole entre le démon et l'extension](0019-protocole-demon-extension.md) | Accepté | 4 |
| 0020 | [Système de design de la webview](0020-systeme-de-design.md) | Accepté | 4 |
| 0021 | [Carte d'architecture : rôles, zoom sémantique, mise en page](0021-carte-d-architecture.md) (complétée par la 0042) | Accepté | 4 |
| 0022 | [Vérifier les usages et les dépendants par le serveur de langage](0022-verification-des-usages-par-le-lsp.md) | Accepté | 4 |
| 0023 | [Suivre les fichiers d'exclusion](0023-fichiers-d-exclusion-suivis.md) | Accepté | 4 |
| 0024 | [Diff de revue](0024-diff-de-revue.md) | Accepté | 5 |
| 0025 | [Snapshot de commit : les paquets du commit](0025-paquets-du-commit.md) | Accepté | 5 |
| 0026 | [Densité de la carte](0026-densite-de-la-carte.md) | Accepté | 5 |
| 0027 | [Serveur MCP : rmcp sur stdio, mince proxy vers le démon](0027-serveur-mcp.md) | Accepté | 6 |
| 0028 | [En-tête de boîte aux lettres](0028-en-tete-boite-aux-lettres.md) | Accepté | 6 |
| 0029 | [Formats du suivi : plan, todo et sessions](0029-formats-du-suivi.md) | Accepté | 6 |
| 0030 | [Dette de revue](0030-dette-de-revue.md) | Accepté | 6 |
| 0031 | [Vue du suivi : plan, todo et journal dans l'IDE](0031-vue-du-suivi.md) | Accepté | 6 |
| 0032 | [Le pont : la vue d'accueil](0032-le-pont.md) | Accepté | 6 |
| 0033 | [`timonier init` : détection, blocs délimités, hooks et `actual.json`](0033-timonier-init.md) | Accepté | 6 |
| 0034 | [Packs de langage, volet init : runners de tests, JUnit XML et conventions](0034-packs-volet-init.md) | Accepté | 6 |
| 0035 | [Les tâches dans `plan.json` : la bascule](0035-taches-dans-plan-json.md) | Accepté | 6 |
| 0036 | [Le tableau blanc : React Flow et perfect-freehand, à la place de tldraw](0036-tableau-blanc-react-flow.md) | Accepté | 7 |
| 0037 | [Le panneau du tableau blanc : une troisième page, ses commandes, son état](0037-panneau-du-tableau-blanc.md) | Accepté | 7 |
| 0038 | [Les formes typées du tableau blanc](0038-formes-typees.md) | Accepté | 7 |
| 0039 | [`.timonier/intent.json` : le format du tableau blanc et son enregistrement](0039-intent-json.md) (complète la 0019 : `daemon/shutdown` sans poignée de main) | Accepté | 7 |
| 0040 | [Le dessin libre pour l'agent : une image et des emplacements](0040-croquis-pour-l-agent.md) | Accepté | 7 |
| 0041 | [Les outils `get_intended_architecture` et `get_drift`, et l'ancrage](0041-outils-intention-et-derive.md) | Accepté | 7 |
| 0042 | [La carte comparée à l'intention](0042-carte-comparee-a-l-intention.md) (complète la 0021) | Accepté | 7 |
| 0043 | [Où vivent les mesures de performance](0043-stockage-des-mesures.md) | Accepté | 8 |
| 0044 | [L'instrumentation de Timonier : la feature `otel` et `code.function.name`](0044-feature-otel.md) | Accepté | 8 |
| 0045 | [Niveau 1 : mesurer un run de tests sous Linux et macOS](0045-niveau-1.md) | Accepté | 8 |
| 0046 | [Les compteurs d'instructions CPU](0046-compteurs-materiels.md) | Accepté | 8 |
| 0047 | [Le run se signale aux tests, et chaque test devient un span racine](0047-test-span-racine.md) | Accepté | 8 |
| 0048 | [Le récepteur OTLP : ports, protocoles, ce qu'il garde, et le rattachement au graphe](0048-recepteur-otlp.md) | Accepté | 8 |
| 0049 | [Les trous d'instrumentation et le surcoût](0049-trous-d-instrumentation.md) | Accepté | 8 |
| 0050 | [Le volet instrumentation de `timonier init`, et le test de fumée de `doctor`](0050-init-instrumentation.md) | Accepté | 8 |
| 0051 | [Les vues de performances : le calque de la carte, et le panneau](0051-vues-de-performances.md) | Accepté | 8 |
| 0052 | [Les budgets de performance vérifiés sur les runs](0052-budgets.md) | Accepté | 8 |
| 0053 | [La détection des régressions de performances](0053-regressions.md) | Accepté | 8 |
| 0054 | [Ce que la carte dit d'un symbole : sa sorte et sa signature](0054-symboles-de-la-carte.md) | Accepté | 8 |
| 0055 | [L'inventaire des technologies](0055-inventaire-des-technologies.md) | Proposé | 9 |
| 0056 | [Le processus de développement : `workflow.json`, GitHub par `gh`, la CI](0056-processus-de-developpement.md) | Proposé | 9 |
| 0057 | [Ce qu'un dépôt cloné et un processus local peuvent faire faire à Timonier](0057-securite-locale.md) | Proposé | 9 |
| 0058 | [Le coût de l'IA : la télémétrie de Claude Code, reçue sur un port de la machine](0058-cout-de-l-ia.md) | Proposé | 9 |
| 0059 | [Le programme `timonier` livré dans l'extension, et dans le PATH](0059-binaire-embarque.md) | Proposé | 9 |
| 0060 | [Timonier sous Windows](0060-windows.md) | Proposé | 9 |
| 0061 | [Le pack JavaScript, et ce que tous les packs ne parcourent pas](0061-pack-javascript.md) | Proposé | 10 |
