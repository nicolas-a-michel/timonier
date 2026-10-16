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
