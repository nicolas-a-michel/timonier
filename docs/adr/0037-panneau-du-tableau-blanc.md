# 0037 — Le panneau du tableau blanc : une troisième page, ses commandes, son état

- Statut : Accepté (2026-10-21, validation de la Phase 7 par l'humain)
- Date : 2026-10-20
- Phase : 7
- Liens : spec « Tableau blanc », « Expérience et design » ; ADR 0019 (protocole), 0021 (carte),
  0031 (pages construites à part), 0036 (React Flow et perfect-freehand) ; tâche PLAN.md 7.1

## Contexte

L'ADR 0036 choisit la bibliothèque. Restent où vit le tableau blanc dans l'extension, comment
la palette le commande (chaque action a sa commande, skill `vscode-webview-ui`), et où il garde
son dessin avant que `intent.json` existe (7.3). Le critère de 7.1 demande que le dessin libre,
l'annulation et le clavier marchent dans le build de production, pas seulement en développement.

## Décision

**Page** : une troisième page de la webview, `whiteboard.html`, construite à part
(`vite build --mode whiteboard`, `dist/whiteboard/`), pour la même raison que celle du suivi
(ADR 0031) : sous la CSP par nonce, une page ne charge qu'un script. Un panneau d'éditeur,
`timonier.whiteboard`, un par fenêtre, ouvert par « Timonier : Ouvrir le tableau blanc ».

**Dessin** : chaque trait est un nœud React Flow (`stroke`) dont le contenu est l'encre SVG ;
il se sélectionne, se déplace (souris ou flèches) et s'efface (Suppr) comme une boîte de la
carte, avec un nom pour les lecteurs d'écran. Seule l'encre prend le pointeur, pas la boîte
autour. Trois outils : sélection (V), crayon (P), gomme (E) ; la gomme a un rayon de 6 px.
L'historique est une pile d'opérations inversibles (ajouter, retirer à sa place dans l'ordre,
déplacer), 200 pas ; Ctrl+Z, Ctrl+Maj+Z et Ctrl+Y.

**Commandes** : un message de l'hôte de plus, `whiteboard { command }`, avec `WhiteboardCommand`
(annuler, rétablir, outil, supprimer la sélection, tout afficher, zoom) et `WhiteboardTool`,
générés depuis Rust comme le reste (ADR 0019). Une commande de palette par action. Le message ne
va pas au démon : la version du protocole ne change pas.

**État** : le dessin est `.timonier/intent.json`, écrit après chaque geste (ADR 0039, Q66) ;
l'état de la webview (`setState`), qui le gardait en 7.1, ne sert plus.

**Preuve** : les tests Playwright de `webview/test/production/` construisent la page pour la
production, la servent dans la page que l'extension écrit (même `renderWebviewHtml`, même CSP),
et y dessinent, gomment, annulent et passent par le clavier ; puis axe-core et les contrastes
dans chaque thème capturé. Un test d'intégration ouvre le panneau dans VSCode.

## Alternatives écartées

- **Le tableau blanc dans la page de la carte** : un seul bundle, mais une page qui fait deux
  choses ; le panneau du tableau blanc chargerait la mise en page elk sans s'en servir.
- **Les traits dans un calque SVG à part, sous les nœuds** : sélection, déplacement, focus et
  noms seraient à refaire ; un nœud les a déjà, et React Flow ne dessine que ce qui est visible.

## Conséquences

- Mesuré par `npm run bench:whiteboard -w webview` (`webview/test/perf/whiteboard.perf.ts`, build
  de production, Chromium, 2026-10-21) : 1 000 traits se chargent en 0,31 s, un trait s'ajoute en
  27 ms, une annulation prend 35 ms ; 3 000 traits : 0,63 s, 63 ms et 95 ms.
- Un trait ne se trace pas au clavier ; tout le reste s'y fait.
