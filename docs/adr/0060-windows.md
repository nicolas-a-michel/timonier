# 0060 — Timonier sous Windows

- Statut : Proposé
- Date : 2026-10-21
- Phase : 9
- Liens : spec « Technical decisions » (protocole démon ↔ extension) ; Q87 ; ADR 0019 (socket),
  0045 (mesure niveau 1), 0057 (dossier de données), 0059 (programme embarqué) ; tâche PLAN.md 9.9

## Contexte

Q87 : la préversion attend Windows. Le démon et ses trois clients (extension, proxy MCP, CLI)
parlaient par un socket Unix ; le dossier d'exécution était vérifié par son propriétaire et son
mode ; la mesure des tests lisait `getrusage` et les cgroups de systemd ; les commandes de test
passaient par `sh -c` ; le démon se détachait par un groupe de processus Unix. La spec a décidé
le canal : « JSON-RPC 2.0 sur un socket local (Unix ou named pipe) ». Le code du workspace
n'employait pas `unsafe` (ADR 0004).

## Décision

- **Canal** : sous Windows, le named pipe `\\.\pipe\timonier-<16 chiffres hex>`, le même nom
  dérivé de la racine que le socket. Côté démon, `tokio::net::windows::named_pipe` (une instance
  par client, la suivante créée dès qu'un client se connecte, ou à la place d'une instance qui
  échoue ; module `transport.rs`) ; côté client Rust bloquant, `NamedPipeClient` de tokio sur un
  runtime à lui (un pipe ouvert comme un simple fichier sérialise lectures et écritures : une
  lecture en attente bloquait la requête) ; côté proxy MCP, `ClientOptions` de tokio ; côté extension, `net.createConnection` de Node, qui
  ouvre les pipes. Le pipe n'existe qu'une fois sa première instance créée dans le runtime : les
  trois clients réessaient 5 s sur « pipe absent » ou « pipe occupé ».
- **Dossier d'exécution** : `%LOCALAPPDATA%\timonier\run` (le verrou, le journal) ; pas de
  vérification de propriétaire ni de mode, le dossier étant celui de l'utilisateur. Dossier de
  données : `%LOCALAPPDATA%\timonier`. Le dossier personnel (`%USERPROFILE%`) n'est pas un projet,
  comme `~` ailleurs.
- **Lancement en arrière-plan** : `CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW` ; Ctrl-C (et la
  fermeture de console) arrête le démon au premier plan.
- **Les handles du lanceur** : Windows donne à un nouveau processus tous les handles héritables de
  son parent, et la bibliothèque standard le demande toujours. Le démon lancé par `timonier
  daemon start`, `timonier mcp` ou un hook gardait ainsi les pipes de qui avait lancé la commande
  (git, l'agent, un test qui lit la sortie), qui attendaient leur fermeture jusqu'à l'arrêt du
  démon (constaté en CI : `start` ne revenait qu'après 15 min). Avant de lancer le démon, la CLI
  marque ses propres handles standard comme non héritables (`SetHandleInformation` de
  `windows-sys` 0.61.2, MIT OR Apache-2.0, déjà dans l'arbre par clap et tokio, vérifié le
  2026-10-22). C'est le seul code `unsafe` du workspace : la règle passe de `forbid` à `deny`
  (ADR 0004), et une seule fonction, `daemon::private_handles`, l'autorise, sous Windows.
- **Mesure des tests** : sous Windows, la durée réelle seulement, et la durée de chaque test par
  le JUnit. Le CPU et la mémoire d'un arbre de processus ne s'obtiennent que par des appels
  système non sûrs, que le workspace ne s'autorise qu'au cas des handles ci-dessus : la run dit
  `Accounting::WallTime`, « CPU et mémoire non mesurés », et compare
  les runs entre elles sur la durée réelle (`CpuSource::WallTime`), jamais sur des zéros.
- **Commandes de test** : `cmd /C` au lieu de `sh -c`. Le hook git reste un script shell : Git
  pour Windows exécute ses hooks avec son propre shell.
- **Tests** : la CI ajoute `windows-latest` aux jobs Rust et extension. Les tests dont les faux
  programmes sont des scripts shell, ou qui jouent sur les modes de fichiers, restent réservés à
  Unix (`tests/init.rs`, `tests/test.rs` de la CLI, quelques tests du démon et de l'extension) ;
  ceux du démon, du proxy MCP et de `timonier daemon`, qui passent par le pipe, tournent sous
  Windows.
- **Vérifié sur Linux** : `cargo check` et `clippy -D warnings` de tout le workspace pour
  `x86_64-pc-windows-gnu`, les dépendances C compilées par le gcc de l'hôte (sans édition de
  liens) ; l'exécution réelle ne se constate que sur un runner Windows.

## Alternatives écartées

- **TCP sur 127.0.0.1 avec un jeton** : la spec le garde pour Zed ou une application à part ;
  un named pipe est réservé à l'utilisateur sans jeton à transmettre.
- **Mesurer le CPU par `windows-sys` ou une crate qui enveloppe les job objects** : du code
  `unsafe` dans le workspace, ou une dépendance de plus pour un seul système ; à reconsidérer
  si la mesure sous Windows devient un besoin.
- **WSL seulement** : demanderait d'installer WSL et laisserait VSCode sous Windows sans démon.

## Conséquences

- Les budgets de CPU et de mémoire du tableau blanc ne sont pas vérifiés sous Windows ; les
  budgets de latence (durée des spans) le sont.
- Un test réservé à Unix ne couvre pas Windows : si un comportement propre à Windows casse, seul
  un test qui tourne sous Windows le dira.
