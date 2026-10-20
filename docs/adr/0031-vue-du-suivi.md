# 0031 — Vue du suivi : plan, todo et journal dans l'IDE

- Statut : Proposé
- Date : 2026-10-20
- Phase : 6
- Liens : spec « Pilotage », « Expérience et design » ; ADR 0019 (protocole du démon), 0024
  (diff de revue), 0029 (formats du suivi), 0030 (dette de revue) ; tâche PLAN.md 6.6

## Contexte

La spec veut le plan en panneau latéral permanent, la todo en direct et un journal des sessions
avec, pour chacune, objectif, tâches, composants touchés, diff structurel et tests. L'ADR 0029
fixe les fichiers et dit que le diff et les composants touchés d'une session se calculent entre
ses deux snapshots ; il ne dit ni comment l'IDE les reçoit, ni avec quoi le diff se calcule :
le diff de revue (ADR 0024) compare le contenu de chaque fichier, que le magasin ne garde que
pour la dernière revue.

## Décision

**Protocole** (version 5) : `tracking/subscribe` répond l'état entier du suivi
(`TrackingSnapshot { subscription, revision, tracking }`) ; ensuite, à chaque changement,
`tracking/update` envoie l'état entier à la révision suivante. Pas de delta : l'état tient en
quelques dizaines de kilo-octets. `view/unsubscribe` vaut pour les deux sortes d'abonnement.
Un envoi perdu (file du client pleine) est refait quand la file s'est vidée.

**L'état** (`TrackingView`), en camelCase comme le reste du protocole, tiré des fichiers par le
démon : les jalons avec leur avancement **calculé par le démon** (même règle que `get_plan`) et
leurs tâches ; la todo de la session ouverte ; le journal, **les 50 sessions les plus récentes**
d'abord, et leur nombre total ; les fichiers qui ne se lisent pas, avec la raison.

**Quand** : le fil des vues refait l'état après chaque `tools/call` (le serveur le prévient),
après chaque changement du graphe (la session ouverte a son diff en direct), et quand un fichier
de `.timonier/` change hors du démon (à la main, un `git checkout`) : tant qu'un abonnement
existe, un fil regarde chaque seconde la taille et la date de `plan.json`, `todo.json` et de
`sessions/`. Le watcher du code ignore `.timonier/`, et une seconde suffit pour ce qui ne vient
pas des outils.

**Diff d'une session** : celui de la revue (ADR 0024, `review::compare`), entre le snapshot du
début et celui de la fin, ou la tête pour la session ouverte. Le magasin garde le contenu des
fichiers aussi pour ces snapshots (`snapshot_content`, sans migration : la table a déjà `seq`) ;
la fin de session écrit le diff dans le journal (`changes` : fichiers changés, boîtes ajoutées,
modifiées, retirées, dépendances ajoutées et retirées) et oublie ces contenus. **Composants
touchés** : les boîtes (paquets, composants) ajoutées, modifiées ou retirées. Une revue n'efface
plus que les contenus de la revue précédente.

**Tests** : rien à montrer avant `timonier test` (Phase 8) ; le journal le dit.

**Interface** : une vue webview, « Plan, todo et sessions », servie par une seconde page de la
webview (`tracking.html`, construite à part : sous la CSP par nonce, une page ne charge qu'un
script), sans React Flow. Par tes demandes (tests des cinq secondes, 2026-10-20) : elle est
seule à droite, dans le conteneur Timonier de la barre latérale secondaire
(`contributes.viewsContainers.secondarySidebar`, id `timonier-journal`) ; la vue d'ensemble
reste à gauche. Deux colonnes collées, le plan et la todo puis les sessions, chacune sous son
titre, quand la barre a 36 rem de large au moins (requête de conteneur) ; l'une sous l'autre
sinon. Ce point de contribution est lu par VSCode 1.141 et 1.142 (vérifié dans leur code et par
le test d'intégration, 2026-10-20) ; la documentation des contributions ne le cite pas encore,
et l'extension demande `^1.140.0` : à revoir si une 1.140 le refuse.

## Alternatives écartées

- **Des deltas comme pour la carte** : de la complexité (révisions, resync) pour un état petit.
- **Écrire le diff au fil de la session** : il dépend de la tête, il change à chaque sauvegarde ;
  seul le diff final est un fait du journal.
- **Contenus fictifs (un même hachage partout sauf les fichiers du diff de snapshots)** : un
  fichier renommé sans changement serait vu comme retiré puis ajouté (Q40).
- **Une arborescence native (`TreeView`)** : ni barre de progression, ni mise en page du journal.
- **Trois vues, Plan et Todo à gauche, Sessions à droite** (essayé) : les deux barres latérales
  sont aux deux bords de la fenêtre, la carte entre elles ; tu voulais les colonnes collées.

## Conséquences

- `snapshot_content` garde au plus les contenus de la dernière revue et ceux de la session
  ouverte (et de sessions abandonnées : effacés à la prochaine fin de session).
- Le journal d'une session finie ne se recalcule jamais : un `changes` absent (session finie
  avant cette décision) reste absent.
