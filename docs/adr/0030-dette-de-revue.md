# 0030 — Dette de revue

- Statut : Accepté (2026-10-20, validation de la Phase 6 par l'humain)
- Date : 2026-10-20
- Phase : 6
- Liens : spec « Pilotage » (dette de revue, non suivi) ; Q43, Q44 ; ADR 0010 (diff de
  snapshots), 0024 (revue), 0029 (sessions, non suivi) ; tâche PLAN.md 6.5

## Contexte

La spec veut une chaleur par composant selon la quantité de code écrit par l'IA et jamais
relu, en calque sur la carte, et les changements faits hors session marqués « non suivis ».
Tes réponses fixent la mesure (les fichiers changés, Q43) et l'affichage (un calque à activer,
trois paliers, Q44). Restent les fenêtres de temps, les paliers et le chemin jusqu'à la carte.

## Décision

**Depuis quand** : la dernière revue (`review_mark`, ADR 0024) ; sans revue, depuis le premier
snapshot. Ce qui a changé avant a été relu.

**Écrit en session** : pour chaque session (ADR 0029), les fichiers que le diff de ses
snapshots touche, entre son début (ou la revue, si elle est venue pendant) et sa fin (ou la
tête, si elle est ouverte). Seules comptent les fenêtres qui finissent après la revue.

**Non suivi** : de même entre deux sessions, d'après les écarts de `untracked.json`, et depuis
la fin de la dernière session jusqu'à la tête quand aucune n'est ouverte. Un fichier changé en
session et hors session est les deux.

Seuls comptent les fichiers encore à la tête.

**Par boîte** : le nombre de fichiers à relire qu'elle contient, sur ses fichiers ; et s'il y en
a de non suivis. Le démon le calcule à chaque lecture de la tête, comme le diff de revue, et le
met dans la vue : `ViewNode.debt { files, of, untracked }` (protocole version 4). La webview en
tire le palier, et additionne les membres d'un rôle fermé.

**Paliers** (Q44 en laisse le seuil) : au moins un fichier à relire ; palier 1 sous le tiers des
fichiers de la boîte, palier 2 sous les deux tiers, palier 3 au-delà. Une petite boîte entièrement
réécrite est au palier 3, une grande avec un fichier au palier 1.

**Coût** : une requête de diff par fenêtre (ADR 0010). Les fenêtres finies ne changent plus : le
fil des vues garde leurs fichiers, et ne refait que celle qui finit à la tête.

## Alternatives écartées

- **Compter dans les sessions au fil de l'eau** : un fichier écrit puis relu avant la fin de la
  session serait compté quand même ; le diff des snapshots dit ce qui a changé.
- **Paliers en nombre absolu de fichiers** : une boîte de trois fichiers et une de trois cents
  ne pèsent pas pareil devant un relecteur.

## Conséquences

- Marquer comme revu remet la dette à zéro, comme le diff de revue.
- Un commentaire seul ne fait pas de dette (il ne change ni nœud ni arête).
