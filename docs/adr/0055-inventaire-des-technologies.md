# 0055 — L'inventaire des technologies

- Statut : Accepté (2026-10-22, Phase 9 validée)
- Date : 2026-10-21
- Phase : 9
- Liens : spec « Inventaire des technologies et santé des composants », « Surface de sécurité »
  (CVE depuis l'inventaire) ; Q79 ; ADR 0003, 0013 ; tâche PLAN.md 9.1

## Contexte

La spec veut un SBOM CycloneDX construit depuis les manifestes (langages, frameworks,
dépendances avec versions, licences, CVE connues), qui signale d'abord les doublons
fonctionnels et les dépendances ajoutées depuis la dernière revue. Les manifestes donnent des
plages de versions, pas les versions résolues ni les licences ; les catégories d'usage des packs
(réseau, sérialisation…) sont trop larges pour dire qu'une dépendance en double une autre. Q79
décide OSV.dev, à la demande.

## Décision

**Sources, par écosystème.** Rust : `cargo metadata --format-version 1 --offline` (versions
résolues, licences, dépendances directes et leur sorte : normale, développement, build) ; sans
cargo ou hors cache, `Cargo.lock` seul, licences « inconnues ». npm : `package-lock.json`
(versions 2 et 3 : version, licence, `dev`), les dépendances directes lues dans les
`package.json` du projet et de ses workspaces. Les autres écosystèmes de la spec (`pyproject.toml`,
`go.mod`) attendent leurs packs.

**Langages et frameworks.** Les langages viennent du graphe (nombre de fichiers par langage) ;
un framework est une dépendance directe que la nouvelle table `[inventory]` de son pack nomme
dans `frameworks`.

**Doublons fonctionnels.** La table `[inventory.functions]` du pack associe une fonction précise
(`http-client`, `async-runtime`, `cli-parser`, `logging`, `date-time`, `test-runner`…) à ses
paquets. Deux dépendances directes de production d'un même paquet, ou de paquets du même dépôt,
qui ont la même fonction sont un doublon (pur, dans `timonier-core::inventory`).

**Ajoutées depuis la revue.** Les arêtes `DependsOn` de provenance `Manifest` vers des paquets
externes, ajoutées entre la dernière marque de revue (ADR 0024) et la tête : une requête de diff
du graphe (ADR 0010), sans état de plus.

**CVE (Q79).** OSV.dev, à la demande seulement : `POST /v1/querybatch` pour toutes les
dépendances, puis `GET /v1/vulns/{id}` pour le résumé et la gravité, au plus 50 détails par
appel. Client `ureq` 3.4.2 (MIT OR Apache-2.0, vérifié le 2026-10-21 ; bloquant, appelé hors
du runtime tokio). Les réponses sont gardées 24 h dans `.timonier/cache/osv.json`, hors de git ;
le rapport dit leur date. L'outil prend `cve: false`, et `TIMONIER_OFFLINE=1` coupe tout appel
réseau du démon ; sans réponse, les CVE sont « non vérifiées », jamais « aucune ».

**SBOM.** CycloneDX 1.5 en JSON par `cyclonedx-bom` 0.8.1 (Apache-2.0, vérifié le 2026-10-21 :
`Bom`, `Component::new`, `Purl`, `Validate::validate`, `output_as_json`), dans le démon, qui
seul fait les I/O : un composant `application` par paquet du dépôt, un composant `library` par
dépendance avec sa licence SPDX et son purl, ses vulnérabilités quand OSV a répondu. Le démon le
valide avant de le rendre. `inventory/sbom` le donne en JSON ; `timonier inventory --sbom
FICHIER` l'écrit.

**MCP.** `get_tech_inventory { cve?, limit?, cursor? }` (nom de la spec) : langages, frameworks,
nombre de dépendances par écosystème et par licence, doublons, ajoutées depuis la revue,
vulnérabilités (ou leur absence de vérification), puis les dépendances, paginées.

## Alternatives écartées

- **Lire les licences dans `~/.cargo/registry`** : `cargo metadata` le fait, avec la résolution
  des fonctionnalités et des cibles.
- **Écrire le JSON CycloneDX à la main** : la validation de la bibliothèque est ce qui rend le
  SBOM « valide », critère de la tâche.
- **`reqwest`** : un client asynchrone de plus, plus lourd, pour quelques requêtes à la demande.
- **RustSec et la base de npm séparément** : deux sources, deux formats ; OSV les agrège.

## Conséquences

- L'inventaire d'un projet Rust appelle `cargo` ; sans lui, versions sans licences.
- Les doublons ne valent que pour les fonctions que les packs connaissent : la table s'étend
  avec eux.
- Interroger OSV lui envoie les noms et versions des dépendances (Q79).
