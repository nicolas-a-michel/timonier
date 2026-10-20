# 0028 — En-tête de boîte aux lettres

- Statut : Accepté (2026-10-20, validation de la Phase 6 par l'humain)
- Date : 2026-10-20
- Phase : 6
- Liens : spec « Collaboration entre modèles » (en-tête de boîte aux lettres) ; H10 ;
  ADR 0027 ; skill `mcp-tools` ; tâche PLAN.md 6.2

## Contexte

La spec veut que chaque réponse d'un outil MCP de Timonier porte le nombre de messages non lus
et leur priorité, pour que tout agent le voie à son prochain appel. La messagerie est en v2
(H10) : en Phase 6, la boîte est toujours vide, mais l'en-tête doit déjà être là, et sa forme ne
plus changer.

## Décision

- **Forme structurée** : chaque résultat d'outil, erreur comprise, est un objet
  `{ "mailbox": { "unread": 0, "priority": null }, … }`. `unread` est un entier positif ou nul ;
  `priority` vaut `null` sans message non lu, sinon la plus haute des non lus : `low`, `normal`,
  `high` ou `blocking` (les messages bloquants de la spec).
  - Succès : `{ "mailbox": …, "result": … }`.
  - Erreur : `{ "mailbox": …, "error": { "code": "no_open_session", "message": "…" } }`, codes
    en snake_case, stables.
- **Forme texte** : le premier bloc de texte du résultat MCP commence par une ligne
  « Mailbox: 0 unread. » (ou « Mailbox: 2 unread, highest priority high. »), puis le résumé
  d'une ligne de l'outil. Un agent qui ne lit que le texte voit l'en-tête aussi.
- **Qui le calcule** : le démon, dans `tools/call` (ADR 0027) ; le proxy le recopie sans le
  toucher.
- En Phase 6, `unread` vaut toujours 0 et `priority` `null`. Un test vérifie sa présence sur
  chaque outil, en succès comme en erreur.

## Alternatives écartées

- **Le champ `_meta` de MCP** : les agents ne le montrent pas toujours au modèle ; la spec veut
  que l'agent le voie.
- **Un outil `read_inbox` seul** : l'agent devrait penser à l'appeler ; la spec veut l'en-tête
  partout.

## Conséquences

- La v2 remplira l'en-tête sans changer sa forme ni les outils.
