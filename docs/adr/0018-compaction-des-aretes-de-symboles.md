# 0018 — Compaction des arêtes de symboles

- Statut : Accepté (2026-10-16, validation de la phase par l'humain)
- Date : 2026-10-16
- Phase : 3
- Liens : spec « Décisions techniques » (granularité des `Calls`) ; ADR 0009, 0014 ; tâche
  PLAN.md 3.10

## Contexte

La spec décide une « compaction au-delà d'environ 50 000 arêtes de symboles », sans dire ce
qu'on compacte. Les arêtes de symbole à symbole viennent de l'affinage LSP (ADR 0014), et
leur nombre croît avec les fichiers vérifiés. Sur un gros dépôt, une session longue en ouvre
des centaines. La carte agrège de toute façon au niveau du zoom. Le détail fonction →
fonction ne sert que pour les fichiers qu'on regarde.

## Décision

- **Budget** : 50 000 arêtes `Calls` de symbole à symbole issues du LSP dans la tête du graphe.
- **Ce qu'on compacte** : quand un affinage fait dépasser le budget, on compacte d'abord les
  fichiers vérifiés depuis le plus longtemps. Leurs appels fonction → fonction deviennent
  des appels fichier → fichier pondérés :
  - la provenance reste `Lsp` ;
  - le fichier reste vérifié (`verifying([Calls])`), au même hash ;
  - la précision n'est perdue qu'en profondeur : la carte agrégée ne change pas.
- **Hystérésis** : on compacte jusqu'à 90 % du budget, pour ne pas recommencer à chaque
  affinage.
- **Retour au détail** : un fichier compacté qu'on rouvre ou qu'on affiche à nouveau est
  affiné au niveau des symboles. Il redevient le plus récent, et d'autres sont compactés à sa
  place si besoin.
- Les arêtes `Implements` et `Inherits` de tree-sitter ne comptent pas : elles sont peu
  nombreuses.

## Alternatives écartées

- **Effacer les arêtes LSP des fichiers anciens** : le fichier redeviendrait non vérifié, et
  les appels tree-sitter, moins précis, reviendraient.
- **Compacter l'historique** (les versions fermées) : c'est un autre sujet, le volume de la
  base, et la spec parle des arêtes du graphe.
- **Ne garder le détail que pour les fichiers ouverts** : un fichier affiché sur la carte
  mérite aussi son détail. Le budget laisse la place à des milliers de fichiers.

## Conséquences

- Le pipeline garde, pour chaque fichier vérifié, ses appels avec le fichier de chaque
  appelé, et l'ordre des vérifications.
- Le seuil est une constante, à revoir avec les mesures de la Phase 8.
- Après un redémarrage, les vérifications sont relues de la base (ADR 0014), mais leur ordre
  ne s'y trouve pas. Elles comptent comme également anciennes, dans l'ordre de leurs chemins.
  Celles qui suivent sont plus récentes.
