# 0074 — La certitude de la carte : les liens confirmés, comptés et dessinés

- Statut : Proposé
- Date : 2026-10-23
- Phase : 11
- Liens : spec « Précision affichée » ; ADR 0020 (système de design), 0021 (carte), 0073 (SCIP) ;
  tâche PLAN.md 11.5

## Contexte

La carte distinguait deux traits : en tirets quand tous les liens d'une ligne venaient de
tree-sitter, plein dès qu'un seul était confirmé. Une ligne de groupe qui réunit cent liens
approchés et un lien confirmé paraissait donc sûre. Avec SCIP (ADR 0073), la part confirmée varie
d'un endroit à l'autre de la carte, et 11.1 a montré que tree-sitter n'en voit qu'une partie :
la carte doit dire combien elle est sûre, et où.

## Décision

- **Le compte** : chaque arête agrégée porte `confirmed`, le nombre des liens stockés qu'elle
  représente dont la provenance n'est pas approchée (manifeste, LSP, SCIP). La carte l'additionne
  par ligne.
- **Trois traits**, distincts par leur motif, jamais par la couleur seule : plein (tous
  confirmés), tirets longs `12 3` (une partie), tirets courts `5 4` (tree-sitter seul). La
  légende ne montre les tirets longs que si la carte en dessine.
- **Les mots** : le nom accessible d'une ligne dit « sûr », « approché » ou « N confirmés ».
- **Le calque « Certitude »** (barre d'outils, commande `timonier.map.certainty`, état gardé
  comme les autres calques) : chaque élément dit sa part confirmée (« 67 % sûr »), le détail dans
  son infobulle et son nom accessible ; une boîte compte une fois chaque ligne qui la touche ou
  touche ce qu'elle contient ; la légende dit la part de la carte entière.
- Une vue enregistrée sans le compte (les instantanés de démonstration) le prend tout ou rien
  selon `approximateOnly`.

## Alternatives écartées

- **La couleur selon la certitude** : la couleur dit le rôle ; et elle ne suffit jamais seule
  (spec).
- **La part sur chaque boîte, toujours** : la carte doit rester calme ; le calque s'allume quand
  on veut juger.
- **La part de tout le graphe, pas des liens dessinés** : les liens internes à une boîte fermée
  ne se voient pas ; la carte parle de ce qu'elle montre.

## Conséquences

Le protocole gagne un champ (`ViewEdge.confirmed`) ; les vues écrites avant restent lisibles. Le
test des cinq secondes du calque est passé (PLAN.md 11.5, 2026-10-23), et le contraste AA du
calque est mesuré dans les six thèmes (`webview/test/a11y/certainty.a11y.ts`). Une ligne dit sa
part confirmée par son motif (trois) et par son nom accessible, pas en chiffres sur la carte.
