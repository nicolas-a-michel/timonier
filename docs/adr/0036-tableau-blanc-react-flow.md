# 0036 — Le tableau blanc : React Flow et perfect-freehand, à la place de tldraw

- Statut : Accepté (2026-10-20, Q60)
- Date : 2026-10-20
- Phase : 7
- Liens : spec « Tableau blanc », « Expérience et design » ; Q3 ; ADR 0020 (design system), 0021
  (carte avec React Flow) ; tâches PLAN.md 7.0 à 7.3

## Contexte

La spec voulait tldraw (« formes custom, export JSON propre, plutôt qu'Excalidraw, moins adapté
aux formes typées ») ; ta réponse Q3 l'écarte (licence propriétaire, avec clé). Le tableau blanc
doit offrir le dessin libre partout et des formes typées (boîte, flèche avec politique, zone,
note, note budget), au clavier et dans le build de production du panneau (7.1), sous la CSP des
webviews : `default-src 'none'`, un seul script par nonce, polices et images de l'extension
seulement, pas de ressource distante. Paquets vérifiés sur npm le 2026-10-20 ; le paquet
d'Excalidraw a été lu (sans l'exécuter).

## Décision

**React Flow** (`@xyflow/react` 12.12.0, MIT, déjà dans le dépôt pour la carte) porte les formes
typées, et **perfect-freehand** (1.2.3, MIT, aucune dépendance, 112 ko) trace le dessin libre :
- chaque forme typée est un nœud (boîte, zone, note, note budget) ou une arête (flèche) React
  Flow, avec un nom accessible et le focus clavier, comme sur la carte ;
- un trait libre est une suite de points avec pression ; perfect-freehand en fait le contour,
  dessiné en SVG dans un calque sous les nœuds, aux coordonnées du canevas ;
- l'annulation est une pile d'opérations tenue par le tableau blanc (ajouter, déplacer,
  retyper, effacer), pas une copie de l'état ;
- le même langage visuel que la carte (tokens, formes, icônes) : une boîte voulue ressemble à
  une boîte réelle, avec un trait qui la dit dessinée.

Le format (`intent.json`, sémantique et `sketch` séparés) est l'objet de 7.3.

## Alternatives écartées

- **Excalidraw** (`@excalidraw/excalidraw` 0.18.1, MIT) : lu dans son paquet,
  - les polices (13 Mo, 8 familles) se chargent depuis `esm.sh` sauf si
    `window.EXCALIDRAW_ASSET_PATH` est posé : il faudrait les embarquer dans le `.vsix` ;
  - un worker de module (sous-ensembles de polices) et un module WebAssembly compilé avec
    `new Function` : la CSP devrait ouvrir `worker-src` et `'unsafe-eval'` ;
  - 18 Mo de build, une trentaine de dépendances (radix, jotai, sass…), son propre thème et sa
    propre barre d'outils, loin du design system ;
  - un canevas : les formes ne sont ni focalisables ni nommées pour un lecteur d'écran ;
  - pas de formes personnalisées : le type se garderait dans `customData`, que l'éditeur ne
    montre ni ne valide. C'est la réserve de la spec.
- **tldraw** 5.5.2 : licence propriétaire avec clé (Q3).
- **Konva / react-konva** (10.9.0 / 19.3.2, MIT) ou **Fabric** (7.4.0, MIT) : un canevas aussi,
  sans accessibilité ; tout l'éditeur resterait à écrire, sans le gain de React Flow.

## Conséquences

- Une seule bibliothèque de graphe dans la webview, une CSP inchangée, une dépendance de 112 ko.
- Plus de code à nous : outils de dessin, gomme, sélection d'un trait, annulation. Un trait libre
  ne se tape pas au clavier ; il est sélectionnable et effaçable au clavier.
- À revoir si le dessin libre est lent au-delà de quelques milliers de traits (mesure en 7.1).
