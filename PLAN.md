# PLAN — Timonier (MVP)

Plan de réalisation du MVP défini dans [docs/spec.md](docs/spec.md) (section « Découpage »),
selon les phases du prompt de démarrage. La spec fait autorité. Les décisions techniques non
couvertes par la spec sont dans [docs/adr/](docs/adr/README.md).

Format (voir [ADR 0002](docs/adr/0002-format-de-plan-md.md)) :

- une section `## Phase N — Titre` par phase, avec une ligne `Statut : à faire | en cours | terminée` ;
- une case de premier niveau par tâche, avec son critère d'acceptation ;
- en fin de phase, une ligne `Décisions :` et une ligne `Écarts :`.

Le hook de démarrage de session lit ce format.

---

## Questions ouvertes

Contradictions ou manques de la spec. J'attends tes réponses : je n'invente pas. Chaque
question indique la phase qu'elle bloque et ma recommandation.

**Q1 — Licence du projet** — ✅ répondu le 2026-10-14 : **MIT**, titulaire Nicolas Michel.
- Options : MIT OR Apache-2.0 (convention Rust), Apache-2.0 seule, AGPL-3.0, licence
  non libre (BSL…).
- Contraintes connues : elkjs 0.12 est sous EPL-2.0 OR GPL-3.0, un copyleft faible au niveau
  du fichier, compatible avec toutes ces options. tldraw étant écarté (Q3), aucune dépendance
  prévue n'impose de choix.
- Titulaire des droits : Nicolas Michel. Projet public.

**Q2 — Dépôt GitHub pour la CI** — ✅ répondu le 2026-10-14
- Réponse : propriétaire Nicolas Michel, dépôt public créé par toi :
  <https://github.com/nicolas-a-michel/timonier>.
- Conséquence : le workflow de CI est écrit en Phase 1 et ses commandes sont vérifiées en
  local (Linux). Le critère « vert en CI sur Linux et macOS » sera constaté au premier push.

**Q3 — Licence de tldraw** — ✅ répondu le 2026-10-14 : **pas de tldraw**. On utilise autre
chose. Le remplaçant sera choisi au début de la Phase 7, par une ADR soumise à validation.
Rappel de ce qui avait été vérifié :
- Vérifié le 2026-10-14 sur tldraw 5.5.2 : le SDK n'est pas open source. Hors développement,
  il exige une clé de licence.
- Une webview VSCode publiée compte comme production. Sans clé, le canevas s'affiche cinq
  secondes puis devient vide.
- Les clés possibles :
  - « hobby » : discrétionnaire, non commerciale, filigrane « made with tldraw » obligatoire ;
  - commerciale : payante ;
  - essai : 100 jours.
- Tout fork de l'extension aurait besoin de sa propre clé.
- Options :
  - (a) garder tldraw et obtenir une clé ;
  - (b) Excalidraw (licence MIT, à vérifier), que la spec juge moins adapté aux formes
    typées ;
  - (c) un tableau blanc construit sur React Flow (MIT) avec une bibliothèque de dessin à
    main levée sous licence permissive : plus de travail en Phase 7, mais aucune contrainte.
- Candidats pour la Phase 7 :
  - **Excalidraw** : licence MIT d'après GitHub le 2026-10-14 ; la spec l'avait jugé moins
    adapté aux formes typées ;
  - **un tableau blanc construit sur React Flow**, déjà dans la pile : cohérent avec « un seul
    langage visuel », et les formes typées y sont des nœuds de première classe.
- Piste privilégiée à ce stade : React Flow.

**Q4 — Place des composants dans l'arbre strict** — ✅ répondu le 2026-10-14 : option (a).
- La spec dit que la hiérarchie `parent` est un arbre strict contenant Repo, Component,
  Package, File et Symbol. Elle dit aussi que les composants viennent d'une agrégation
  structurelle (crates, packages, dossiers) suivie d'un clustering Louvain/Leiden, avec une
  carte récursive (« des boîtes dans des boîtes »).
- Elle ne dit pas où se placent les packages et les dossiers par rapport aux composants, ni si
  un cluster peut chevaucher deux packages.
- Options :
  - (a) les composants sont dans l'arbre (Repo → Component* → Package → Component* → File →
    Symbol), et le clustering ne coupe jamais un package : il regroupe des packages entiers
    en haut et découpe à l'intérieur d'un package ;
  - (b) arbre physique strict (Repo → Package → File → Symbol), plus une hiérarchie de
    composants séparée, en appartenance, qui peut chevaucher les packages.
- Recommandation : (a), plus fidèle au texte (arbre strict, composants comme nœuds).

**Q5 — Accès au LSP** — ✅ répondu le 2026-10-14 : option (c), en commençant par (a).
- La spec parle du « LSP (déjà lancé par l'IDE) », mais le prompt place un « client LSP »
  dans `timonier-index`, côté démon.
- Options :
  - (a) l'extension relaie au démon les requêtes de l'IDE (`vscode.executeDefinitionProvider`,
    `executeReferenceProvider`, hiérarchie d'appels) : un seul serveur de langage en mémoire,
    mais pas de LSP hors IDE (CLI, CI) ;
  - (b) le démon lance ses propres serveurs (rust-analyzer, serveur TypeScript) : autonome,
    mais la mémoire double ;
  - (c) les deux, derrière une même interface.
- Recommandation : (c) avec (a) d'abord pour le MVP, et (b) quand la CI en aura besoin (v3).

**Q6 — Quel LLM, appelé par qui ?** — ✅ répondu le 2026-10-14 : (c) par défaut, enrichi
par (a) ; pas de clé d'API dans le démon.
- La spec dit : « Le LLM nomme et résume les clusters », les rôles sont « confirmés par le
  LLM », et le dessin libre est « interprété par un modèle vision ». Elle ne dit pas d'où
  vient ce modèle.
- Options :
  - (a) l'agent connecté via MCP : le démon expose des clusters non nommés et l'agent soumet
    noms et rôles par un outil d'écriture structuré, puis lit l'image du croquis via
    `get_intended_architecture` ;
  - (b) le démon appelle lui-même une API de LLM avec une clé de l'utilisateur ;
  - (c) heuristiques seules dans le MVP.
- Recommandation : (c) par défaut, enrichi par (a). Pas de clé d'API dans le démon.

**Q7 — Coût de l'IA : MVP ou v2 ?** — ✅ répondu le 2026-10-14 : collecte par session et
par tâche en Phase 9 ; le calque par composant reste en v2.
- Le prompt le met en Phase 9. Le « Découpage » de la spec le classe en v2, et seule sa
  méthode de mesure est décidée.
- Recommandation : en Phase 9, la seule collecte par session et par tâche via la télémétrie
  OpenTelemetry de Claude Code. Le calque par composant reste en v2.

**Q8 — Assistant « Nouveau projet piloté »** (Phase 9)
- Il est décrit dans « Initialisation » mais absent de la liste du MVP et du prompt.
- Recommandation : en Phase 9, sinon v2. La détection à l'ouverture d'un dépôt non configuré
  est prévue en Phase 6.

**Q9 — Projet pilote du critère de sortie du MVP** (Phase 9)
- Critère : « un projet réel piloté de bout en bout sans lire le code ligne à ligne ».
- À trancher : lequel ?

**Q10 — Outils MCP de lecture du suivi** (Phase 6)
- La liste de la spec n'a aucun outil pour relire le plan ou la todo (`get_plan`,
  `get_todo`), alors que l'agent doit les lire sans ouvrir les fichiers à la main.
- Recommandation : les ajouter. La spec dit que les noms sont provisoires.

**Q11 — `PLAN.md` et `.timonier/plan.json`** (Phase 6)
- À partir de la Phase 6, le plan est tenu via MCP.
- Options : migrer ce plan dans `plan.json` et générer `PLAN.md`, ou garder `PLAN.md` comme
  feuille de route humaine et `plan.json` pour les tâches.
- À trancher au début de la Phase 6.

**Q12 — Identifiant d'éditeur sur la Marketplace VSCode** (avant toute publication) — ⏸
reportée le 2026-10-16 (« la Q12 attendra »). `nicolas-a-michel` reste provisoire.
- Le paquet utilise `publisher: nicolas-a-michel`, qui n'existe pas encore sur la
  Marketplace. Cet identifiant doit être créé par toi sur marketplace.visualstudio.com.
- Il ne bloque que la publication, pas le développement.
- À trancher : garder `nicolas-a-michel`, ou en choisir un autre (par exemple `timonier`).

**Q13 — Langue de l'interface** — ✅ répondu le 2026-10-16 : **bilingue** (anglais de base,
traduction française) pour l'instant ; d'autres langues seront ajoutées plus tard. ADR 0006
acceptée.
- La spec ne la fixe pas.
- J'ai mis en place l'anglais comme langue de base, avec une traduction française qui suit la
  langue de VSCode.
- À trancher : garder ce choix, ou passer au français seul.

**Q14 — Fichiers hors de tout package** — ✅ répondu le 2026-10-16 : extension gardée. Un
fichier hors de tout package se place sous le Repo ou sous un Component de haut niveau.
- Ta réponse à Q4 place les fichiers sous un Package, ou sous un Component à l'intérieur d'un
  package.
- J'ai aussi permis un fichier directement sous le Repo ou sous un Component de haut niveau,
  pour les fichiers qui n'appartiennent à aucun package (scripts à la racine, par exemple).
- À trancher : garder cette extension, ou refuser ces fichiers (ils ne seraient alors pas
  indexés).

**Q15 — Validation des ADR de la Phase 2 et du critère 2.1** — ✅ répondu le 2026-10-16 :
ADR 0008 à 0011 acceptées. Pour la suite, option (a) : les ADR sont écrites (« Proposé ») avant
le code, et tu les acceptes ensemble à la validation de la phase.
- Le critère de 2.1 disait « ADR acceptées avant le code qui en dépend ». Je l'ai réécrit en
  « écrites avant le code, acceptées à la validation de la phase ».
- Les ADR 0008 à 0011 sont donc implémentées sans ton acceptation préalable. La relecture l'a
  signalé à juste titre.
- L'ADR 0009 a beaucoup évolué au fil des trois relectures. Les sections « Propriété » et
  « Contenu de référence des observations » méritent d'être lues : propriété à la plus petite
  clé pour un nœud, à la source la plus fiable pour une arête, contributions en attente.
- À trancher : accepter les ADR 0008 à 0011 (avec Q14) et le nouveau libellé, ou demander des
  changements avant la suite.

## Hypothèses de travail

Ce sont mes interprétations. Elles valent tant que tu ne les corriges pas.

- **H1** — `webview/` est à la racine (réutilisable par d'autres front-ends, comme le
  prévoit la spec pour Zed ou une application autonome) ; l'extension est dans
  `editors/vscode/`.
- **H2** — Les « < 100 ms » de la Phase 3 se mesurent de la fin de l'anti-rebond (~300 ms,
  spec) jusqu'au graphe validé dans SQLite. Valeur p95, sur le dépôt Timonier, en build
  release, sur la machine de développement.
- **H3** — « Quelques secondes » pour indexer Timonier : cible ≤ 3 s à froid, en build
  release.
- **H4** — SCIP est en v2 (Découpage). La consolidation au commit du MVP se limite au
  re-clustering et au snapshot figé.
- **H5** — Le pont (vue d'accueil) arrive en Phase 6, quand plan, todo et diff de revue
  existent.
- **H6** — Le volet instrumentation de `timonier init` et le test de fumée tracé de
  `timonier doctor` se terminent en Phase 8, car ils dépendent du récepteur OTLP.
- **H7** — Le lien entre une tâche du plan et un composant du tableau blanc existe dès la
  Phase 6. Il ne peut être renseigné qu'après la Phase 7.
- **H8** — Plateformes du MVP : Linux et macOS, les cibles de la CI. Windows (named pipes)
  viendra plus tard. Sur macOS, le niveau 1 est réduit (pas de cgroups ni de procfs ; durée,
  CPU et mémoire max via rusage). Les compteurs d'instructions matériels ne sont visés que
  sur Linux.
- **H9** — Langues :
  - en anglais : code, commentaires, commits, ainsi que `CLAUDE.md`, les skills, les
    subagents et les messages des hooks, qui sont des fichiers pour les agents ;
  - en français : `PLAN.md`, les ADR, les rapports de relecture et la documentation
    utilisateur.
- **H10** — L'en-tête de boîte aux lettres est présent dès la Phase 6, avec zéro message non
  lu (la messagerie est en v2).
- **H11** — TypeScript est figé en 6.0.x : typescript-eslint 8.71 ne prend pas en charge
  TypeScript 7.
- **H12** — npm workspaces plutôt que pnpm, car `vsce` gère mal les liens symboliques de
  pnpm. Une ADR le confirmera en Phase 1.

---

## Phase 0 — Équiper l'environnement Claude Code du projet

Statut : terminée (0.11 reste à confirmer par toi dans une nouvelle session)

- [x] 0.1 Lire `docs/spec.md` en entier — critère : contradictions et manques relevés dans
  « Questions ouvertes ».
- [x] 0.2 Vérifier dans la documentation officielle le format actuel des skills, hooks,
  subagents et `settings.json` — critère : sources et date citées dans l'ADR 0001.
- [x] 0.3 Vérifier existence, version et licence des paquets nommés par la spec — critère :
  tableau « Dépendances vérifiées » plus bas.
- [x] 0.4 Initialiser le dépôt git (branche `main`) et écrire `.gitignore` — critère :
  `.claude/state/` et `.claude/settings.local.json` sont ignorés.
- [x] 0.5 Écrire `CLAUDE.md` — critère : vision, carte du dépôt, commandes, règles, pointeurs,
  hooks documentés avec leur test manuel.
- [x] 0.6 Écrire les sept skills (`rust-daemon`, `graph-model`, `language-pack`,
  `vscode-webview-ui`, `mcp-tools`, `otel-instrumentation`, `adr`) — critère : frontmatter
  conforme à la doc, avec une description qui dit quand les utiliser.
- [x] 0.7 Écrire les hooks (`.claude/settings.json`, `.claude/hooks/`) — critère :
  `node --test .claude/hooks/hooks.test.mjs` vert, et chaque script exécuté à la main avec
  une entrée réelle.
- [x] 0.8 Écrire le subagent `phase-reviewer` — critère : outils en lecture seule (`Read`,
  `Grep`, `Glob`, `Bash`), rapport en français.
- [x] 0.9 Écrire les ADR 0001 et 0002, l'index `docs/adr/README.md` et le modèle d'ADR —
  critère : fichiers présents, index à jour.
- [x] 0.10 Rédiger et soumettre `PLAN.md` — critère : chaque tâche a un critère vérifiable.
- [ ] 0.11 Vérification par l'humain dans une nouvelle session (hooks, skills, subagent,
  CLAUDE.md) — critère de sortie du prompt :
  - le message de démarrage « Timonier · Phase … » s'affiche ;
  - `/hooks` liste SessionStart, UserPromptSubmit, PreToolUse, PostToolUse et Stop ;
  - `/skills` liste les sept skills ;
  - `/agents` liste `phase-reviewer` ;
  - `CLAUDE.md` suffit à reprendre le travail.
- [x] 0.12 Validation de ce plan et réponses à Q1–Q7 — critère : réponses consignées ici. Feu
  vert donné le 2026-10-14.

Décisions : [ADR 0001](docs/adr/0001-hooks-claude-code-en-node.md),
[ADR 0002](docs/adr/0002-format-de-plan-md.md).
Écarts :
- `git init` fait ici, car le dossier n'était pas un dépôt. Phase 0 commitée (`33e2d76`).
- Pour 0.11, ce qui a déjà été constaté dans la session de travail :
  - Claude Code a chargé les hooks à chaud, et la garde `intent.json` s'est déclenchée ;
  - les sept skills sont listés ;
  - `phase-reviewer` est proposé comme type d'agent.

  Le message de démarrage ne peut être vu que dans une nouvelle session.
- Corrigé avant le commit de la Phase 0 : la garde `intent.json` ne juge plus que la
  sous-commande qui écrit (faux positif constaté en réel).
- Corrigé pendant la Phase 1 :
  - deux références à un mauvais nom de fichier pour l'ADR 0002 dans les hooks ;
  - les erreurs de typecheck indiquent leur workspace ;
  - le hook de formatage couvre tous les types que la CI vérifie.

## Phase 1 — Squelette du dépôt

Statut : terminée (validée par toi le 2026-10-14)

Prérequis levés : licence MIT (Q1) ; dépôt public
<https://github.com/nicolas-a-michel/timonier> (Q2).

- [x] 1.1 Ajouter la licence MIT (« Copyright (c) 2026 Nicolas Michel ») — critère : fichier
  `LICENSE` présent ; `license = "MIT"` dans `Cargo.toml` et `package.json` ; `cargo metadata`
  et `npm pkg get license` renvoient `MIT`.
- [x] 1.2 Fixer la toolchain : `rust-toolchain.toml` (stable, rustfmt, clippy), édition 2024 et
  `rust-version` dans `[workspace.package]`, `rustfmt.toml` — critère : `cargo fmt --all
  --check` vert ; ADR toolchain et MSRV (stable actuelle 1.99, 1.96 installée en local).
- [x] 1.3 Créer le workspace Cargo avec les sept crates (`timonier-core`, `-store`, `-index`,
  `-daemon`, `-otlp`, `-mcp`, `-cli`) — critère : `cargo build --workspace` vert ; ADR sur
  le découpage, le sens des dépendances et le crate qui porte les types du protocole.
- [x] 1.4 Binaire `timonier` et alias `tim` (clap 4.6), avec `--version` et `--help` — critère :
  `timonier --version` et `tim --version` impriment la même chaîne (test assert_cmd) ; ADR
  sur le mécanisme d'alias.
- [x] 1.5 Lints du workspace (`[workspace.lints]`) — critère : `cargo clippy --workspace
  --all-targets -- -D warnings` vert.
- [x] 1.6 Un test de fumée par crate — critère : `cargo test --workspace` vert.
- [x] 1.7 Monorepo Node — critère : `npm ci && npm run lint && npm run typecheck && npm test`
  verts ; ADR gestionnaire de paquets et bundler de l'extension (H12). Contenu :
  - `package.json` racine, workspaces `editors/vscode` et `webview` ;
  - scripts `build`, `test`, `lint`, `typecheck`, `format` ;
  - TypeScript 6.0.x strict (H11), ESLint en configuration plate avec typescript-eslint,
    prettier.
- [x] 1.8 `webview/` : application React 19 + Vite 8 qui affiche un état vide (tokens `--tim-*`
  minimaux sur les variables VSCode) — critère : `npm run build -w webview` produit le
  bundle ; test vitest vert.
- [x] 1.9 `editors/vscode/` : extension avec la commande « Timonier : Ouvrir la carte », qui
  ouvre un panneau webview chargeant le bundle de `webview/` sous une CSP stricte (nonce,
  `localResourceRoots`) — critère : F5 (« Run Extension ») ouvre le panneau vide en thème
  clair et sombre ; un test d'intégration `@vscode/test-cli` exécute la commande et vérifie
  la création du panneau.
- [x] 1.10 Packaging — critère : `npx @vscode/vsce package` produit un `.vsix` que
  `code --install-extension` installe.
- [x] 1.11 Activer les hooks sur le squelette — critère : test manuel documenté dans
  `CLAUDE.md` (une erreur de type introduite bloque la fin de tour une fois). Changements :
  - le hook Stop appelle le script `typecheck` racine ;
  - le hook de formatage trouve prettier local.
- [x] 1.12 CI GitHub Actions sur ubuntu-latest et macos-latest — critère : chaque étape du
  workflow passe en local sous Linux. Le vert sur les deux OS sera constaté au premier push
  sur le dépôt public créé par toi (Q2). Étapes :
  - Rust : vérification du format, clippy `-D warnings`, `cargo test` ;
  - Node : `npm ci`, lint, typecheck, tests, build, package ;
  - tests de l'extension sous xvfb (Linux) ;
  - tests des hooks ;
  - caches cargo et npm.
- [x] 1.13 `README.md` minimal en français (ce qu'est Timonier, état, comment construire) et
  commandes de `CLAUDE.md` à jour — critère : relu.
- [x] 1.15 Icône Timonier dans la barre d'activité, ajoutée à ta demande du 2026-10-14 —
  critère : un conteneur Timonier avec une icône 24×24 d'une seule couleur, une vue « Vue
  d'ensemble » dont l'accueil propose « Ouvrir la carte », et un bouton de carte dans sa barre
  de titre ; tests unitaires du manifeste et test d'intégration verts ; ADR 0007.
  - Constat : 19 tests unitaires et 5 tests d'intégration verts.
  - L'icône a été rendue pixel par pixel à 24 px, et la version à 6 rayons retenue.
  - Le rendu réel est à regarder avec F5, en même temps que 1.9.
- [x] 1.14 Relecture par `phase-reviewer` — critère : verdict « prête », ou réserves traitées.
  Réserves traitées (voir plus bas) ; il n'y a pas eu de seconde relecture.

Critère de sortie : `cargo test` et le build de l'extension passent en CI (constaté au
premier push, voir Q2) ; l'extension s'installe en mode développement et affiche un panneau
vide.

Décisions : [ADR 0003](docs/adr/0003-decoupage-des-crates.md) (crates, dépendances, alias),
[ADR 0004](docs/adr/0004-toolchain-rust.md) (toolchain, MSRV 1.96),
[ADR 0005](docs/adr/0005-monorepo-node-et-build-de-l-extension.md) (npm, TS 6, esbuild, Vite,
tests, CSP), [ADR 0006](docs/adr/0006-langue-de-l-interface.md) (langue de l'interface,
acceptée le 2026-10-16 : Q13), [ADR 0007](docs/adr/0007-entree-dans-la-barre-d-activite.md) (icône et vue
d'ensemble dans la barre d'activité).

Constats (2026-10-14, Linux, machine de développement, après la relecture) :
- Rust : `cargo fmt --check` et `clippy -D warnings` verts ; `cargo test` : 9 tests verts ;
  `cargo +1.96 check` vert (MSRV). `timonier --version` et `tim --version` affichent tous deux
  `timonier 0.1.0`.
- Après suppression de tous les builds (état d'un checkout neuf), `npm test` est vert :
  - webview : 14 tests vitest, dont un vérificateur de tokens qui est lui-même testé ;
  - extension : 14 tests `node:test` (CSP nonce seul, échappement de chaque champ, manifeste,
    traductions complètes, sous-dossiers compris) ;
  - hooks : 24 tests.
- `npm run test:integration` (qui construit d'abord) donne 4 tests verts dans VSCode
  1.141.2 :
  - l'extension s'active et la commande existe ;
  - le bundle s'exécute sous la CSP stricte : la webview répond « ready » ;
  - le panneau ouvert est révélé et redevient l'onglet actif, sans second panneau.
- Lint, typecheck et `format:check` sont verts.
- `npm run package` produit `timonier.vsix`. Il contient le bundle, le manifeste, les
  `THIRD-PARTY-NOTICES.md` (React, react-dom, scheduler : MIT), les traductions, le README et
  la licence. L'installation réussit dans un profil temporaire.
- Le bloc « Testing by hand » de `CLAUDE.md`, exécuté tel quel, produit les sept sorties
  annoncées. En particulier, une erreur de type bloque la fin de tour une fois, puis n'est
  plus que signalée.

CI GitHub (commit `6d3e0f8`, run 38604588410) : 5 jobs verts au premier push.
- Rust sur ubuntu-latest et macos-latest ; MSRV 1.96.
- Extension et webview sur les deux OS : 4 tests d'intégration verts dans VSCode, sous xvfb
  sur Linux et directement sur macOS.
- `timonier.vsix` publié comme artefact du run.

Relecture `phase-reviewer` (verdict initial : « pas prête ») — tout est traité :
- B1 : la CI lançait les tests d'intégration sans webview construite. Corrigé :
  `test:integration` construit d'abord, et le scénario a été vérifié depuis un état propre.
- I1 : 1.9 décochée (contrôle visuel F5) ; 1.11 documentée et vérifiée.
- I2 : skills `vscode-webview-ui` et `rust-daemon` alignés sur les ADR 0003 à 0006.
- I3 : dépendances et actions ajoutées au tableau ; action tierce épinglée par SHA.
- I4 : la langue de l'interface (ADR 0006) a été implémentée avant ta réponse. C'est une
  exception assumée, car réversible ; validée ensuite (Q13, 2026-10-16).
- I5 : notices tierces livrées dans le `.vsix`.
- Mineurs :
  - CSP réduite au nonce seul ;
  - `.vscodeignore` explicite ;
  - tests renforcés (échappement, révélation, tokens, traductions) ;
  - job CI `msrv` ;
  - configuration « Extension Tests » corrigée ;
  - ADR 0005 corrigée (scripts npm, raison de `node:test`).

Écarts :
- 1.9 : contrôle visuel fait par toi avec F5 (« très bien pour l'icône », 2026-10-14). Le
  rendu dans chaque thème n'a pas été commenté en détail.
- Checklist d'accessibilité de l'état vide :
  - aucun élément interactif, donc ni focus ni clavier à traiter ;
  - le texte passe par les variables de thème VSCode ;
  - aucune animation.

  Le contraste réel dépend du thème : il sera mesuré par axe-core en Phase 4.
- `npm audit` signale 4 vulnérabilités, toutes dans mocha (via `@vscode/test-cli`) : 1 haute
  (serialize-javascript), 1 modérée, 2 basses. C'est un outil de test, absent du `.vsix`.
- L'éditeur de la Marketplace (`publisher`) est provisoire : `nicolas-a-michel` (Q12).
- Les tests de fumée des crates sans code (store, index, otlp, daemon, mcp) ne prouvent que la
  compilation et le lien avec core. Ils seront remplacés par de vrais tests dans les phases
  suivantes.

## Phase 2 — Modèle de graphe et stockage

Statut : terminée (validée le 2026-10-16)

- [x] 2.1 Écrire les ADR — critère : ADR écrites (statut « Proposé ») avant le code qui en
  dépend, puis acceptées quand tu valides la phase (procédure du skill `adr`). Sujets :
  - place des composants : consigne la réponse à Q4, option (a) ;
  - classement de fiabilité des sources pour la fusion ;
  - sémantique des snapshots `working_tree` ;
  - outil de migration (`rusqlite_migration` 2.6 ou `refinery` 0.10) ;
  - pragmas SQLite.
- [x] 2.2 `timonier-core` : les types `Node`, `NodeKind`, `SymbolKind`, `Edge`, `EdgeKind`,
  `Provenance`, la confiance, le poids et le span, sérialisables (serde) — critère :
  round-trip JSON testé pour chaque type.
- [x] 2.3 Identité stable : symbole SCIP ou `lang:qualified_name` — critère : tests unitaires
  et proptest (`parse(format(id)) == id`).
- [x] 2.4 Invariant d'arbre strict : un seul parent, pas de cycle — critère : un proptest
  génère arbres et reparentages, et toute violation est refusée par une erreur typée.
- [x] 2.5 Règles de fusion multi-sources — critère : tests en table :
  - une arête vue par TreeSitter et Lsp garde la provenance Lsp ;
  - une arête TreeSitter contredite par le Lsp est supprimée ;
  - `Intended` n'est jamais fusionnée.
- [x] 2.6 Détection des déplacements par signature et hash du corps normalisé, avec un seuil
  fixé par ADR — critère : un fichier renommé, une fonction déplacée entre deux fichiers et
  une fonction renommée à corps égal donnent « déplacé », jamais « supprimé + ajouté ».
- [x] 2.7 `timonier-store` — critère : migrations testées sur base vide ; test de lectures
  concurrentes pendant les écritures. Contenu :
  - schéma SQLite : nœuds, arêtes avec `valid_from` / `valid_to`, `snapshot(seq, commit_sha,
    timestamp, kind)`, état par fichier (hash, « vérifié par LSP au hash X ») ;
  - migrations ;
  - WAL, écrivain unique sur un thread dédié, lecteurs en lecture seule.
- [x] 2.8 Mise à jour par différence d'un fichier — critère : réappliquer un fichier identique
  modifie 0 ligne ; modifier un symbole ferme un intervalle et en ouvre un.
- [x] 2.9 Snapshots `commit`, `review_mark` et `working_tree` — critère : création, liste et
  cohérence des intervalles testées.
- [x] 2.10 Requête de diff entre deux snapshots : ajouts, suppressions, modifications,
  déplacements — critère : tests sur fixtures ; un déplacement de fichier est rapporté comme
  déplacement.
- [x] 2.11 Arêtes entre composants agrégées par requête depuis les arêtes de symboles, jamais
  stockées à la main — critère : test du poids agrégé attendu.
- [x] 2.12 Mesure de référence : insertion et diff sur un graphe synthétique — critère :
  chiffres consignés dans ce plan.
- [x] 2.13 Relecture par `phase-reviewer` — critère : verdict « prête ». Obtenu à la troisième
  relecture ; ses mineurs ont ensuite été corrigés.

Critère de sortie : tests couvrant l'insertion, la mise à jour par différence, le diff de
snapshots et la détection d'un déplacement de fichier.

Décisions :
- [ADR 0008](docs/adr/0008-hierarchie-et-identite-des-noeuds.md) : hiérarchie (Q4 a), forme
  des identifiants, positions ;
- [ADR 0009](docs/adr/0009-portees-et-fusion-des-sources.md) : portées, observations,
  fusion, arêtes pendantes, agrégation ;
- [ADR 0010](docs/adr/0010-versionnement-snapshots-et-diff.md) : tête ouverte, snapshots
  figés, diff par intervalles, déplacements ;
- [ADR 0011](docs/adr/0011-sqlite-migrations-et-ecrivain-unique.md) : rusqlite `bundled`,
  rusqlite_migration, pragmas, fil d'écriture unique.

Constats (2026-10-14, Linux, après deux relectures) :
- `cargo test --workspace` : 105 tests verts.
  - `timonier-core`, 55 tests, dont 5 proptests :
    - aller-retour des identifiants ;
    - toute construction d'arbre valide est acceptée ;
    - un reparentage sous un descendant est un cycle ;
    - sur des mutations aléatoires, le vérificateur est d'accord avec un juge naïf écrit à part.
  - Allers-retours JSON de chaque variante de diff, de `EdgeKey` et de `AggregatedEdge`.
  - `timonier-store`, 9 tests unitaires, dont un test des plans de requête sur le SQLite
    embarqué : les lectures à la tête passent par les index partiels.
  - `timonier-store`, 34 tests d'intégration. Ils couvrent :
    - le critère de sortie : insertion, mise à jour par différence (0 ligne écrite à
      l'identique, fermeture et ouverture de version après gel), diff, déplacement de fichier ;
    - les déplacements de fichier, avec nouveaux identifiants ou identifiants conservés
      (`src/foo/mod.rs` → `src/foo.rs`, dans les deux ordres du lot) ;
    - un symbole déplacé entre fichiers ;
    - un nœud retiré puis rétabli après gel ;
    - un lot où les enfants précèdent leurs parents ;
    - des diffs entre snapshots non consécutifs ;
    - la fusion, une réponse LSP tardive ignorée, et après suppression du fichier ;
    - le refus :
      - des observations précises sans hash, ou qui posent le contenu ;
      - des appels entre symboles hors LSP et SCIP ;
      - de l'intention et des arêtes de composants ;
    - le contenu des portées imposé ;
    - un nœud et une arête fournis par deux fichiers : la tête ne dépend pas de l'ordre, la
      réapplication n'écrit rien, aucun faux déplacement, puis passage de relais à l'autre
      fichier ;
    - la convergence d'un symbole déplacé ;
    - la propriété passée de main en main entre trois portées, sans attente qui reste en
      table ;
    - une attente qui change de version sans toucher la tête ;
    - le retrait d'une portée propriétaire ;
    - la version d'arête la plus fiable, qui l'emporte sur la plus petite clé ;
    - un fichier qui ne peut pas quitter la structure tant qu'une de ses contributions attend ;
    - le refus des cycles, paquets imbriqués, parents interdits, orphelins et second repo ;
    - un proptest d'équivalence entre vérifications incrémentales et vérification complète,
      avec reparentages, retraits et rétablissements ;
    - les lecteurs concurrents ;
    - le test sur modèle : à chaque snapshot figé, nœuds (tous attributs) et arêtes
      identiques ; diffs vérifiés entre toute paire de points ; réapplication sans écriture.
- `cargo clippy --workspace --all-targets -- -D warnings` et `cargo fmt --check` sont verts.
- Mesures (2.12), en build release, sur Ryzen 9 5950X. Le bench donne à chaque fichier 3 appels
  fichier → fichier (tree-sitter) et 2 appels vérifiés par fonction (LSP).

  | `bench [packages] [fichiers] [symboles]` | `20 50 20` (défaut) | `50 100 40` |
  | ------------------------------------------ | ------------------: | ----------: |
  | Taille | 1 000 fichiers, 20 000 symboles, ~40 000 arêtes | 5 000 fichiers, 200 000 symboles, 400 000 arêtes |
  | Chargement initial (une transaction) | 536 ms | 4 739 ms |
  | Réappliquer un fichier identique (médiane) | 0,14 ms | 0,19 ms |
  | Changer un fichier, lignes nées dans la tête | 0,84 ms | 1,28 ms |
  | Changer un fichier, après un gel à chaque échantillon | 1,09 ms | 1,61 ms |
  | Changer 10 % des fichiers, tous distincts | 85 ms (100 fichiers) | 801 ms (500) |
  | Diff depuis la marque de revue | 22 ms (2 000 nœuds, 8 000 arêtes) | 247 ms (20 000, 80 000) |
  | Lire tous les nœuds à la tête | 16 ms | 163 ms |
  | Agréger au niveau des packages | 48 ms | 627 ms |

CI GitHub (2026-10-16) :
- commit `48e397f` (run 38860803467) : rouge. Clippy 1.99, plus récent que le 1.96 installé en
  local, refusait un `chunks_exact(2)` (lint `chunks_exact_to_as_chunks`). Un test
  d'intégration a aussi échoué sous Linux : la fenêtre de VSCode 1.142.0, sortie la veille, a
  été déclarée « unresponsive » sous xvfb avant le premier test.
- commit `72f2ae5` (run 38861207962) : les 5 jobs sont verts. Le correctif utilise
  `as_chunks::<2>()`, stable depuis 1.88 et donc compatible MSRV. Le même test d'intégration est
  passé, avec la même version de VSCode : l'échec Linux était intermittent.
- La toolchain stable locale est passée à 1.99.1, pour lancer en local le même clippy que la
  CI.

Relecture n° 1 (`phase-reviewer`) : « pas prête », avec 1 bloquant et 6 importants (B1, I1 à
I6) et 10 mineurs. Tous ont été corrigés ou consignés :
- B1 : déplacement « même identifiant », que le fichier suit ;
- I1 : `computed_on` ;
- I2 : reprise entre portées, revue ensuite (N1) ;
- I3 : granularité des `Calls` imposée ;
- I4 : vérifications incrémentales testées ;
- I5 : critère 2.1 soumis à ta validation (Q15) ;
- I6 : bench corrigé.

Relecture n° 2 : plus aucun bloquant ; deux défauts importants introduits ou laissés par les
corrections, et 8 mineurs. Tout est corrigé ou consigné :
- N1 : avec « la dernière portée l'emporte », la tête dépendait de l'historique quand deux
  fichiers fournissent le même identifiant (bascules, écritures inutiles, faux déplacements).
  Corrigé : propriété à la portée de plus petite clé, et versions des autres en attente qui
  prennent le relais (ADR 0009).
- N2 : une réponse LSP tardive portant un hash de contenu ramenait l'ancien contenu. Corrigé :
  une source précise ne peut plus poser le hash de contenu.
- Mineurs corrigés :
  - n1 : contenu des portées imposé, et arêtes observées qui partent de ce que la portée
    fournit ;
  - n2 : observations sur une portée sans contenu périmées, sources précises toujours ancrées ;
  - n3 : appels entre symboles venant de `Manifest` refusés ;
  - n4 : appariement par signature seulement s'il est unique des deux côtés ;
  - n5 : tests élargis ;
  - n6 : allers-retours JSON complets ;
  - n7 : test des plans de requête.
- Mineur consigné (n8) : les deux reports sont maintenant écrits dans les tâches 3.7 et 5.2.

Relecture n° 3 : « prête », sans bloquant ni important ; 10 mineurs. Corrigés :
- m1 : les contributions en attente comptent dans le contrôle des orphelins ;
- m2 : une portée de fichier sans contenu ne prend aucune observation ;
- m3 : une source précise n'apporte pas de nœuds ;
- m4 : pour une arête, la provenance la plus fiable l'emporte avant la plus petite clé
  (conforme à la spec « Fusion ») ;
- m5 : commentaire périmé ;
- m6 : tests à trois portées, attente qui change, retrait du propriétaire, absence de fuite ;
- m7 : une version fermée garde le propriétaire de son époque, et les rétrogradations sont
  comptées ;
- m8 : un retrait n'apporte rien d'autre ;
- m9 : allers-retours avec `computed_on` et `Freeze` ;
- m10 : skill corrigé.

Écarts :
- Le stockage ne menace pas l'objectif de 100 ms de la Phase 3 : moins de 2 ms par fichier.
- L'agrégation relit tout le graphe : 0,63 s à 200 000 symboles. La Phase 4 devra la mettre en
  cache par vue.
- Le critère de 2.1 a été reformulé ; tu as accepté le nouveau libellé (Q15).
- Limites assumées (ADR 0010) :
  - un fichier sans symbole déplacé reste « supprimé + ajouté » ;
  - un type supprimé et un autre ajouté, chacun avec une seule méthode de même nom et même
    signature, s'apparient encore.

## Phase 3 — Indexation

Statut : terminée (validée par toi le 2026-10-16, sans verdict « prête » : voir 3.12)

Réponses appliquées : Q5 (interface LSP commune, relais de l'IDE d'abord) et Q6 (heuristiques,
enrichies par l'agent via MCP).

- [x] 3.1 ADR sur le format des packs de langage : manifeste versionné, requêtes `.scm`,
  heuristiques, emplacement — critère : ADR écrite (« Proposé »), acceptée quand tu valides la
  phase (Q15) ; skill `language-pack` mis à jour.
  - [ADR 0012](docs/adr/0012-format-des-packs-de-langage.md) : packs dans
    `crates/timonier-index/packs/<langue>/`, incorporés au binaire ; contrat des captures
    vérifié au chargement.
  - Restent du code : la table des grammaires (une ligne par grammaire) et les écosystèmes
    `cargo` et `npm`.
- [x] 3.2 Packs Rust et TypeScript : grammaires (tree-sitter 0.27.1, tree-sitter-rust 0.24.2,
  tree-sitter-typescript 0.23.2), requêtes de définitions, d'imports et
  d'appels/références — critère : snapshots insta des nœuds et arêtes extraits de fixtures
  Rust et TS.
  - Trois instantanés relus ligne à ligne (Rust, TS, TSX), dans
    `crates/timonier-index/tests/snapshots/`.
  - La résolution des chemins en arêtes est testée à part
    (`crates/timonier-index/src/project.rs`), dont les réexportations Rust et TS.
- [x] 3.3 Noms qualifiés stables et hash de corps normalisé, par langage — critère : reformater
  un fichier ne change aucune identité (test).
  - Testé en Rust et en TypeScript : un fichier reformaté garde chaque nom qualifié, chaque
    signature et chaque hash de corps.
  - Réordonner les définitions ne change aucune identité (test).
  - Un déplacement de fonction entre fichiers et un renommage de fichier donnent « déplacé »
    dans le diff (test de bout en bout, `crates/timonier-daemon/tests/indexing.rs`).
- [x] 3.4 Indexation initiale parallélisée, avec un cache persistant (hash par fichier) —
  critère : une deuxième indexation sans changement ne reparse aucun fichier (compteur).
  - Cache dans SQLite (migration 0002, [ADR 0013](docs/adr/0013-extraction-resolution-et-cache.md)).
  - Seconde indexation : 0 fichier reparsé ; au redémarrage sur la même base, 0 ligne écrite
    (tests).
- [x] 3.5 Vitesse 1, à la sauvegarde — critère : latence mesurée selon H2. Contenu :
  - watcher (notify 8.2 + notify-debouncer-full 0.7), anti-rebond ~300 ms ;
  - respect de `.gitignore` (crate `ignore`) ;
  - hash du fichier, reparse tree-sitter incrémental, mise à jour par différence.

  Constat (2026-10-16, build release, Ryzen 9 5950X), sur une copie du dépôt Timonier
  (71 fichiers) et 200 sauvegardes : chiffres de 3.11 (p50 2,5 ms, p95 10,4 ms, max 24 ms) ;
  - la mesure va du début de la passe (fin de l'anti-rebond) au commit SQLite ;
  - outil : `cargo run --release -p timonier-daemon --example index_bench`.

  Le watcher réel est testé de bout en bout : sauvegarde, anti-rebond, graphe mis à jour.
- [x] 3.6 Vitesse 2, en arrière-plan, selon Q5 — critère : test avec un serveur LSP réel ou un
  double conforme. Contenu :
  - les fichiers dépendants sont marqués quand une signature publique change ;
  - file LSP basse priorité, en commençant par ce qui est affiché ;
  - arêtes fonction → fonction issues du LSP ;
  - « vérifié par LSP au hash X ».

  [ADR 0014](docs/adr/0014-affinage-lsp-en-arriere-plan.md) : interface `LanguageServer`
  (hiérarchie d'appels), file à trois priorités (affiché, ouvert, dépendant), seuls les `Calls`
  vérifiés.

  Constat : 3 tests avec un double conforme au protocole (`crates/timonier-daemon/tests/refinement.rs`) :
  - arêtes fonction → fonction en provenance `Lsp`, qui remplacent les `Calls` tree-sitter du
    fichier ;
  - « vérifié par LSP » perdu quand le contenu change, et le fichier revient en file ;
  - dépendants renvoyés en file quand une signature change ;
  - cession du travail à une sauvegarde ;
  - fichier sans serveur ignoré ;
  - affinage par le watcher quand il est libre.

  Le relais des serveurs de l'IDE est branché en Phase 4, avec le lien démon ↔ extension.
- [x] 3.7 Vitesse 3, au commit : détection du commit, re-clustering complet initialisé par la
  partition précédente, snapshot `commit` figé, sans SCIP (H4) — critère : test d'intégration
  sur un dépôt git temporaire.
  - Le snapshot doit décrire l'arbre commité, pas l'arbre de travail indexé avec ses
    modifications non commitées (ADR 0010, « Conséquences ») : indexer le commit, ou marquer le
    snapshot comme « sale ».

  [ADR 0015](docs/adr/0015-consolidation-au-commit.md) : on indexe l'arbre commité. Les
  fichiers qui diffèrent du commit prennent leur version commitée le temps du gel, puis
  l'arbre de travail revient. La détection passe par le watcher, sur le dossier git.

  Constat : 2 tests sur un dépôt git temporaire (`crates/timonier-daemon/tests/commit.rs`) :
  - le snapshot du commit contient la modification commitée, mais ni le travail en cours ni le
    fichier non suivi, que la tête garde ;
  - le retour à l'arbre de travail n'écrit aucune ligne ;
  - le watcher voit un vrai `git commit`.

  Le re-clustering à ce moment est fait avec 3.8.
- [x] 3.8 Composants — critère : stabilité mesurée (une petite modification change moins de
  X % des affectations, seuil fixé par ADR). Contenu :
  - agrégation par structure (membres du workspace Cargo, workspaces npm, dossiers) ;
  - clustering Louvain/Leiden stable. ADR : `graphrs` 0.12 (MIT) est le seul crate vérifié
    qui offre Louvain et Leiden, avec une adoption faible ; l'alternative est d'implémenter
    Leiden nous-mêmes.

  [ADR 0016](docs/adr/0016-composants-et-clustering.md) :
  - un niveau ne se découpe qu'au-delà de 12 éléments, par dossiers d'abord, puis par Leiden ;
  - Leiden écrit par nous, car graphrs ne part pas d'une partition initiale ;
  - les clés restent stables d'un clustering à l'autre ;
  - seuil de stabilité : 5 %.

  Constats :
  - Graphe synthétique de 300 nœuds : moins de 5 % des affectations changent (test).
  - Sur Timonier, après 200 sauvegardes dont 100 ajoutent un appel vers un autre fichier du
    paquet : 0 affectation changée sur 23 ; re-clustering en 4,4 ms.
  - Seul `timonier-index` dépasse 12 fichiers. On y trouve un composant `tests` et deux
    groupes : les composants (`cluster`, `components`, `structure`) et l'extraction avec la
    résolution.
  - Un redémarrage reprend les composants de la tête, sans clustering, et n'écrit rien
    (test). Un fichier laissé seul par Leiden le reste (test).
- [x] 3.9 Rôles architecturaux et dépendances externes — critère : sur Timonier, les rôles
  attendus sont vérifiés par un test (par exemple `timonier-daemon` = serveur,
  `timonier-store` = persistance). Contenu :
  - rôles par heuristiques des packs (TcpListener, routeur HTTP, ORM, types sérialisés…) ;
  - dépendances externes regroupées par usage (réseau, sérialisation, base de données,
    async) ;
  - nommage et confirmation selon Q6.

  [ADR 0017](docs/adr/0017-roles-architecturaux.md) : un attribut `role` sur les paquets et
  les composants (migration 0003), calculé à partir de trois signaux, les bibliothèques, les
  imports et le code. Les tests et exemples ne comptent pas.

  Constat (`crates/timonier-daemon/tests/timonier.rs`, sur le dépôt lui-même) :

  | Paquet | Rôle |
  | ------ | ---- |
  | cli | point d'entrée |
  | core | modèle de données |
  | daemon | tâche de fond |
  | index | utilitaire |
  | store | persistance |
  | webview | interface |
  | extension | interface |

  - Les dépendances externes sont rangées par usage (base de données, sérialisation, ui…).
  - Écart : `timonier-daemon` ne peut pas encore être « serveur ». Il n'a pas de serveur
    JSON-RPC avant la Phase 4, et le test sera mis à jour alors.
  - Le nommage et la confirmation par l'agent (Q6) arrivent avec les outils MCP de la
    Phase 6.
- [x] 3.10 Compaction au-delà d'environ 50 000 arêtes de symboles — critère : test de charge
  synthétique.

  [ADR 0018](docs/adr/0018-compaction-des-aretes-de-symboles.md) : budget de 50 000 appels
  fonction → fonction issus du LSP. Au-delà, les fichiers vérifiés les plus anciens gardent
  leurs appels de fichier à fichier, toujours en provenance `Lsp` et vérifiés, jusqu'à 90 %
  du budget.

  Constats :
  - Test de comportement : compaction du plus ancien, vérification conservée, retour au
    détail quand le fichier est rouvert.
  - Test de charge, en build release : 260 fichiers de 20 fonctions, 52 000 appels vérifiés
    en 0,83 s. Il reste 46 800 appels de symbole à symbole dans la tête, et 26 fichiers sont
    compactés.
  - Commande : `cargo test --release -p timonier-daemon --test compaction -- --ignored`.
- [x] 3.11 Mesures sur le dépôt Timonier — critère : indexation à froid ≤ 3 s (H3) et mise à
  jour p95 < 100 ms (H2), chiffres et machine consignés.

  Constats (2026-10-16, après les corrections de la relecture n° 1 ; build release, Ryzen 9
  5950X, 32 fils, Linux), sur 71 fichiers Rust et TypeScript, soit 1 557 nœuds et 666 arêtes :

  | Mesure | Valeur | Cible |
  | ------ | -----: | ----: |
  | Indexation à froid du dépôt, consolidation git comprise | 456 ms | ≤ 3 s |
  | Indexation à froid d'une copie, sans git | 145 ms | |
  | Redémarrage avec le cache | 95 ms, 0 reparse, 0 ligne écrite | |
  | Sauvegarde, p50 / p95 / max (200 sauvegardes) | 2,5 / 10,4 / 24,4 ms | p95 < 100 ms |
  | Re-clustering | 4,4 ms, 0 affectation changée | |

  - La consolidation git est la plus coûteuse ici : les fichiers non commités prennent leur
    version de HEAD le temps du gel.
  - Les arêtes passent de 690 à 666 : les appels de méthode sur un receveur inconnu ne sont
    plus résolus par leur seul nom (relecture n° 1, I6).
  - Outil : `cargo run --release -p timonier-daemon --example index_bench`.
- [ ] 3.12 Relecture par `phase-reviewer` — critère : verdict « prête ». **Non atteint.**
  - Les trois relectures concluent « pas prête » ; tous leurs constats sont corrigés
    (ci-dessous).
  - Tu as validé la phase le 2026-10-16 pendant la relecture n° 3. La case restait cochée à
    tort : la relecture n° 3 l'a relevé (P1). L'écart est consigné plus bas.

  Relecture n° 1 : « pas prête », avec 1 bloquant, 7 importants et 10 mineurs. Tous sont
  corrigés, chaque scénario reproduit est gardé comme test
  (`crates/timonier-daemon/tests/robustness.rs` et les tests cités) :
  - B1 :
    - un paquet renommé ou retiré laissait un composant sous un paquet disparu, et le
      stockage refusait ensuite toute structure ;
    - corrigé : les composants sont recalculés quand les paquets changent, et seuls les
      composants valides sont émis ;
    - après une écriture refusée, tout est indexé à nouveau pour remettre la mémoire
      d'accord avec la base.
  - I1 : une réexportation qui change de cible résout à nouveau ceux qui la citent.
  - I2 : un manifeste illisible ou un chemin sans identifiant possible sont écartés et
    signalés, sans arrêter la passe ; un manifeste en cours d'édition garde les paquets.
  - I3 : la consolidation ne prend que les fichiers que le parcours indexerait (dossiers
    cachés, `.gitignore`).
  - I4 : un `lib.rs` enregistré n'entraîne plus de resynchronisation ; une resynchronisation
    n'a lieu que si les paquets changent vraiment.
  - I5 : l'état d'affinage LSP est relu de la base au démarrage (dépendants, budget). Les
    vérifications tombent quand les identifiants changent.
  - I6 : les appels sur un receveur inconnu (`x.iter()`) ne sont plus résolus par leur seul
    nom (capture `@reference.method`, version d'extracteur 2). Les composants de Timonier en
    sont plus justes.
  - I7 : un redémarrage reprend les composants de la tête au lieu de les recalculer
    (ADR 0016).
  - Mineurs corrigés :
    - m1 : un singleton de Leiden reste seul ; un nœud isolé quitte sa communauté ;
    - m2 : le bench ajoute de vraies arêtes ; chiffres mis à jour ;
    - m3 : nom de test corrigé ;
    - m4 : tests de `git.rs` (sous-dossier, worktree, dépôt sans commit), de
      `Reader::symbol_at`, `scopes` et `observations` ;
    - m5 : erreur de création du fil rendue, pas de `expect` ;
    - m6 : ADR 0012 complétée ; la clé du cache inclut la version de la grammaire, et un test
      la compare à l'espace de travail ;
    - m7 : préfixes `@tests`, `@bin`… pour les cibles Rust ; `src/` transparent sans fusion de
      dossiers ;
    - m8 : `proptest` retiré de `timonier-index` ;
    - m9 : le watcher ne surveille que les dossiers parcourus et indexe une fois les
      surveillances posées ;
    - m10 : les échecs de git sont distingués et signalés ; un fichier devenu trop gros quitte
      le graphe.
  - Trouvé en corrigeant : le commit pouvait arriver au watcher avant les sauvegardes qui
    l'ont précédé. La consolidation relit d'abord les fichiers changés depuis le dernier
    commit (ADR 0015).

  Relecture n° 2 : « pas prête ». Elle confirme B1, I1 à I6 et presque tous les mineurs
  corrigés, mais trouve 1 bloquant, 4 importants et 5 mineurs. Tous sont corrigés, et chacun
  est un test de `crates/timonier-daemon/tests/robustness.rs` :
  - N1 (bloquant, introduit par m9) : un dossier supprimé puis recréé (changement de branche)
    n'était plus surveillé. Le watcher oublie les dossiers disparus et resurveille ceux qui
    reviennent.
  - N2 : un manifeste illisible quelque part gelait tous les paquets ; seul son paquet reste
    figé.
  - N3 : `git diff` sans `--no-renames` laissait l'ancien chemin d'un renommage dans le
    snapshot de commit.
  - R1 : une tête sans composant déclenchait un clustering au redémarrage. Seule une base
    jamais indexée en déclenche un.
  - N4 : une sauvegarde dans un sous-dossier pendant l'indexation initiale était perdue. Tous
    les dossiers sont surveillés avant, et chaque dossier ajouté ensuite est relu.
  - Mineurs :
    - m-a : un paquet renommé pendant l'arrêt garde ses composants (nouveau clustering) ;
    - m-b : la version de grammaire des packs est comparée à `Cargo.lock` ;
    - m-c : la file d'affinage garde ses demandes en attente quand la base est relue ; un
      fichier vérifié sans appel n'est plus « compacté » ; l'ordre après redémarrage est
      documenté (ADR 0018) ;
    - m-d : en Rust, la résolution par nom seul reste dans la même cible ;
    - m-e : chiffres de 3.5 et mentions du redémarrage mis à jour.

  Relecture n° 3 : « pas prête ». Tous les constats de la n° 2 sont corrigés, sauf N4 dans les
  faits ; elle trouve 1 bloquant, 1 important et 1 mineur, corrigés après ta validation :
  - F1 (bloquant) : le watcher prenait les lectures de fichiers pour des modifications, y
    compris celles du démon. Il s'en nourrissait sans fin :
    - 46 passes en 8 s sur un dépôt au repos ;
    - un affinage LSP qui cédait toujours ;
    - sur 20 040 fichiers, une sauvegarde jamais vue et un arrêt de 158 s.

    Corrigé :
    - seuls les événements qui peuvent changer le graphe sont gardés (création, suppression,
      écriture, renommage) ;
    - `refresh` ne parcourt plus tous les fichiers pour chaque chemin reçu.

    Les sondes de la relecture donnent maintenant :
    - 2 passes en 8 s (l'indexation, puis la sauvegarde) et un affinage vérifié ;
    - sur 20 040 fichiers, la sauvegarde faite à 1 s est vue et l'arrêt prend 67 ms.

    N4 est donc corrigé lui aussi. Test : `the_watch_rests_when_nothing_changes`.
  - P1 : 3.12 était cochée sans verdict « prête ». Décochée, écart consigné.
  - m-f : un `refresh` sur un nouveau dossier ne trouvait pas son manifeste. Un dossier reçu
    est maintenant parcouru, et les paquets sont redécouverts quand l'ensemble des manifestes
    change. Test : `a_new_crate_is_a_package`.

  Contre-vérification de ces trois corrections par `phase-reviewer` :
  - F1 et P1 sont corrigés, N4 aussi ; aucune régression trouvée.
  - m-f ne l'était qu'en partie. Les parcours qui ne redécouvrent pas les paquets mettaient
    aussi à jour l'ensemble des manifestes : celui d'un nouveau fichier, celui du watcher et
    celui de la consolidation.
    - Effet : un crate créé de 100 à 250 ms après un autre fichier n'était jamais reconnu.
    - Corrigé : l'ensemble est celui d'où les paquets ont été découverts. Tout parcours de
      `refresh` qui en trouve un autre les redécouvre.
    - Test : `a_new_crate_seen_by_another_walk_first_is_a_package`, qui compare le résultat
      à une indexation neuve. Les sondes de la relecture reconnaissent maintenant le crate
      quel que soit le délai.
  - Hypothèse de la relecture : sous macOS, une écriture serait vue comme un simple changement
    de métadonnées. Elle est écartée : notify traduit `ITEM_MODIFIED` en `Modify(Data)`, et la
    CI macOS voit les sauvegardes.

Critère de sortie : le dépôt Timonier s'indexe en quelques secondes ; une modification de
fichier met le graphe à jour en moins de 100 ms (mesuré).

Décisions :
- [ADR 0012](docs/adr/0012-format-des-packs-de-langage.md) : format des packs de langage ;
- [ADR 0013](docs/adr/0013-extraction-resolution-et-cache.md) : extraction, noms qualifiés,
  empreinte, résolution approchée, réexportations, cache ;
- [ADR 0014](docs/adr/0014-affinage-lsp-en-arriere-plan.md) : affinage LSP, file à trois
  priorités ;
- [ADR 0015](docs/adr/0015-consolidation-au-commit.md) : snapshot de commit fidèle à l'arbre
  commité ;
- [ADR 0016](docs/adr/0016-composants-et-clustering.md) : composants, Leiden écrit par nous,
  stabilité ;
- [ADR 0017](docs/adr/0017-roles-architecturaux.md) : rôles et dépendances externes ; un
  attribut `role` sur les nœuds ;
- [ADR 0018](docs/adr/0018-compaction-des-aretes-de-symboles.md) : compaction des arêtes de
  symboles.

Écarts :
- 3.12 n'est pas atteinte : aucune relecture n'a conclu « prête ». C'est ta validation du
  2026-10-16 qui clôt la phase. Les constats de la relecture n° 3 ont été corrigés ensuite,
  sans quatrième relecture complète.
- Le relais des serveurs LSP de l'IDE et le rôle « serveur » du démon attendent la Phase 4.
  L'affinage est testé avec un double conforme, comme le critère de 3.6 le permet.
- Les composants sont ceux du dernier commit (ADR 0016). Avec beaucoup de travail non
  commité, la tête montre les composants calculés sur le commit. Un fichier nouveau rejoint
  ses voisins de dossier.
- Limites assumées :
  - les appels hors de toute fonction disparaissent quand le LSP vérifie le fichier
    (ADR 0014) ;
  - les noms qualifiés Rust sont approchés (`#[path]` ignoré, fichiers de `main.rs`) ;
  - une mise en forme qui ajoute des jetons change le hash du corps (ADR 0013).
- Packs livrés dans le binaire seulement ; les packs de l'utilisateur viendront plus tard
  (ADR 0012). Un nouvel écosystème (Python, Go) demande du code dans `timonier-index`.
- Pour le diagnostic : `cargo run --release -p timonier-daemon --example index_dump` montre
  l'arbre, les rôles et les arêtes les plus lourdes d'un dépôt.

## Phase 4 — Démon, extension et carte

Statut : à faire

- [ ] 4.1 ADR sur le protocole — critère : ADR acceptée. Sujets :
  - socket local, un démon par projet avec verrou, découverte ;
  - méthodes et versionnement ;
  - génération des types partagés Rust → TypeScript ;
  - jsonrpsee 0.26 ou implémentation dédiée.
- [ ] 4.2 Serveur JSON-RPC 2.0 sur socket Unix : `initialize`, abonnement et désabonnement par
  vue, deltas versionnés, snapshot complet en cas de désynchronisation, arrêt — critère :
  tests d'intégration client Rust ↔ démon, dont une désynchronisation simulée qui renvoie un
  snapshot complet.
- [ ] 4.3 CLI `timonier daemon start|stop|status` — critère : tests assert_cmd.
- [ ] 4.4 Extension : lancement et supervision du démon (redémarrage avec backoff, canal de
  sortie « Timonier »), client JSON-RPC (vscode-jsonrpc 9.0), relais postMessage typé —
  critère : test d'intégration extension ↔ démon réel ; un démon tué est relancé et la vue
  se resynchronise.
- [ ] 4.5 Relais du LSP de l'IDE vers le démon, si Q5 le retient — critère : définitions et
  références obtenues via l'API VSCode et reçues par le démon (test).
- [ ] 4.6 Système de design minimal, avant la première vue — critère : contraste AA vérifié
  automatiquement (axe-core) en clair, sombre et contraste élevé ; ADR sur le système de
  design. Contenu :
  - tokens : couleurs sémantiques sur les variables VSCode, typographie, espacements, rayons,
    durées ;
  - composants : boîte de composant, arête TreeSitter vs arête confirmée, badge, état vide,
    info-bulle, barre d'outils ;
  - une page de démonstration en mode développement.
- [ ] 4.7 Carte (React Flow 12.12 + elkjs 0.12) — critère : sur Timonier, la vue d'ouverture
  tient en quelques boîtes nommées par rôle. Contenu :
  - vue haut niveau par rôles ;
  - zoom sémantique récursif : rôles → composants → sous-composants → fichiers → symboles ;
  - divulgation progressive au survol et au clic.
- [ ] 4.8 Mise en page stable, avec les positions précédentes comme contraintes d'elkjs —
  critère : après la modification d'un fichier, le déplacement moyen des nœuds inchangés
  reste sous un seuil mesuré et consigné.
- [ ] 4.9 Navigation — critère : test d'intégration de l'ouverture du code ; checklist
  d'accessibilité du skill. Contenu :
  - clic vers le code, ouvert au bon span ;
  - navigation au clavier ;
  - une commande de palette par action ;
  - respect de la réduction des mouvements.
- [ ] 4.10 Rendu par niveau de détail, avec agrégation au-delà d'un seuil de nœuds — critère :
  images par seconde mesurées et consignées sur un graphe synthétique de plusieurs milliers
  de nœuds.
- [ ] 4.11 Test des cinq secondes avec toi — critère : réussi et consigné.
- [ ] 4.12 Relecture par `phase-reviewer` — critère : verdict « prête ».

Critère de sortie : la carte de Timonier s'affiche, reste stable quand tu modifies un fichier,
et passe le test des cinq secondes avec toi.

Décisions : —
Écarts : —

## Phase 5 — Diff de revue

Statut : à faire

- [ ] 5.1 Commande et bouton « Marquer comme revu », qui crée un snapshot `review_mark` (méthode
  JSON-RPC) avec une confirmation discrète — critère : test d'intégration.
- [ ] 5.2 Diff structurel depuis la dernière `review_mark` — critère : tests sur des paires de
  snapshots fixtures. Les arêtes pendantes, que les lectures brutes renvoient, sont filtrées
  (ADR 0009). Le diff montre :
  - composants ajoutés et supprimés ;
  - dépendances ajoutées et supprimées ;
  - cycles ajoutés (composantes fortement connexes du graphe des composants) ;
  - composants dont le fan-in ou le fan-out a bondi (seuil fixé par ADR) ;
  - un badge « modifié » pour un refactoring interne, où les déplacements comptent comme des
    modifications.
- [ ] 5.3 Rendu sur la carte : badges avec icône et texte (jamais la couleur seule), filtre
  « seulement les changements », résumé — critère : checklist d'accessibilité ; test des
  cinq secondes.
- [ ] 5.4 Relecture par `phase-reviewer` — critère : verdict « prête ».

Critère de sortie (proposé, à valider) : après « marquer comme revu », on ajoute à Timonier
une dépendance entre deux crates et un module ; la carte montre exactement ces changements, et
un cycle introduit dans une fixture est signalé.

Décisions : —
Écarts : —

## Phase 6 — MCP, initialisation et suivi

Statut : à faire

Prérequis : Q10 et Q11.

- [ ] 6.1 ADR — critère : ADR acceptées. Sujets :
  - SDK MCP (rmcp 3.5) et transport stdio ;
  - forme de l'en-tête de boîte aux lettres ;
  - formats versionnés (schémas JSON) de `.timonier/plan.json`, `todo.json` et
    `sessions/<id>.json`.
- [ ] 6.2 `timonier-mcp` : serveur MCP, mince proxy vers le démon (qu'il démarre au besoin),
  avec l'en-tête de boîte aux lettres dans chaque réponse (H10) — critère : test d'intégration
  client MCP → proxy → démon ; snapshot de la liste des outils.
- [ ] 6.3 Outils de suivi `session_start`, `session_end`, `plan_add_task`, `plan_update_task`,
  `todo_add`, `todo_complete`, plus les outils de lecture selon Q10. Écritures structurées et
  validées, le démon est le seul écrivain — critère : les entrées invalides et les transitions
  d'état interdites sont refusées (tests).
- [ ] 6.4 Les changements faits hors session sont marqués « non suivis » — critère : une
  modification sans session ouverte est marquée (test).
- [ ] 6.5 Dette de revue : chaleur par composant (code écrit en session et non couvert par une
  `review_mark` ultérieure ; changements hors session = non suivis), en calque sur la carte —
  critère : test sur fixtures ; checklist d'accessibilité.
- [ ] 6.6 Panneaux — critère : les panneaux se mettent à jour en direct quand l'agent appelle
  les outils (test d'intégration) ; test des cinq secondes. Contenu :
  - plan : jalons → tâches, avancement calculé, panneau latéral permanent ;
  - todo en direct ;
  - journal des sessions : objectif, tâches, composants touchés, diff structurel, tests
    (l'effet sur les performances arrive en Phase 8).
- [ ] 6.7 Le pont (H5) : où en est le plan, ce qui s'est passé depuis la dernière revue, où
  regarder — critère : test des cinq secondes.
- [ ] 6.8 `timonier init` — critère : tests sur des projets fixtures Rust et TS ; une deuxième
  exécution ne change rien ; `--dry-run` affiche le diff exact. Contenu :
  - détection : langages, gestionnaire de paquets, runner de tests, agents installés, état
    git ;
  - squelette de `.timonier/` ;
  - bloc géré, délimité par des marqueurs, dans `AGENTS.md` et ses équivalents ;
  - déclaration MCP dans `.mcp.json` et `.vscode/mcp.json` ;
  - hooks d'agent (refus de clore une session si la todo n'est pas à jour ou si le journal
    est vide) ;
  - hook git qui régénère `actual.json`, base SQLite ignorée par git ;
  - plan avec un jalon « Initialisation » et une première session ;
  - `--dry-run` et idempotence.
- [ ] 6.9 `timonier uninit` — critère : init puis uninit rend un arbre identique à l'original
  (test).
- [ ] 6.10 `timonier doctor` : vérifie et répare la configuration (MCP déclaré, bloc géré à
  jour, conventions présentes). Le test de fumée tracé arrive en Phase 8 (H6) — critère :
  chaque diagnostic et chaque réparation sont testés.
- [ ] 6.11 Packs Rust et TypeScript, volet init : détection du runner, configuration JUnit XML,
  conventions copiées dans `.timonier/conventions/` — critère : tests sur fixtures.
- [ ] 6.12 Détection dans VSCode d'un dépôt non configuré, avec une proposition discrète de
  lancer init — critère : test d'intégration.
- [ ] 6.13 Bascule : `timonier init` sur le dépôt Timonier. Ensuite, une session Timonier par
  tâche, et plan et todo tenus via MCP selon Q11 — critère : vérifiable dans le journal des
  sessions.
- [ ] 6.14 Relecture par `phase-reviewer` — critère : verdict « prête ».

Critère de sortie : Timonier pilote son propre développement.

Décisions : —
Écarts : —

## Phase 7 — Tableau blanc et intention

Statut : à faire

Réponses appliquées : Q3 (pas de tldraw) et Q6.

- [ ] 7.0 ADR sur le remplaçant de tldraw (React Flow ou Excalidraw, voir Q3), soumise à ta
  validation car la spec en parlait explicitement — critère : ADR acceptée, licence vérifiée.
- [ ] 7.1 Tableau blanc dans la webview, avec la bibliothèque retenue en 7.0 — critère : dessin
  libre, annulation et clavier fonctionnent dans le build de production du panneau, pas
  seulement en développement.
- [ ] 7.2 Formes typées optionnelles — critère : sérialisation testée. Correspondances :
  - boîte → composant voulu (nom, rôle) ;
  - flèche → arête `allowed`, `forbidden` ou `required` ;
  - zone → couche ;
  - note → contrainte en texte rattachée à un élément ;
  - note budget (latence p50/p95, mémoire max, allocations, nombre d'appels).
- [ ] 7.3 `.timonier/intent.json` version 1, conforme à la spec : sémantique séparée de la
  géométrie, IDs `C-` / `E-` / `L-` / `N-` stables, écrit seulement sur action de l'humain,
  ordre stable pour des diffs git lisibles, schéma JSON — critère : round-trip testé ; les
  IDs ne changent pas quand une forme est déplacée.
- [ ] 7.4 Interprétation du dessin libre (texte et vision), selon Q6 — critère : défini avec la
  réponse.
- [ ] 7.5 Outils MCP `get_intended_architecture` et `get_drift` — critère : tests
  d'intégration MCP.
- [ ] 7.6 Option « comparer à l'intention » sur la carte — critère : des fixtures d'intention et
  de graphe donnent la dérive attendue. Contenu :
  - ancrage boîte dessinée ↔ composant réel par nom et emplacement, calculé et jamais écrit ;
  - composants dessinés mis en évidence, composants manquants listés ;
  - arêtes interdites franchies signalées (une arête entre couches s'applique à tout leur
    contenu) ;
  - arêtes requises absentes signalées.
- [ ] 7.7 Relecture par `phase-reviewer` — critère : verdict « prête ».

Critère de sortie (proposé, à valider) : tu dessines trois boîtes et une flèche interdite pour
Timonier ; la carte les met en évidence, liste les manquants et signale la violation ; l'agent
lit l'intention via MCP.

Décisions : —
Écarts : —

## Phase 8 — Performances

Statut : à faire

- [ ] 8.1 ADR — critère : ADR acceptées. Sujets :
  - stockage des traces et des métriques ;
  - nom de la feature qui désactive l'instrumentation ;
  - niveau 1 sur macOS (H8) ;
  - compteurs matériels.
- [ ] 8.2 `timonier test -- <commande>`, niveau 1 — critère : test d'intégration sur un projet
  fixture. Contenu :
  - durée, CPU, mémoire max et I/O via cgroups v2 / procfs sur Linux, avec un repli sur
    macOS ;
  - durée par test lue dans le JUnit XML (quick-junit 0.8).
- [ ] 8.3 Injection de `OTEL_EXPORTER_OTLP_ENDPOINT` et `OTEL_RESOURCE_ATTRIBUTES`
  (`timonier.run_id`, `timonier.snapshot`) ; chaque test devient un span racine — critère :
  test.
- [ ] 8.4 `timonier-otlp` : récepteur gRPC 4317 et HTTP 4318 (tonic 0.14, opentelemetry-proto
  0.33), ports occupés gérés, rattachement au graphe par les attributs `code.*` — critère :
  test d'intégration avec un vrai exporteur OTLP.
- [ ] 8.5 Vérification de l'instrumentation : fonctions publiques jamais vues, spans sans
  `code.*`, surcoût estimé contre le niveau 1 ; outils MCP `get_instrumentation_gaps` et
  `get_instrumentation_conventions(lang)` — critère : une fixture aux trous connus les fait
  tous remonter.
- [ ] 8.6 Instrumentation — critère : `timonier doctor` produit une trace rattachée au graphe.
  Contenu :
  - Timonier s'instrumente lui-même, derrière la feature ;
  - volet instrumentation de `timonier init` pour Rust et TS (SDK OpenTelemetry, module
    d'initialisation) ;
  - test de fumée de `timonier doctor` (H6).
- [ ] 8.7 Vues — critère : test des cinq secondes. Contenu :
  - timeline par test (spans emboîtés, ressources du processus dessous, uPlot) ;
  - carte annotée (temps cumulé, nombre d'appels, arêtes épaissies selon le trafic) ;
  - évolution par commit.
- [ ] 8.8 Budgets — critère : un budget dépassé volontairement est visible sur la carte et
  lisible via MCP. Contenu :
  - chaque note budget de l'intention est comparée à la médiane de plusieurs runs ;
  - un dépassement apparaît en rouge, avec une icône, sur la carte et dans le journal ;
  - outils MCP `get_budgets`, `get_budget_violations` et `get_perf(component)`.
- [ ] 8.9 Détection de régressions selon la méthode décidée dans la spec — critère : tests
  statistiques sur des jeux de données connus ; une fonction ralentie exprès passe de
  « probable » à « confirmée ». Méthode :
  - instructions CPU par compteurs matériels quand c'est possible, temps réel affiché ;
  - 5 exécutions, jusqu'à 15 si la variance est forte ; médiane et MAD ;
  - référence = médiane des 5 derniers snapshots de la branche principale ;
  - régression signalée si Mann-Whitney p < 0,01 et écart ≥ max(5 %, 3 × MAD) ;
  - « probable », puis « confirmée » après un nouveau run ;
  - référence réinitialisée si l'empreinte de la machine change.
- [ ] 8.10 Relecture par `phase-reviewer` — critère : verdict « prête ».

Critère de sortie (proposé, à valider) : `timonier test` sur Timonier produit une timeline par
test, avec des spans rattachés aux nœuds du graphe ; une régression provoquée est détectée ;
un budget dépassé apparaît en rouge.

Décisions : —
Écarts : —

## Phase 9 — Fin du MVP

Statut : à faire

- [ ] 9.1 Inventaire des technologies, exposé par l'outil MCP `get_tech_inventory` — critère :
  sur Timonier, un SBOM valide ; les doublons d'une fixture sont détectés. Contenu :
  - lecture des manifestes (`Cargo.toml`, `package.json` ; `pyproject.toml` et `go.mod` si le
    temps le permet) ;
  - SBOM CycloneDX (cyclonedx-bom 0.8) : langages, frameworks, dépendances, versions,
    licences ;
  - CVE connues (source fixée par ADR, accès réseau) ;
  - doublons fonctionnels, et dépendances ajoutées depuis la dernière revue.
- [ ] 9.2 Processus de développement — critère : le scénario « Solo discipliné » fonctionne de
  bout en bout sur un dépôt de test. Contenu :
  - préréglages Solo rapide, Solo discipliné et Équipe ; `.timonier/workflow.json` ;
    panneau de réglages ; bloc géré dérivé du processus ;
  - GitHub via `gh` : un ticket par tâche, branche `tim/<ticket>-<slug>`, PR en fin de
    session avec une description générée ;
  - génération du workflow de CI ;
  - outils MCP `get_workflow`, `task_start`, `open_pull_request` et `link_ticket` ;
  - jetons dans le trousseau de l'IDE.
- [ ] 9.3 Coût de l'IA, selon Q7 : la télémétrie OpenTelemetry native de Claude Code est
  pointée par init vers le récepteur, puis les tokens sont comptés par session et par tâche.
  Sans source, la valeur est « non mesuré », jamais une estimation inventée — critère : une
  session réelle donne des tokens attribués.
- [ ] 9.4 Assistant « Nouveau projet piloté », selon Q8 — critère : défini avec la réponse.
- [ ] 9.5 Relecture complète du MVP par `phase-reviewer` contre la section « Découpage » —
  critère : verdict « prête ».
- [ ] 9.6 Critère de sortie du MVP selon la spec : un projet réel (Q9) piloté de bout en bout
  sans lire le code ligne à ligne — critère : constaté avec toi.

Décisions : —
Écarts : —

---

## Dépendances vérifiées

Vérifiées le 2026-10-14 sur crates.io, le registre npm et les dépôts officiels. Revérifier
avant ajout si plus d'un mois s'est écoulé.

| Paquet | Version | Licence | Remarque |
| ------ | ------- | ------- | -------- |
| tokio | 1.53.3 | MIT | |
| thiserror / anyhow | 2.0.21 / 1.0.104 | MIT OR Apache-2.0 | |
| tracing / tracing-subscriber | 0.1.44 / 0.3.23 | MIT | |
| tracing-opentelemetry | 0.34.0 | MIT | Va avec opentelemetry 0.33 |
| opentelemetry, _sdk, -otlp, -proto | 0.33.1 | Apache-2.0 | Avant la 1.0 ; l'API traces est encore « Beta » |
| rusqlite | 0.40.2 | MIT | |
| rusqlite_migration / refinery | 2.6.0 / 0.10.0 | Apache-2.0 / MIT | rusqlite_migration retenu (ADR 0011) ; refinery écarté |
| tree-sitter / -rust / -typescript | 0.27.1 / 0.24.2 / 0.23.2 | MIT | Revérifiés le 2026-10-16 ; utilisés (ADR 0012) |
| notify / notify-debouncer-full | 8.2.0 / 0.7.0 | CC0-1.0 / MIT OR Apache-2.0 | notify 9 encore en RC ; notify vient par le débounceur, qui le réexporte |
| ignore / globset | 0.4.33 / 0.4.20 | Unlicense OR MIT | Revérifiés le 2026-10-16 |
| rayon | 1.12.0 | MIT OR Apache-2.0 | Vérifié le 2026-10-16 ; extraction parallèle (ADR 0013) |
| toml | 1.1.8 | MIT OR Apache-2.0 | Vérifié le 2026-10-16 ; manifestes des packs et de Cargo, features `std`, `serde`, `parse` |
| serde / serde_json / schemars | 1.0.229 / 1.0.154 / 1.2.3 | MIT (OR Apache-2.0) | |
| jsonrpsee | 0.26.1 | MIT | |
| async-lsp / tower-lsp-server | 0.2.4 / 0.23.0 | MIT OR Apache-2.0 | tower-lsp n'est plus maintenu ; lsp-types est figé |
| rmcp | 3.5.1 | Apache-2.0 (parties en MIT) | SDK Rust officiel de MCP |
| tonic / prost / axum | 0.14.6 / 0.14.4 / 0.8.9 | MIT / Apache-2.0 / MIT | |
| clap | 4.6.7 | MIT OR Apache-2.0 | |
| blake3 | 1.8.7 | CC0-1.0 OR Apache-2.0 | |
| graphrs | 0.12.0 | MIT | **Écarté** (ADR 0016) : son Leiden ne part pas d'une partition initiale |
| quick-junit | 0.8.0 | Apache-2.0 OR MIT | |
| cyclonedx-bom | 0.8.1 | Apache-2.0 | |
| scip | 0.10.0 | Apache-2.0 | v2 ; dépend de rust-protobuf, pas de prost |
| procfs / cgroups-rs | 0.18.0 / 0.5.1 | MIT OR Apache-2.0 | |
| insta / proptest / tempfile / assert_cmd | 1.49.0 / 1.12.0 / 3.27.0 / 2.2.2 | Apache-2.0 / MIT OR Apache-2.0 | |
| @xyflow/react | 12.12.0 | MIT | Pas `reactflow` (figé) |
| elkjs | 0.12.0 | EPL-2.0 OR GPL-3.0-or-later | |
| tldraw | 5.5.2 | Propriétaire, avec clé | **Écarté** (Q3) |
| uplot / pixi.js | 1.6.32 / 8.22.0 | MIT | |
| react, react-dom | 19.3.0 | MIT | |
| vite / @vitejs/plugin-react / vitest | 8.3.4 / 6.1.2 / 5.0.4 | MIT | |
| typescript | 6.0.3 (7.0.2 existe) | Apache-2.0 | H11 |
| eslint / typescript-eslint / prettier | 10.12.0 / 8.71.2 / 3.9.10 | MIT | |
| @vscode/vsce / test-cli / test-electron | 4.0.0 / 0.0.15 / 3.1.0 | MIT | Node ≥ 22 |
| @types/vscode | 1.140.0 | MIT | VSCode stable : 1.141.2 |
| @vscode-elements/elements | 2.5.1 | MIT | `@vscode/webview-ui-toolkit` est déprécié |
| vscode-jsonrpc | 9.0.3 | MIT | |
| axe-core / @playwright/test | 4.14.0 / 1.64.1 | MPL-2.0 / Apache-2.0 | Développement seulement |
| esbuild | 0.28.2 | MIT | Bundle de l'extension |
| @eslint/js / eslint-plugin-react-hooks | 10.0.1 / 7.1.1 | MIT | |
| @types/node / @types/react / @types/react-dom / @types/mocha | 24.19.2 / 19.3.0 / 19.3.0 / 10.0.10 | MIT | `@types/node` 24 = Node 24.21 de VSCode 1.141 |
| actions/checkout / setup-node / upload-artifact | v7.0.1 / v7.1.0 / v7.0.2 | MIT | Actions GitHub officielles, par tag majeur |
| Swatinem/rust-cache | v2.9.2 (`6323deb`) | LGPL-3.0 | Action tierce, épinglée par SHA |

Toolchain : Rust stable 1.99.1 (installée en local le 2026-10-16 ; 1.96 reste pour la MSRV), édition 2024 stable depuis 1.85 ;
Node 24 LTS (26 devient LTS le 2026-10-28) ; VSCode 1.141.2.

## Journal

- 2026-10-14 — Phase 0 :
  - spec lue ;
  - formats Claude Code et dépendances vérifiés ;
  - CLAUDE.md, skills, hooks (23 tests verts), subagent et ADR 0001–0002 écrits ;
  - plan soumis pour validation.
- 2026-10-14 — Réponses à Q2–Q7 :
  - dépôt public de Nicolas Michel, créé plus tard ;
  - tldraw écarté ;
  - options recommandées retenues pour Q4, Q5, Q6 et Q7.

  Q1 (licence) a été tranchée ensuite : MIT.
- 2026-10-14 — Phase 1 :
  - squelette réalisé ;
  - relecture `phase-reviewer`, verdict « pas prête » : 1 bloquant, 5 importants, 9 mineurs,
    tous traités ;
  - CI verte au premier push sur Linux et macOS (run 38604588410) ;
  - reste ton contrôle visuel F5 (1.9).
- 2026-10-14 — Ajout de 1.15 à ta demande : icône Timonier dans la barre d'activité, avec une
  vue d'ensemble et le bouton « Ouvrir la carte » (ADR 0007).
- 2026-10-14 — Phase 1 validée par toi (« très bien pour l'icône, passe à la suite »). ADR 0001
  à 0005 et 0007 acceptées ; 0006 attend Q13. Phase 2 commencée.
- 2026-10-14 — Phase 2 :
  - ADR 0008 à 0011 ;
  - modèle (`timonier-core`) et stockage versionné (`timonier-store`) ;
  - trois relectures `phase-reviewer` :
    - n° 1 « pas prête » (1 bloquant, 6 importants) ;
    - n° 2 : 2 importants introduits ou laissés par les corrections ;
    - n° 3 « prête » ;
    - tout corrigé ;
  - 105 tests Rust verts ;
  - mesures consignées ;
  - Q14 et Q15 posées.
- 2026-10-16 — Phase 2 validée par toi (« tout me semble bien ») :
  - ADR 0008 à 0011 acceptées ; Q14 (fichiers hors package gardés) et Q15 (ADR acceptées à la
    validation de la phase) tranchées ;
  - Q13 : bilingue pour l'instant, ADR 0006 acceptée ;
  - Q12 reportée ;
  - Phase 3 commencée.
- 2026-10-16 — Phase 3 :
  - ADR 0012 à 0018 ;
  - packs Rust et TypeScript, extraction, résolution, cache, watcher, affinage LSP,
    consolidation au commit, composants (Leiden écrit par nous), rôles, compaction ;
  - relecture n° 1 « pas prête » (1 bloquant, 7 importants, 10 mineurs), tout corrigé ;
  - 192 tests Rust verts (un test de charge à part, en release) ;
  - mesures sur Timonier : 456 ms à froid, p95 de 10 ms par sauvegarde.
- 2026-10-16 — Phase 3 validée par toi (« commit push, vérifie que la CI passe, et ensuite passe
  à la Phase 4 ») :
  - ADR 0012 à 0018 acceptées ;
  - relecture n° 2 « pas prête » (1 bloquant, 4 importants, 5 mineurs), tout corrigé ;
  - 198 tests Rust verts ;
  - captures du plugin prises (VSCode 1.142, en français, thèmes sombre et clair) ;
  - Phase 4 commencée.
- 2026-10-16 — Relecture n° 3 de la Phase 3, arrivée après ta validation : « pas prête ».
  F1 (le watcher se nourrissait de ses propres lectures), P1 (3.12 cochée à tort) et m-f sont
  corrigés, avec deux tests de régression. 3.12 est décochée.
- 2026-10-16 — Contre-vérification : F1 et P1 corrigés ; m-f ne l'était qu'en partie (un crate
  vu d'abord par un autre parcours). Corrigé, avec un troisième test.
