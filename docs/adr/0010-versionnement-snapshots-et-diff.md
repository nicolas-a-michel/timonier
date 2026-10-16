# 0010 — Versionnement : tête de travail ouverte, snapshots figés, diff et déplacements

- Statut : Accepté (2026-10-16, validation de la phase par l'humain)
- Date : 2026-10-14
- Phase : 2
- Liens : spec « Stockage », « Identité stable », « Diff depuis ma dernière revue » ;
  ADR 0008, 0009

## Contexte

La spec demande trois choses :

- des intervalles `valid_from` / `valid_to` sur chaque nœud et chaque arête ;
- une table `snapshot(seq, commit_sha, timestamp, kind)`, avec `kind` parmi `commit`,
  `review_mark` et `working_tree` ;
- un diff entre deux snapshots qui soit une requête, et des déplacements reconnus par
  similarité (signature et hash du corps normalisé).

Le graphe change à chaque sauvegarde : créer un snapshot par sauvegarde ferait exploser
l'historique.

## Décision

**Snapshots**

- Il existe toujours exactement un snapshot ouvert, la tête (`working_tree`, non figée).
  Toute modification est datée de la tête.
- Figer la tête (`commit` avec son SHA, `review_mark`, ou `working_tree`) lui donne son type et
  son horodatage, puis ouvre une nouvelle tête au numéro suivant.
- L'état au snapshot s est l'ensemble des lignes telles que `valid_from ≤ s` et
  (`valid_to` nul ou `valid_to > s`). L'état de la tête est l'ensemble des lignes où `valid_to`
  est nul.

**Écritures**

- Une ligne créée puis supprimée dans la même tête est effacée physiquement : aucun snapshot
  figé ne l'a vue.
- Une modification d'une ligne née dans la tête se fait sur place.
- Sinon, l'ancienne ligne est fermée (`valid_to` = tête) et une nouvelle version est ouverte.

**Attributs versionnés**

- Nœuds : type, nom, langue, parent, signature, hash du corps.
- Arêtes : provenance, confiance, poids.
- Le span n'est pas versionné : il est mis à jour sur place dans la version courante. Sans
  cela, ajouter une ligne en haut d'un fichier créerait une version de chaque symbole placé
  en dessous.

**Diff de a à b** (a < b)

- Seules les lignes dont `valid_from` ou `valid_to` tombe dans ]a, b] sont lues. Elles sont
  ensuite regroupées par identité : nœud par identifiant, arête par (source, destination,
  type). On obtient ajouts, suppressions et modifications.
- Le coût est proportionnel aux changements, pas à la taille du graphe.

**Déplacements**, détectés au diff

- Entre un symbole supprimé et un symbole ajouté de même type, l'appariement se fait en deux
  passes, un à un, dans l'ordre des identifiants :
  1. le même hash de corps (« même corps »). Un corps que plusieurs symboles supprimés, ou
     plusieurs symboles ajoutés, partagent (corps vides, une ligne) n'apparie que des symboles
     qui ont gardé leur nom : on n'invente pas de renommage ;
  2. parmi les symboles restants, le même nom et la même signature (« même signature », corps
     modifié). Il faut qu'un seul symbole supprimé et un seul symbole ajouté partagent ce
     triplet, pour ne pas apparier deux `new` sans rapport. Limite assumée : un type supprimé
     et un autre ajouté, chacun avec une seule méthode `new()`, s'apparient encore.
- Un symbole qui garde son identifiant mais change de fichier est aussi un déplacement
  (« même identifiant »). Exemples : un module Rust passé de `foo/mod.rs` à `foo.rs`, une
  méthode déplacée d'un bloc `impl` à un autre, un symbole SCIP.
- Un fichier supprimé R et un fichier ajouté A sont un déplacement si au moins la moitié des
  symboles changés de R, et au moins un, sont partis dans A. Les symboles qui ont gardé leur
  identifiant comptent aussi.
- Une arête qui ne change qu'à travers des nœuds déplacés n'est pas signalée.

**Lectures à la tête** : elles utilisent les index partiels des lignes courantes
(`valid_to IS NULL`), sans parcourir l'historique.

## Alternatives écartées

- **Un snapshot `working_tree` par sauvegarde** : l'historique grossit sans fin, pour un gain
  nul. Le voyage dans le temps (v2) s'appuiera sur les snapshots figés.
- **Recalculer le diff en comparant deux états complets** : le coût serait proportionnel au
  graphe entier, et la spec demande une requête.
- **Une similarité floue des corps** (distance d'édition) : plus coûteuse, et le hash du corps
  normalisé suffit à couvrir les renommages et déplacements des refactorings courants.

## Conséquences

- Un snapshot `commit` fige ce qui est indexé au moment du gel, modifications non commitées
  comprises. La consolidation au commit (Phase 3, tâche 3.7) devra soit indexer l'arbre commité,
  soit noter que le snapshot est « sale ».

- Un fichier sans symbole qui est déplacé apparaît comme supprimé puis ajouté. C'est une
  limite connue.
- Les spans des versions fermées sont les derniers connus, pas ceux de l'époque. À revoir pour
  le voyage dans le temps (v2).
