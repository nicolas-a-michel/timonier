# Feuille de route

[English](ROADMAP.md) · **Français**

Ce que Timonier fera ensuite, dans l'ordre. Timonier concentre ses efforts sur deux choses :
l'architecture du code et ses performances. Aucune date n'est promise : chaque étape se termine
quand son critère d'acceptation est vérifié sur un vrai projet ([PLAN.md](PLAN.md) les donne).
Traduction de la version anglaise du 22 octobre 2026.

## La fin du MVP

- **Relecture complète du MVP** face à la spécification ([docs/spec.md](docs/spec.md)).
- **Critère de sortie** : un vrai projet piloté de bout en bout sans en lire le code ligne à
  ligne.

## Ensuite : une carte fiable

- **Indexation SCIP**, pour des références exactes dans les langages qui ont un indexeur.
- **Le degré de certitude de chaque boîte et de chaque lien** : la part de ses liens qu'un
  manifeste, un serveur de langage ou SCIP a confirmés, montrée sur la carte, pour qu'une carte
  approchée le dise.
- **Renommer une boîte** : le développeur donne son propre nom à une boîte de la carte (F2 ou
  double clic), conservé dans `.timonier/` à travers les réindexations et les commits ; les
  agents peuvent proposer des noms par un outil MCP, et le nom du développeur l'emporte.
- **Chaque langage au même endroit.** Aujourd'hui, un langage est son pack (grammaire,
  requêtes, règles) plus du code réparti dans le démon, la ligne de commande et le cœur
  (manifestes, inventaire, éditions de `timonier init`, ce qui est public). Les règles qui sont
  des données passent dans le pack ; le reste va dans un module par écosystème derrière une
  seule interface, pour qu'ajouter un langage ne touche jamais le démon. Rien ne change pour
  l'utilisateur.

## Puis : les performances en profondeur

- **Mémoire par composant.** Le module d'instrumentation que Timonier ajoute compte les octets
  alloués pendant chaque span, et le calque Performances de la carte montre la part de chaque
  composant, comme il montre sa part du temps. D'abord Rust, Java, Kotlin, Scala, C# et Python,
  dont l'environnement compte les allocations par fil d'exécution ; sur les boîtes des autres
  langages, la carte dit que leur mémoire n'est pas mesurée.
- **Tests et performances dans le journal de session** : chaque session d'agent dit quels
  tests ont tourné et comment les performances ont bougé, à côté des budgets qu'elle montre déjà.
- **Profilage de niveau 2** : où passe le temps à l'intérieur d'un test, au-delà de ses spans.
- **Modifié mais non testé** : dans le diff depuis la dernière relecture, les symboles qu'aucun
  test n'a exécutés.

## Puis : l'architecture dans le temps

- **Ce qu'un commit a changé** : choisir un commit dans l'historique et voir sur la carte les
  composants, fichiers et liens qu'il a ajoutés, modifiés ou supprimés, comme le diff de
  relecture les montre depuis « Marquer comme revu ».
- **Croissance de chaque composant** : une petite courbe sur chaque boîte, de ses lignes, de ses
  symboles et de ses dépendances au fil des commits.
- **Raccourcis pris par les agents** : tests désactivés, assertions affaiblies, avertissements
  réduits au silence (`#[allow]`, `@ts-ignore`), ébauches et `TODO` laissés, blocs `catch`
  vides, montrés en badges sur les boîtes de la carte avec le commit et la session qui les ont
  introduits.
- **Code dupliqué** : une fonction réécrite alors qu'une autre au même corps existe déjà, reliée
  à elle sur la carte.
- **Code mort ajouté** : les nouveaux symboles que rien n'appelle ni ne référence, là où un
  serveur de langage a confirmé les liens.
- **Surfaces publique et de sécurité** : ce qu'un composant expose, et où une entrée venue de
  l'extérieur l'atteint.
- **Couverture, santé et impact** par composant, et **voyage dans le temps** à travers les
  instantanés de la carte.
- **Animation des flux de données** sur la carte.

## Plus tard

- **Gestionnaire de skills** : voir, installer et mettre à jour, depuis l'IDE, les skills et les
  instructions que chaque agent utilise dans un projet (skills de Claude Code, instructions de
  GitHub Copilot et de Cursor, Codex), et les partager entre projets.
- **Travail hors du périmètre d'une tâche** : chaque tâche du plan nomme les composants qu'elle
  doit modifier ; la carte marque les composants qu'une session a touchés en dehors.
- **Coût de l'IA par composant** : une couche de la carte qui montre où sont passés les tokens
  des agents.
- **Plusieurs agents à la fois** : couloirs, réservation de composants, worktrees git.
- **Collaboration entre modèles** : topologies, protocole, canaux, notifications et supervision.
- **GitLab, Jira et Linear** pour le processus de développement, à côté de GitHub.
- **Les équipes** : l'instantané et le diff structurel de la carte calculés en CI pour chaque
  pull request, avec un commentaire automatique ; règles et intention relues en pull request ;
  journal des sessions et dette de relecture partagés, pour une équipe de 3 à 5 personnes sur
  un même dépôt.

## Pas prévu pour l'instant

- **Zed** : ses extensions n'offrent pas encore les panneaux dont Timonier a besoin ; à revoir.
