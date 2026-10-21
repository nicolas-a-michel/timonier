# 0038 — Les formes typées du tableau blanc

- Statut : Accepté (2026-10-21, validation de la Phase 7 par l'humain)
- Date : 2026-10-20
- Phase : 7
- Liens : spec « Tableau blanc » (mode mixte, budgets de performance, `intent.json`) ; Q62 à Q65 ;
  ADR 0036 (bibliothèque), 0037 (panneau) ; tâches PLAN.md 7.2, 7.3

## Contexte

La spec veut, à côté du dessin libre, des formes typées qui comptent comme contraintes : boîte
(composant voulu : nom, rôle), flèche (politique `allowed`, `forbidden`, `required`), zone
(couche ; une arête entre couches vaut pour tout leur contenu), note (contrainte en texte,
attachée à un élément), note budget (latence p50/p95, mémoire max, allocations, appels), avec des
IDs stables `C-`, `E-`, `L-`, `N-`. Tes réponses : la couche d'une boîte vient du dessin (Q62),
le rôle est un texte libre (Q63), une flèche ne dit qu'une dépendance (Q64), une note s'attache
à ce qui est sélectionné ou à ce sur quoi elle est posée, et se change dans ses propriétés (Q65).

## Décision

**Modèle** (`webview/src/whiteboard/board.ts`, état de la page version 2, la version 1 se lit
encore) : les traits, puis les formes, chacune avec son ID. Une boîte et une zone ont un
rectangle et un nom (la boîte, son rôle) ; une flèche relie deux boîtes ou zones (`src`, `dst`)
et a une politique ; une note a un texte, une note budget ses limites (`p50Ms`, `p95Ms`,
`memoryMb`, `allocations`, `calls`, toutes facultatives) ; les deux ont une position et
`attachedTo` (une boîte, une zone ou une flèche, ou rien). Les IDs sont numérotés par préfixe à
la création (`C-1`, `L-1`, `E-1`, `N-1` ; notes et budgets partagent `N-`), jamais réutilisés,
même après une annulation ; ils ne changent ni au déplacement ni au renommage. La couche d'une
boîte est la plus petite zone qui contient son milieu : calculée, jamais gardée. Déplacer une zone
emporte ce qui est dessiné entièrement dedans. Supprimer une boîte ou une zone retire ses
flèches et détache ses notes, en un seul pas d'annulation. Chaque champ relu est vérifié : un
état qui ne tient pas (une flèche vers rien, un ID au mauvais préfixe, une limite négative) est
refusé en entier.

**Dessin** : outils Boîte (B), Zone (Z), Flèche (A), Note (N), Budget (M). Un clic pose la forme
(boîte et zone centrées sur le clic), qui est sélectionnée et dont le premier champ prend le
focus ; Entrée le garde et rend le focus à la forme. La flèche se trace en deux clics, d'une
boîte ou zone à une autre. Une boîte ressemble à une boîte de la carte, en trait tireté (voulue,
pas réelle) ; une zone est un grand cadre tireté, son nom en capitales dans le coin ; une note,
une carte à bord gauche épais, reliée par un pointillé à ce qu'elle concerne (sur une flèche,
elle le dit en mots). Une flèche autorisée est un trait plein ; interdite, un tireté, la couleur
d'erreur et le mot « interdite » ; requise, un trait épais et le mot « requise ».

**Clavier** : activer un outil de forme au clavier (Entrée dans la barre d'outils), ou la
commande de palette « Ajouter … au tableau blanc », pose la forme au milieu de la vue, et une
note à droite de l'élément sélectionné. Le panneau des propriétés porte tout ce que dit une
forme : nom, rôle, taille, politique, texte, attache, limites, et « Nouvelle flèche vers ». Suppr
et Retour arrière sont des touches du tableau, pas celles de React Flow, pour que la suppression
et ce qu'elle emporte soient un seul pas.

## Alternatives écartées

- **Couche choisie dans un formulaire** (Q62) : deux vérités, le dessin et le champ.
- **IDs tirés du nom** (`C-parser` de l'exemple de la spec) : un renommage changerait l'ID, et
  les références de l'agent.
- **Le raccourci Suppr de React Flow** : il retire les nœuds et les arêtes en deux appels, donc
  deux pas d'annulation, et ne s'est pas déclenché sur une boîte reliée.

## Conséquences

- 7.3 écrit `intent.json` depuis ce modèle : la sémantique (composants avec leur couche, arêtes,
  notes, budgets) d'un côté, la géométrie dans `sketch` de l'autre.
- Un trait libre ne devient pas une forme typée : c'est l'interprétation du dessin (7.4).
