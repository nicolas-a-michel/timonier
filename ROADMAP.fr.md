# Feuille de route

[English](ROADMAP.md) · **Français**

Ce que Timonier fera ensuite, dans l'ordre. Aucune date n'est promise : chaque étape se termine
quand son critère d'acceptation est vérifié sur un vrai projet ([PLAN.md](PLAN.md) les donne).
Traduction de la version anglaise du 22 octobre 2026.

## Avant la préversion

Les phases 0 à 8 du MVP sont faites, et la phase 10 a donné à Timonier ses treize langages. Il
reste, avant une première version publique :

- **Windows sur une vraie machine.** Les tests passent sous Windows en CI ; un essai sur une vraie
  machine y confirme l'extension et le démon.
- **Préversion sur le Marketplace VSCode**, pour Linux, macOS et Windows (x64 et arm64).

## La fin du MVP

- **Assistant « Nouveau projet piloté »** : démarrer un projet depuis VSCode avec ses agents, ses
  conventions et son tableau blanc déjà en place.
- **Relecture complète du MVP** face à la spécification ([docs/spec.md](docs/spec.md)).
- **Critère de sortie** : un vrai projet piloté de bout en bout sans en lire le code ligne à
  ligne.

## Ensuite (v2 : la profondeur)

- **Gestionnaire de skills** : voir, installer et mettre à jour, depuis l'IDE, les skills et les
  instructions que chaque agent utilise dans un projet (skills de Claude Code, instructions de
  GitHub Copilot et de Cursor, Codex), et les partager entre projets.
- **Indexation SCIP**, pour des références exactes dans les langages qui ont un indexeur.
- **Profilage de niveau 2** : où passe le temps à l'intérieur d'un test, au-delà de ses spans.
- **Surfaces publique et de sécurité** : ce qu'un composant expose, et où une entrée venue de
  l'extérieur l'atteint.
- **Plusieurs agents à la fois** : couloirs, réservation de composants, worktrees git.
- **Collaboration entre modèles** : topologies, protocole, canaux, notifications et supervision.
- **Couverture, santé et impact** par composant, et **voyage dans le temps** à travers les
  instantanés de la carte.
- **Animation des flux de données** sur la carte.
- **GitLab, Jira et Linear** pour le processus de développement, à côté de GitHub.

## Plus tard (v3 : les équipes)

- L'instantané et le diff structurel de la carte calculés en CI pour chaque pull request, avec un
  commentaire automatique.
- Règles et intention relues en pull request.
- Journal des sessions et dette de relecture partagés, pour une équipe de 3 à 5 personnes sur un
  même dépôt.

## Pas prévu pour l'instant

- **Zed** : ses extensions n'offrent pas encore les panneaux dont Timonier a besoin ; à revoir.
