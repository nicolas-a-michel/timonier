# Décisions d'architecture (ADR)

Chaque décision technique non couverte par [la spécification](../spec.md) est consignée ici.
La procédure et le modèle sont dans le skill [`adr`](../../.claude/skills/adr/SKILL.md).

| N°   | Titre                                                                  | Statut  | Phase |
| ---- | ---------------------------------------------------------------------- | ------- | ----- |
| 0001 | [Hooks Claude Code écrits en Node.js, sans dépendance](0001-hooks-claude-code-en-node.md) | Accepté | 0 |
| 0002 | [Format de PLAN.md, lisible par l'humain et par les hooks](0002-format-de-plan-md.md)       | Accepté | 0 |
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
| 0021 | [Carte d'architecture : rôles, zoom sémantique, mise en page](0021-carte-d-architecture.md) | Accepté | 4 |
| 0022 | [Vérifier les usages et les dépendants par le serveur de langage](0022-verification-des-usages-par-le-lsp.md) | Accepté | 4 |
| 0023 | [Suivre les fichiers d'exclusion](0023-fichiers-d-exclusion-suivis.md) | Accepté | 4 |
| 0024 | [Diff de revue](0024-diff-de-revue.md) | Accepté | 5 |
| 0025 | [Snapshot de commit : les paquets du commit](0025-paquets-du-commit.md) | Accepté | 5 |
| 0026 | [Densité de la carte](0026-densite-de-la-carte.md) | Accepté | 5 |
| 0027 | [Serveur MCP : rmcp sur stdio, mince proxy vers le démon](0027-serveur-mcp.md) | Proposé | 6 |
| 0028 | [En-tête de boîte aux lettres](0028-en-tete-boite-aux-lettres.md) | Proposé | 6 |
| 0029 | [Formats du suivi : plan, todo et sessions](0029-formats-du-suivi.md) | Proposé | 6 |
| 0030 | [Dette de revue](0030-dette-de-revue.md) | Proposé | 6 |
| 0031 | [Vue du suivi : plan, todo et journal dans l'IDE](0031-vue-du-suivi.md) | Proposé | 6 |
| 0032 | [Le pont : la vue d'accueil](0032-le-pont.md) | Proposé | 6 |
