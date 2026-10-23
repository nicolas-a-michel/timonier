# 0076 — Le graphe pour l'agent : `get_component`, `query_graph` et les résumés

- Statut : Proposé
- Date : 2026-10-23
- Phase : 11
- Liens : Q6 (a), Q98 (I2, I3) ; ADR 0027 à 0029 (outils), 0074 (certitude), 0075 (noms) ;
  tâche PLAN.md 11.10

## Contexte

La relecture de 9.5 a montré que l'agent ne voit pas la carte : il lit le code fichier par
fichier, sans savoir quelles boîtes le développeur regarde ni ce qu'elles font (I2), et il ne
peut pas dire à quoi sert une boîte (I3). La spec nomme `query_graph` et `get_component` dans
le catalogue sans fixer leur forme. Q6 (a) voulait que l'agent affine noms et rôles par un outil
structuré ; 11.6 l'a fait pour les noms et les rôles, pas pour les résumés.

## Décision

- **Le même graphe que la carte** : les deux outils lisent `view::load`, la tête du graphe avec
  les noms, rôles et résumés de `names.json` posés dessus (ADR 0075). Le calcul est pur, dans
  `timonier-core::explore` ; le démon donne le graphe.
- **`get_component { component }`** : une boîte (composant ou paquet ; un fichier, un symbole
  ou un identifiant inconnu donnent `not_found`) avec son nom, son rôle, son résumé, son parent,
  les boîtes et fichiers qu'elle contient (200 au plus, et le total des fichiers), et les liens
  qu'elle a avec les boîtes voisines, dans les deux sens. Un lien est agrégé comme sur la carte
  quand la boîte est ouverte au milieu de ses sœurs et de celles de ses ancêtres : genre,
  nombre de liens stockés, nombre confirmés (ADR 0074).
- **`query_graph { expanded?, limit?, cursor? }`** : la carte telle que le développeur la voit
  avec `expanded` ouvertes (50 au plus) : le haut, puis ce que contient chaque boîte ouverte,
  en ordre de parcours ; page de 200 nœuds au plus, curseur numérique ; les liens qui partent
  des nœuds de la page.
- **Les résumés** : `propose_component_name` prend un `summary` (300 caractères au plus, une
  phrase ou deux), gardé dans `names.json` à côté du nom ; seul l'agent l'écrit, le développeur
  le lit. `ViewNode.summary` le porte à la carte : dans le panneau de détail, le nom accessible
  et l'infobulle de la boîte, sans changer la taille des boîtes.
- **Bornes vérifiées dans le démon** (skill `mcp-tools`), refus `invalid_argument`.

## Alternatives écartées

- **Un seul outil `query_graph` avec un filtre** : l'agent ouvre la carte comme le développeur ;
  un langage de requête serait un second modèle à apprendre et à valider.
- **Les arêtes symbole → symbole brutes** : trop nombreuses, et pas ce que voit le développeur ;
  l'agent les trouve dans le code une fois la boîte choisie.
- **Un résumé écrit par le développeur** : le résumé sert à l'agent pour expliquer ce qu'il a
  compris ; le développeur garde le nom et le rôle (Q96). À reprendre s'il le demande.

## Conséquences

L'agent lit la carte en deux ou trois appels au lieu de parcourir les fichiers. Un résumé vit
avec l'identifiant de sa boîte : un regroupement recalculé le perd, comme un nom (ADR 0075).
