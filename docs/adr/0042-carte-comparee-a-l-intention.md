# 0042 — La carte comparée à l'intention

- Statut : Accepté (2026-10-21, validation de la Phase 7 par l'humain)
- Date : 2026-10-21
- Phase : 7
- Liens : spec « Comparer à l'intention » ; Q68 ; ADR 0019 (protocole), 0024 (diff de revue, son
  bandeau), 0030 (dette, sa couche), 0041 (dérive et ancrage) ; tâche PLAN.md 7.6

## Contexte

La spec veut une option d'affichage : composants dessinés mis en évidence, composants manquants
listés, arêtes interdites franchies signalées, l'ancrage calculé sans écrire le fichier
d'intention. La dérive se calcule déjà dans le démon (ADR 0041). La vue de la carte ne contient
que ce qui est ouvert : une boîte ancrée en profondeur n'y est pas forcément.

## Décision

**Données** : la méthode `intent/drift` du démon rend une `DriftView` : la dérive (ADR 0041), les
noms des couches et composants dessinés, et la lignée d'ancêtres de chaque boîte réelle citée
(ancrages, extrémités des dépendances qui franchissent). Un tableau sans forme ni trait (le vide
d'`init`) n'est pas « dessiné ». L'extension la demande quand la carte compare, puis 500 ms après
chaque révision montrée et chaque changement d'`intent.json`, et la passe à la page (`drift`) ;
`null` sans démon. Protocole : `compareIntent` et `openWhiteboard` de la page, `drift` de l'hôte,
la commande de carte `toggleIntent`.

**Où se montre une boîte** : elle-même si la carte la dessine, sinon la boîte dessinée la plus
profonde de sa lignée, sinon le groupe de rôle de son sommet. La page le calcule (pure,
`map/intent.ts`) à chaque mise en page.

**Interface** : un bouton de la barre d'outils, « Comparer à l'intention » (gardé dans l'état de la
page), et la commande de palette. Allumé :
- un bandeau sous celui de la revue, même facture : le ton (forme et couleur), « 4 composants
  dessinés, 3 trouvés dans le code, 1 manquant · 1 arête interdite franchie · 1 arête requise
  absente », puis les listes (manquants, interdites franchies avec leurs dépendances, requises
  absentes) et « Ouvrir le tableau blanc » ; rien de dessiné, il le dit et propose le tableau ;
- sur une boîte, ouverte ou fermée, une icône dans sa barre de titre, à côté de la marque de
  revue : une coche cerclée pour la boîte que désigne un composant dessiné, la même dans un cercle
  tireté pour une boîte qui en contient un plus profond (deux formes, pas deux couleurs) ; la
  barre de titre a toujours la place et reste au zoom lointain, quand la dette ou la densité
  compacte remplissent la ligne du bas ; les mots (« Dessiné : Parser », « Contient du dessiné :
  … ») dans le nom de la boîte pour les lecteurs d'écran, son infobulle et le panneau de détails ;
  dans une carte de rôle fermée, la même icône sur chaque membre concerné, et dans son titre ;
  la légende explique les deux formes ;
- sur un lien qui porte une dépendance franchissant une arête interdite : le ton de violation et
  le mot « interdite », avec la marque de revue en icône s'il en a une ; dans son nom aussi ; la
  légende l'explique. React Flow dessine les boîtes par-dessus les liens et leurs étiquettes, et
  deux liens de sens contraires partagent leur milieu : le mot se place sur un tronçon de son lien
  qu'aucune boîte ne cache, là où il couvre le moins, une barre de titre comptant beaucoup plus
  qu'un corps de boîte, et plutôt hors du milieu (`map/labelSpot.ts`) ; les obstacles se
  calculent une fois par mise en page, pas à chaque image d'un déplacement. Il est dessiné dans le
  calque au-dessus des boîtes (`ViewportPortal`) : quand il doit couvrir, il couvre un corps de
  boîte ou le vide d'un en-tête de cadre, pas un nom ni une marque. En dernier recours, quand tout
  autre tronçon couvrirait une barre de titre, il revient au milieu, sur l'étiquette du lien
  inverse (relecture n° 8) : il porte lui-même ses marques, et le lien inverse garde son ton et
  son nom. C'est la seule étiquette qui
  passe sur une boîte : il complète l'ADR 0021 (« les liens sous les boîtes »), dont la ligne de
  statut y renvoie ; un mot caché ne dit rien. Pas plus épais : l'épaisseur est au trafic mesuré (spec, ADR 0021,
  Phase 8).

## Alternatives écartées

- **Le démon marque les nœuds de la vue** : la vue ne sait pas ce que la page a groupé ou plié ;
  la page sait ce qu'elle dessine.
- **Pousser la dérive avec chaque delta** : un calcul par révision même quand personne ne compare.
- **Un lien interdit plus épais** (relecture n° 4) : l'ADR 0021 réserve l'épaisseur au trafic.
- **Des boîtes fantômes pour les manquants** : la carte ne montre que le code réel (spec) ; les
  manquants sont listés.

## Conséquences

- La carte, l'agent (`get_drift`) et le bandeau disent la même dérive, d'un seul calcul.
- Une arête requise absente n'a pas de lien à marquer : elle est listée seulement.
