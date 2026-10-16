# 0016 — Composants : structure, clustering stable, identité

- Statut : Accepté (2026-10-16, validation de la phase par l'humain)
- Date : 2026-10-16
- Phase : 3
- Liens : spec « Composants », « Expérience et design » (mise en page stable, quelques boîtes) ;
  Q4 (a) ; ADR 0008, 0015 ; tâche PLAN.md 3.8

## Contexte

D'après la spec, les composants viennent d'abord de la structure (crates, paquets, dossiers),
puis d'un clustering Louvain/Leiden. La carte s'ouvre sur quelques boîtes. Au commit, le
re-clustering complet part de la partition précédente.

Q4 (a) place les composants dans l'arbre : au-dessus des paquets ou à l'intérieur, et le
clustering ne coupe jamais un paquet. Le critère de 3.8 demande une stabilité mesurée, avec un
seuil fixé par ADR.

Vérifié le 2026-10-16 : graphrs 0.12.0 (MIT). Son Leiden part toujours d'une partition en
singletons, sans moyen de lui donner la partition précédente. Il tire aussi nalgebra, ndarray
et sprs.

## Décision

**Deux niveaux**, construits de la même façon :
- au-dessus des paquets, les éléments sont les paquets du dépôt ;
- dans un paquet, ce sont ses fichiers.

**On ne découpe qu'un niveau trop chargé.** Un niveau qui garde au plus 12 éléments reste à
plat : sur Timonier, les 9 paquets sont les boîtes de la vue d'ensemble. Au-delà de 12 :

1. **Structure d'abord.** Un dossier devient un composant s'il contient au moins deux
   éléments et qu'il ne les contient pas tous. On recommence dans chaque dossier qui garde
   plus de 12 éléments.
   - Le dossier principal des sources d'un paquet (`src/`) n'en est pas un : son contenu est
     le paquet lui-même. Ses sous-dossiers en sont, sous leur vrai chemin : `src/tests/` et
     `tests/` restent deux composants.
   - En haut, `crates/` peut ainsi regrouper les crates d'un gros espace de travail.
2. **Clustering ensuite**, pour un dossier qui garde plus de 12 éléments à plat.
   - Algorithme : Leiden sur le graphe non orienté des arêtes agrégées entre éléments (imports,
     appels, références, dépendances de manifeste), avec la modularité comme qualité, une
     résolution de 1 et aucun hasard. L'ordre des éléments est trié.
   - Une communauté d'un seul élément ne devient pas un composant, et l'élément reste seul,
     directement sous son paquet ou sous le dépôt.
   - Le clustering ne coupe jamais un paquet : en haut, il ne déplace que des paquets entiers.

**Dépendances externes** : un composant `external` en haut, puis un composant par catégorie
d'usage du pack (`external/network`, `external/serialization`…). Les dépendances sans
catégorie vont dans `external/other`.

**Implémentation maison** de Leiden, dans `timonier-index` :
- déplacement local, raffinement qui garantit des communautés connexes, puis agrégation ;
- partant d'une partition initiale, de quelques centaines de lignes.

**Partition initiale et identité**
- Le clustering a lieu au commit (spec), en partant des composants actuels. Il a lieu aussi à
  la toute première indexation, ou quand les paquets changent (un paquet renommé emporte ses
  composants).
- Au démarrage, les composants de la tête sont repris tels quels, sans clustering : la carte
  ne bouge pas d'un redémarrage à l'autre, y compris quand la tête n'en a aucun. Seule une
  base jamais indexée déclenche le premier clustering.
- Si un paquet de la tête a disparu au démarrage (renommé pendant l'absence du démon), ses
  composants sont écartés et le clustering repart de ce qui était là.
- Un composant garde sa clé tant qu'il garde la majorité de ses membres. On apparie les
  communautés aux anciens composants par recouvrement décroissant, chacun une seule fois.
- Clés :
  - `dir/<chemin>` pour un dossier ;
  - `cluster/<chemin du membre le plus petit>` pour une nouvelle communauté ;
  - `external/<catégorie>` pour les dépendances.
- Entre deux commits, un fichier arrivé depuis le dernier clustering rejoint le composant de la
  majorité de ses voisins de dossier, sinon son paquet.
- Leiden repart de son propre résultat jusqu'à ce que rien ne bouge : partir d'une partition
  qu'il a donnée la rend telle quelle, si bien qu'un clustering sans changement n'écrit rien.
  Un nœud sans lien avec sa communauté la quitte, pour qu'aucune ne soit non connexe.

**Nom** : le nom du dossier ; pour une communauté, le nom du fichier le plus connecté du
groupe ; pour les dépendances, la catégorie. L'agent pourra les renommer par MCP (Q6, Phase 6).

**Seuil de stabilité** : une modification d'un fichier ne change pas l'affectation de plus de
5 % des éléments. C'est mesuré sur un graphe synthétique de 300 fichiers en 10 groupes, et
sur Timonier.

## Alternatives écartées

- **graphrs** : il ne part pas d'une partition initiale, ce qui est contraire à la spec ; il
  tire des dépendances lourdes ; il est peu adopté.
- **Louvain seul** : il peut produire des communautés non connexes, ce que la spec ne veut pas
  sur une carte de boîtes.
- **Clustering à chaque sauvegarde** : la carte bougerait sans cesse. La spec le réserve au
  commit.
- **Communautés sous un même paquet de plusieurs langages** : hors du MVP.

## Conséquences

- Sur un petit dépôt comme Timonier, presque tout reste à plat. Seul `timonier-index`
  dépasse 12 fichiers : ses tests forment un composant.
- Le seuil de 12 et la résolution sont à revoir sur un gros dépôt (Phase 8). Les deux sont des
  constantes.
