# 0019 — Protocole entre le démon et l'extension

- Statut : Accepté (2026-10-18, validation de la phase par l'humain)
- Date : 2026-10-16
- Phase : 4
- Liens : spec « Architecture technique », « Décisions techniques » (protocole) ; ADR 0003,
  0011, 0014 ; H8 ; tâches PLAN.md 4.1 à 4.5

## Contexte

La spec décide du protocole :
- JSON-RPC 2.0 sur un socket local (Unix, ou named pipe sous Windows) entre le démon et
  l'extension, puis un relais postMessage vers la webview ;
- des abonnements par vue, avec des deltas versionnés, et un snapshot complet en cas de
  désynchronisation ;
- le serveur MCP comme mince proxy vers le même démon ;
- JSON d'abord ;
- plus tard, le même protocole sur un WebSocket local avec jeton, pour Zed ou une application
  autonome.

Il reste à fixer :
- où vit le socket, et comment on garantit un démon par projet ;
- les méthodes et leur versionnement ;
- comment les types sont partagés entre Rust et TypeScript ;
- la bibliothèque côté Rust.

Vérifié le 2026-10-16 :
- jsonrpsee 0.26.1 (MIT) ;
- ts-rs 12.0.1 (MIT) ;
- vscode-jsonrpc 9.0.3 (MIT), qui fournit `SocketMessageReader`/`SocketMessageWriter` avec le
  cadrage `Content-Length` du protocole de base LSP ;
- `std::fs::File::try_lock`, stable depuis Rust 1.89, et donc dans la MSRV 1.96.

## Décision

**Un démon par projet**
- Le projet est identifié par le chemin canonique de sa racine. Les 16 premiers caractères
  hexadécimaux d'un hash BLAKE3 de ce chemin donnent le nom de ses fichiers d'exécution :
  - sous Linux, dans `$XDG_RUNTIME_DIR/timonier/`, sinon dans `timonier/run/` du dossier de
    cache de l'utilisateur (`$XDG_CACHE_HOME`, ou `~/.cache`). Un `/tmp/timonier-<uid>/`
    partagé est écarté : un autre utilisateur pourrait le créer avant et bloquer le démon ;
  - sous macOS, dans `$TMPDIR/timonier/` ;
  - `TIMONIER_RUNTIME_DIR` remplace ce dossier : pour les tests, ou une session sans
    `$XDG_RUNTIME_DIR`.
- Le dossier est créé en `0700` : seul l'utilisateur peut s'y connecter. Un dossier existant
  qui appartient à un autre utilisateur, ou que d'autres peuvent ouvrir, est refusé.
- `<hash>.sock` est le socket ; `<hash>.lock` est un verrou exclusif (`File::try_lock`), tenu
  par le démon toute sa vie.
- Un second démon qui ne prend pas le verrou s'arrête aussitôt. Un socket sans verrou tenu est
  périmé : le démon qui démarre le remplace.
- À l'arrêt, le démon supprime son socket, puis libère le verrou. Le fichier de verrou reste :
  le supprimer laisserait deux démons verrouiller deux fichiers différents.
- Le chemin court évite la limite de 104 octets des sockets Unix sous macOS, qu'un chemin dans
  le dépôt dépasserait vite.
- La base reste dans le dépôt, `.timonier/graph.sqlite` (spec « Initialisation » : « base
  SQLite ignorée »). Le démon écrit `.timonier/.gitignore` (`graph.sqlite*`, avec les fichiers
  du WAL) s'il manque. Un `.gitignore` existant est celui de l'utilisateur : il n'y touche pas.
- Le démon crée ainsi `.timonier/` dans tout dépôt où il sert, avant tout `timonier init`.
  L'humain garde ce comportement pour le MVP (Q18, 2026-10-16) : la Phase 6 jugera un dépôt
  « configuré » d'après ses fichiers de suivi, pas d'après le dossier.
- Windows (named pipe) attend H8.

**Cycle de vie**
- L'extension ou `timonier daemon start` lance `timonier daemon run` détaché, après avoir vérifié
  le verrou. `stop` le demande au démon par `daemon/shutdown`.
- Le démon s'arrête seul après 15 minutes sans aucun client (extension, CLI, proxy MCP), et
  sur SIGTERM ou SIGINT.
- L'extension relance un démon perdu, avec un délai qui double à chaque échec (de 1 s à 30 s).
  Elle se réabonne ensuite : chaque vue reçoit un snapshot complet.
- Un arrêt demandé (`tim daemon stop`) pendant qu'une fenêtre est ouverte ne se distingue pas
  d'un plantage : l'extension relance le démon. L'humain garde ce comportement pour l'instant
  (Q17, 2026-10-16).

**CLI** (`timonier daemon start|stop|status|run`, messages en anglais selon l'ADR 0006)
- Sans `--root`, le projet est le dossier le plus proche, en remontant depuis le dossier
  courant, qui contient `.timonier/` ou `.git` ; sinon, le dossier courant. Une commande lancée
  dans un sous-dossier vise ainsi le démon du projet. L'extension passe `--root` explicitement.
- `run` sert au premier plan et écrit son journal sur la sortie d'erreur (`RUST_LOG` le règle).
  Si un démon sert déjà le projet, il le dit et s'arrête sans erreur.
- Une écriture du journal ne peut pas échouer : si le processus qui lisait la sortie d'erreur
  est parti, les lignes sont perdues, et le démon continue. Sans cela, `tracing-subscriber`
  signalait l'échec par `eprintln!`, qui panique sur un tube fermé : le démon mourait avec la
  fenêtre qui l'avait lancé (test `run_outlives_the_process_that_read_its_output`).
- Une fois le socket à l'écoute, `run` écrit une ligne JSON sur sa sortie standard :
  `{"socket": "…", "pid": 1234}`. `pid` vaut `null` quand un autre processus sert. L'extension
  lit cette ligne, car elle ne sait pas calculer le nom du socket : Node n'a pas BLAKE3.
- `start` lance `run` dans son propre groupe de processus, puis attend que le démon réponde à
  `initialize` (15 s au plus). Le journal va dans `<hash>.log`, à côté du socket, vidé à
  chaque lancement.
- `stop` demande `daemon/shutdown`, puis attend que le verrou soit libéré.
- `status` affiche l'état et sort avec le code 0 si un démon répond, sinon avec le code 3
  (comme le `status` des scripts d'init LSB). `start` et `stop` sans objet ne sont pas des
  erreurs.

**Transport** : JSON-RPC 2.0, messages encadrés par `Content-Length` comme le protocole de base
LSP, dans les deux sens. Le démon envoie aussi des requêtes à l'extension, pour le relais LSP
(Q5).

**Méthodes** (version de protocole 2 ; la 1 n'avait pas `lsp/definitions` ni `lsp/references`)

| Méthode | Sens | Rôle |
| ------- | ---- | ---- |
| `initialize` | client → démon | Versions du protocole et du démon, capacités, extensions des fichiers que lisent les packs (ADR 0022). Une version majeure différente est refusée avec un code d'erreur stable |
| `daemon/status`, `daemon/shutdown` | client → démon | État (racine, dernière indexation, file d'affinage et tâches gardées de côté, questions auxquelles le relais a répondu, recherches de dépendants), arrêt |
| `view/subscribe` | client → démon | Une vue (niveau de zoom, nœuds dépliés) ; réponse : un identifiant et un snapshot à la révision `r` |
| `view/update` | démon → client | Notification : un delta de `r` à `r + 1` |
| `view/resync` | client → démon | Le client a vu un trou dans les révisions : un snapshot complet |
| `view/unsubscribe` | client → démon | Fin de la vue |
| `editor/opened`, `view/displayed` | client → démon | Fichiers ouverts ou affichés : priorités d'affinage (ADR 0014) |
| `editor/saved` | client → démon | Notification : les documents du dépôt sont de nouveau ceux du disque ; ce qui attendait leur enregistrement peut être demandé (ADR 0022) |
| `lsp/outgoingCalls` | démon → client | Le relais des serveurs de l'IDE : les appels d'une fonction (ADR 0014) |
| `lsp/definitions` | démon → client | Où sont définis les noms à ces positions d'un fichier, en un lot ; chaque question dit si tree-sitter place la définition ailleurs (ADR 0022) |
| `lsp/references` | démon → client | Les fichiers qui utilisent le symbole nommé à une position (ADR 0022) |

- Une vue est un graphe agrégé : les nœuds visibles, et les arêtes agrégées entre eux
  (`Reader::aggregate`, ADR 0009).
- Un delta liste les nœuds et arêtes ajoutés, retirés ou modifiés.
- Le client garde la dernière révision reçue. Un delta dont la base n'est pas la sienne
  déclenche `view/resync`.
- Les erreurs portent des codes stables : ceux de JSON-RPC, puis ceux du démon :
  - `-32000` : `initialize` d'abord ;
  - `-32001` : version refusée, avec les versions du démon dans `data` ;
  - `-32002` : abonnement inconnu sur cette connexion ;
  - `-32003` : le démon s'arrête.

  Le client qui relaie répond avec deux codes de LSP : `-32803` (`RequestFailed`), pas de
  serveur ou pas encore prêt ; `-32801` (`ContentModified`), ses documents ne sont pas ceux du
  disque (ADR 0022). Le fichier TypeScript généré les exporte (`ERRORS`).
- Un message illisible reçoit une erreur d'id `null`. Une requête mal formée dont l'id se lit
  reçoit son erreur sous cet id. Une notification inconnue, par exemple `$/cancelRequest`, est
  ignorée.

**Vues**
- Le haut de la carte, ce sont les enfants du dépôt. Un nœud déplié montre ses enfants, s'il
  est lui-même affiché ; un nœud déplié sous un nœud replié reste caché.
- Chaque abonnement numérote ses révisions à partir de 0. Un delta n'est envoyé que si la vue
  a changé, avec une exception : le delta vide qui signale une perte (voir plus bas).
- Le client applique un delta dont la base est sa révision. Il ignore un delta plus ancien :
  il a été envoyé avant le snapshot que le client tient. Un delta plus récent signale un trou :
  le client demande `view/resync`, qui renvoie la vue entière à la révision courante.
- Un abonnement appartient à sa connexion ; une autre connexion ne peut ni le relire, ni le
  fermer.
- Un fil dédié tient toutes les vues. À chaque changement du graphe, il relit la tête une fois
  pour toutes les vues, recalcule chacune, et la compare à la dernière envoyée. Il envoie aussi
  les réponses à `view/subscribe`, `view/resync` et `view/unsubscribe` : un snapshot passe
  toujours avant les deltas qui le suivent.
- Rien n'attend un client lent. Chaque client a une file de 1 024 messages :
  - un delta qui n'y tient pas est perdu, et le client se resynchronise ;
  - une réponse qui n'y tient pas ferme la connexion.
- Une perte se signale même si le graphe ne change plus. Quand la file du client se vide,
  chaque vue qui a perdu un delta lui envoie un delta vide : sa base est la révision que le
  client n'a pas reçue, et il fait avancer la révision. Le client y voit un trou et demande
  `view/resync`. Un client du protocole (l'extension, le futur proxy MCP) doit donc accepter
  un delta sans changement.

**Types partagés** :
- les types du protocole vivent dans `timonier-core` (ADR 0003, « protocol types ») et
  dérivent `ts_rs::TS`. Les messages du relais entre l'extension et la webview y vivent
  aussi (`HostMessage`, `WebviewMessage`) : tous les messages ont une seule source ;
- un test génère `generated.ts` et échoue si le fichier du dépôt diffère : le TypeScript ne
  peut pas dériver du Rust sans que la CI le voie ;
- il en existe deux copies identiques, `webview/src/protocol/generated.ts` et
  `editors/vscode/src/protocol/generated.ts`. Chaque paquet compile ainsi ses propres sources :
  le `rootDir` de l'extension n'inclut pas la webview. Prettier ignore ces deux fichiers.

**Extension** (tâche 4.4)
- Le démon sert le premier dossier de la fenêtre. Il est lancé à l'activation de l'extension :
  vue ou commande Timonier, ou dossier qui contient déjà `.timonier/`. Un dépôt qui n'a jamais
  utilisé Timonier n'est donc pas indexé à son ouverture : la spec y propose `timonier init`.
- Le programme : `$TIMONIER_DAEMON_PATH`, sinon le réglage `timonier.daemon.path`, sinon
  `timonier` dans le `PATH`. Le journal du démon va dans le canal de sortie « Timonier ».
- Le réglage ne se lit que dans les réglages de l'utilisateur (portée `machine`) : un dossier
  ne peut pas choisir, par son `.vscode/settings.json`, le programme que l'extension lance.
  L'extension ne s'active pas dans un dossier non approuvé (`untrustedWorkspaces`).
- Une perte (connexion fermée, lancement raté) relance après 1 s, puis 2, 4… jusqu'à 30 s. Une
  connexion qui a tenu 30 s remet le délai à 1 s. La commande « Relancer le démon » relance tout
  de suite.
- Le démon est lancé détaché (son propre groupe de processus) : la fenêtre fermée, il continue
  pour ses autres clients, puis s'arrête seul.
- Chaque carte a son relais. La webview demande une vue, et le relais s'y abonne. Il transmet
  le snapshot, puis les deltas de cet abonnement. Après une relance, il se réabonne et envoie
  la vue entière.
- Le relais traite ses opérations une à une : abonnement, resynchronisation, fermeture. Chacune
  part de l'abonnement que la précédente a laissé, et il ferme celui qu'il remplace. Sinon,
  deux demandes rapprochées laissaient un abonnement ouvert, que le démon recalculait à chaque
  changement.
- Une resynchronisation qui échoue se transforme en nouvel abonnement : la webview attend un
  snapshot, et sans lui elle laisserait passer tous les trous suivants.
- La webview applique les règles de révision ci-dessus. Elle ne demande qu'une
  resynchronisation à la fois, et dit au relais ce qu'elle montre (`rendered`).
- Le relais n'en retient que ce qui vaut pour l'abonnement courant, à une révision qu'il a
  déjà transmise. Les numéros d'abonnement repartent de 1 à chaque démon : un `rendered`
  tardif de l'ancien porterait le même numéro. Une page masquée, qui perd sa page, efface ce
  qu'elle montrait.
- Elle dit aussi quels fichiers la carte dessine (`displayed`), quand ils changent. Le relais
  les transmet par `view/displayed`, et les redit à chaque nouvelle connexion : ils passent en
  premier pour l'affinage (ADR 0014).
- Le relais vérifie chaque message de la webview, champs compris. « Aller au code » n'ouvre
  qu'un fichier du dossier du projet.

**Relais des serveurs de langage de l'IDE** (tâche 4.5, complète l'ADR 0014)
- Le client qui dit `lspRelay` à `initialize` relaie. S'il y en a plusieurs, c'est le dernier.
- L'affinage lui envoie `lsp/outgoingCalls` (fichier, position du nom), puis
  `lsp/definitions` et `lsp/references` (ADR 0022), sans jamais attendre le client :
  - la question part sans attendre de place dans sa file. Si la file est pleine, la question
    échoue : le client ne lit plus ;
  - la réponse est attendue 10 s au plus, et 50 ms de plus par position d'un lot de
    définitions, par tranches de 20 ms. Entre deux tranches, l'attente
    cède dès qu'un autre travail arrive (une sauvegarde, une demande d'affinage) : le fichier
    retourne en file, et l'indexation passe d'abord ;
  - pour cela, le trait `LanguageServer` de l'ADR 0014 gagne `outgoing_calls_or_yield`, dont
    la version par défaut ne cède pas, et l'erreur `Interrupted` ;
  - une question à laquelle le démon renonce (cédée, ou sans réponse dans le délai) est
    annulée par `$/cancelRequest`, comme en LSP.
- Quand le client qui relaie part, le précédent encore connecté reprend le relais.
- Le trait `LanguageServer` gagne `ready()`. Sans client qui relaie, l'affinage attend et garde
  sa file. Le client qui arrive la relance. Une question en attente quand le client part, ou
  quand un autre prend sa place, rend sa tâche à la file : rien n'est perdu (relecture n° 6,
  m6-3). Un client dont la connexion n'écrit plus, même s'il lit encore, cesse de relayer :
  sans cela, la tâche lui reviendrait sans fin, et le démon tournerait à vide (relecture n° 7,
  I7-1). Si elle cesse d'écrire après qu'une question est partie, l'attente le voit et rend la
  tâche aussitôt, sans attendre le délai (relecture n° 8, m8-1). Le pipeline voit lui-même que
  le client qui relaie a changé, et redemande ce que le précédent avait refusé (m8-2).
- L'extension convertit les colonnes UTF-8 du modèle en UTF-16 pour VSCode, et
  inversement. Elle répond par `vscode.prepareCallHierarchy` puis `vscode.provideOutgoingCalls`.
  Un appelé hors du dossier a un chemin `null`.
- Pas de hiérarchie d'appels à la position (pas de serveur pour ce langage, ou serveur pas
  encore prêt) : c'est une erreur, pas « aucun appel ». Le fichier reste non vérifié.
- L'extension ne répond que sur le contenu enregistré : sinon `ContentModified`, et le démon
  garde la tâche de côté jusqu'à `editor/saved`, un autre client qui relaie, ou un fichier qui
  change sur le disque (ADR 0022, « Le contenu enregistré seulement »).
- L'extension envoie `editor/opened` pour les fichiers des éditeurs visibles, à la connexion
  et à chaque changement.

**Implémentation dédiée côté Rust**
- tokio, `UnixListener`, le cadrage `Content-Length` et un répartiteur par méthode, sans
  jsonrpsee.
- Les messages JSON-RPC et le cadrage vivent dans `timonier-core` (`rpc`). Ils sont purs, sans
  entrée-sortie : le serveur du démon et le futur proxy MCP les partagent.
- Le démon fournit aussi un client bloquant, pour la CLI et les tests d'intégration.
- Le pipeline d'indexation garde son fil (ADR 0013) ; le serveur lui parle par des canaux et
  lit la base avec ses propres lecteurs.

## Alternatives écartées

- **jsonrpsee** : ses serveurs sont HTTP et WebSocket. Un socket Unix demanderait un transport
  sur mesure ou `RpcModule::raw_json_request`, et le cadrage LSP de vscode-jsonrpc ne se branche
  pas dessus. Pour une dizaine de méthodes, il apporte hyper, tower et soketto sans rien
  simplifier.
- **Un socket dans `.timonier/`** : la limite de 104 octets de macOS, et un fichier spécial dans
  le dépôt.
- **Un port TCP local** : n'importe quel processus de la machine pourrait s'y connecter. Il est
  réservé, avec un jeton, au WebSocket de Zed ou de l'application autonome (plus tard).
- **Des types TypeScript écrits à la main** : ils dériveraient du Rust sans que rien ne le voie.
- **specta** : encore en release candidate (2.0.0-rc.25).

## Conséquences

- Le démon devient asynchrone (tokio), mais le pipeline reste synchrone, sur son fil.
- Chaque changement relit toute la tête, ce qui coûte 3 ms sur Timonier. Ce coût croît avec
  le graphe : la Phase 2 mesurait 0,63 s d'agrégation à 200 000 symboles. Une lecture
  incrémentale de la tête reste à faire si les mesures de 4.10 ou de la Phase 8 l'exigent.
- Ajouter une méthode, c'est ajouter un type dans `timonier-core`, régénérer les types
  TypeScript, et ajouter un test d'intégration client ↔ démon.
- Le proxy MCP de la Phase 6 parlera ce même protocole.
