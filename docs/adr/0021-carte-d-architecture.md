# 0021 — Carte d'architecture : rôles, zoom sémantique, mise en page

- Statut : Accepté (2026-10-18, validation de la phase par l'humain)
- Date : 2026-10-16
- Phase : 4
- Liens : spec « Expérience et design », « Composants », « Précision affichée » ; ADR 0016,
  0017, 0019, 0020 ; tâches PLAN.md 4.7 à 4.10

## Contexte

La spec veut une carte vivante. Elle s'ouvre sur une vue d'ensemble, puis se zoome
sémantiquement : rôles, composants, sous-composants, fichiers, symboles. Les détails viennent au
survol, la profondeur au clic, et la mise en page reste stable. Le plan fixe React Flow 12.12 et
elkjs 0.12, et le critère de 4.7 : sur Timonier, la vue d'ouverture tient en quelques boîtes
nommées par rôle. Le démon envoie une vue (ADR 0019) : les enfants du dépôt, ceux de chaque nœud
déplié, et les arêtes agrégées entre eux.

Vérifié le 2026-10-16 :
- @xyflow/react 12.12.0, MIT, avec zustand 4.5.7, classcat 5.0.5, @xyflow/system 0.0.83 et
  use-sync-external-store 1.7.0, tous MIT. @xyflow/system tire d3-drag, d3-zoom, d3-selection,
  d3-transition, d3-interpolate, d3-color, d3-dispatch et d3-timer (ISC), et d3-ease
  (BSD-3-Clause). React tire scheduler 0.28.0 (MIT). Toutes ces notices sont dans
  `THIRD-PARTY-NOTICES.md` ;
- elkjs 0.12.0, EPL-2.0 OR GPL-3.0-or-later, utilisé sous EPL-2.0. Il est livré tel quel. Sa
  notice figure dans `THIRD-PARTY-NOTICES.md`, avec le lien vers le paquet de cette version
  exacte, comme l'EPL le demande pour le code source (§ 3.2).

## Décision

**Vue d'ouverture par rôle**
- Les nœuds du haut (paquets, composants) sont regroupés par rôle (ADR 0017) dans des groupes
  propres à la webview : `@role/<rôle>`. Aucun identifiant du modèle n'a cette forme.
- Un groupe fermé est une carte : le rôle (icône et mot) et ses membres, cinq au plus, puis
  « et N autres ».
- Ordre : points d'entrée, interfaces, tâches de fond, modèles de données, persistance,
  utilitaires, bibliothèques externes.
- Une arête entre deux groupes représente les arêtes de leurs membres. Elle est « approchée »
  si toutes le sont.

**Zoom sémantique**
- Un clic, ou Entrée sur le nœud qui a le focus, ouvre un groupe sur ses membres : sans le
  démon, la webview les a déjà.
- Sur une boîte, il demande au démon de la déplier (`view/subscribe` avec la nouvelle liste),
  jusqu'aux symboles.
- Replier une boîte replie aussi ce qu'elle contient. « Revenir aux rôles » referme tout.
- Une boîte ouverte devient un cadre autour de son contenu (`parentId` de React Flow).
- Les groupes ouverts et les boîtes dépliées sont gardés par `vscode.setState` : une webview
  rechargée retrouve sa carte.
- La molette ne fait que grossir ou réduire. La spec dit « cliquer ou zoomer sur une boîte
  ouvre son contenu » : c'est un comportement produit, posé en Q16. L'humain garde le clic
  seul pour l'instant (2026-10-16).

**Mise en page**
- elkjs, algorithme `layered`, de gauche à droite.
- Chaque conteneur est mis en page seul, de l'intérieur vers l'extérieur : la carte, un groupe
  ouvert, une boîte ouverte. Ses enfants directs forment un graphe plat. Une arête entre deux
  éléments plus profonds compte entre leurs ancêtres à ce niveau.
- Tailles fixes pour les boîtes et les cartes : la mise en page n'attend pas de mesure.

**Stabilité** (tâche 4.8)
- `hierarchyHandling: INCLUDE_CHILDREN` est écarté : il met toute la hiérarchie en page d'un
  coup, et ses stratégies interactives comparent des positions relatives à des conteneurs
  différents. Relancé sur son propre résultat, il déplaçait encore les boîtes d'environ
  1 450 px.
- Une mise en page part de celle qui est à l'écran, conteneur par conteneur, de l'intérieur
  vers l'extérieur :
  - un conteneur dont les enfants et leurs tailles n'ont pas changé garde leurs positions
    exactes. Une arête ajoutée ou retirée ne déplace rien : React Flow redessine les arêtes
    lui-même ;
  - un conteneur dont un enfant a grandi ou rapetissé fait de la place, comme on le ferait sur
    un tableau. Ses enfants se rangent en colonnes, d'après leurs étendues en x qui se
    chevauchent. Les colonnes qui suivent un élément plus large se décalent vers la droite
    d'autant. Dans sa colonne, ce qui est sous un élément plus haut descend d'autant. Les
    arêtes entre éléments déjà placés gardent ainsi leur sens, de gauche à droite ;
  - un nouvel élément va où mènent ses arêtes, en bas d'une colonne : celle qui suit la colonne
    de ce qui l'appelle, sinon celle qui précède la colonne de ce qu'il appelle, sinon la
    première. Ce qu'il chevauche descend ;
  - seul un conteneur ouvert pour la première fois passe par ELK. Le premier affichage est
    « posé » par deux passes interactives d'ELK sur son propre résultat.
- Ce qui n'est pas garanti :
  - une arête apparue après le premier affichage, entre deux éléments déjà placés, peut aller
    de droite à gauche : elle ne relance pas la mise en page ;
  - refermer ce qu'on vient d'ouvrir rend la mise en page de départ, au pixel près, tant que
    rien d'autre n'a bougé entre-temps (testé sur la vue de Timonier). Si un nouvel élément a
    fait descendre des voisins, ils ne remontent pas ;
  - « Revenir aux rôles » refait tout depuis zéro.
- Les deux chemins calculent la taille d'un conteneur de la même façon : son contenu plus sa
  marge. Sinon, l'écart de quelques pixels entre la taille donnée par ELK et celle du calcul
  incrémental se propageait aux colonnes : 4,4 px de déplacement pour un simple appel ajouté.
- Historique : la première méthode faisait concourir, à chaque changement, une mise en page
  interactive d'ELK et une mise en page neuve (17,6, 13,8 et 17,3 px). Ces mesures étaient
  fausses : l'indice de position recopiait aussi l'ancienne taille des boîtes. Une fois
  l'erreur corrigée, la méthode incrémentale a remplacé les deux candidates (4,9, 0 et 0 px).
- Seuil : 22 px de déplacement moyen des boîtes inchangées, un quart de la hauteur d'une
  boîte (88 px). Le test le vérifie sur la vue de Timonier, tous rôles et boîtes ouverts.
- Le premier affichage s'ajuste à la fenêtre ; les suivants gardent le cadrage de la personne.
- Les positions sont calculées par défaut. Une boîte déplacée à la main garde la sienne : voir
  « Boîtes déplacées ».

**Boîtes déplacées** (Q20, réponse b de l'humain, 2026-10-17 : « on les calcule par défaut,
mais il faut pouvoir les déplacer »)
- Une boîte ou une carte de rôle se déplace à la souris. Un cadre ouvert se déplace par sa
  barre de titre, et emporte son contenu ; son corps laisse passer le pointeur, vers les
  boîtes qu'il contient et vers le fond. Glisser sur le fond, ou sur le corps d'un cadre, fait
  défiler la carte.
- Une boîte déplacée est épinglée : sa position, relative à son conteneur, est gardée.
  - Les mises en page suivantes la respectent : ELK ou la méthode incrémentale placent les
    autres éléments, puis ce qui chevauche une boîte épinglée descend sous elle.
  - Son conteneur grandit pour la contenir.
- Au clavier, Alt et une flèche déplacent l'élément qui a le focus d'une case du quadrillage
  (16 px). La description de chaque nœud le dit aux lecteurs d'écran.
- Sans glisser, avec un pointeur seul (WCAG 2.5.7) : le panneau de détail du dernier élément
  pointé a quatre boutons qui le déplacent d'une case. Le panneau reste sur cet élément quand
  le pointeur le quitte, pour qu'on atteigne ses boutons. La commande de palette « Déplacer la
  sélection sur la carte… » fait de même pour l'élément qui a le focus.
- « Réinitialiser la disposition » (barre d'outils et palette) oublie les épingles et refait
  la mise en page. « Revenir aux rôles » referme tout, mais garde les épingles.
- Où sont gardées les épingles (H13, confirmée par l'humain : Q21) : la page les envoie à
  l'extension (`pins`), qui les garde dans l'état de l'espace de travail de VSCode, et les rend
  à chaque nouvelle page.
  Elles n'entrent pas dans le dépôt. Au plus 5 000 : au-delà, les plus anciennes tombent.
- Une épingle dont l'élément n'est pas dessiné ne fait rien, et reste pour son retour.

**ELK dans le fil de la webview**
- La version groupée d'elkjs (`elk.bundled.js`) tourne dans le fil de la webview : la CSP
  (`default-src 'none'`, ADR 0005) n'autorise aucun worker.
- Conclusion de la mesure de 4.10 :
  - un changement du code ne passe plus par ELK et coûte 1 à 2 ms ;
  - ELK ne sert qu'au premier affichage et à la première ouverture d'un conteneur : 670 ms
    pour les 183 éléments de Timonier tout ouvert, 1,2 à 1,7 s pour les grandes cartes
    synthétiques, pendant lesquelles la page ne répond pas.
- C'est acceptable pour la Phase 4 : le worker reste écarté. Il sera repris si un premier
  affichage dépasse 2 s sur un vrai dépôt.

**Niveau de détail** (tâche 4.10)
- Budget de 500 éléments dessinés : des niveaux entiers, du haut vers le bas, tant qu'ils
  tiennent. Les enfants d'une boîte sont dessinés tous ou aucun. Plus bas, une boîte est
  « repliée » : le démon a envoyé son contenu, mais la carte le dessine comme elle, avec un
  badge.
- Un clic ou Entrée sur une boîte repliée la met au premier plan (`focus` de l'état de vue) :
  son contenu et la chaîne de ses ancêtres passent en premier dans le budget.
- Budget de 600 arêtes : au-delà, les extrémités les plus profondes remontent à leur conteneur,
  un niveau à la fois.
- Au zoom éloigné (sous 0,5), les boîtes ne montrent que leur nom : une classe CSS qui ne
  change qu'au franchissement du seuil.
- Après un clic ou Entrée qui ouvre quelque chose, la mise en page suivante le cadre, sans
  dépasser la taille réelle.
- Tout ce qui est dessiné reste dans le DOM. `onlyRenderVisibleElements` de React Flow gagnait
  quelques images par seconde, mais sortait du DOM ce qui est hors de la fenêtre, hors de
  portée du clavier et des lecteurs d'écran : écarté.

**Navigation** (tâche 4.9)
- Entrée ou un clic ouvre ce qui s'ouvre, sinon va au code. Maj+Entrée, un double-clic ou le
  bouton du panneau de détail va au code. Les flèches mènent à la boîte la plus proche dans
  leur direction (un pas de côté compte double), Début à la première.
- Le code s'ouvre à côté de la carte, le curseur au début du span du modèle (UTF-8 converti en
  UTF-16). Les lignes sont bornées si le fichier a changé depuis.
- Chaque action a sa commande de palette (`timonier.map.*`), qui agit sur l'élément qui a le
  focus. Une commande peut aussi nommer un nœud du démon que la carte ne dessine pas : un
  paquet dans un rôle fermé se déplie quand même.
- Un panneau caché perd sa page : VSCode la recharge quand il revient. Une commande attend donc
  la page (`ready`), puis la carte (le premier snapshot), avant de s'appliquer.

**Rendu**
- Inspiration, à ta demande du 2026-10-16 : les Blueprints d'Unreal Engine. Des nœuds à barre
  de titre, des fils qui vont d'une broche de sortie à droite à une broche d'entrée à gauche,
  des cadres comme leurs boîtes de commentaire, un fond quadrillé.
- **Nœuds** : la boîte de composant et les icônes du système de design (ADR 0020).
  - Une barre de titre teintée par le rôle, avec l'icône et le nom.
  - Un corps neutre, avec le rôle écrit et la taille.
  - Un rôle fermé est une carte construite de la même façon : le rôle dans la barre de titre,
    ses membres dans le corps.
- **Broches** : un point à droite, où les arêtes sortent, et un triangle à gauche, où elles
  entrent.
  - Elles sont à hauteur de la barre de titre, dessinées par-dessus la boîte.
  - Elles ne sont montrées que là où passe une arête.
  - Elles remplacent les pointes de flèche : quel que soit le nombre d'arêtes qui s'y
    rejoignent, la direction est dessinée une fois.
- **Fils** : la courbe de Bézier de React Flow, à tangentes horizontales.
  - Trait plein si l'arête est confirmée, tirets si elle est approchée.
  - Une seule épaisseur : la spec réserve l'épaisseur au trafic mesuré (Phase 8).
  - Une seule arête par paire de nœuds, toutes sortes confondues.
- **Survol ou focus** : les fils de l'élément prennent sa couleur et passent au-dessus des
  autres fils, toujours sous les boîtes. Un liseré du fond les détache des lignes qu'ils
  croisent : le repère n'est pas la couleur seule. Un panneau montre le détail du dernier élément pointé ou qui a
  le focus ; Échap le range (WCAG 1.4.13).
  - Seuls les objets de ces fils changent : React Flow ne redessine pas les autres, et pointer
    des boîtes en défilant reste léger.
- **Cadres** (un rôle ou une boîte ouverts), comme les boîtes de commentaire.
  - Leur barre de titre est opaque : aucune arête ne barre leur nom.
  - Leur corps est un voile de 5 % de la couleur du rôle : les arêtes intérieures restent
    visibles à travers.
  - Leur trait est plein : sur la carte, les tirets ne veulent dire qu'une arête approchée.
    Le cadre d'un rôle se reconnaît à sa barre de titre, qui porte le rôle et ses membres ;
    celui d'une boîte porte son nom, son rôle et sa taille.
- **Fond** : un quadrillage discret, avec des lignes tous les 16 px et tous les 128 px.
  - Les lignes fines disparaissent au zoom éloigné, où elles brouilleraient.
  - En contraste élevé, il n'y a pas de quadrillage, et la couleur du rôle devient la ligne
    sous la barre de titre.
- **Superposition** : les arêtes restent sous toutes les boîtes, celles d'une boîte ouverte
  aussi (`zIndexMode="manual"`).
  - En mode « basic », React Flow élevait une arête reliée à un nœud enfant au niveau de cet
    enfant.
  - Elle passait alors au-dessus des boîtes du niveau supérieur et barrait leur texte.
- **Zoom** : les boutons et les touches + et −, comme les commandes de palette.
- **Accessibilité** : seuls les nœuds prennent le focus, chacun avec un nom accessible : quoi,
  son rôle, sa taille, ce que fait Entrée. La description commune à tous les nœuds ne dit que
  ce qu'ils partagent (« les flèches mènent à la boîte la plus proche ; Alt et une flèche la
  déplacent ») : elle contredisait
  le nom d'une boîte ouverte. Les broches sont cachées aux lecteurs d'écran, car le nom de
  l'arête dit où elle va.
- **Attribution** : celle de React Flow reste affichée, discrète, avec les jetons. Sa licence
  MIT permet de la masquer, mais ses auteurs demandent de ne le faire qu'avec un abonnement
  Pro.
- **Pas repris des Blueprints** :
  - la couleur des fils par sorte : un arc-en-ciel, contraire à « calme par défaut ». La
    couleur ne vient qu'au survol ;
  - plusieurs broches nommées par nœud : une arête par paire suffit à ce niveau ;
  - la disposition entièrement à la main : les positions restent calculées par défaut, et
    seules les boîtes déplacées gardent la leur (Q20) ;
  - l'animation des fils : c'est la v2 (pixi.js).

**Vérification**
- Tests unitaires : le modèle de la carte (regroupement, arêtes, ouverture, repli) et la mise
  en page.
- Le critère de 4.7 se vérifie sur la vraie vue de Timonier. L'exemple `view_snapshot` l'écrit
  dans `webview/test/views/timonier/`. La page de démonstration (`?map=timonier`) la sert
  avec un démon simulé qui déplie comme le vrai.
- Playwright vérifie :
  - les six boîtes de rôle de Timonier, en anglais et en français ;
  - l'ouverture au clic et à Entrée ;
  - le retour aux rôles ;
  - WCAG AA et le 3:1 des lignes dans les six thèmes.
- Dans un nœud de la carte, axe ne tranche pas le contraste des textes, pour deux raisons :
  - les couches de React Flow entre le nœud et le plan n'ont pas d'événements de pointeur, et
    axe y voit un recouvrement (`bgOverlap`) ;
  - les arêtes, des images pour les lecteurs d'écran, sont dans la pile sous le texte
    (`imgNode`).
  Ces textes-là sont mesurés par le test (4,5:1). Partout ailleurs, un contraste indécis reste un
  échec.
- Le survol aussi est mesuré : contour, fils colorés et texte, une fois les transitions
  finies. Les broches doivent atteindre 3:1, comme les lignes.
- Un test vérifie la mise en page incrémentale sur la vue de Timonier : une fois un rôle
  ouvert, ce qui le suivait est à sa droite, et le refermer rend la mise en page de départ, au
  pixel près.

## Alternatives écartées

- **Ouvrir sur les paquets** : douze boîtes ou plus dès un dépôt moyen, et aucune réponse à
  « de quoi est fait ce projet ? ». Le critère demande des rôles.
- **Le worker d'elkjs** : il demande `worker-src` dans la CSP, un fichier de plus, et une
  frontière de messages, pour un gain qui n'est pas mesuré sur les tailles de la Phase 4.
- **Les routes d'arêtes calculées par ELK** : React Flow redessine depuis les poignées. Les
  routes d'ELK seront à reprendre si les croisements gênent le test des cinq secondes.
- **Les pointes de flèche** : là où dix arêtes entrent dans une boîte, dix pointes
  s'empilaient. La broche d'entrée dit la même chose une seule fois.
- **Une mise en page incrémentale qui ne fait que descendre** ce qui chevauche : un groupe
  ouvert s'élargissait par-dessus la colonne suivante, et ses arêtes repartaient vers la
  gauche, en boucle, à travers les boîtes.
- **Une arête par sorte** (appels, imports, dépendances) : elle multiplie les lignes entre les
  mêmes boîtes. Le détail viendra au survol.

## Conséquences

- La webview porte un peu de modèle (les groupes de rôles). Le graphe et l'agrégation restent
  dans le démon.
- Un changement du code coûte 1 à 2 ms de mise en page. Le premier affichage, et la première
  ouverture d'un grand conteneur, figent la page le temps d'ELK (voir plus haut).
- Ouvrir un groupe décale ce qui le suit : la carte bouge, mais garde sa forme, et la
  refermer la rend telle quelle.
- Déplier une boîte coûte un nouvel abonnement, donc un snapshot entier. C'est simple et sûr ;
  une méthode `view/change` à deltas pourra suivre si c'est lent.
- Les arêtes sont nombreuses dès qu'un groupe est ouvert. Le survol les démêle : celles de
  l'élément pointé prennent sa couleur.
