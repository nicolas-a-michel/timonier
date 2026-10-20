# 0032 — Le pont : la vue d'accueil

- Statut : Proposé
- Date : 2026-10-20
- Phase : 6
- Liens : spec « Expérience et design » (le pont) ; H5 ; Q45, Q46, Q47 ; ADR 0024 (diff de
  revue), 0030 (dette de revue), 0031 (vue du suivi) ; tâche PLAN.md 6.7

## Contexte

La spec veut une vue d'accueil qui réponde en quelques secondes à trois questions : où en est
le plan, que s'est-il passé depuis ma dernière revue, où dois-je regarder. Tes réponses fixent
le classement de « Où regarder » (Q45 : les alertes, puis la dette), le clic (Q46 : la carte
s'ouvre sur la boîte) et la place (Q47 : la vue d'ensemble, à gauche). Restent le chemin des
données et la façon de montrer une boîte sur la carte.

## Décision

**Données** : l'état du suivi (ADR 0031) s'enrichit de ce que le fil des vues sait déjà de la
tête : le résumé du diff de revue, le total de la dette (le nœud du dépôt), le nombre de
sessions commencées depuis la revue, et « Où regarder », classé par le démon. Il est donc
recalculé à chaque changement du graphe tant qu'une vue du suivi est ouverte, plus seulement
pendant une session.

**« Où regarder »** (Q45), cinq éléments au plus, dans cet ordre :
1. les fichiers de `.timonier/` illisibles ;
2. les tâches bloquées ;
3. les cycles ajoutés depuis la revue ;
4. les boîtes (paquets, composants) en dette, les non suivies d'abord, puis par fichiers à
   relire. Une boîte dont toute la dette tient dans une de ses boîtes filles laisse sa place à
   celle-ci, plus précise.

Chaque boîte porte son chemin depuis le haut de la carte (ses ancêtres, sans le dépôt).

**Révéler une boîte** (Q46) : une commande de carte `reveal { target, path }`. La carte ouvre le
groupe de rôle du haut du chemin, déplie les boîtes du chemin (un abonnement de plus au démon),
puis, une fois la mise en page faite, centre la boîte et la sélectionne. La page du pont envoie
`reveal` à l'extension, qui ouvre ou montre la carte et lui passe la commande.

**Interface** (Q47) : la vue d'ensemble (`timonier.home`) devient une vue webview, servie par la
page du suivi (`tracking.html`, partie `bridge`) et le même relais. Elle garde le bouton
« Ouvrir la carte » et « Marquer comme revu ».

## Alternatives écartées

- **Classer dans la webview** : la règle vivrait loin des données (plan, diff, dette) et sans
  test Rust ; la webview ne voit pas l'arbre entier du graphe.
- **Une seconde souscription pour le pont** : il lit les mêmes fichiers et la même tête que les
  panneaux ; un état de plus à garder cohérent.

## Conséquences

- Le pont et les panneaux montrent toujours le même état, à la même révision.
- Protocole version 6 (champs du suivi, commande `reveal`, messages `reveal` et `openMap`).
