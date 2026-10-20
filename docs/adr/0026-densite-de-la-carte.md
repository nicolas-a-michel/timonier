# 0026 — Densité de la carte

- Statut : Accepté (2026-10-20)
- Date : 2026-10-18
- Phase : 5
- Liens : spec « Expérience et design » (densité réglable, confortable ou compacte) ; Q32 ;
  ADR 0020 (jetons), 0021 (mise en page) ; tâche PLAN.md 5.3

## Contexte

La spec veut une densité réglable, confortable ou compacte. Le système de design n'avait que
des jetons d'espacement compacts, que rien n'activait, et la mise en page de la carte fixe la
taille des boîtes : le CSS doit donner les mêmes. Tu as demandé de la faire en Phase 5 (Q32).

## Décision

- **Réglage** `timonier.map.density` : « comfortable » (par défaut, la carte d'avant) ou
  « compact ». L'extension l'envoie à la page à son ouverture, avant la vue, et à chaque
  changement (message `density`).
- **Mesures** : un jeu par densité dans `webview/src/map/layout.ts` (`METRICS`) : taille des
  boîtes et des cartes de rôle, hauteur des en-têtes, place des broches, écarts entre boîtes
  et entre couches, marge des cadres. Compact : boîtes de 184 × 80 au lieu de 208 × 88, écarts
  de 16 et 48 au lieu de 24 et 64.
- **CSS** : l'attribut `data-density` de la page active les jetons compacts (`--tim-space-*`,
  et les mesures de la carte `--tim-map-*`), qui donnent les mêmes hauteurs que la mise en
  page.
- Changer de densité refait la mise en page d'un coup : toutes les tailles ont changé. Les
  boîtes déplacées à la main gardent leurs positions (ADR 0021).

## Alternatives écartées

- **Mettre la carte à l'échelle** : le texte rétrécirait avec les boîtes, et le contraste
  comme la lisibilité en pâtiraient ; la densité resserre l'espace, pas le texte.
- **Une commande de palette pour basculer** : c'est un réglage, que VSCode offre déjà dans ses
  paramètres, pas une action sur la carte.

## Conséquences

- Les tests d'accessibilité vérifient la carte compacte dans les six thèmes capturés, et
  qu'aucune ligne ne déborde de sa boîte.
- Une boîte compacte coupe plus tôt un nom long : le nom entier reste dans le détail et dans
  le nom accessible.
