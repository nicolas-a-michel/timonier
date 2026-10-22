# Timonier

[English](README.md) · **Français**

Timonier est une extension VSCode et un démon local qui vous laissent piloter des agents de
code IA sans lire chaque ligne qu'ils écrivent. Il tient à jour une carte de l'architecture de
votre code, la compare avec l'architecture que vous avez dessinée, montre ce qui a changé
depuis votre dernière relecture, et donne aux agents le même plan, la même todo et les mêmes
contraintes que vous.

**Langages :** Rust, TypeScript, JavaScript, Python, Go, Java, Kotlin, Scala, C, C++, C#, Swift et PHP.

![La carte d'architecture de Timonier lui-même : six boîtes nommées par rôle, avec leurs paquets et les dépendances entre elles](docs/images/map.png)

## Le problème

Quand un agent écrit le code, le goulot n'est plus l'écriture mais la compréhension. Le code
change plus vite que l'idée que vous vous en faites, et relire chaque diff ne tient pas la
distance. Timonier garde ce modèle pour vous et répond à tout moment à quatre questions :

- **Ce que je veux** : l'architecture voulue, dessinée sur un tableau blanc.
- **Ce qui existe** : l'architecture réelle, extraite du code et tenue à jour.
- **Comment ça se comporte** : temps et ressources, mesurés pendant les tests.
- **Ce qui se passe** : le plan, la todo en direct, ce qu'a fait l'agent et où regarder.

Le nom : le timonier tient la barre pendant que l'équipage, ici les agents, fait avancer le
navire.

## Visite

Les captures ci-dessous viennent de la page de développement de l'extension, avec des données
d'exemple, dans le thème Dark Modern de VSCode, en anglais ; l'interface existe aussi en
français. L'extension suit votre thème : clair, sombre ou contraste élevé.

### La carte d'architecture

La carte s'ouvre sur quelques boîtes nommées par rôle : point d'entrée, modèle de données,
persistance, interface, utilitaire, bibliothèques externes. Timonier lui-même tient en six. Un
clic ouvre une boîte sur ses paquets, puis sur ses composants, fichiers et symboles. Un lien en
tirets n'est connu que par la syntaxe (tree-sitter) ; un lien plein est déclaré dans un
manifeste ou confirmé par les serveurs de langage de l'IDE. La disposition reste stable quand le
code change, et Maj+Entrée ouvre le code d'une boîte.

### Ce qui a changé depuis votre relecture

« Marquer comme revu » fige la carte telle que vous l'avez vue. Ensuite, un bandeau dit ce qui a
changé : boîtes nouvelles, modifiées et supprimées, dépendances ajoutées et retirées, cycles
ajoutés. La carte marque chaque changement d'une forme et d'un mot : vous savez où regarder
avant d'ouvrir un fichier.

![La carte après une relecture : un bandeau liste une boîte nouvelle, quatre modifiées, un cycle ajouté ; les boîtes portent « 1 modified » et une étoile sur la nouvelle crate](docs/images/review.png)

La couche de dette de relecture compte, sur chaque boîte, les fichiers changés depuis votre
relecture et pas encore relus. Les fichiers changés sans session d'agent ouverte sont hachurés
et marqués « untracked ».

![La couche de dette : les boîtes teintées selon le nombre de fichiers à relire, « 12 to read again » sur Utility, une marque « untracked » sur Entry point](docs/images/debt.png)

### Le tableau blanc

Vous dessinez l'architecture voulue, à main levée ou avec des formes typées : une boîte est un
composant, une zone une couche, une flèche une dépendance permise, interdite ou requise, une
note une contrainte, une note de budget une limite de performance. Chaque geste est enregistré
dans `.timonier/intent.json`, que les agents lisent et n'écrivent jamais.

![Le tableau blanc : deux couches, Core et Front ends, quatre boîtes, une flèche requise, une interdite en rouge, une note et un budget de 5 ms au p95](docs/images/whiteboard.png)

« Comparer à l'intention » confronte les deux sur la carte : les composants dessinés absents
du code, les arêtes interdites qu'il franchit (et quelles dépendances les franchissent), les
arêtes requises manquantes.

![La carte comparée au tableau blanc : un bandeau liste un composant manquant, une arête interdite franchie et une arête requise manquante ; le lien fautif est rouge et marqué « forbidden »](docs/images/intent.png)

### Plan, todo et sessions

Les agents travaillent avec les outils de Timonier : une session par tâche, une todo pour la
découper, le plan mis à jour au fil de l'eau. Un panneau latéral le montre en direct, avec le
journal de chaque session : son objectif, les composants touchés, son résumé, et son coût en IA :
les tokens que Claude Code déclare dans sa propre télémétrie pendant la session, les dollars
comme son estimation. `timonier init` active cette télémétrie dans `~/.claude/settings.json` si
vous l'acceptez ; sans elle, la session dit « non mesuré ».

![Le panneau de suivi : jalons avec barres d'avancement et états des tâches, la todo de la session ouverte, le journal des sessions](docs/images/tracking.png)

Le pont, la vue d'accueil de Timonier, répond d'un coup d'œil à trois questions : où en est le
plan, ce qui s'est passé depuis votre dernière relecture, et où regarder d'abord.

![Le pont : le jalon en cours, ce qui a changé depuis la relecture, et une liste « Where to look » qui commence par une tâche bloquée](docs/images/bridge.png)

### Les agents, par MCP

`timonier mcp` est un serveur MCP pour les agents de code (Claude Code, GitHub Copilot, Cursor,
Codex). En plus des outils de suivi, il donne à l'agent le tableau blanc (avec une image du
croquis), l'écart du code avec lui, les budgets de performance et leurs dépassements, les
conventions d'instrumentation, l'inventaire des technologies (licences, doublons par fonction,
vulnérabilités connues d'OSV.dev) et le processus de développement du projet : Solo rapide, Solo
discipliné (un ticket, une branche et une pull request par tâche, par `gh`) ou Équipe. Le bouton
**Configurer** du pont prépare les agents que vous cochez ; `timonier init` fait de même depuis
un terminal, `timonier doctor` vérifie et répare cette installation, `timonier uninit` la retire.

### Performances

`timonier test` lance vos tests et les mesure : temps, CPU, mémoire maximale (de tout l'arbre
de processus sous Linux, dans un scope systemd ; du plus gros processus sous macOS ; la durée
réelle seule sous Windows), durée de chaque test, et traces OpenTelemetry du code testé, rattachées aux boîtes de la carte. La couche
Performances de la carte montre le temps passé dans chaque boîte et dans ce qu'elle appelle, et
épaissit les liens où passent le plus d'appels.

![La couche Performances de la carte : 13,95 s sur Entry point et ce qu'elle appelle, 950 ms sur Data model, des liens plus épais là où plus d'appels ont été mesurés](docs/images/performance-layer.png)

Un budget du tableau blanc que les derniers runs dépassent marque sa boîte en rouge, que la
couche soit affichée ou non.

![La carte avec un budget dépassé : une marque rouge « Budget exceeded » sur Entry point, expliquée par la légende](docs/images/budget.png)

Le panneau Performances montre un run : la chronologie de chaque test avec ses spans et la
mémoire et le CPU du processus, l'arbre d'appels avec temps total et temps propre, et
l'évolution des runs par commit. Avec `timonier test --runs auto`, il compare un lot de runs à
la branche principale et signale une mesure plus lente sans doute possible (Mann-Whitney,
p < 0,01, et au moins max(5 %, 3 MAD)).

![La chronologie d'un test : ses spans par profondeur, et dessous la mémoire et le CPU du processus](docs/images/performance-timeline.png)

![L'arbre d'appels d'un run : chaque test déplié en ses appels, avec temps total, temps propre, appels, mémoire et CPU](docs/images/performance-tree.png)

![L'évolution des runs par commit, avec une régression confirmée et une probable](docs/images/performance-evolution.png)

La liste complète de ce qui fonctionne, avec les chiffres mesurés sur Timonier lui-même, est
dans [docs/features.md](docs/features.md) (en anglais).

## Fonctionnement

- Un **démon** en Rust, un par projet, indexe Rust, TypeScript, JavaScript, Python, Go, Java, Kotlin, Scala, C, C++, C#, Swift et PHP avec tree-sitter, affine les
  appels avec les serveurs de langage de l'IDE, et garde le graphe et son historique dans
  SQLite.
- L'**extension VSCode** s'abonne à des vues de ce graphe en JSON-RPC et les dessine avec React
  Flow.
- Les **agents** joignent le même démon par MCP ; le plan, la todo et le journal sont des
  fichiers JSON versionnés dans `.timonier/`.
- Les **tests** envoient leurs traces OpenTelemetry à un récepteur du démon, qui rattache chaque
  span à son nœud du graphe.

La spécification est [docs/spec.md](docs/spec.md), en anglais ; le plan ([PLAN.md](PLAN.md)) et
les décisions d'architecture ([docs/adr/](docs/adr/README.md)) sont en français.

## État

Développement précoce, rien de publié (22 octobre 2026). Le MVP est découpé en phases 0 à 9
([PLAN.md](PLAN.md)) : les phases 0 à 8 sont faites. La phase 9, en cours, a ajouté l'inventaire
des technologies, le processus de développement et le coût de l'IA, et porté Timonier sous
Windows ; elle publie une préversion sur le Marketplace VSCode, puis se termine en pilotant un
nouveau projet de bout en bout. Timonier pilote son propre développement.

La phase 10, faite avant la préversion, a fait passer Timonier de deux langages à treize :
chacun a été vérifié sur un vrai projet, de la carte à la première trace de ses tests. Timonier
fonctionne sous Linux, macOS et Windows ; ses tests y passent en CI, et un premier essai sur une
vraie machine Windows est en cours. Rien n'est encore publié sur crates.io, npm ou le
Marketplace. Les versions suivantes ajoutent un gestionnaire de skills, l'indexation SCIP,
plusieurs agents qui travaillent ensemble, et l'usage en équipe : voir la
[feuille de route](ROADMAP.fr.md).

## Essayer

Prérequis : Rust stable (`rustup`), Node.js 24 ou plus, VSCode 1.140 ou plus.

```sh
npm ci
npm run build                              # la webview, puis l'extension
cargo install --path crates/timonier-cli   # met timonier et tim dans le PATH
```

Ouvrez ce dépôt dans VSCode et appuyez sur F5 (« Run Extension »). Dans la fenêtre qui s'ouvre,
ouvrez un projet dans l'un de ces langages, cliquez sur l'icône de Timonier (une barre de navire) dans
la barre d'activité, puis **Open Map**.

Pour préparer un projet pour vos agents, dans le projet :

```sh
tim init --dry-run   # montre ce qu'il changerait
tim init             # --yes pour ne rien demander
tim doctor           # vérifie l'installation et la répare
tim test             # lance les tests qu'init a enregistrés, et les mesure
tim inventory        # dépendances, licences, doublons, vulnérabilités connues
```

## Développer

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npm test                          # tests unitaires de la webview, de l'extension et des hooks
npm run test:integration          # l'extension dans un vrai VSCode, face à un vrai démon
npm run test:a11y                 # WCAG AA dans chaque thème VSCode capturé
npm run screenshots -w webview    # les images de ce README
```

[CLAUDE.md](CLAUDE.md) liste toutes les commandes et explique comment le projet est piloté.

## Remerciements

Merci à Iban Guinebert et Michaël Roynard pour l'idée de Timonier.

## Licence

[MIT](LICENSE) © 2026 Nicolas Michel. Les mentions des dépendances tierces accompagnent
l'extension.
