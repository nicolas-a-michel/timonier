# 0007 — Point d'entrée Timonier dans la barre d'activité

- Statut : Proposé
- Date : 2026-10-14
- Phase : 1 (demande de l'humain du 2026-10-14)
- Liens : spec « Pilotage » (plan en « panneau latéral permanent ») et « Expérience et
  design » ; guides UX VSCode « Activity Bar » et « Views », consultés le 2026-10-14

## Contexte

L'humain veut une icône Timonier dans VSCode pour ouvrir facilement les panneaux de gestion,
dont la carte. Les guides UX de VSCode posent trois limites :

- un élément de la barre d'activité ne doit pas ouvrir directement une webview dans l'éditeur ;
- un élément d'arbre ne doit pas servir de simple bouton qui lance une commande ;
- chaque vue doit avoir une icône, et le nombre de vues doit rester minimal.

## Décision

- **Un conteneur `timonier` dans la barre d'activité**, avec une icône de barre à roue dessinée
  pour le projet : SVG 24×24 d'une seule couleur (`currentColor`), comme le demande VSCode.
  - Six rayons plutôt que huit : rendue pixel par pixel à 24 px, la version à huit rayons
    ressemblait à un flocon.
- **Une seule vue native, « Vue d'ensemble » (`timonier.home`)**, avec son propre fournisseur
  de données.
  - Tant qu'elle n'a pas de données, VSCode y affiche un contenu d'accueil (`viewsWelcome`) :
    une phrase, le bouton « Ouvrir la carte » et l'annonce des panneaux à venir.
- **Un bouton de carte dans la barre de titre de la vue**, la place prévue par VSCode pour les
  actions d'une vue.
- **Plus tard**, cette vue accueillera le plan, la todo et le journal (Phase 6), sous forme de
  vrais arbres de données.

## Alternatives écartées

- **Une liste de liens (« Carte », « Plan »…) en éléments d'arbre** : contraire aux guides
  VSCode (éléments d'arbre utilisés comme boutons).
- **Une webview latérale** (le pont tout de suite) : les guides demandent d'en limiter
  l'usage, et le pont est prévu en Phase 6 avec de vraies données.
- **Ouvrir la carte d'un clic sur l'icône** : contraire aux guides VSCode, et cela empêcherait
  d'ajouter d'autres panneaux au même endroit.
- **Une icône existante (codicon)** : les guides demandent de ne pas dupliquer une icône
  existante dans la barre d'activité.

## Conséquences

- L'extension s'active à l'ouverture de la vue. Le coût est négligeable tant qu'elle ne fait
  qu'afficher l'accueil.
- Chaque nouveau panneau devra choisir sa place : un nœud de la vue d'ensemble, une action de
  barre de titre ou une commande. On ajoutera une seconde vue seulement si c'est vraiment
  nécessaire.
