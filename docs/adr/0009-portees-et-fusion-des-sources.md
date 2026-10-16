# 0009 — Portées, observations et fusion des sources

- Statut : Accepté (2026-10-16, validation de la phase par l'humain)
- Date : 2026-10-14
- Phase : 2
- Liens : spec « Indexation et modèle de graphe » et « Décisions techniques » (indexation en
  trois vitesses) ; ADR 0008

## Contexte

La spec demande :

- qu'une arête vue par plusieurs sources garde la provenance la plus fiable ;
- qu'une arête tree-sitter contredite par le LSP soit supprimée ;
- que chaque fichier sache s'il a été « vérifié par LSP au hash X » ;
- que les arêtes entre composants soient agrégées, jamais stockées à la main.

Les mises à jour arrivent à trois vitesses : tree-sitter à la sauvegarde, LSP en arrière-plan,
consolidation au commit. Il faut donc remplacer exactement ce qu'une source a produit, sans
toucher au reste.

## Décision

**Portées**

- Une portée est l'unité de remplacement. Chaque nœud et chaque arête appartiennent à une seule
  portée.
- La portée `structure` contient le Repo, les Packages, les Components et le placement des
  Files.
- Une portée `file:<chemin>` contient les symboles du fichier et les arêtes qui partent de son
  code.
- Une mise à jour remplace d'un coup ce qu'elle fournit pour sa portée : ses nœuds, et/ou les
  observations d'une source.
- Ce qu'une portée peut contenir est contrôlé :
  - `structure` ne contient ni symbole ni rien d'autre que le Repo, les Packages, les
    Components et les Files ;
  - une portée de fichier ne contient que des symboles placés sous son fichier ou sous un autre
    symbole du même fichier ;
  - une arête observée part toujours de ce que la portée fournit : son fichier, ou l'un de ses
    nœuds.

**Observations**

- Pour chaque portée et chaque source, le stockage garde les dernières observations d'arêtes,
  avec les types d'arêtes que cette source a vérifiés entièrement.
- Elles valent pour le hash de contenu courant de la portée. Un nouveau hash invalide les
  observations et vérifications des autres sources, devenues périmées. Tree-sitter repasse
  toujours en premier.
- « Vérifié par LSP au hash X » est donc une vérification présente pour la portée, au hash
  courant X.

**Contenu de référence des observations** (ajouté après la relecture de la Phase 2)

- Les observations d'une source disent sur quel contenu elles ont été calculées
  (`computed_on`).
- Des observations calculées sur un autre contenu que le contenu courant de la portée sont
  ignorées et comptées comme périmées. C'est le cas d'une réponse lente du LSP arrivée après
  une nouvelle sauvegarde.
- Une source précise doit toujours donner ce hash, et n'apporte jamais ni le hash de contenu
  d'une portée ni des nœuds : seule la passe qui lit le contenu (tree-sitter, la structure) le
  fait. « Vérifié
  par LSP au hash X » ne peut alors jamais se rattacher au mauvais contenu, même si la réponse
  arrive en retard.
- Une portée de fichier sans contenu (supprimée, ou pas encore lue) ne prend aucune
  observation, même approximative. La portée `structure`, qui n'a pas de hash de contenu,
  prend les observations des manifestes.
- Tree-sitter peut omettre `computed_on`, car il fournit le hash de contenu dans la même mise à
  jour.

**Propriété** (revue après deux relectures de la Phase 2)

- Quand plusieurs portées fournissent le même nœud, la portée de plus petite clé en est
  propriétaire : c'est sa version qui est dans le graphe. Pour une même arête, la provenance la
  plus fiable l'emporte d'abord (spec « Fusion »), puis la plus petite clé. Exemples : un symbole
  déclaré dans deux fichiers, ou un couper-coller pas encore sauvegardé des deux côtés.
- Les versions des autres portées sont gardées en attente (`node_claim`, `edge_claim`). Quand le
  propriétaire cesse de fournir l'élément, l'attente de plus petite clé prend sa place, sans
  passer par une suppression.
- La tête ne dépend donc que de ce que chaque portée fournit actuellement :
  - jamais de l'ordre des mises à jour ;
  - réappliquer une portée inchangée n'écrit rien ;
  - un symbole déplacé entre deux fichiers converge quel que soit l'ordre de réindexation.
- Reprises et attentes sont comptées dans les statistiques d'écriture.
- Les contributions en attente comptent dans le contrôle des orphelins. Un fichier ne peut donc
  pas quitter la structure tant que sa portée fournit encore des symboles, qu'ils soient
  propriétaires ou en attente. Contrat pour les producteurs (Phase 3) : retirer la portée d'un
  fichier dans le même lot que sa sortie de la structure, ou avant.
- Une mise à jour de retrait (`remove`) n'apporte rien d'autre : ni hash, ni nœuds, ni
  observations.

**Fusion**, calculée par portée et par type d'arête

- Si une source précise (Scip ou Lsp) a vérifié ce type, les observations approximatives
  (TreeSitter, Inferred) de ce type sont écartées. Elles sont soit contredites, soit confirmées
  avec une meilleure provenance. Les observations Manifest restent, car elles sont déclaratives.
- Pour une même arête (source, destination, type), la provenance la mieux classée l'emporte,
  avec sa confiance et son poids.
- Classement : Scip > Lsp > Manifest > TreeSitter > Inferred.
- `Intended` est refusé par le modèle réel : l'intention n'est jamais fusionnée.

**Refusé à l'entrée**, et non stocké :

- **Les appels entre symboles venant d'une source approximative** (TreeSitter, Inferred). La
  spec ne stocke d'arêtes `Calls` fonction → fonction que si elles viennent du LSP ou de SCIP ;
  sinon, des arêtes fichier → fichier pondérées. Les packs de langage (Phase 3) produisent donc
  les appels de tree-sitter au niveau des fichiers.
- **Les arêtes qui touchent un Component ou le Repo**. Ce sont des boîtes calculées : leurs
  arêtes s'obtiennent par agrégation.

**Arêtes pendantes** : une arête peut désigner un nœud absent, parce qu'il n'est pas encore
indexé ou qu'il est externe. Elle est conservée, mais reste invisible dans les vues tant que
ses deux extrémités n'existent pas. Les lectures brutes (`edges_at`, `diff`) les renvoient :
l'agrégation les écarte, et les vues de la Phase 5 devront les filtrer.

**Agrégation** : les arêtes au niveau d'un zoom se calculent à la lecture. Chaque extrémité
est remplacée par son ancêtre visible le plus proche ; on somme les poids et on garde la
meilleure provenance, avec l'indication « approximative seulement ». Rien n'est stocké.

## Alternatives écartées

- **« La dernière portée qui fournit l'élément l'emporte »** (essayé, puis écarté à la deuxième
  relecture) : avec deux portées qui fournissent le même identifiant, l'élément bascule d'une
  portée à l'autre à chaque réindexation. Une portée inchangée écrit alors des lignes, et le
  diff de revue montre de faux déplacements.
- **Refuser qu'une portée fournisse un élément déjà détenu ailleurs** : un symbole déplacé
  bloquerait l'indexation du fichier destination tant que le fichier source n'est pas réindexé.

- **Fusion dans des requêtes SQL sur toutes les observations historisées** : il faudrait
  historiser aussi les vérifications. Les requêtes deviennent complexes pour un gain nul :
  l'historique utile est celui du graphe fusionné.
- **Ne supprimer que les arêtes tree-sitter explicitement absentes du LSP** : à granularité
  différente (fichier → fichier contre fonction → fonction), la comparaison arête par arête est
  impossible. La vérification par type d'arête règle les deux cas.
- **Refuser les arêtes pendantes** : l'indexation devrait suivre un ordre strict, et les appels
  vers les dépendances externes seraient perdus.

## Conséquences

- Un producteur fournit toujours la totalité de sa contribution pour une portée, jamais un
  morceau.
- Les arêtes pendantes doivent être filtrées par chaque lecture qui joint les nœuds.
